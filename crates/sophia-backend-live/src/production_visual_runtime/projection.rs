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

    /// Publishes the committed scene for runtimes whose output tick is also
    /// their presentation boundary (currently the non-native/headless path).
    pub(super) fn publish_committed_input_layers(&mut self) {
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

    /// Publishes only pixels whose native frame has crossed an accepted page
    /// flip. Sophia's current pointer coordinate domain follows the primary
    /// output; output-local pointer domains must be introduced before these
    /// snapshots may be merged across independently retiring heads.
    pub(super) fn publish_presented_input_layers(
        &mut self,
        native_scanout: &LiveProductionNativeScanout,
    ) {
        let Some(primary) = self.outputs.primary_output() else {
            self.input_layers.clear();
            return;
        };
        let Some(index) = self.outputs.output_index(primary) else {
            self.input_layers.clear();
            return;
        };
        let Some(presented) = native_scanout.presented_output_frame(index) else {
            // Initial native setup may not yet have an interaction-bearing
            // frame. Never substitute newer committed state.
            self.input_layers.clear();
            return;
        };
        self.input_layers = presented_input_layer_snapshots(presented, &self.surface_metadata);
        tracing::trace!(
            output = primary.raw(),
            presented_scene_surfaces = presented.surfaces.len(),
            input_layers = self.input_layers.len(),
            "published input projection from retired native frame"
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

fn presented_input_layer_snapshots(
    presented: &OutputFrameDamageSnapshot,
    metadata: &BTreeMap<SurfaceId, LiveSurfaceProjectionMetadata>,
) -> Vec<LayerSnapshot> {
    presented
        .surfaces
        .iter()
        .enumerate()
        .map(|(index, state)| LayerSnapshot {
            surface: state.surface,
            authority_local_id: None,
            namespace: metadata
                .get(&state.surface)
                .and_then(|metadata| metadata.namespace),
            stack_rank: u32::try_from(index).unwrap_or(u32::MAX),
            geometry: state.geometry,
            source: state.buffer,
            damage: Region::default(),
            opacity: 1.0,
            crop: None,
            transform: Transform::IDENTITY,
            generation: state.committed_generation,
            resize_sync: ResizeSyncCapability::ImplicitOnly,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn surface(index: u32, generation: u32) -> SurfaceId {
        SurfaceId::new(index, generation)
    }

    #[test]
    fn presented_projection_keeps_retired_geometry_and_excludes_unpresented_surface() {
        let retired = surface(11, 2);
        let committed_only = surface(12, 1);
        let presented = OutputFrameDamageSnapshot {
            output: HeadlessOutput::deterministic(),
            surfaces: vec![OutputFrameSurfaceState {
                surface: retired,
                committed_generation: 7,
                geometry: Rect {
                    x: 10,
                    y: 20,
                    width: 300,
                    height: 200,
                },
                buffer: BufferSource::CpuBuffer { handle: 41 },
            }],
            compositor_display_list: CompositorDisplayList {
                output: OutputId::from_raw(1),
                commands: vec![CompositorDisplayCommand::Surface { surface: retired }],
            },
            software_cursor: None,
        };
        let metadata = BTreeMap::from([
            (
                retired,
                LiveSurfaceProjectionMetadata {
                    namespace: Some(NamespaceId::from_raw(8)),
                },
            ),
            (
                committed_only,
                LiveSurfaceProjectionMetadata {
                    namespace: Some(NamespaceId::from_raw(9)),
                },
            ),
        ]);

        let layers = presented_input_layer_snapshots(&presented, &metadata);

        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].surface, retired);
        assert_eq!(layers[0].generation, 7);
        assert_eq!(layers[0].geometry, presented.surfaces[0].geometry);
        assert_eq!(layers[0].namespace, Some(NamespaceId::from_raw(8)));
        assert!(!layers.iter().any(|layer| layer.surface == committed_only));
    }

    #[test]
    fn presented_projection_preserves_retired_stacking_order() {
        let lower = surface(21, 1);
        let upper = surface(22, 1);
        let presented = OutputFrameDamageSnapshot {
            output: HeadlessOutput::deterministic(),
            surfaces: [lower, upper]
                .into_iter()
                .enumerate()
                .map(|(index, surface)| OutputFrameSurfaceState {
                    surface,
                    committed_generation: 1,
                    geometry: Rect {
                        x: i32::try_from(index).unwrap_or_default() * 10,
                        y: 0,
                        width: 100,
                        height: 100,
                    },
                    buffer: BufferSource::CpuBuffer {
                        handle: u64::try_from(index).unwrap_or_default() + 1,
                    },
                })
                .collect(),
            compositor_display_list: CompositorDisplayList {
                output: OutputId::from_raw(1),
                commands: vec![
                    CompositorDisplayCommand::Surface { surface: lower },
                    CompositorDisplayCommand::Surface { surface: upper },
                ],
            },
            software_cursor: None,
        };

        let layers = presented_input_layer_snapshots(&presented, &BTreeMap::new());

        assert_eq!(layers[0].stack_rank, 0);
        assert_eq!(layers[1].stack_rank, 1);
    }

    #[test]
    fn committed_authority_state_does_not_publish_before_output_run() {
        let output = HeadlessOutput::deterministic();
        let mut runtime = LiveProductionVisualRuntime::new(&[output], None, None).unwrap();
        let transaction = SurfaceTransaction {
            transaction: TransactionId::from_raw(1),
            authority: AuthorityKind::SophiaX,
            surface: surface(31, 1),
            namespace: Some(NamespaceId::from_raw(4)),
            target_geometry: Rect {
                x: 50,
                y: 60,
                width: 200,
                height: 100,
            },
            target_content_size: Size {
                width: 200,
                height: 100,
            },
            target_buffer: BufferSource::CpuBuffer { handle: 77 },
            damage: Region::single(Rect {
                x: 0,
                y: 0,
                width: 200,
                height: 100,
            }),
            readiness: SurfaceTransactionReadiness::Ready,
            timeout_msec: 250,
            previous_committed_generation: 0,
        };

        runtime
            .prepare_authority_transactions(
                TransactionId::from_raw(1),
                std::slice::from_ref(&transaction),
                &[],
            )
            .unwrap();

        assert_eq!(runtime.committed_surfaces().len(), 1);
        assert!(runtime.input_layers().is_empty());
    }
}
