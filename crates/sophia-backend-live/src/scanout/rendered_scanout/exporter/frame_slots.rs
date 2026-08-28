use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const LIVE_RENDERER_FRAME_SLOT_CAPACITY: usize = 3;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LiveRendererFrameSlotId(u8);

impl LiveRendererFrameSlotId {
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LiveRendererFrameSlotToken {
    slot_id: LiveRendererFrameSlotId,
    incarnation: u64,
}

impl LiveRendererFrameSlotToken {
    pub const fn slot_id(self) -> LiveRendererFrameSlotId {
        self.slot_id
    }

    pub const fn incarnation(self) -> u64 {
        self.incarnation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererFrameSlotAcquire {
    Acquired(LiveRendererFrameSlotToken),
    Deferred,
    IncarnationExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererFrameSlotRelease {
    Released,
    Stale,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LiveRendererFrameSlotMetrics {
    pub acquisitions: usize,
    pub reuses: usize,
    pub deferrals: usize,
    pub stale_releases: usize,
    pub leased: usize,
    pub high_watermark: usize,
}

#[derive(Default)]
struct LiveRendererFrameSlotMetricCounters {
    acquisitions: AtomicUsize,
    reuses: AtomicUsize,
    deferrals: AtomicUsize,
    stale_releases: AtomicUsize,
    leased: AtomicUsize,
    high_watermark: AtomicUsize,
}

#[derive(Clone, Default)]
pub(super) struct LiveRendererFrameSlotMetricsHandle {
    counters: Arc<LiveRendererFrameSlotMetricCounters>,
}

impl LiveRendererFrameSlotMetricsHandle {
    pub fn snapshot(&self) -> LiveRendererFrameSlotMetrics {
        LiveRendererFrameSlotMetrics {
            acquisitions: self.counters.acquisitions.load(Ordering::Relaxed),
            reuses: self.counters.reuses.load(Ordering::Relaxed),
            deferrals: self.counters.deferrals.load(Ordering::Relaxed),
            stale_releases: self.counters.stale_releases.load(Ordering::Relaxed),
            leased: self.counters.leased.load(Ordering::Relaxed),
            high_watermark: self.counters.high_watermark.load(Ordering::Relaxed),
        }
    }

    fn acquired(&self, reused: bool, leased: usize) {
        self.counters.acquisitions.fetch_add(1, Ordering::Relaxed);
        if reused {
            self.counters.reuses.fetch_add(1, Ordering::Relaxed);
        }
        self.counters.leased.store(leased, Ordering::Relaxed);
        self.counters
            .high_watermark
            .fetch_max(leased, Ordering::Relaxed);
    }

    fn deferred(&self) {
        self.counters.deferrals.fetch_add(1, Ordering::Relaxed);
    }

    fn released(&self, leased: usize) {
        self.counters.leased.store(leased, Ordering::Relaxed);
    }

    fn stale_release(&self) {
        self.counters.stale_releases.fetch_add(1, Ordering::Relaxed);
    }
}

pub struct LiveRendererFrameSlotPool {
    incarnations: [u64; LIVE_RENDERER_FRAME_SLOT_CAPACITY],
    occupied: [Option<u64>; LIVE_RENDERER_FRAME_SLOT_CAPACITY],
    next_slot: usize,
    leased: usize,
    metrics: LiveRendererFrameSlotMetricsHandle,
}

impl Default for LiveRendererFrameSlotPool {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveRendererFrameSlotPool {
    pub fn new() -> Self {
        Self::with_metrics(LiveRendererFrameSlotMetricsHandle::default())
    }

    pub(super) fn with_metrics(metrics: LiveRendererFrameSlotMetricsHandle) -> Self {
        Self {
            incarnations: [0; LIVE_RENDERER_FRAME_SLOT_CAPACITY],
            occupied: [None; LIVE_RENDERER_FRAME_SLOT_CAPACITY],
            next_slot: 0,
            leased: 0,
            metrics,
        }
    }

    pub fn try_acquire(&mut self) -> LiveRendererFrameSlotAcquire {
        for offset in 0..LIVE_RENDERER_FRAME_SLOT_CAPACITY {
            let index = (self.next_slot + offset) % LIVE_RENDERER_FRAME_SLOT_CAPACITY;
            if self.occupied[index].is_some() {
                continue;
            }
            let Some(incarnation) = self.incarnations[index].checked_add(1) else {
                return LiveRendererFrameSlotAcquire::IncarnationExhausted;
            };
            let reused = self.incarnations[index] != 0;
            self.incarnations[index] = incarnation;
            self.occupied[index] = Some(incarnation);
            self.next_slot = (index + 1) % LIVE_RENDERER_FRAME_SLOT_CAPACITY;
            self.leased += 1;
            self.metrics.acquired(reused, self.leased);
            return LiveRendererFrameSlotAcquire::Acquired(LiveRendererFrameSlotToken {
                slot_id: LiveRendererFrameSlotId(index as u8),
                incarnation,
            });
        }
        self.metrics.deferred();
        LiveRendererFrameSlotAcquire::Deferred
    }

    pub fn release(&mut self, token: LiveRendererFrameSlotToken) -> LiveRendererFrameSlotRelease {
        let index = token.slot_id.index();
        if self.occupied.get(index).copied().flatten() != Some(token.incarnation) {
            self.metrics.stale_release();
            return LiveRendererFrameSlotRelease::Stale;
        }
        self.occupied[index] = None;
        self.leased -= 1;
        self.metrics.released(self.leased);
        LiveRendererFrameSlotRelease::Released
    }

    pub(super) fn refuse_stale_release(&self) {
        self.metrics.stale_release();
    }

    pub const fn leased(&self) -> usize {
        self.leased
    }

    pub fn metrics(&self) -> LiveRendererFrameSlotMetrics {
        self.metrics.snapshot()
    }
}
