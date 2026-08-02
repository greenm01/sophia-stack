use std::collections::BTreeMap;

use sophia_protocol::{BufferHandle, LayerSnapshot, Size, SurfaceId, TransactionId};
use sophia_x_authority::XAuthorityObservedTransactionBatch;

pub use sophia_engine::{
    LayoutEpochCoordinator as ResizeRollbackCoordinator,
    LayoutRecoveryConfigure as ResizeRollbackRequest,
};

pub const RESIZE_VISUAL_COMMIT_CAPACITY: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResizeVisualCommit {
    pub transaction: TransactionId,
    pub surface: SurfaceId,
    pub size: Size,
}

/// Bounded visual candidates selected by a layout epoch but not yet committed
/// by the asynchronous presentation backend.
#[derive(Debug, Default)]
pub struct ResizeVisualCommitTracker {
    awaiting: BTreeMap<(TransactionId, SurfaceId), ResizeVisualCommit>,
}

impl ResizeVisualCommitTracker {
    pub fn arm(&mut self, candidate: ResizeVisualCommit) -> Result<(), &'static str> {
        if !candidate.transaction.is_valid() || !candidate.surface.is_valid() {
            return Err("resize visual commit candidate has an invalid identity");
        }
        if candidate.size.width <= 0 || candidate.size.height <= 0 {
            return Err("resize visual commit candidate has an invalid size");
        }
        let key = (candidate.transaction, candidate.surface);
        if self.awaiting.contains_key(&key) {
            return Err("resize visual commit candidate identity is duplicated");
        }
        if self.awaiting.len() == RESIZE_VISUAL_COMMIT_CAPACITY {
            return Err("resize visual commit candidate capacity exceeded");
        }
        self.awaiting.insert(key, candidate);
        Ok(())
    }

    pub fn surface_awaiting(&self, surface: SurfaceId) -> bool {
        self.awaiting
            .values()
            .any(|candidate| candidate.surface == surface)
    }

    pub fn complete(
        &mut self,
        transaction: TransactionId,
        surface: SurfaceId,
        size: Size,
    ) -> Option<ResizeVisualCommit> {
        let key = (transaction, surface);
        let candidate = self.awaiting.get(&key).copied()?;
        if candidate.size != size {
            return None;
        }
        self.awaiting.remove(&key)
    }

    pub fn remove_surface(&mut self, surface: SurfaceId) -> usize {
        let before = self.awaiting.len();
        self.awaiting
            .retain(|_, candidate| candidate.surface != surface);
        before.saturating_sub(self.awaiting.len())
    }

    pub fn len(&self) -> usize {
        self.awaiting.len()
    }

    pub fn is_empty(&self) -> bool {
        self.awaiting.is_empty()
    }
}

/// Projects authority pixels onto the current layout without dropping any
/// generation-bearing transaction or associated buffer update.
pub fn project_authority_batch_onto_layout(
    mut batch: XAuthorityObservedTransactionBatch,
    layers: &BTreeMap<SurfaceId, LayerSnapshot>,
) -> XAuthorityObservedTransactionBatch {
    for transaction in &mut batch.transactions {
        if let Some(layer) = layers.get(&transaction.surface) {
            transaction.target_geometry = layer.geometry;
        }
    }
    batch
}

pub fn present_pixels_conflict_with_requested_sizes(
    requested_sizes: &BTreeMap<SurfaceId, Size>,
    dma_buf_sizes: &BTreeMap<BufferHandle, Size>,
    batch: &XAuthorityObservedTransactionBatch,
) -> bool {
    batch.present_submissions.iter().any(|submission| {
        requested_sizes
            .get(&submission.surface)
            .zip(dma_buf_sizes.get(&submission.buffer))
            .is_some_and(|(expected, actual)| actual != expected)
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PendingLayoutObservationMerge {
    Inserted,
    Merged,
    ResizeOwned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PendingLayoutGeometryAuthority {
    Layout,
    Observation,
}

/// Keeps authority observations that are outside a pending resize proposal in
/// the proposal snapshot. Without this merge, committing an older proposal
/// can discard a surface admitted while its resize pixels were pending.
pub fn merge_unrequested_layout_observation(
    pending_layers: &mut Vec<LayerSnapshot>,
    requested_sizes: &BTreeMap<SurfaceId, Size>,
    observed: LayerSnapshot,
    geometry_authority: PendingLayoutGeometryAuthority,
) -> PendingLayoutObservationMerge {
    if requested_sizes.contains_key(&observed.surface) {
        return PendingLayoutObservationMerge::ResizeOwned;
    }
    match pending_layers
        .iter_mut()
        .find(|layer| layer.surface == observed.surface)
    {
        Some(layer) => {
            if geometry_authority == PendingLayoutGeometryAuthority::Observation {
                layer.geometry = observed.geometry;
            }
            layer.authority_local_id = observed.authority_local_id;
            layer.namespace = observed.namespace;
            layer.source = observed.source;
            layer.damage = observed.damage;
            layer.opacity = observed.opacity;
            layer.crop = observed.crop;
            layer.transform = observed.transform;
            layer.generation = observed.generation;
            layer.resize_sync = observed.resize_sync;
            PendingLayoutObservationMerge::Merged
        }
        None => {
            pending_layers.push(observed);
            PendingLayoutObservationMerge::Inserted
        }
    }
}
