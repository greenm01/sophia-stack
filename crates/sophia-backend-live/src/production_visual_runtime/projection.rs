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
        if self.native_suspended {
            return;
        }
        let input_layers = input_layer_snapshots(
            self.production.committed_surfaces(),
            &self.presentation_order,
            &self.surface_metadata,
        );
        for index in 0..self.input_projections.len() {
            let Some(output) = self.outputs.output_id(index) else {
                continue;
            };
            if let Some(bounds) = self.outputs.logical_viewport(output)
                && let Ok(list) = self.display_list_for_output(
                    output,
                    bounds,
                    self.production.committed_surfaces(),
                    &self.presentation_order,
                )
            {
                self.tab_frames.insert(output, list);
            }
            let (chrome_targets, chrome_occlusion) = self
                .outputs
                .logical_viewport(output)
                .map(|bounds| (output, bounds))
                .and_then(|(output, bounds)| {
                    self.indicator_strip_enabled
                        .then_some(self.indicator_publication.as_ref())
                        .flatten()
                        .and_then(|publication| {
                            sophia_engine::indicator_strip_display_command(
                                publication,
                                output,
                                bounds,
                            )
                        })
                })
                .map_or_else(
                    || (Vec::new(), None),
                    |command| direct_chrome_projection(&command),
                );
            let (descriptor_targets, descriptor_occlusion) = direct_descriptor_projection(
                self.descriptor_overlay.as_ref(),
                output,
                self.descriptor_overlay_interactive,
            );
            self.replace_presented_input_projection(
                index,
                input_layers.clone(),
                chrome_targets,
                chrome_occlusion,
                descriptor_targets,
                descriptor_occlusion,
            );
        }
        tracing::trace!(
            committed_scene_surfaces = self.production.committed_surfaces().len(),
            input_layers = input_layers.len(),
            "rebuilt input projection from committed scene"
        );
    }

    /// Publishes only pixels whose native frame has crossed an accepted page
    /// flip. Each output keeps its own snapshot and semantic epoch so one
    /// head's retirement cannot publish or invalidate another head's input.
    pub(super) fn publish_presented_input_layers(
        &mut self,
        native_scanout: &LiveProductionNativeScanout,
    ) {
        for index in 0..self.outputs.output_count() {
            let Some(output) = self.outputs.output_id(index) else {
                continue;
            };
            if let Some(frame) = native_scanout.presented_output_frame(output) {
                self.tab_frames
                    .insert(output, frame.compositor_display_list.clone());
            } else {
                self.tab_frames.remove(&output);
            }
            let (
                input_layers,
                chrome_targets,
                chrome_occlusion,
                descriptor_targets,
                descriptor_occlusion,
                presented_scene_surfaces,
            ) = native_scanout.presented_output_frame(output).map_or_else(
                || (Vec::new(), Vec::new(), None, Vec::new(), None, 0),
                |presented| {
                    let logical_viewport =
                        self.outputs.logical_viewport(output).unwrap_or_default();
                    let (chrome_targets, chrome_occlusion) =
                        presented_chrome_projection(presented, logical_viewport);
                    let (descriptor_targets, descriptor_occlusion) =
                        presented_descriptor_projection(
                            presented,
                            self.descriptor_overlay.as_ref(),
                            output,
                            self.descriptor_overlay_interactive,
                        );
                    (
                        presented_input_layer_snapshots(presented, &self.surface_metadata),
                        chrome_targets,
                        chrome_occlusion,
                        descriptor_targets,
                        descriptor_occlusion,
                        presented.surfaces.len(),
                    )
                },
            );
            self.replace_presented_input_projection(
                index,
                input_layers,
                chrome_targets,
                chrome_occlusion,
                descriptor_targets,
                descriptor_occlusion,
            );
            tracing::trace!(
                output = output.raw(),
                presented_scene_surfaces,
                input_layers = self.input_projections[index].layers.len(),
                "published output-local input projection from retired native frame"
            );
        }
    }

    fn replace_presented_input_projection(
        &mut self,
        index: usize,
        input_layers: Vec<LayerSnapshot>,
        chrome_targets: Vec<sophia_engine::IndicatorChromeHitTarget>,
        chrome_occlusion: Option<Rect>,
        descriptor_targets: Vec<sophia_engine::PresentedChromeTarget>,
        descriptor_occlusion: Option<Rect>,
    ) {
        let Some(output) = self.input_projections.get(index).map(|p| p.output) else {
            return;
        };
        let mut descriptor_targets = descriptor_targets;
        let mut tab_occlusions = Vec::new();
        if let Some(frame) = self.tab_frames.get(&output) {
            for rect in frame.rects() {
                if let sophia_engine::CompositorNodeId::TabBar { group, .. } = rect.node {
                    tab_occlusions.push(rect.geometry);
                    if let Some(bar) = self.tab_bars.iter().find(|b| {
                        b.output == output
                            && b.group == group
                            && b.generation == rect.generation
                            && self
                                .indicator_publication
                                .as_ref()
                                .is_some_and(|p| p.tab_groups.contains(&b.policy))
                    }) {
                        descriptor_targets.extend(
                            bar.targets
                                .iter()
                                .filter(|t| {
                                    t.geometry == rect.geometry
                                        && !input_layers.iter().any(|l| {
                                            sophia_engine::tab_rects_overlap(l.geometry, t.geometry)
                                        })
                                })
                                .cloned(),
                        );
                    }
                }
            }
        }
        let Some(projection) = self.input_projections.get_mut(index) else {
            return;
        };
        if !same_interaction_projection(&projection.layers, &input_layers)
            || projection.chrome_targets != chrome_targets
            || projection.chrome_occlusion != chrome_occlusion
            || projection.descriptor_targets != descriptor_targets
            || projection.descriptor_occlusion != descriptor_occlusion
            || projection.tab_occlusions != tab_occlusions
        {
            projection.epoch = projection
                .epoch
                .checked_add(1)
                .expect("presented input epoch exhausted");
        }
        projection.layers = input_layers;
        projection.chrome_targets = chrome_targets;
        projection.chrome_occlusion = chrome_occlusion;
        projection.descriptor_targets = descriptor_targets;
        projection.descriptor_occlusion = descriptor_occlusion;
        projection.tab_occlusions = tab_occlusions;
    }

    pub(super) fn compositor_layer_templates(&self) -> Vec<LayerSnapshot> {
        committed_layer_snapshots(self.production.committed_surfaces(), &self.surface_metadata)
    }

    /// Both views of the scene, from one read.
    ///
    /// The engine pairs a tick's committed surfaces against its layer templates
    /// by `SurfaceId` and fails closed on a committed surface with no template.
    /// Templates travel in the tick input while the committed list is cloned from
    /// the per-output assembly, so any site that captures them at different
    /// moments -- or from different coordinators -- can violate that invariant.
    /// Building the templates FROM the committed slice makes the two set-equal by
    /// construction, which turns a discipline every tick site had to remember
    /// into a property none of them can break.
    pub(super) fn scene_views(&self) -> (Vec<LayerSnapshot>, Vec<CommittedSurfaceState>) {
        let committed = self.production.committed_surfaces().to_vec();
        let templates = committed_layer_snapshots(&committed, &self.surface_metadata);
        (templates, committed)
    }
}

fn direct_descriptor_projection(
    overlay: Option<&sophia_engine::DescriptorOverlayProjection>,
    output: OutputId,
    interactive: bool,
) -> (Vec<sophia_engine::PresentedChromeTarget>, Option<Rect>) {
    overlay
        .filter(|overlay| overlay.output == output)
        .map_or_else(
            || (Vec::new(), None),
            |overlay| {
                (
                    if interactive {
                        overlay.targets.clone()
                    } else {
                        Default::default()
                    },
                    Some(overlay.geometry),
                )
            },
        )
}

fn presented_descriptor_projection(
    presented: &OutputFrameDamageSnapshot,
    overlay: Option<&sophia_engine::DescriptorOverlayProjection>,
    output: OutputId,
    interactive: bool,
) -> (Vec<sophia_engine::PresentedChromeTarget>, Option<Rect>) {
    let presented_identity =
        presented
            .compositor_display_list
            .commands
            .iter()
            .find_map(|command| match command {
                sophia_engine::CompositorDisplayCommand::Rect(rect) => match rect.node {
                    sophia_engine::CompositorNodeId::DescriptorOverlay {
                        projection,
                        slot: u16::MAX,
                        role: sophia_engine::DescriptorOverlayNodeRole::Panel,
                    } => Some((projection, rect.geometry)),
                    _ => None,
                },
                _ => None,
            });
    let Some((projection, geometry)) = presented_identity else {
        return (Vec::new(), None);
    };
    let targets = overlay
        .filter(|overlay| {
            interactive
                && overlay.output == output
                && overlay.commands.iter().any(|command| {
                    matches!(
                        command,
                        sophia_engine::CompositorDisplayCommand::Rect(rect)
                            if matches!(
                                rect.node,
                                sophia_engine::CompositorNodeId::DescriptorOverlay {
                                    projection: candidate,
                                    slot: u16::MAX,
                                    role: sophia_engine::DescriptorOverlayNodeRole::Panel,
                                } if candidate == projection
                            )
                    )
                })
        })
        .map(|overlay| overlay.targets.clone())
        .unwrap_or_default();
    (targets, Some(geometry))
}

fn direct_chrome_projection(
    command: &sophia_engine::CompositorDisplayCommand,
) -> (Vec<sophia_engine::IndicatorChromeHitTarget>, Option<Rect>) {
    match command {
        sophia_engine::CompositorDisplayCommand::IndicatorStrip(strip) => {
            (strip.strip.hit_targets.clone(), Some(strip.strip.geometry))
        }
        sophia_engine::CompositorDisplayCommand::Surface { .. }
        | sophia_engine::CompositorDisplayCommand::Border(_)
        | sophia_engine::CompositorDisplayCommand::Rect(_)
        | sophia_engine::CompositorDisplayCommand::Text(_) => (Vec::new(), None),
    }
}

fn presented_chrome_projection(
    presented: &OutputFrameDamageSnapshot,
    logical_viewport: Rect,
) -> (Vec<sophia_engine::IndicatorChromeHitTarget>, Option<Rect>) {
    let Some(strip) = presented.compositor_display_list.indicator_strips().last() else {
        return (Vec::new(), None);
    };
    if strip.strip.geometry.is_empty() || logical_viewport.is_empty() {
        return (Vec::new(), None);
    }
    let logical_strip = Rect {
        x: logical_viewport.x,
        y: logical_viewport.y,
        width: logical_viewport.width,
        height: sophia_engine::INDICATOR_STRIP_HEIGHT.min(logical_viewport.height),
    };
    let targets = sophia_engine::project_indicator_chrome_targets(
        &strip.strip.hit_targets,
        strip.strip.geometry,
        logical_strip,
    );
    (targets, Some(logical_strip))
}

fn same_interaction_projection(previous: &[LayerSnapshot], next: &[LayerSnapshot]) -> bool {
    previous.len() == next.len()
        && previous.iter().zip(next).all(|(previous, next)| {
            previous.surface == next.surface
                && previous.namespace == next.namespace
                && previous.stack_rank == next.stack_rank
                && previous.geometry == next.geometry
                && previous.transform == next.transform
                && (previous.opacity > 0.0) == (next.opacity > 0.0)
                && (previous.source != BufferSource::None) == (next.source != BufferSource::None)
        })
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
            source_size: state.source_size,
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
        // The committed record measured this raster; the geometry is only
        // where it was placed, and the two part company during a resize.
        source: state.buffer(),
        source_size: state.content.canonical_variant().pixel_size,
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
    fn layer_templates_cover_every_committed_surface() {
        // The property scene_views rests on, and the one whose loss reopens the
        // invalid-surface tick: the engine pairs committed surfaces against
        // templates by SurfaceId and fails closed on a committed surface with no
        // template, so the template projection must be total over its committed
        // slice -- one template per entry, same id, metadata or not. A filter
        // added here, however reasonable it looks, is that bug again.
        let with_metadata = surface(21, 1);
        let without_metadata = surface(22, 3);
        let committed = vec![
            CommittedSurfaceState {
                surface: with_metadata,
                committed_generation: 4,
                geometry: Rect {
                    x: 0,
                    y: 0,
                    width: 640,
                    height: 480,
                },
                content: sophia_protocol::SurfaceContentSet::singleton(
                    BufferSource::CpuBuffer { handle: 7 },
                    sophia_protocol::Size {
                        width: 640,
                        height: 480,
                    },
                ),
                damage: Region::empty(),
            },
            CommittedSurfaceState {
                surface: without_metadata,
                committed_generation: 1,
                geometry: Rect {
                    x: 640,
                    y: 0,
                    width: 640,
                    height: 480,
                },
                content: sophia_protocol::SurfaceContentSet::singleton(
                    BufferSource::CpuBuffer { handle: 8 },
                    sophia_protocol::Size {
                        width: 640,
                        height: 480,
                    },
                ),
                damage: Region::empty(),
            },
        ];
        let metadata = BTreeMap::from([(
            with_metadata,
            LiveSurfaceProjectionMetadata {
                namespace: Some(NamespaceId::from_raw(3)),
            },
        )]);

        let templates = committed_layer_snapshots(&committed, &metadata);

        assert_eq!(templates.len(), committed.len());
        for (template, state) in templates.iter().zip(&committed) {
            assert_eq!(template.surface, state.surface);
        }
        // Missing metadata degrades the namespace, never the layer.
        assert_eq!(templates[1].namespace, None);
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
                source_size: sophia_protocol::Size {
                    width: 300,
                    height: 200,
                },
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
                    source_size: sophia_protocol::Size {
                        width: 100,
                        height: 100,
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
    fn output_local_interaction_epochs_retire_independently() {
        let primary = HeadlessOutput::deterministic();
        let secondary = HeadlessOutput {
            id: OutputId::from_raw(2),
            ..primary
        };
        let mut runtime = LiveProductionVisualRuntime::new(&[secondary, primary], None).unwrap();
        assert_eq!(runtime.input_projections()[0].output, primary.id);
        assert_eq!(runtime.input_projections()[1].output, secondary.id);
        let layer_for = |surface, handle| LayerSnapshot {
            surface,
            authority_local_id: None,
            namespace: Some(NamespaceId::from_raw(3)),
            stack_rank: 0,
            geometry: Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            },
            source: BufferSource::CpuBuffer { handle },
            source_size: sophia_protocol::Size {
                width: 100,
                height: 100,
            },
            damage: Region::default(),
            opacity: 1.0,
            crop: None,
            transform: Transform::IDENTITY,
            generation: 1,
            resize_sync: ResizeSyncCapability::ImplicitOnly,
        };

        runtime.replace_presented_input_projection(
            0,
            vec![layer_for(surface(40, 1), 1)],
            Vec::new(),
            None,
            Vec::new(),
            None,
        );
        assert_eq!(runtime.input_projections()[0].epoch, 1);
        assert_eq!(runtime.input_projections()[1].epoch, 0);

        runtime.replace_presented_input_projection(
            1,
            vec![layer_for(surface(41, 1), 2)],
            Vec::new(),
            None,
            Vec::new(),
            None,
        );
        assert_eq!(runtime.input_projections()[0].epoch, 1);
        assert_eq!(runtime.input_projections()[1].epoch, 1);

        // A buffer-only presentation is visual, not a lease-identity change.
        runtime.replace_presented_input_projection(
            0,
            vec![layer_for(surface(40, 1), 99)],
            Vec::new(),
            None,
            Vec::new(),
            None,
        );
        assert_eq!(runtime.input_projections()[0].epoch, 1);
        assert_eq!(runtime.input_projections()[1].epoch, 1);

        runtime.replace_presented_input_projection(
            0,
            Vec::new(),
            Vec::new(),
            None,
            Vec::new(),
            None,
        );
        assert_eq!(runtime.input_projections()[0].epoch, 2);
        assert_eq!(runtime.input_projections()[1].epoch, 1);
        assert_eq!(
            runtime.input_projections()[1].layers[0].surface,
            surface(41, 1)
        );
    }

    #[test]
    fn descriptor_interaction_revokes_without_withdrawing_presented_occlusion() {
        let output = HeadlessOutput::deterministic();
        let geometry = Rect {
            x: 20,
            y: 20,
            width: 240,
            height: 80,
        };
        let target = sophia_engine::PresentedChromeTarget {
            id: sophia_engine::PresentedChromeTargetId {
                authority_session_epoch: 4,
                slot: 1,
                generation: 7,
            },
            output: output.id,
            geometry,
            action: sophia_protocol::ToplevelActionCapabilityRef {
                token: 9,
                issuer_epoch: 2,
                issuer_revocation_epoch: 3,
                recipient_epoch: 4,
                target_slot: 1,
                target_generation: 7,
            },
        };
        let mut runtime = LiveProductionVisualRuntime::new(&[output], None).unwrap();
        runtime.replace_presented_input_projection(
            0,
            Vec::new(),
            Vec::new(),
            None,
            vec![target],
            Some(geometry),
        );
        assert_eq!(runtime.input_projections()[0].epoch, 1);

        assert_eq!(runtime.revoke_descriptor_overlay_interaction(), 1);
        assert!(runtime.input_projections()[0].descriptor_targets.is_empty());
        assert_eq!(
            runtime.input_projections()[0].descriptor_occlusion,
            Some(geometry)
        );
        assert_eq!(runtime.input_projections()[0].epoch, 2);
    }

    #[test]
    fn retired_descriptor_frame_publishes_only_current_interaction() {
        let output = HeadlessOutput::deterministic();
        let geometry = Rect {
            x: 10,
            y: 10,
            width: 300,
            height: 64,
        };
        let command = CompositorDisplayCommand::Rect(sophia_engine::CompositorRect {
            node: sophia_engine::CompositorNodeId::DescriptorOverlay {
                projection: 5,
                slot: u16::MAX,
                role: sophia_engine::DescriptorOverlayNodeRole::Panel,
            },
            generation: 5,
            geometry,
            color: sophia_engine::CompositorRgb8 {
                red: 1,
                green: 2,
                blue: 3,
            },
        });
        let target = sophia_engine::PresentedChromeTarget {
            id: sophia_engine::PresentedChromeTargetId {
                authority_session_epoch: 4,
                slot: 1,
                generation: 7,
            },
            output: output.id,
            geometry,
            action: sophia_protocol::ToplevelActionCapabilityRef {
                token: 9,
                issuer_epoch: 2,
                issuer_revocation_epoch: 3,
                recipient_epoch: 4,
                target_slot: 1,
                target_generation: 7,
            },
        };
        let overlay = sophia_engine::DescriptorOverlayProjection {
            output: output.id,
            generation: 8,
            geometry,
            commands: vec![command.clone()],
            targets: vec![target.clone()],
        };
        let presented = OutputFrameDamageSnapshot {
            output,
            surfaces: Vec::new(),
            compositor_display_list: CompositorDisplayList {
                output: output.id,
                commands: vec![command],
            },
            software_cursor: None,
        };

        assert_eq!(
            presented_descriptor_projection(&presented, Some(&overlay), output.id, true),
            (vec![target], Some(geometry))
        );
        assert_eq!(
            presented_descriptor_projection(&presented, Some(&overlay), output.id, false),
            (Vec::new(), Some(geometry))
        );
    }

    #[test]
    fn committed_authority_state_does_not_publish_before_output_run() {
        let output = HeadlessOutput::deterministic();
        let mut runtime = LiveProductionVisualRuntime::new(&[output], None).unwrap();
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
            presentation_extent: sophia_protocol::Size {
                width: 200,
                height: 100,
            },
            content: sophia_protocol::SurfaceContentSet::singleton(
                BufferSource::CpuBuffer { handle: 77 },
                sophia_protocol::Size {
                    width: 200,
                    height: 100,
                },
            ),

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
