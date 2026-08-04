use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceTransaction {
    pub transaction: TransactionId,
    pub authority: AuthorityKind,
    pub surface: SurfaceId,
    pub namespace: Option<NamespaceId>,
    pub target_geometry: Rect,
    pub target_buffer: BufferSource,
    pub damage: Region,
    pub readiness: SurfaceTransactionReadiness,
    pub timeout_msec: u32,
    pub previous_committed_generation: u64,
}

/// Exact passive identity for one surface-content candidate.
///
/// A transaction may carry more than one source for a surface. Keeping the
/// buffer in the key prevents a backing snapshot from impersonating a Present
/// merely because both share a transaction and extent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceTransactionKey {
    pub transaction: TransactionId,
    pub surface: SurfaceId,
    pub target_buffer: BufferSource,
}

/// Passive identity joining one DMA-BUF surface transaction to its Present.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DmaBufPresentKey {
    pub transaction: TransactionId,
    pub surface: SurfaceId,
    pub buffer: BufferHandle,
}

/// Returns true when DMA-BUF transactions and Presents form exact pairs.
///
/// Non-DMA-BUF transactions do not participate. This cold-path validator is
/// shared by protocol frontends and production intake so neither boundary can
/// silently weaken atomic visual ownership.
pub fn dma_buf_present_pairs_are_exact(
    transactions: &[SurfaceTransaction],
    presents: &[DmaBufPresentKey],
) -> bool {
    transactions.iter().all(|transaction| {
        let BufferSource::DmaBuf { handle } = transaction.target_buffer else {
            return true;
        };
        presents
            .iter()
            .filter(|present| {
                present.transaction == transaction.transaction
                    && present.surface == transaction.surface
                    && present.buffer.raw() == handle
            })
            .count()
            == 1
    }) && presents.iter().all(|present| {
        transactions
            .iter()
            .filter(|transaction| {
                transaction.transaction == present.transaction
                    && transaction.surface == present.surface
                    && transaction.target_buffer
                        == BufferSource::DmaBuf {
                            handle: present.buffer.raw(),
                        }
            })
            .count()
            == 1
    })
}

impl SurfaceTransaction {
    pub const fn key(&self) -> SurfaceTransactionKey {
        SurfaceTransactionKey {
            transaction: self.transaction,
            surface: self.surface,
            target_buffer: self.target_buffer,
        }
    }

    pub fn from_layer_snapshot(
        transaction: TransactionId,
        authority: AuthorityKind,
        layer: &LayerSnapshot,
        readiness: SurfaceTransactionReadiness,
        timeout_msec: u32,
        previous_committed_generation: u64,
    ) -> Self {
        layer.to_surface_transaction(
            transaction,
            authority,
            readiness,
            timeout_msec,
            previous_committed_generation,
        )
    }

    pub fn from_surface_snapshot(
        transaction: TransactionId,
        authority: AuthorityKind,
        surface: &SurfaceSnapshot,
        readiness: SurfaceTransactionReadiness,
        timeout_msec: u32,
        previous_committed_generation: u64,
    ) -> Self {
        surface.to_surface_transaction(
            transaction,
            authority,
            readiness,
            timeout_msec,
            previous_committed_generation,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceTransactionReadiness {
    Pending,
    Ready,
    Failed,
    TimedOut,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommittedSurfaceState {
    pub surface: SurfaceId,
    pub committed_generation: u64,
    pub geometry: Rect,
    pub buffer: BufferSource,
    pub damage: Region,
}

impl CommittedSurfaceState {
    pub fn from_layer_snapshot(layer: &LayerSnapshot) -> Self {
        Self {
            surface: layer.surface,
            committed_generation: layer.generation,
            geometry: layer.geometry,
            buffer: layer.source,
            damage: layer.damage.clone(),
        }
    }
}
