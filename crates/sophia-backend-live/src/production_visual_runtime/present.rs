use super::*;

impl LiveProductionVisualRuntime {
    pub fn drive_gpu_presentation(
        &mut self,
        native_scanout: Option<&mut LiveProductionNativeScanout>,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let transaction = match self
            .present_scheduler
            .poll_gate(self.presentation_feedback.resources_mut(), Instant::now())?
        {
            LiveProductionPresentGate::Idle
            | LiveProductionPresentGate::SubmittedInFlight
            | LiveProductionPresentGate::WaitingAcquire => {
                return self.run_observation_tick();
            }
            LiveProductionPresentGate::Reject(transaction) => {
                self.reject_gpu_presentation(transaction);
                return self.run_observation_tick();
            }
            LiveProductionPresentGate::Ready(transaction) => transaction,
        };
        let Some(native_scanout) = native_scanout else {
            self.present_scheduler.pop_front();
            self.reject_gpu_presentation(transaction);
            return self.run_observation_tick();
        };
        let queued = self
            .present_scheduler
            .front()
            .ok_or("ready Present gate has no queued presentation")?;
        let queued_surface = queued.surface;
        let queued_candidate = queued.candidate.key();
        if !self.presentation_order.contains(&queued_surface) {
            self.present_scheduler.pop_front();
            self.reject_gpu_presentation(transaction);
            return self.run_observation_tick();
        }

        tracing::trace!(
            transaction = transaction.raw(),
            candidate_surfaces = 1,
            committed_scene_surfaces = self.production.committed_surfaces().len(),
            queued_presents = self.present_scheduler.has_queued(),
            "preparing queued Present against committed scene"
        );
        let prepared = self
            .production
            .prepare_present_transaction(&queued.candidate);
        if !prepared.is_ready() {
            self.present_scheduler.pop_front();
            self.reject_gpu_presentation(transaction);
            return self.run_observation_tick();
        }
        let primary_output = self
            .outputs
            .primary_output()
            .ok_or("persistent backend runtime has no primary output")?;
        let primary_index = self
            .outputs
            .output_index(primary_output)
            .ok_or("persistent backend primary output was not registered")?;
        if native_scanout.output_index(primary_output) != Some(primary_index) {
            return Err("persistent backend and native primary output ordering diverged".into());
        }
        if self
            .outputs
            .output_native_scanout_in_flight(primary_index)
            .ok_or("persistent backend primary in-flight state was not registered")?
            || self
                .outputs
                .output_native_cleanup_pending(primary_index)
                .ok_or("persistent backend primary cleanup state was not registered")?
        {
            return self.run_observation_tick();
        }
        let mut mixed = self.presentation_feedback.resources().build_mixed_frame(
            transaction,
            None,
            queued.target,
            Some(queued.surface_clip),
            1.0,
        )?;
        let current_layer = mixed
            .layers
            .iter()
            .find_map(|layer| match layer {
                LiveOwnedMixedCompositionLayer::DmaBuf {
                    image_id,
                    frame,
                    placement,
                } => Some(LiveRetainedDmaBufLayer {
                    image_id: *image_id,
                    frame: frame.try_clone().ok()?,
                    placement: *placement,
                }),
                LiveOwnedMixedCompositionLayer::Cpu { .. }
                | LiveOwnedMixedCompositionLayer::Solid { .. } => None,
            })
            .ok_or("ready Present frame did not retain its DMA-BUF")?;
        if !current_layer.has_unit_scale() {
            self.present_scheduler.pop_front();
            tracing::warn!(
                transaction = transaction.raw(),
                surface = queued_surface.index(),
                source_width = current_layer.frame.width,
                source_height = current_layer.frame.height,
                logical_width = current_layer
                    .placement
                    .clip
                    .unwrap_or(current_layer.placement.target)
                    .width,
                logical_height = current_layer
                    .placement
                    .clip
                    .unwrap_or(current_layer.placement.target)
                    .height,
                "rejected Present whose pixels do not match the logical X11 surface"
            );
            self.reject_gpu_presentation(transaction);
            return self.run_observation_tick();
        }
        let mut current_owned = Some(
            mixed
                .layers
                .pop()
                .ok_or("ready Present frame lost its current DMA-BUF layer")?,
        );
        let cpu_surfaces = queued
            .cpu_layers
            .iter()
            .map(|layer| layer.surface)
            .collect::<Vec<_>>();
        let retained_surfaces = self.displayed_surfaces.keys().copied().collect::<Vec<_>>();
        let display_list = self.display_list(prepared.candidate(), &self.presentation_order)?;
        let border_candidate = prepared.candidate().to_vec();
        let output = self
            .outputs
            .output_descriptor(0)
            .ok_or("mixed composition has no output descriptor")?;
        mixed.output_damage_snapshot = Some(output_frame_damage_snapshot(
            output,
            display_list.clone(),
            prepared.candidate(),
            None,
        )?);
        for command in display_list.commands {
            match command {
                CompositorDisplayCommand::Surface { surface } if surface == queued_surface => {
                    mixed.layers.push(
                        current_owned
                            .take()
                            .ok_or("current Present appeared twice in the layout")?,
                    );
                }
                CompositorDisplayCommand::Surface { surface }
                    if cpu_surfaces.contains(&surface) =>
                {
                    let layer = queued
                        .cpu_layers
                        .iter()
                        .find(|layer| layer.surface == surface)
                        .ok_or("ordered CPU layer disappeared from queued Present")?;
                    mixed.layers.push(LiveOwnedMixedCompositionLayer::Cpu {
                        buffer: layer.buffer.clone(),
                        placement: LiveCompositionPlacement {
                            target: layer.geometry,
                            clip: None,
                            transform: Transform::IDENTITY,
                            alpha: 1.0,
                        },
                    });
                }
                CompositorDisplayCommand::Surface { surface }
                    if retained_surfaces.contains(&surface) =>
                {
                    let displayed = self
                        .displayed_surfaces
                        .get(&surface)
                        .ok_or("ordered retained DMA-BUF layer disappeared")?;
                    mixed.layers.push(LiveOwnedMixedCompositionLayer::DmaBuf {
                        image_id: displayed.layer.image_id,
                        frame: displayed.layer.frame.try_clone()?,
                        placement: displayed.layer.placement,
                    });
                }
                CompositorDisplayCommand::Surface { .. } => {}
                CompositorDisplayCommand::Border(border) => {
                    for band in compositor_border_bands(border) {
                        if !band.geometry.is_empty() {
                            mixed.layers.push(LiveOwnedMixedCompositionLayer::Solid {
                                geometry: band.geometry,
                                color: band.color,
                            });
                        }
                    }
                }
            }
        }
        if current_owned.is_some() {
            return Err("visible Present surface is missing from the presentation order".into());
        }
        self.record_focus_ring_observation(&border_candidate, false)?;
        if self.present_scheduler.take_diagnose_first_mixed_export() {
            let (cpu_layers, dmabuf_layers) =
                mixed
                    .layers
                    .iter()
                    .fold((0usize, 0usize), |(cpu, dmabuf), layer| match layer {
                        crate::LiveOwnedMixedCompositionLayer::Cpu { .. } => {
                            (cpu.saturating_add(1), dmabuf)
                        }
                        crate::LiveOwnedMixedCompositionLayer::DmaBuf { .. } => {
                            (cpu, dmabuf.saturating_add(1))
                        }
                        crate::LiveOwnedMixedCompositionLayer::Solid { .. } => (cpu, dmabuf),
                    });
            let (status, detail) = native_scanout.diagnose_mixed_frame(primary_index, mixed);
            self.present_scheduler.pop_front();
            let _ = self
                .presentation_feedback
                .resources_mut()
                .reject(transaction);
            let _ = self.presentation_feedback.disconnect();
            return Err(Box::new(crate::LiveNativeMixedDiagnosticComplete {
                status,
                detail,
                cpu_layers,
                dmabuf_layers,
                live_sources: self.presentation_feedback.resources().source_count(),
                live_fences: self.presentation_feedback.resources().fence_count(),
                live_transactions: self.presentation_feedback.resources().presentation_count(),
            }));
        }
        let output_count = self.outputs.output_count();
        native_scanout.queue_mixed_frame(primary_index, transaction, mixed);

        let layer_templates = self.compositor_layer_templates();
        let production = &self.production;
        let outputs = &mut self.outputs;
        let mut adapter = crate::LiveProductionOutputRuntimeAdapter::new(
            output_count,
            |index, committed: &[CommittedSurfaceState]| -> Result<_, Box<dyn std::error::Error>> {
                outputs.run_output(index, committed, |runtime| {
                    native_scanout.run_tick(
                        index,
                        runtime,
                        compositor_tick_input(&layer_templates, 0, Vec::new(), None),
                    )
                })
            },
        );
        let report = production
            .run_outputs(&mut adapter)?
            .into_iter()
            .nth(primary_index)
            .ok_or("persistent backend runtime has no outputs")?;
        use crate::LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus as Status;
        match report
            .rendered_primary_plane_scanout_submit
            .map(|submit| submit.status)
        {
            Some(Status::SubmittedWaitingForPageFlip) => {
                native_scanout.discard_presentation_feedback(self.outputs.primary_output());
                self.presentation_feedback
                    .resources_mut()
                    .mark_submitted(transaction)?;
                self.present_scheduler.pop_front();
                self.surface_content_fence.begin(queued_surface)?;
                self.present_scheduler
                    .mark_submitted(LiveProductionSubmittedPresent {
                        candidate: queued_candidate,
                        transaction,
                        surface: queued_surface,
                        prepared,
                        displayed_layer: current_layer,
                    });
            }
            Some(Status::ScanoutExportPending) => {
                self.present_scheduler.pop_front();
                self.surface_content_fence.begin(queued_surface)?;
                self.present_scheduler
                    .mark_rendering(LiveProductionSubmittedPresent {
                        candidate: queued_candidate,
                        transaction,
                        surface: queued_surface,
                        prepared,
                        displayed_layer: current_layer,
                    });
            }
            Some(Status::AlreadyInFlight | Status::CleanupPending) | None => {}
            Some(_) => {
                self.present_scheduler.pop_front();
                self.reject_gpu_presentation(transaction);
            }
        }
        Ok(report)
    }
}
