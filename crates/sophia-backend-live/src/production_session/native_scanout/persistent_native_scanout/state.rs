use sophia_protocol::TransactionId;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionCpuFrameQueueStatus {
    Queued,
    BaselineRequired,
    UnchangedPending,
    UnchangedSubmitted,
    UnchangedPresented,
}

pub fn reduce_live_production_cpu_frame_queue(
    pending: Option<LiveProductionScanoutContent>,
    submitted: Option<LiveProductionScanoutContent>,
    presented: Option<LiveProductionScanoutContent>,
    callback_observed: bool,
    checksum: u64,
) -> LiveProductionCpuFrameQueueStatus {
    let content = LiveProductionScanoutContent::Cpu { checksum };
    if pending == Some(content) {
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
