use crate::EngineHeadRegistry;
use crate::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct FramePlanRequest {
    pub output: OutputId,
    pub frame_serial: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameClockTick {
    pub output: OutputId,
    pub frame_serial: u64,
    pub target_msec: u64,
}

pub trait FrameClock {
    fn next_frame(&mut self, output: OutputId) -> FrameClockTick;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeterministicFrameClock {
    next_serial: u64,
    frame_interval_msec: u64,
}

impl DeterministicFrameClock {
    pub const fn new(start_serial: u64, frame_interval_msec: u64) -> Self {
        Self {
            next_serial: start_serial,
            frame_interval_msec,
        }
    }

    pub const fn next_serial(&self) -> u64 {
        self.next_serial
    }

    pub const fn frame_interval_msec(&self) -> u64 {
        self.frame_interval_msec
    }
}

impl Default for DeterministicFrameClock {
    fn default() -> Self {
        Self::new(1, 16)
    }
}

impl FrameClock for DeterministicFrameClock {
    fn next_frame(&mut self, output: OutputId) -> FrameClockTick {
        let frame_serial = self.next_serial;
        self.next_serial = self.next_serial.saturating_add(1);

        FrameClockTick {
            output,
            frame_serial,
            target_msec: frame_serial.saturating_mul(self.frame_interval_msec),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerOutputFrameClock {
    clocks: BTreeMap<OutputId, DeterministicFrameClock>,
    fallback: DeterministicFrameClock,
}

impl PerOutputFrameClock {
    pub fn from_outputs(outputs: &EngineHeadRegistry, fallback: DeterministicFrameClock) -> Self {
        let clocks = outputs
            .outputs()
            .map(|output| {
                // The primary head owns the logical clock. Mirror siblings
                // consume its generations independently at their own vblanks.
                let refresh_millihz = outputs.logical_refresh_millihz(output);
                let interval_msec = if refresh_millihz == 0 {
                    fallback.frame_interval_msec()
                } else {
                    (1_000_000u64 / u64::from(refresh_millihz)).max(1)
                };
                (
                    output,
                    DeterministicFrameClock::new(fallback.next_serial(), interval_msec),
                )
            })
            .collect();
        Self { clocks, fallback }
    }

    pub fn get(&self, output: OutputId) -> Option<&DeterministicFrameClock> {
        self.clocks.get(&output)
    }

    pub fn outputs(&self) -> impl Iterator<Item = OutputId> + '_ {
        self.clocks.keys().copied()
    }
}

impl FrameClock for PerOutputFrameClock {
    fn next_frame(&mut self, output: OutputId) -> FrameClockTick {
        self.clocks
            .get_mut(&output)
            .unwrap_or(&mut self.fallback)
            .next_frame(output)
    }
}

/// Coalesces primary-plane repaints onto a monotonic refresh-relative cadence.
///
/// Input owners deliberately do not call this reducer. They may update a
/// hardware cursor immediately, but only committed visual work requests a
/// primary repaint. That separation prevents pointer motion from changing an
/// application's visible animation rate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrimaryFramePacer {
    interval: std::time::Duration,
    next_deadline: Option<std::time::Instant>,
    repaint_pending: bool,
}

impl PrimaryFramePacer {
    pub fn new(interval: std::time::Duration) -> Self {
        assert!(!interval.is_zero(), "a frame cadence must advance time");
        Self {
            interval,
            next_deadline: None,
            repaint_pending: false,
        }
    }

    /// Rephases the next deadline when output policy changes refresh.
    pub fn set_interval(&mut self, now: std::time::Instant, interval: std::time::Duration) {
        assert!(!interval.is_zero(), "a frame cadence must advance time");
        if self.interval == interval {
            return;
        }
        self.interval = interval;
        if self.next_deadline.is_some() {
            self.next_deadline = now.checked_add(interval);
        }
    }

    /// Returns true when this production turn should retain state but defer
    /// composition. Backpressure is folded into the same latest-wins pending
    /// repaint instead of creating a second scheduling mechanism.
    pub fn defer_production(&mut self, now: std::time::Instant, backpressured: bool) -> bool {
        let Some(deadline) = self.next_deadline else {
            if backpressured {
                self.repaint_pending = true;
                self.next_deadline = now.checked_add(self.interval);
                return true;
            }
            self.next_deadline = now.checked_add(self.interval);
            return false;
        };

        if backpressured || now < deadline {
            self.repaint_pending = true;
            true
        } else {
            self.next_deadline = advance_deadline(deadline, now, self.interval);
            false
        }
    }

    /// Reconciles the requested admission with what the renderer actually did.
    pub fn observe_production(&mut self, now: std::time::Instant, composed: bool) {
        if composed {
            self.repaint_pending = false;
            let deadline = self
                .next_deadline
                .unwrap_or_else(|| now.checked_add(self.interval).unwrap_or(now));
            self.next_deadline = advance_deadline(deadline, now, self.interval);
        }
    }

    pub fn repaint_due(self, now: std::time::Instant) -> bool {
        self.repaint_pending && self.next_deadline.is_none_or(|deadline| now >= deadline)
    }

    pub fn observe_repaint(&mut self, now: std::time::Instant) {
        self.repaint_pending = false;
        let deadline = self.next_deadline.unwrap_or(now);
        self.next_deadline = advance_deadline(deadline, now, self.interval);
    }

    pub const fn repaint_pending(self) -> bool {
        self.repaint_pending
    }

    pub const fn interval(self) -> std::time::Duration {
        self.interval
    }

    /// Caps an owner wait without coupling the cadence to unrelated wakeups.
    pub fn cap_wait(
        self,
        now: std::time::Instant,
        maximum: std::time::Duration,
    ) -> std::time::Duration {
        if !self.repaint_pending {
            return maximum;
        }
        self.next_deadline
            .map_or(std::time::Duration::ZERO, |deadline| {
                deadline.saturating_duration_since(now).min(maximum)
            })
    }
}

fn advance_deadline(
    deadline: std::time::Instant,
    now: std::time::Instant,
    interval: std::time::Duration,
) -> Option<std::time::Instant> {
    if deadline <= now {
        now.checked_add(interval)
    } else {
        Some(deadline)
    }
}
