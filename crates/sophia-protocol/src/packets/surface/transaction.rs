use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceTransaction {
    pub transaction: TransactionId,
    pub authority: AuthorityKind,
    pub surface: SurfaceId,
    pub namespace: Option<NamespaceId>,
    /// Logical placement of the authority-owned surface.
    pub target_geometry: Rect,
    /// The bounded raster content asserted for this surface generation.
    ///
    /// The set's logical extent is the exact pixel extent the geometry
    /// presents. Protocol authorities may project a descendant content window
    /// onto a larger policy-managed surface; keeping the content extent
    /// distinct from the geometry lets the Engine retain pixel-exact
    /// presentation without teaching it a client protocol's window hierarchy.
    pub content: SurfaceContentSet,
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
        transaction.content.variants().iter().all(|variant| {
            let BufferSource::DmaBuf { handle } = variant.source else {
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
        })
    }) && presents.iter().all(|present| {
        transactions
            .iter()
            .flat_map(|transaction| {
                transaction
                    .content
                    .variants()
                    .iter()
                    .map(move |variant| (transaction, variant))
            })
            .filter(|(transaction, variant)| {
                transaction.transaction == present.transaction
                    && transaction.surface == present.surface
                    && variant.source
                        == BufferSource::DmaBuf {
                            handle: present.buffer.raw(),
                        }
            })
            .count()
            == 1
    })
}

impl SurfaceTransaction {
    pub fn key(&self) -> SurfaceTransactionKey {
        SurfaceTransactionKey {
            transaction: self.transaction,
            surface: self.surface,
            target_buffer: self.target_buffer(),
        }
    }

    /// The canonical single content value, until per-head variant selection
    /// consumes the full set.
    pub fn target_buffer(&self) -> BufferSource {
        self.content.canonical_source()
    }

    pub fn target_content_size(&self) -> Size {
        self.content.logical_extent()
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
    /// The committed content set. Its generation identity is
    /// `committed_generation`: content advances exactly with authority
    /// commits, so the record carries one clock, not two.
    pub content: SurfaceContentSet,
    pub damage: Region,
}

impl CommittedSurfaceState {
    pub fn from_layer_snapshot(layer: &LayerSnapshot) -> Self {
        Self::with_source(
            layer.surface,
            layer.generation,
            layer.geometry,
            layer.source,
            layer.damage.clone(),
        )
    }

    /// Builds a committed state around a single canonical raster whose
    /// pixels span the geometry, which is every current producer's shape.
    pub fn with_source(
        surface: SurfaceId,
        committed_generation: u64,
        geometry: Rect,
        source: BufferSource,
        damage: Region,
    ) -> Self {
        Self {
            surface,
            committed_generation,
            geometry,
            content: SurfaceContentSet::singleton(
                source,
                Size {
                    width: geometry.width,
                    height: geometry.height,
                },
            ),
            damage,
        }
    }

    /// The canonical single content value, until per-head variant selection
    /// consumes the full set.
    pub fn buffer(&self) -> BufferSource {
        self.content.canonical_source()
    }
}
