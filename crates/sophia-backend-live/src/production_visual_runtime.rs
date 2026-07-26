use crate::*;
use sophia_engine::*;
use sophia_protocol::*;
use sophia_renderer_live::*;
use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

mod authority;
mod compositor_graphics;
mod native;
mod service;
pub use native::*;
pub use service::*;

#[derive(Debug)]
struct LiveDisplayedSurface {
    layer: LiveRetainedDmaBufLayer,
    retained_transaction: Option<TransactionId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveFocusedBorderObservation {
    pub surface: SurfaceId,
    pub generation: u64,
    pub primitives: usize,
}

fn replace_displayed_surface(
    displayed_surfaces: &mut BTreeMap<SurfaceId, LiveDisplayedSurface>,
    surface: SurfaceId,
    transaction: TransactionId,
    layer: LiveRetainedDmaBufLayer,
) -> Option<TransactionId> {
    let previous = displayed_surfaces.insert(
        surface,
        LiveDisplayedSurface {
            layer,
            retained_transaction: Some(transaction),
        },
    );
    previous.and_then(|displayed| displayed.retained_transaction)
}

pub struct LiveProductionVisualRuntime {
    production: sophia_engine::ProductionSessionCoordinator,
    outputs: LiveProductionOutputRuntimeSet,
    layers: BTreeMap<SurfaceId, SurfaceTransaction>,
    input_layers: Vec<LayerSnapshot>,
    presentation_feedback: crate::LiveProductionPresentFeedbackCoordinator,
    present_scheduler: LiveProductionPresentScheduler,
    displayed_surfaces: BTreeMap<SurfaceId, LiveDisplayedSurface>,
    presentation_order: Vec<SurfaceId>,
    focused_surface: Option<SurfaceId>,
    pending_focused_border_observation: Option<LiveFocusedBorderObservation>,
    last_focused_border_observation: Option<LiveFocusedBorderObservation>,
    present_feedback: VecDeque<crate::LivePresentFeedbackOutcome>,
    present_feedback_overflowed: bool,
    present_scheduling_blocked: bool,
}

const PRESENT_FEEDBACK_CAPACITY: usize = 8_192;

pub struct LiveProductionCycleRequest<'a> {
    pub batch: &'a LiveProductionAuthorityBatch,
    pub scene: &'a mut LiveProductionCpuScene,
    pub updates: Vec<crate::LiveCpuBufferUpdate>,
    pub raised_surface: Option<SurfaceId>,
    pub focused_surface: Option<SurfaceId>,
    pub cursor_presentation: LiveProductionCursorPresentation,
    pub defer_frame: bool,
    pub defer_present: bool,
    pub reject_present_for_layout: bool,
    pub output_descriptors: &'a [sophia_engine::HeadlessOutput],
    pub native_scanout: Option<&'a mut LiveProductionNativeScanout>,
    pub wm_update: Option<WmTransactionUpdate>,
    pub presentation_layout: &'a [LayerSnapshot],
}

pub struct LiveAuthorityTransactionRun<'a> {
    pub transaction_id: TransactionId,
    pub transactions: &'a [SurfaceTransaction],
    pub removed_surfaces: &'a [SurfaceId],
    pub event_count: usize,
    pub native_scanout: Option<&'a mut LiveProductionNativeScanout>,
    pub native_frames: Option<Vec<LiveProductionComposedFrame>>,
    pub wm_update: Option<WmTransactionUpdate>,
}

impl LiveProductionVisualRuntime {
    pub fn new(
        outputs: &[sophia_engine::HeadlessOutput],
        native_scanout: Option<&mut LiveProductionNativeScanout>,
        initial_native_frames: Option<Vec<LiveProductionComposedFrame>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let production = sophia_engine::ProductionSessionCoordinator::new(
            sophia_engine::HeadlessEngine::default(),
        );
        let output_runtimes = LiveProductionOutputRuntimeSet::new(
            outputs,
            &[],
            native_scanout,
            initial_native_frames,
        )?;
        Ok(Self {
            production,
            outputs: output_runtimes,
            layers: BTreeMap::new(),
            input_layers: Vec::new(),
            presentation_feedback: Default::default(),
            present_scheduler: LiveProductionPresentScheduler::default(),
            displayed_surfaces: BTreeMap::new(),
            presentation_order: Vec::new(),
            focused_surface: None,
            pending_focused_border_observation: None,
            last_focused_border_observation: None,
            present_feedback: VecDeque::with_capacity(PRESENT_FEEDBACK_CAPACITY),
            present_feedback_overflowed: false,
            present_scheduling_blocked: false,
        })
    }

    pub fn initialize_native_scanout(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
        frames: &[LiveProductionComposedFrame],
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.outputs
            .initialize_native_scanout(native_scanout, frames)
    }

    pub fn stable_present(
        &self,
        native_scanout: &LiveProductionNativeScanout,
        transaction: TransactionId,
    ) -> bool {
        self.outputs
            .primary_output()
            .is_some_and(|output| native_scanout.stable_present(output, transaction))
    }

    pub fn with_m4_proof_controls(
        mut self,
        first_acquire_delay: Option<Duration>,
        reject_first_present: bool,
        diagnose_first_mixed_export: bool,
    ) -> Self {
        self.present_scheduler = self.present_scheduler.with_controls(
            first_acquire_delay,
            reject_first_present,
            diagnose_first_mixed_export,
        );
        self
    }

    pub fn run_cpu_production_cycle(
        &mut self,
        request: LiveProductionCycleRequest<'_>,
    ) -> Result<
        (
            LiveProductionCpuCycleSubmission<crate::LiveBackendRuntimeTickReport>,
            Vec<CommittedSurfaceState>,
        ),
        Box<dyn std::error::Error>,
    > {
        let LiveProductionCycleRequest {
            batch,
            scene,
            updates,
            raised_surface,
            focused_surface,
            cursor_presentation,
            defer_frame,
            defer_present,
            reject_present_for_layout: _,
            output_descriptors,
            mut native_scanout,
            wm_update,
            presentation_layout,
        } = request;
        self.present_scheduling_blocked = defer_present;
        let focus_changed = self.focused_surface != focused_surface;
        self.focused_surface = focused_surface;
        let presentation_order_changed = self.apply_presentation_layout(presentation_layout);
        let visual_projection_changed = presentation_order_changed || focus_changed;
        let retained_cpu_layers = scene.presentation_layers(
            self.production.committed_surfaces(),
            &self.presentation_order,
        );
        let retained_projection_queued = if visual_projection_changed {
            match (
                native_scanout.as_deref_mut(),
                self.retained_mixed_frame(&retained_cpu_layers)?,
            ) {
                (Some(native_scanout), Some((transaction, frame))) => {
                    let primary = self
                        .outputs
                        .primary_output()
                        .ok_or("persistent backend runtime has no primary output")?;
                    let primary_index = self
                        .outputs
                        .output_index(primary)
                        .ok_or("persistent backend primary output was not registered")?;
                    native_scanout.queue_mixed_frame(primary_index, transaction, frame);
                    true
                }
                _ => false,
            }
        } else {
            false
        };
        self.presentation_feedback
            .observe_authority_resources(batch)?;
        self.release_removed_presentations(&batch.removed_surfaces);
        let rebased_transactions = sophia_engine::rebase_full_state_present_transactions(
            &batch.transactions,
            self.production.committed_surfaces(),
        );
        self.layers
            .retain(|surface, _| !batch.removed_surfaces.contains(surface));
        self.displayed_surfaces
            .retain(|surface, _| !batch.removed_surfaces.contains(surface));
        for transaction in &rebased_transactions {
            self.layers.insert(transaction.surface, transaction.clone());
        }
        self.rebuild_input_layers();
        let active_transactions = self.layers.values().cloned().collect::<Vec<_>>();
        let preserve_gpu_scanout = native_scanout.is_some()
            && (retained_projection_queued
                || (!presentation_order_changed
                    && live_production_projection_requires_gpu_scanout(
                        &active_transactions,
                        &self.presentation_order,
                    )));
        let defer_frame = reduce_live_production_frame_defer(
            defer_frame,
            visual_projection_changed,
            preserve_gpu_scanout,
        );
        let native_scanout = if preserve_gpu_scanout {
            None
        } else {
            native_scanout
        };
        let intake = AuthorityTransactionIntake::new(batch.transaction, rebased_transactions)
            .with_surface_removals(batch.removed_surfaces.clone());
        let (production, outputs) = (&mut self.production, &mut self.outputs);
        let output_count = outputs.output_count();
        let event_count = authority_transaction_count(&batch.transactions);
        let mut native_scanout = native_scanout;
        let create_native_frames = native_scanout.is_some();
        let mut adapter = LiveProductionCpuCycleAdapter::new(
            scene,
            &self.presentation_order,
            updates,
            raised_surface,
            focused_surface,
            cursor_presentation.composition_position(),
            defer_frame,
            create_native_frames,
            output_descriptors,
            move |_cycle,
                  committed: &[CommittedSurfaceState],
                  authority_commits: &[TransactionCommit],
                  native_frames: Option<Vec<LiveProductionComposedFrame>>| {
                let native_frames = native_frames.unwrap_or_default();
                if let Some(native_scanout) = native_scanout.as_deref_mut() {
                    outputs.initialize_native_scanout(native_scanout, &native_frames)?;
                }
                let mut native_frames = native_frames.into_iter();
                let mut output_adapter = crate::LiveProductionOutputRuntimeAdapter::new(
                    output_count,
                    |index,
                     snapshot: &[CommittedSurfaceState]|
                     -> Result<_, Box<dyn std::error::Error>> {
                        outputs.run_output(index, snapshot, |runtime| {
                            let input = compositor_tick_input(
                                &active_transactions,
                                event_count,
                                authority_commits.to_vec(),
                                wm_update.clone(),
                            );
                            Ok(match native_scanout.as_deref_mut() {
                                Some(native_scanout) => {
                                    if let Some(next_frame) = native_frames.next() {
                                        native_scanout.queue_frame(index, next_frame);
                                    }
                                    if runtime.rendered_primary_plane_scanout_in_flight() {
                                        runtime.run_tick(input)?
                                    } else {
                                        native_scanout.run_tick(index, runtime, input)?
                                    }
                                }
                                None => runtime.run_tick(input)?,
                            })
                        })
                    },
                );
                (0..output_count)
                    .map(|index| output_adapter.run_output(index, committed))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .next()
                    .ok_or_else(|| "persistent backend runtime has no outputs".into())
            },
        );
        let report = production
            .run_cycle(std::slice::from_ref(&intake), &mut adapter)
            .map_err(|error| {
                format!(
                    "production CPU cycle failed in phase {:?}: {}",
                    error.phase, error.source
                )
            })?;
        if report.submission.composed {
            self.record_focused_border_observation(&report.committed_surfaces, false)?;
        }
        Ok((report.submission, report.committed_surfaces))
    }

    pub fn run_gpu_production_cycle(
        &mut self,
        request: LiveProductionCycleRequest<'_>,
    ) -> Result<(LiveProductionCpuSubmission, Vec<CommittedSurfaceState>), Box<dyn std::error::Error>>
    {
        let LiveProductionCycleRequest {
            batch,
            scene,
            updates,
            raised_surface,
            focused_surface,
            cursor_presentation,
            defer_frame,
            defer_present,
            reject_present_for_layout,
            output_descriptors,
            mut native_scanout,
            wm_update,
            presentation_layout,
        } = request;
        self.present_scheduling_blocked = defer_present;
        self.focused_surface = focused_surface;
        let _ = self.apply_presentation_layout(presentation_layout);
        let committed_surfaces = self.committed_surfaces().to_vec();
        scene.apply_updates(updates, &committed_surfaces)?;
        let compose_started = Instant::now();
        let composition = if defer_frame {
            scene
                .last_report()
                .cloned()
                .ok_or("software redraw coalescing has no prior composed frame")?
        } else {
            let presentation_order =
                raised_presentation_order(&self.presentation_order, raised_surface);
            let display_list = self.display_list(&committed_surfaces, &presentation_order)?;
            scene
                .compose_display_list(
                    &committed_surfaces,
                    &display_list,
                    cursor_presentation.composition_position(),
                )?
                .clone()
        };
        let native_frames = if defer_frame {
            None
        } else {
            native_scanout
                .as_ref()
                .map(|_| scene.frames_for_outputs(output_descriptors))
                .transpose()?
        };
        let cpu_layers = scene.presentation_layers(&committed_surfaces, &self.presentation_order);
        if let (Some(native_scanout), Some(frames)) =
            (native_scanout.as_deref_mut(), native_frames.as_ref())
        {
            self.initialize_native_scanout(native_scanout, frames)?;
        }
        let tick = self.run_batch(
            batch,
            presentation_layout,
            if defer_frame { None } else { native_scanout },
            native_frames,
            cpu_layers,
            wm_update,
            defer_present,
            reject_present_for_layout,
        )?;
        Ok((
            LiveProductionCpuSubmission {
                tick,
                composition,
                composed: !defer_frame,
                compose_elapsed: if defer_frame {
                    Duration::ZERO
                } else {
                    compose_started.elapsed()
                },
            },
            committed_surfaces,
        ))
    }

    fn apply_presentation_layout(&mut self, layout: &[LayerSnapshot]) -> bool {
        let order_changed = self.presentation_order.len() != layout.len()
            || self
                .presentation_order
                .iter()
                .zip(layout)
                .any(|(surface, layer)| *surface != layer.surface);
        self.presentation_order.clear();
        self.presentation_order
            .extend(layout.iter().map(|layer| layer.surface));
        for layer in layout {
            self.present_scheduler
                .reproject_surface(layer.surface, layer.geometry);
            if let Some(transaction) = self.layers.get_mut(&layer.surface) {
                transaction.target_geometry = layer.geometry;
            }
            if let Some(displayed) = self.displayed_surfaces.get_mut(&layer.surface) {
                displayed.layer.reproject(layer.geometry);
            }
        }
        order_changed
    }

    pub fn run_batch(
        &mut self,
        batch: &LiveProductionAuthorityBatch,
        presentation_layout: &[LayerSnapshot],
        mut native_scanout: Option<&mut LiveProductionNativeScanout>,
        native_frames: Option<Vec<LiveProductionComposedFrame>>,
        cpu_layers: Vec<LiveCpuPresentationLayer>,
        wm_update: Option<WmTransactionUpdate>,
        defer_present: bool,
        reject_present_for_layout: bool,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        self.presentation_feedback
            .observe_authority_resources(batch)?;
        self.release_removed_presentations(&batch.removed_surfaces);
        self.displayed_surfaces
            .retain(|surface, _| !batch.removed_surfaces.contains(surface));
        if !batch.present_submissions.is_empty() {
            let superseded = self.present_scheduler.enqueue_batch(
                batch,
                presentation_layout,
                cpu_layers,
                defer_present,
                reject_present_for_layout,
                self.presentation_feedback.resources_mut(),
                Instant::now(),
            )?;
            for transaction in superseded {
                self.reject_gpu_presentation(transaction, 0, 0);
            }
            self.layers
                .retain(|surface, _| !batch.removed_surfaces.contains(surface));
            for transaction in &batch.transactions {
                self.layers.insert(transaction.surface, transaction.clone());
            }
            self.rebuild_input_layers();
            if defer_present {
                return self.run_observation_tick();
            }
            return self.drive_gpu_presentation(native_scanout.as_deref_mut());
        }
        self.run_authority_transactions(LiveAuthorityTransactionRun {
            transaction_id: batch.transaction,
            transactions: &batch.transactions,
            removed_surfaces: &batch.removed_surfaces,
            event_count: authority_transaction_count(&batch.transactions),
            native_scanout,
            native_frames,
            wm_update,
        })
    }

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
                self.reject_gpu_presentation(transaction, 0, 0);
                return self.run_observation_tick();
            }
            LiveProductionPresentGate::Ready(transaction) => transaction,
        };
        let Some(native_scanout) = native_scanout else {
            self.present_scheduler.pop_front();
            self.reject_gpu_presentation(transaction, 0, 0);
            return self.run_observation_tick();
        };
        let queued = self
            .present_scheduler
            .front()
            .ok_or("ready Present gate has no queued presentation")?;
        let queued_surface = queued.surface;
        if !self.presentation_order.contains(&queued_surface) {
            self.present_scheduler.pop_front();
            self.reject_gpu_presentation(transaction, 0, 0);
            return self.run_observation_tick();
        }

        let prepared = self
            .production
            .prepare_full_state_present(transaction, &queued.transactions);
        if !prepared.is_ready() {
            self.present_scheduler.pop_front();
            self.reject_gpu_presentation(transaction, 0, 0);
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
                LiveOwnedMixedCompositionLayer::DmaBuf { frame, placement } => {
                    Some(LiveRetainedDmaBufLayer {
                        frame: frame.try_clone().ok()?,
                        placement: *placement,
                    })
                }
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
            self.reject_gpu_presentation(transaction, 0, 0);
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
        mixed.compositor_display_list = Some(display_list.clone());
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
                        frame: displayed.layer.frame.try_clone()?,
                        placement: displayed.layer.placement,
                    });
                }
                CompositorDisplayCommand::Surface { .. } => {}
                CompositorDisplayCommand::SolidRect(rect) => {
                    mixed.layers.push(LiveOwnedMixedCompositionLayer::Solid {
                        geometry: rect.geometry,
                        color: rect.color,
                    });
                }
            }
        }
        if current_owned.is_some() {
            return Err("visible Present surface is missing from the presentation order".into());
        }
        self.record_focused_border_observation(&border_candidate, false)?;
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

        let transactions = self.layers.values().cloned().collect::<Vec<_>>();
        let production = &self.production;
        let outputs = &mut self.outputs;
        let mut adapter = crate::LiveProductionOutputRuntimeAdapter::new(
            output_count,
            |index, committed: &[CommittedSurfaceState]| -> Result<_, Box<dyn std::error::Error>> {
                outputs.run_output(index, committed, |runtime| {
                    native_scanout.run_tick(
                        index,
                        runtime,
                        compositor_tick_input(&transactions, 0, Vec::new(), None),
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
                // run_tick polls callbacks before it submits this frame. Any
                // feedback already queued here therefore belongs to an older
                // CPU frame and must not retire this Present transaction.
                native_scanout.discard_presentation_feedback(self.outputs.primary_output());
                self.presentation_feedback
                    .resources_mut()
                    .mark_submitted(transaction)?;
                self.present_scheduler.pop_front();
                self.present_scheduler
                    .mark_submitted(LiveProductionSubmittedPresent {
                        transaction,
                        surface: queued_surface,
                        prepared,
                        displayed_layer: current_layer,
                    });
            }
            Some(Status::AlreadyInFlight | Status::CleanupPending) | None => {}
            Some(_) => {
                self.present_scheduler.pop_front();
                self.reject_gpu_presentation(transaction, 0, 0);
            }
        }
        Ok(report)
    }

    pub fn run_cpu_repaint(
        &mut self,
        scene: &mut LiveProductionCpuScene,
        raised_surface: Option<SurfaceId>,
        cursor_position: Option<Point>,
        output_descriptors: &[sophia_engine::HeadlessOutput],
        native_scanout: &mut LiveProductionNativeScanout,
    ) -> Result<LiveProductionCpuSubmission, Box<dyn std::error::Error>> {
        let committed = self.production.committed_surfaces().to_vec();
        self.focused_surface = raised_surface;
        let presentation_order =
            raised_presentation_order(&self.presentation_order, raised_surface);
        let display_list = self.display_list(&committed, &presentation_order)?;
        let compose_started = Instant::now();
        let composition = scene
            .compose_display_list(&committed, &display_list, cursor_position)?
            .clone();
        self.record_focused_border_observation(&committed, true)?;
        let frames = scene.frames_for_outputs(output_descriptors)?;
        self.initialize_native_scanout(native_scanout, &frames)?;
        let transactions = self.layers.values().cloned().collect::<Vec<_>>();
        let output_count = self.outputs.output_count();
        let production = &self.production;
        let outputs = &mut self.outputs;
        let mut frames = frames.into_iter();
        let mut adapter = crate::LiveProductionOutputRuntimeAdapter::new(
            output_count,
            |index, snapshot: &[CommittedSurfaceState]| -> Result<_, Box<dyn std::error::Error>> {
                outputs.run_output(index, snapshot, |runtime| {
                    if let Some(frame) = frames.next() {
                        native_scanout.queue_frame(index, frame);
                    }
                    let input = compositor_tick_input(&transactions, 0, Vec::new(), None);
                    Ok(if runtime.rendered_primary_plane_scanout_in_flight() {
                        runtime.run_tick(input)?
                    } else {
                        native_scanout.run_tick(index, runtime, input)?
                    })
                })
            },
        );
        let tick = production
            .run_outputs(&mut adapter)?
            .into_iter()
            .next()
            .ok_or("persistent backend runtime has no outputs")?;
        Ok(LiveProductionCpuSubmission {
            tick,
            composition,
            composed: true,
            compose_elapsed: compose_started.elapsed(),
        })
    }

    pub fn run_observation_tick(
        &mut self,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let transactions = self.layers.values().cloned().collect::<Vec<_>>();
        let output = self
            .outputs
            .values_mut()
            .next()
            .ok_or("persistent backend runtime has no outputs")?;
        Ok(output
            .runtime
            .run_tick(compositor_tick_input(&transactions, 0, Vec::new(), None))?)
    }
}
fn compositor_tick_input(
    transactions: &[SurfaceTransaction],
    x_event_count: usize,
    authority_commits: Vec<TransactionCommit>,
    wm_update: Option<WmTransactionUpdate>,
) -> CompositorBackendTickInput {
    CompositorBackendTickInput {
        x_event_count: u32::try_from(x_event_count).unwrap_or(u32::MAX),
        authority_commits,
        authority_batches: Vec::new(),
        wm_update,
        portal_commands: Vec::new(),
        chrome_command_count: 0,
        layer_templates: sophia_engine::layer_templates_from_surface_transactions(transactions),
        scanout_submit_state: None,
        scanout_lifecycle_states: Vec::new(),
    }
}

fn authority_transaction_count(transactions: &[SurfaceTransaction]) -> usize {
    transactions.len()
}

pub fn live_production_transactions_require_gpu_scanout(
    transactions: &[SurfaceTransaction],
) -> bool {
    transactions
        .iter()
        .any(|transaction| matches!(transaction.target_buffer, BufferSource::DmaBuf { .. }))
}

pub fn live_production_projection_requires_gpu_scanout(
    transactions: &[SurfaceTransaction],
    presentation_order: &[SurfaceId],
) -> bool {
    transactions.iter().any(|transaction| {
        presentation_order.contains(&transaction.surface)
            && matches!(transaction.target_buffer, BufferSource::DmaBuf { .. })
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionMixedLayerSource {
    CurrentDmaBuf,
    Cpu(SurfaceId),
    RetainedDmaBuf(SurfaceId),
}

pub fn live_production_mixed_layer_order(
    presentation_order: &[SurfaceId],
    current: SurfaceId,
    cpu_surfaces: &[SurfaceId],
    retained_dma_buf_surfaces: &[SurfaceId],
) -> Vec<LiveProductionMixedLayerSource> {
    presentation_order
        .iter()
        .filter_map(|surface| {
            if *surface == current {
                Some(LiveProductionMixedLayerSource::CurrentDmaBuf)
            } else if cpu_surfaces.contains(surface) {
                Some(LiveProductionMixedLayerSource::Cpu(*surface))
            } else if retained_dma_buf_surfaces.contains(surface) {
                Some(LiveProductionMixedLayerSource::RetainedDmaBuf(*surface))
            } else {
                None
            }
        })
        .collect()
}

pub const fn reduce_live_production_frame_defer(
    requested_defer: bool,
    presentation_order_changed: bool,
    preserved_gpu_projection: bool,
) -> bool {
    preserved_gpu_projection || (requested_defer && !presentation_order_changed)
}
