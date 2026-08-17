use super::*;

pub(super) struct LiveProductionRetainedCompositionSourceSet {
    pub committed: Vec<CommittedSurfaceState>,
    pub display_list: CompositorDisplayList,
    pub presentation_order: Vec<SurfaceId>,
    pub scene_generation: u64,
    pub sources: Vec<sophia_renderer_live::LiveOwnedHeadCompositionSource>,
}

impl LiveProductionVisualRuntime {
    pub(super) fn cpu_output_head_composition_frames_from_layers(
        &self,
        native_scanout: &LiveProductionNativeScanout,
        cpu_layers: &[LiveCpuPresentationLayer],
        scene_generation: u64,
    ) -> Result<
        Vec<(OutputId, Vec<crate::LiveProductionHeadCompositionFrame>)>,
        Box<dyn std::error::Error>,
    > {
        let committed = self.production.committed_surfaces();
        let display_list = self.display_list(committed, &self.presentation_order)?;
        let sources = cpu_layers
            .iter()
            .map(
                |source| sophia_renderer_live::LiveOwnedHeadCompositionSource {
                    surface: source.surface,
                    source: BufferSource::CpuBuffer {
                        handle: source.buffer.handle,
                    },
                    kind: sophia_renderer_live::LiveOwnedHeadCompositionSourceKind::Cpu(
                        source.buffer.clone().into(),
                    ),
                },
            )
            .collect::<Vec<_>>();
        self.outputs
            .logical_viewports()
            .map(|(output, _)| {
                Ok((
                    output,
                    self.compose_native_head_frames_from_sources(
                        native_scanout,
                        output,
                        committed,
                        display_list.clone(),
                        scene_generation.max(1),
                        &sources,
                    )?,
                ))
            })
            .collect()
    }

    pub(super) fn compose_native_head_frames_from_sources(
        &self,
        native_scanout: &LiveProductionNativeScanout,
        output: OutputId,
        committed: &[CommittedSurfaceState],
        display_list: CompositorDisplayList,
        scene_generation: u64,
        sources: &[sophia_renderer_live::LiveOwnedHeadCompositionSource],
    ) -> Result<Vec<crate::LiveProductionHeadCompositionFrame>, Box<dyn std::error::Error>> {
        let logical_viewport = self
            .outputs
            .logical_viewport(output)
            .ok_or("head composition targets an unknown logical output")?;
        let snapshot = sophia_engine::output_scene_snapshot_from_committed_in_view(
            output,
            scene_generation.max(1),
            logical_viewport,
            committed,
            display_list,
            None,
        )?;
        let targets = native_scanout.head_render_targets(output);
        let plans = sophia_engine::build_output_head_plans(&snapshot, &targets)?;
        if plans.len() != targets.len() {
            return Err("head composition planner returned partial target coverage".into());
        }
        for plan in &plans {
            trace_live_head_composition_plan(plan);
        }
        plans
            .iter()
            .map(|plan| {
                Ok(crate::LiveProductionHeadCompositionFrame {
                    head: plan.head,
                    scene_generation: plan.scene_generation,
                    target_generation: plan.target_generation,
                    mapping: plan.mapping,
                    logical_content_checksum: plan.logical_content_checksum,
                    frame: sophia_renderer_live::lower_head_composition_plan(plan, sources)?,
                })
            })
            .collect()
    }

    pub(super) fn retained_composition_source_set(
        &self,
        scene: &LiveProductionCpuScene,
    ) -> Result<LiveProductionRetainedCompositionSourceSet, Box<dyn std::error::Error>> {
        let committed = self
            .present_scheduler
            .in_flight_candidate()
            .unwrap_or_else(|| self.production.committed_surfaces())
            .to_vec();
        let retained_order =
            live_production_retained_surface_order(&self.presentation_order, &committed);
        let display_list = self.display_list(&committed, &retained_order)?;
        let cpu_layers = scene.presentation_variant_layers(&committed, &retained_order);
        let in_flight = self.present_scheduler.in_flight_displayed_layer();
        let mut sources = Vec::new();
        for command in &display_list.commands {
            let CompositorDisplayCommand::Surface { surface } = command else {
                continue;
            };
            let committed_source = committed
                .iter()
                .find(|state| state.surface == *surface)
                .map(CommittedSurfaceState::buffer)
                .ok_or("retained display list escaped committed Engine membership")?;
            if let Some((_, displayed)) =
                in_flight.filter(|(in_flight_surface, _)| *in_flight_surface == *surface)
            {
                sources.push(sophia_renderer_live::LiveOwnedHeadCompositionSource {
                    surface: *surface,
                    source: committed_source,
                    kind: sophia_renderer_live::LiveOwnedHeadCompositionSourceKind::RendererImage {
                        image_id: displayed.image_id,
                        size: displayed.size,
                        format: displayed.format,
                    },
                });
                continue;
            }
            let cpu_sources = cpu_layers
                .iter()
                .filter(|layer| layer.surface == *surface)
                .collect::<Vec<_>>();
            if !cpu_sources.is_empty() {
                sources.extend(cpu_sources.into_iter().map(|layer| {
                    sophia_renderer_live::LiveOwnedHeadCompositionSource {
                        surface: *surface,
                        source: BufferSource::CpuBuffer {
                            handle: layer.buffer.handle,
                        },
                        kind: sophia_renderer_live::LiveOwnedHeadCompositionSourceKind::Cpu(
                            layer.buffer.clone().into(),
                        ),
                    }
                }));
                continue;
            }
            if let Some(displayed) = self.displayed_surfaces.get(surface) {
                if !matches!(committed_source, BufferSource::DmaBuf { .. }) {
                    return Err("retained renderer image lost its DMA-BUF identity".into());
                }
                sources.push(sophia_renderer_live::LiveOwnedHeadCompositionSource {
                    surface: *surface,
                    source: committed_source,
                    kind: sophia_renderer_live::LiveOwnedHeadCompositionSourceKind::RendererImage {
                        image_id: displayed.layer.image_id,
                        size: displayed.layer.size,
                        format: displayed.layer.format,
                    },
                });
                continue;
            }
            return Err("retained head plan has no authority-owned source".into());
        }
        let scene_generation = committed
            .iter()
            .map(|state| state.committed_generation)
            .max()
            .unwrap_or(1);
        Ok(LiveProductionRetainedCompositionSourceSet {
            committed,
            display_list,
            presentation_order: retained_order,
            scene_generation,
            sources,
        })
    }

    pub(super) fn retained_output_head_composition_frames_from_sources(
        &self,
        native_scanout: &LiveProductionNativeScanout,
        source_set: &LiveProductionRetainedCompositionSourceSet,
    ) -> Result<
        Vec<(OutputId, Vec<crate::LiveProductionHeadCompositionFrame>)>,
        Box<dyn std::error::Error>,
    > {
        self.outputs
            .logical_viewports()
            .map(|(output, _)| {
                Ok((
                    output,
                    self.compose_native_head_frames_from_sources(
                        native_scanout,
                        output,
                        &source_set.committed,
                        source_set.display_list.clone(),
                        source_set.scene_generation,
                        &source_set.sources,
                    )?,
                ))
            })
            .collect()
    }

    pub(super) fn retained_output_head_composition_frames(
        &self,
        scene: &LiveProductionCpuScene,
        native_scanout: &LiveProductionNativeScanout,
    ) -> Result<
        Vec<(OutputId, Vec<crate::LiveProductionHeadCompositionFrame>)>,
        Box<dyn std::error::Error>,
    > {
        let source_set = self.retained_composition_source_set(scene)?;
        self.retained_output_head_composition_frames_from_sources(native_scanout, &source_set)
    }

    /// Lowers one immutable committed scene into candidate native-size frames
    /// for a provisional topology. CPU buffers and retained renderer images
    /// come from the ordinary authority-owned source set; committed DMA-BUF
    /// identities are not independently importable sources. This is read-only
    /// with respect to the live runtime: the caller must not publish or install
    /// the candidate until its KMS transaction and first-presentation barrier
    /// complete.
    pub fn compose_output_topology_head_frames(
        &self,
        scene: &LiveProductionCpuScene,
        resolved: &crate::LiveResolvedOutputTopology,
        scene_generation: u64,
    ) -> Result<Vec<crate::LiveProductionHeadCompositionFrame>, Box<dyn std::error::Error>> {
        if scene_generation == 0 {
            return Err("topology composition requires a valid scene generation".into());
        }
        let source_set = self.retained_composition_source_set(scene)?;
        let targets = resolved.head_render_targets();
        if targets.len() != resolved.targets.len() {
            return Err("topology render-target projection is incomplete".into());
        }
        let mut frames = Vec::with_capacity(targets.len());
        for viewport in &resolved.logical_viewports {
            let mut display_list = surface_chrome_display_list_for_surfaces(
                viewport.output,
                &source_set.presentation_order,
                &self.chrome_surfaces,
                &source_set.committed,
                self.focused_surface,
                self.surface_chrome_style,
            )?;
            if let Some(outline) = self.floating_outline {
                if display_list.commands.len() >= MAX_COMPOSITOR_DISPLAY_COMMANDS {
                    return Err("topology display-list capacity exceeded".into());
                }
                let border = compositor_floating_outline(
                    outline.surface,
                    outline.geometry,
                    self.surface_chrome_style.focus_ring.width.max(2),
                    self.surface_chrome_style.focus_ring.color,
                )
                .ok_or("topology floating outline is invalid")?;
                display_list
                    .commands
                    .push(CompositorDisplayCommand::Border(border));
            }
            let snapshot = sophia_engine::output_scene_snapshot_from_committed_in_view(
                viewport.output,
                scene_generation,
                viewport.logical,
                &source_set.committed,
                display_list,
                None,
            )?;
            let output_targets = targets
                .iter()
                .copied()
                .filter(|target| target.output == viewport.output)
                .collect::<Vec<_>>();
            let plans = sophia_engine::build_output_head_plans(&snapshot, &output_targets)?;
            for plan in &plans {
                frames.push(crate::LiveProductionHeadCompositionFrame {
                    head: plan.head,
                    scene_generation: plan.scene_generation,
                    target_generation: plan.target_generation,
                    mapping: plan.mapping,
                    logical_content_checksum: plan.logical_content_checksum,
                    frame: sophia_renderer_live::lower_head_composition_plan(
                        plan,
                        &source_set.sources,
                    )?,
                });
            }
        }
        if frames.len() != targets.len() {
            return Err("topology composition omitted an enabled head".into());
        }
        let actual = frames
            .iter()
            .map(|frame| frame.head)
            .collect::<BTreeSet<_>>();
        let expected = targets
            .iter()
            .map(|target| target.head)
            .collect::<BTreeSet<_>>();
        if actual != expected || actual.len() != frames.len() {
            return Err("topology composition repeated or targeted an unknown head".into());
        }
        Ok(frames)
    }

    pub(super) fn display_list(
        &self,
        committed_surfaces: &[CommittedSurfaceState],
        presentation_order: &[SurfaceId],
    ) -> Result<CompositorDisplayList, CompositorDisplayListError> {
        let output = self
            .outputs
            .primary_output()
            .ok_or(CompositorDisplayListError::InvalidOutput)?;
        let mut display_list = surface_chrome_display_list_for_surfaces(
            output,
            presentation_order,
            &self.chrome_surfaces,
            committed_surfaces,
            self.focused_surface,
            self.surface_chrome_style,
        )?;
        if let Some(outline) = self.floating_outline {
            if display_list.commands.len() >= MAX_COMPOSITOR_DISPLAY_COMMANDS {
                return Err(CompositorDisplayListError::CapacityExceeded);
            }
            let border = compositor_floating_outline(
                outline.surface,
                outline.geometry,
                self.surface_chrome_style.focus_ring.width.max(2),
                self.surface_chrome_style.focus_ring.color,
            )
            .ok_or(CompositorDisplayListError::InvalidSurface)?;
            display_list
                .commands
                .push(CompositorDisplayCommand::Border(border));
        }
        Ok(display_list)
    }

    pub fn set_floating_outline(
        &mut self,
        outline: Option<LiveFloatingOutline>,
        scene: &LiveProductionCpuScene,
        native_scanout: Option<&mut LiveProductionNativeScanout>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        if self.floating_outline == outline {
            return Ok(false);
        }
        self.floating_outline = outline;
        if let Some(native_scanout) = native_scanout {
            let batches = self.retained_output_head_composition_frames(scene, native_scanout)?;
            native_scanout.queue_retained_output_head_composition_frames(batches)?;
        }
        Ok(true)
    }

    pub(super) fn record_focus_ring_observation(
        &mut self,
        committed_surfaces: &[CommittedSurfaceState],
        force: bool,
    ) -> Result<(), CompositorDisplayListError> {
        let display_list = self.display_list(committed_surfaces, &self.presentation_order)?;
        if let Some(surface) = self.focused_surface {
            if let Some(border) = display_list.borders().find(|border| {
                matches!(
                    border.node,
                    CompositorNodeId::SurfaceChrome {
                        surface: border_surface,
                        role: SurfaceChromeRole::FocusRing,
                    } if border_surface == surface
                )
            }) {
                let observation = LiveFocusRingObservation {
                    surface,
                    generation: border.generation,
                    primitives: compositor_border_bands(border)
                        .into_iter()
                        .filter(|band| !band.geometry.is_empty())
                        .count(),
                };
                if observation.primitives > 0
                    && (force || self.last_focus_ring_observation != Some(observation))
                {
                    self.last_focus_ring_observation = Some(observation);
                    self.pending_focus_ring_observation = Some(observation);
                }
            } else {
                self.last_focus_ring_observation = None;
            }
        } else {
            self.last_focus_ring_observation = None;
        }
        let summary = compositor_chrome_summary(&display_list, self.focused_surface);
        let observation = LiveChromeSetObservation {
            generation: summary.generation,
            eligible_surfaces: self
                .chrome_surfaces
                .iter()
                .filter(|surface| self.presentation_order.contains(surface))
                .count(),
            frames: summary.frames,
            focused_frames: summary.focused_frames,
            unfocused_frames: summary.unfocused_frames,
            focus_rings: summary.focus_rings,
            primitives: summary.primitives,
            clearance: summary.clearance,
        };
        if self.last_chrome_set_observation != Some(observation) {
            self.last_chrome_set_observation = Some(observation);
            self.pending_chrome_set_observation = Some(observation);
        }
        Ok(())
    }

    pub fn take_focus_ring_observation(&mut self) -> Option<LiveFocusRingObservation> {
        self.pending_focus_ring_observation.take()
    }

    pub fn take_chrome_set_observation(&mut self) -> Option<LiveChromeSetObservation> {
        self.pending_chrome_set_observation.take()
    }
}
