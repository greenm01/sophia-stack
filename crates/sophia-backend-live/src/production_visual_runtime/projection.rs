use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct LiveSurfaceProjectionMetadata {
    namespace: Option<NamespaceId>,
}

impl LiveProductionVisualRuntime {
    pub(super) fn observe_surface_metadata(
        &mut self,
        transactions: &[SurfaceTransaction],
        removed_surfaces: &[SurfaceId],
    ) {
        self.surface_metadata
            .retain(|surface, _| !removed_surfaces.contains(surface));
        for transaction in transactions {
            self.surface_metadata.insert(
                transaction.surface,
                LiveSurfaceProjectionMetadata {
                    namespace: transaction.namespace,
                },
            );
        }
    }

    pub(super) fn rebuild_input_layers(&mut self) {
        self.input_layers = input_layer_snapshots(
            self.production.committed_surfaces(),
            &self.presentation_order,
            &self.surface_metadata,
        );
        tracing::trace!(
            committed_scene_surfaces = self.production.committed_surfaces().len(),
            input_layers = self.input_layers.len(),
            "rebuilt input projection from committed scene"
        );
    }

    pub(super) fn compositor_layer_templates(&self) -> Vec<LayerSnapshot> {
        committed_layer_snapshots(self.production.committed_surfaces(), &self.surface_metadata)
    }
}

pub(super) fn committed_layer_snapshots(
    committed: &[CommittedSurfaceState],
    metadata: &BTreeMap<SurfaceId, LiveSurfaceProjectionMetadata>,
) -> Vec<LayerSnapshot> {
    committed
        .iter()
        .enumerate()
        .map(|(index, state)| layer_snapshot(index, state, metadata.get(&state.surface)))
        .collect()
}

fn input_layer_snapshots(
    committed: &[CommittedSurfaceState],
    presentation_order: &[SurfaceId],
    metadata: &BTreeMap<SurfaceId, LiveSurfaceProjectionMetadata>,
) -> Vec<LayerSnapshot> {
    presentation_order
        .iter()
        .enumerate()
        .filter_map(|(index, surface)| {
            let state = committed.iter().find(|state| state.surface == *surface)?;
            Some(layer_snapshot(index, state, metadata.get(surface)))
        })
        .collect()
}

fn layer_snapshot(
    index: usize,
    state: &CommittedSurfaceState,
    metadata: Option<&LiveSurfaceProjectionMetadata>,
) -> LayerSnapshot {
    LayerSnapshot {
        surface: state.surface,
        authority_local_id: None,
        namespace: metadata.and_then(|metadata| metadata.namespace),
        stack_rank: u32::try_from(index).unwrap_or(u32::MAX),
        geometry: state.geometry,
        source: state.buffer,
        damage: state.damage.clone(),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: state.committed_generation,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    }
}
