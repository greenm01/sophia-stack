use sophia_protocol::{OutputId, TransactionId};
use std::time::Duration;

pub const LIVE_PRODUCTION_PAGE_FLIP_HARD_STALL: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LiveProductionNativeFrameId(u64);

impl LiveProductionNativeFrameId {
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionPageFlipWatchdogStatus {
    Idle,
    Healthy,
    HardStall,
}

pub fn reduce_live_production_page_flip_watchdog(
    submitted_age: Option<Duration>,
    hard_stall: Duration,
) -> LiveProductionPageFlipWatchdogStatus {
    match submitted_age {
        None => LiveProductionPageFlipWatchdogStatus::Idle,
        Some(age) if age >= hard_stall => LiveProductionPageFlipWatchdogStatus::HardStall,
        Some(_) => LiveProductionPageFlipWatchdogStatus::Healthy,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveProductionScanoutContent {
    Cpu {
        frame: LiveProductionNativeFrameId,
        checksum: u64,
    },
    MixedPresent {
        frame: LiveProductionNativeFrameId,
        transaction: TransactionId,
        nonzero_rgb_pixels: usize,
    },
    RetainedMixed {
        frame: LiveProductionNativeFrameId,
        nonzero_rgb_pixels: usize,
    },
}

impl LiveProductionScanoutContent {
    pub const fn frame(self) -> LiveProductionNativeFrameId {
        match self {
            Self::Cpu { frame, .. }
            | Self::MixedPresent { frame, .. }
            | Self::RetainedMixed { frame, .. } => frame,
        }
    }

    pub const fn with_nonzero_rgb_pixels(self, nonzero_rgb_pixels: usize) -> Self {
        match self {
            Self::MixedPresent {
                frame, transaction, ..
            } => Self::MixedPresent {
                frame,
                transaction,
                nonzero_rgb_pixels,
            },
            Self::RetainedMixed { frame, .. } => Self::RetainedMixed {
                frame,
                nonzero_rgb_pixels,
            },
            cpu @ Self::Cpu { .. } => cpu,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveProductionNativeFrameRetirement {
    pub output: OutputId,
    pub frame: LiveProductionNativeFrameId,
    pub submission: u64,
    pub content: LiveProductionScanoutContent,
    pub ust: u64,
    pub msc: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionCpuFrameQueueStatus {
    Queued,
    BaselineRequired,
    GpuFrameOwned,
    UnchangedPending,
    UnchangedSubmitted,
    UnchangedPresented,
}

pub fn reduce_live_production_cpu_frame_queue(
    pending: Option<LiveProductionScanoutContent>,
    submitted: Option<LiveProductionScanoutContent>,
    presented: Option<LiveProductionScanoutContent>,
    renderer_in_flight: bool,
    callback_observed: bool,
    checksum: u64,
) -> LiveProductionCpuFrameQueueStatus {
    if renderer_in_flight
        || matches!(
            pending,
            Some(
                LiveProductionScanoutContent::MixedPresent { .. }
                    | LiveProductionScanoutContent::RetainedMixed { .. }
            )
        )
        || matches!(
            submitted,
            Some(
                LiveProductionScanoutContent::MixedPresent { .. }
                    | LiveProductionScanoutContent::RetainedMixed { .. }
            )
        )
    {
        LiveProductionCpuFrameQueueStatus::GpuFrameOwned
    } else if matches!(pending, Some(LiveProductionScanoutContent::Cpu {
        checksum: pending_checksum,
        ..
    }) if pending_checksum == checksum)
    {
        LiveProductionCpuFrameQueueStatus::UnchangedPending
    } else if matches!(submitted, Some(LiveProductionScanoutContent::Cpu {
        checksum: submitted_checksum,
        ..
    }) if submitted_checksum == checksum)
    {
        LiveProductionCpuFrameQueueStatus::UnchangedSubmitted
    } else if pending.is_none()
        && submitted.is_none()
        && matches!(presented, Some(LiveProductionScanoutContent::Cpu {
            checksum: presented_checksum,
            ..
        }) if presented_checksum == checksum)
        && !callback_observed
    {
        LiveProductionCpuFrameQueueStatus::BaselineRequired
    } else if pending.is_none()
        && submitted.is_none()
        && matches!(presented, Some(LiveProductionScanoutContent::Cpu {
            checksum: presented_checksum,
            ..
        }) if presented_checksum == checksum)
    {
        LiveProductionCpuFrameQueueStatus::UnchangedPresented
    } else {
        LiveProductionCpuFrameQueueStatus::Queued
    }
}

pub fn live_production_scanout_is_stable_present(
    presented: Option<LiveProductionScanoutContent>,
    submitted: Option<LiveProductionScanoutContent>,
    pending: bool,
    transaction: TransactionId,
) -> bool {
    matches!(
        presented,
        Some(LiveProductionScanoutContent::MixedPresent {
            transaction: presented_transaction,
            nonzero_rgb_pixels,
            ..
        }) if presented_transaction == transaction && nonzero_rgb_pixels > 0
    ) && submitted.is_none()
        && !pending
}
