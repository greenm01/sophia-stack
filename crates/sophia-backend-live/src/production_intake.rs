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
    pub x_offset: i32,
    pub y_offset: i32,
    pub acquire_fence: Option<FenceHandle>,
    pub idle_fence: Option<FenceHandle>,
    pub layout_disposition: LiveProductionPresentDisposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveProductionSoftwarePresentSubmission {
    pub transaction: TransactionId,
    pub surface: SurfaceId,
    pub acquire_fence: Option<FenceHandle>,
    pub idle_fence: Option<FenceHandle>,
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
    pub software_present_submissions: Vec<LiveProductionSoftwarePresentSubmission>,
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
        if self
            .software_present_submissions
            .iter()
            .any(|submission| submission.transaction != self.transaction)
        {
            return Err(
                "production authority group contains a mismatched software Present submission",
            );
        }
        let present_keys = self
            .present_submissions
            .iter()
            .map(|submission| sophia_protocol::DmaBufPresentKey {
                transaction: submission.transaction,
                surface: submission.surface,
                buffer: submission.buffer,
            })
            .collect::<Vec<_>>();
        if !sophia_protocol::dma_buf_present_pairs_are_exact(&self.transactions, &present_keys) {
            return Err("production DMA-BUF transactions and Presents are not exact pairs");
        }
        for (index, submission) in self.software_present_submissions.iter().enumerate() {
            if self.software_present_submissions[..index]
                .iter()
                .any(|prior| prior.surface == submission.surface)
            {
                return Err("production authority group contains a duplicate software Present");
            }
            let matches = self
                .transactions
                .iter()
                .filter(|transaction| {
                    transaction.transaction == submission.transaction
                        && transaction.surface == submission.surface
                        && matches!(
                            transaction.target_buffer,
                            sophia_protocol::BufferSource::CpuBuffer { .. }
                        )
                })
                .count();
            if matches != 1 {
                return Err(
                    "production CPU transactions and software Presents are not exact pairs",
                );
            }
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

    /// Whether this batch needs the DMA-BUF Present composition path.
    ///
    /// Software Present remains a presentation semantically, but its
    /// materialized CPU snapshot must enter the CPU production cycle.
    pub fn has_dma_buf_present_submissions(&self) -> bool {
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
