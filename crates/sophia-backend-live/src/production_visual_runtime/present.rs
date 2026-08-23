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
        let previous_geometry = self
            .production
            .committed_surfaces()
            .iter()
            .find(|state| state.surface == queued_surface)
            .map(|state| state.geometry);
        let candidate_geometry = prepared
            .candidate()
            .iter()
            .find(|state| state.surface == queued_surface)
            .map(|state| state.geometry)
            .ok_or("ready Present candidate lost its surface geometry")?;
        let logical_viewports = self.outputs.logical_viewports().collect::<Vec<_>>();
        let applicable_outputs = sophia_engine::applicable_output_retirement_set(
            &logical_viewports,
            previous_geometry,
            candidate_geometry,
        )?;
        if applicable_outputs.is_empty() {
            self.present_scheduler.pop_front();
            self.reject_gpu_presentation(transaction);
            return self.run_observation_tick();
        }
        for output in &applicable_outputs {
            let index = self
                .outputs
                .output_index(*output)
                .ok_or("Present cohort targets an unknown logical output")?;
            let in_flight = self
                .outputs
                .output_native_scanout_in_flight(index)
                .ok_or("Present output in-flight state was not registered")?;
            let cleanup_pending = self
                .outputs
                .output_native_cleanup_pending(index)
                .ok_or("Present output cleanup state was not registered")?;
            let pending_frame = native_scanout.pending_frame(*output);
            if in_flight || cleanup_pending || !native_scanout.frame_queue_ready(*output) {
                // The primary can no longer reach this: the reducer withholds
                // the presentation effect while it owes a native frame. A
                // cohort spanning outputs can still find a secondary busy, and
                // that one is drained by this same pass. Deferring silently is
                // what let the earlier deadlock hide, so it is said out loud --
                // at info, because the sessions that need this run at info and
                // a debug line there is the same silence by another name.
                // Coalesced on powers of two so a flood stays legible.
                self.present_output_busy_defers = self.present_output_busy_defers.saturating_add(1);
                if self.present_output_busy_defers.is_power_of_two() {
                    tracing::info!(
                        "sophia_live_present_defer schema=1 status=output_busy defers={} transaction={} output={} in_flight={in_flight} cleanup_pending={cleanup_pending} pending_frame={pending_frame}",
                        self.present_output_busy_defers,
                        transaction.raw(),
                        output.raw(),
                    );
                }
                return self.run_observation_tick();
            }
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
                } => Some(LiveRetainedRendererImageLayer {
                    image_id: *image_id,
                    size: Size {
                        width: i32::try_from(frame.width).ok()?,
                        height: i32::try_from(frame.height).ok()?,
                    },
                    format: frame.format,
                    placement: *placement,
                }),
                LiveOwnedMixedCompositionLayer::Cpu { .. }
                | LiveOwnedMixedCompositionLayer::RendererImage { .. }
                | LiveOwnedMixedCompositionLayer::Solid { .. } => None,
            })
            .ok_or("ready Present frame did not retain its DMA-BUF")?;
        if !current_layer.has_unit_scale() {
            self.present_scheduler.pop_front();
            tracing::warn!(
                transaction = transaction.raw(),
                surface = queued_surface.index(),
                source_width = current_layer.size.width,
                source_height = current_layer.size.height,
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
        let current_owned = mixed
            .layers
            .pop()
            .ok_or("ready Present frame lost its current DMA-BUF layer")?;
        let mut current_source = Some(match current_owned {
            LiveOwnedMixedCompositionLayer::DmaBuf {
                image_id, frame, ..
            } => sophia_renderer_live::LiveOwnedHeadCompositionSource {
                surface: queued_surface,
                source: queued.candidate.target_buffer(),
                kind: sophia_renderer_live::LiveOwnedHeadCompositionSourceKind::DmaBuf {
                    image_id,
                    frame,
                },
            },
            _ => return Err("ready Present source changed renderer kind".into()),
        });
        let mut head_sources = Vec::new();
        let cpu_surfaces = queued
            .cpu_layers
            .iter()
            .map(|layer| layer.surface)
            .collect::<Vec<_>>();
        let retained_surfaces = self.displayed_surfaces.keys().copied().collect::<Vec<_>>();
        let display_list = self.display_list(prepared.candidate(), &self.presentation_order)?;
        let border_candidate = prepared.candidate().to_vec();
        for command in &display_list.commands {
            match command {
                CompositorDisplayCommand::Surface { surface } if *surface == queued_surface => {
                    head_sources.push(
                        current_source
                            .take()
                            .ok_or("current Present appeared twice in the layout")?,
                    );
                }
                CompositorDisplayCommand::Surface { surface }
                    if cpu_surfaces.contains(&surface) =>
                {
                    let layers = queued
                        .cpu_layers
                        .iter()
                        .filter(|layer| layer.surface == *surface)
                        .collect::<Vec<_>>();
                    if layers.is_empty() {
                        return Err("ordered CPU layer disappeared from queued Present".into());
                    }
                    head_sources.extend(layers.into_iter().map(|layer| {
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
                }
                CompositorDisplayCommand::Surface { surface }
                    if retained_surfaces.contains(&surface) =>
                {
                    let displayed = self
                        .displayed_surfaces
                        .get(&surface)
                        .ok_or("ordered retained DMA-BUF layer disappeared")?;
                    let source = prepared
                        .candidate()
                        .iter()
                        .find(|state| state.surface == *surface)
                        .map(CommittedSurfaceState::buffer)
                        .ok_or("retained DMA-BUF source disappeared from candidate")?;
                    if !matches!(source, BufferSource::DmaBuf { .. }) {
                        return Err("retained renderer image lost its DMA-BUF identity".into());
                    }
                    head_sources.push(
                        sophia_renderer_live::LiveOwnedHeadCompositionSource {
                            surface: *surface,
                            source,
                            kind: sophia_renderer_live::LiveOwnedHeadCompositionSourceKind::RendererImage {
                            image_id: displayed.layer.image_id,
                            size: displayed.layer.size,
                            format: displayed.layer.format,
                            },
                        },
                    );
                }
                CompositorDisplayCommand::Surface { .. } => {}
                CompositorDisplayCommand::Border(_)
                | CompositorDisplayCommand::Rect(_)
                | CompositorDisplayCommand::Text(_)
                | CompositorDisplayCommand::IndicatorStrip(_) => {}
            }
        }
        if current_source.is_some() {
            return Err("visible Present surface is missing from the presentation order".into());
        }
        self.record_focus_ring_observation(&border_candidate, false)?;
        let mut output_head_frames = applicable_outputs
            .iter()
            .map(|output| {
                let logical_viewport = self
                    .outputs
                    .logical_viewport(*output)
                    .ok_or("Present targets an unknown logical output")?;
                let output_display_list = self.display_list_for_output(
                    *output,
                    logical_viewport,
                    prepared.candidate(),
                    &self.presentation_order,
                )?;
                Ok((
                    *output,
                    self.compose_native_head_frames_from_sources(
                        native_scanout,
                        *output,
                        prepared.candidate(),
                        output_display_list,
                        transaction.raw(),
                        &head_sources,
                    )?,
                ))
            })
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
        if self.present_scheduler.take_diagnose_first_mixed_export() {
            let (cpu_layers, dmabuf_layers) =
                head_sources
                    .iter()
                    .fold((0usize, 0usize), |(cpu, dmabuf), source| {
                        match source.kind {
                    sophia_renderer_live::LiveOwnedHeadCompositionSourceKind::Cpu(_) => {
                        (cpu.saturating_add(1), dmabuf)
                    }
                    sophia_renderer_live::LiveOwnedHeadCompositionSourceKind::DmaBuf { .. }
                    | sophia_renderer_live::LiveOwnedHeadCompositionSourceKind::RendererImage {
                        ..
                    } => (cpu, dmabuf.saturating_add(1)),
                }
                    });
            let (diagnostic_output, diagnostic_frames) = output_head_frames
                .first_mut()
                .ok_or("head planner produced no diagnostic output")?;
            let diagnostic = diagnostic_frames
                .pop()
                .ok_or("head planner produced no diagnostic frame")?;
            let (status, detail) =
                native_scanout.diagnose_mixed_frame(*diagnostic_output, diagnostic.frame);
            native_scanout.rollback_renderer_image(current_layer.image_id)?;
            self.present_scheduler.pop_front();
            self.reject_gpu_presentation(transaction);
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
        let frames = native_scanout
            .queue_present_output_head_composition_frames(transaction, output_head_frames)?;
        self.present_scheduler.pop_front();
        self.present_scheduler.mark_rendering(
            LiveProductionSubmittedPresent::new(
                frames.clone(),
                queued_candidate,
                transaction,
                queued_surface,
                prepared,
                current_layer,
            )
            .ok_or("Present rendering has no output cohort")?,
        );

        let production = &self.production;
        let surface_metadata = &self.surface_metadata;
        let outputs = &mut self.outputs;
        let mut adapter = crate::LiveProductionOutputRuntimeAdapter::new(
            output_count,
            |index, committed: &[CommittedSurfaceState]| -> Result<_, Box<dyn std::error::Error>> {
                let output_id = outputs
                    .output_id(index)
                    .ok_or("production output index was not registered")?;
                outputs.run_output(index, committed, |runtime| {
                    native_scanout.run_tick(
                        output_id,
                        runtime,
                        compositor_tick_input_for_committed(
                            committed,
                            surface_metadata,
                            0,
                            Vec::new(),
                            None,
                        ),
                    )
                })
            },
        );
        let reports = production.run_outputs(&mut adapter)?;
        use crate::LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus as Status;
        let mut selected_report = None;
        for (index, report) in reports.into_iter().enumerate() {
            let output = self
                .outputs
                .output_id(index)
                .ok_or("Present tick report lost its logical output")?;
            if !frames.contains_key(&output) {
                continue;
            }
            if selected_report.is_none() {
                selected_report = Some(report.clone());
            }
            match report
                .rendered_primary_plane_scanout_submit
                .map(|submit| submit.status)
            {
                Some(Status::SubmittedWaitingForPageFlip) => {
                    native_scanout.discard_presentation_feedback(Some(output));
                    if let Some(submitted_transaction) =
                        self.present_scheduler.mark_output_submitted(output)?
                    {
                        self.presentation_feedback
                            .resources_mut()
                            .mark_submitted(submitted_transaction)?;
                    }
                    self.observe_software_present_frame_submitted(frames[&output])?;
                }
                Some(Status::ScanoutExportPending)
                | Some(Status::AlreadyInFlight | Status::CleanupPending)
                | None => {}
                Some(status) => {
                    return Err(format!(
                        "Present output cohort failed after admission: output={} status={status:?}",
                        output.raw()
                    )
                    .into());
                }
            }
        }
        selected_report.ok_or_else(|| "Present cohort produced no output tick report".into())
    }
}
