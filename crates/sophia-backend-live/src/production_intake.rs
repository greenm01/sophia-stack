use sophia_protocol::{
    BufferHandle, DmaBufDescriptor, FenceHandle, LayerSnapshot, SurfaceId, SurfaceTransaction,
    TransactionId,
};
use std::os::fd::OwnedFd;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct LiveProductionDmaBufRegistration {
    pub descriptor: DmaBufDescriptor,
    pub plane_fds: Vec<Arc<OwnedFd>>,
}

#[derive(Clone, Debug)]
pub struct LiveProductionFenceRegistration {
    pub handle: FenceHandle,
    pub initially_triggered: bool,
    pub fd: Arc<OwnedFd>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionPresentDisposition {
    Immediate,
    StageLayout { epoch: TransactionId },
    RejectLayoutMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveProductionPresentSubmission {
    pub transaction: TransactionId,
    pub surface: SurfaceId,
    pub buffer: BufferHandle,
    pub x_offset: i16,
    pub y_offset: i16,
    pub acquire_fence: Option<FenceHandle>,
    pub idle_fence: Option<FenceHandle>,
    pub layout_disposition: LiveProductionPresentDisposition,
}

#[derive(Clone, Debug)]
pub struct LiveProductionAuthorityBatch {
    /// Ordered atomic transaction groups carried by this bounded intake
    /// envelope. Resource registrations remain envelope-scoped because DRI3
    /// imports can precede the Present request that consumes them.
    pub groups: Vec<LiveProductionAuthorityGroup>,
    pub dma_buf_registrations: Vec<LiveProductionDmaBufRegistration>,
    pub fence_registrations: Vec<LiveProductionFenceRegistration>,
    pub released_dma_bufs: Vec<BufferHandle>,
    pub released_fences: Vec<FenceHandle>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveProductionAuthorityGroup {
    pub transaction: TransactionId,
    pub transactions: Vec<SurfaceTransaction>,
    pub removed_surfaces: Vec<SurfaceId>,
    pub present_submissions: Vec<LiveProductionPresentSubmission>,
}

impl LiveProductionAuthorityGroup {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.transaction.is_valid() {
            return Err("production authority group has an invalid transaction");
        }
        if self
            .transactions
            .iter()
            .any(|transaction| transaction.transaction != self.transaction)
        {
            return Err("production authority group contains a mismatched surface transaction");
        }
        if self
            .present_submissions
            .iter()
            .any(|submission| submission.transaction != self.transaction)
        {
            return Err("production authority group contains a mismatched Present submission");
        }
        Ok(())
    }
}

impl LiveProductionAuthorityBatch {
    pub fn validate(&self) -> Result<(), &'static str> {
        for group in &self.groups {
            group.validate()?;
        }
        Ok(())
    }

    pub fn transaction_count(&self) -> usize {
        self.groups
            .iter()
            .map(|group| group.transactions.len())
            .sum()
    }

    pub fn has_present_submissions(&self) -> bool {
        self.groups
            .iter()
            .any(|group| !group.present_submissions.is_empty())
    }
}

#[derive(Clone, Debug)]
pub struct LiveProductionPreparedAuthorityBatch {
    pub authority_commits: Vec<sophia_protocol::TransactionCommit>,
    pub layer_templates: Vec<LayerSnapshot>,
}

#[derive(Clone, Debug)]
pub struct LiveProductionCpuSubmission {
    pub tick: crate::LiveBackendRuntimeTickReport,
    pub composition: crate::LiveCpuCompositionReport,
    pub composed: bool,
    pub compose_elapsed: std::time::Duration,
}
