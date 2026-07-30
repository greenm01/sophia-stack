use sophia_protocol::TransactionId;
use std::time::Duration;

pub const LIVE_PRODUCTION_PAGE_FLIP_HARD_STALL: Duration = Duration::from_millis(500);

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
        checksum: u64,
    },
    MixedPresent {
        transaction: TransactionId,
        nonzero_rgb_pixels: usize,
    },
    RetainedMixed {
        nonzero_rgb_pixels: usize,
    },
}

impl LiveProductionScanoutContent {
    pub const fn with_nonzero_rgb_pixels(self, nonzero_rgb_pixels: usize) -> Self {
        match self {
            Self::MixedPresent { transaction, .. } => Self::MixedPresent {
                transaction,
                nonzero_rgb_pixels,
            },
            Self::RetainedMixed { .. } => Self::RetainedMixed { nonzero_rgb_pixels },
            cpu @ Self::Cpu { .. } => cpu,
        }
    }
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
    let content = LiveProductionScanoutContent::Cpu { checksum };
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
    } else if pending == Some(content) {
        LiveProductionCpuFrameQueueStatus::UnchangedPending
    } else if submitted == Some(content) {
        LiveProductionCpuFrameQueueStatus::UnchangedSubmitted
    } else if pending.is_none()
        && submitted.is_none()
        && presented == Some(content)
        && !callback_observed
    {
        LiveProductionCpuFrameQueueStatus::BaselineRequired
    } else if pending.is_none() && submitted.is_none() && presented == Some(content) {
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
        }) if presented_transaction == transaction && nonzero_rgb_pixels > 0
    ) && submitted.is_none()
        && !pending
}
