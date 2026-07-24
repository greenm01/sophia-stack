use crate::*;
use sophia_engine::*;
use sophia_protocol::*;
use sophia_renderer_live::*;
use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

mod native;
mod service;
pub use service::*;

pub struct LiveProductionVisualRuntime {
    production: sophia_engine::ProductionSessionCoordinator,
    outputs: LiveProductionOutputRuntimeSet,
    layers: BTreeMap<SurfaceId, SurfaceTransaction>,
    input_layers: Vec<LayerSnapshot>,
    presentation_feedback: crate::LiveProductionPresentFeedbackCoordinator,
    present_scheduler: LiveProductionPresentScheduler,
    present_feedback: VecDeque<crate::LivePresentFeedbackOutcome>,
    present_feedback_overflowed: bool,
}

const PRESENT_FEEDBACK_CAPACITY: usize = 8_192;

pub struct LiveProductionCycleRequest<'a> {
    pub batch: &'a LiveProductionAuthorityBatch,
    pub scene: &'a mut LiveProductionCpuScene,
    pub updates: Vec<crate::LiveCpuBufferUpdate>,
    pub raised_surface: Option<SurfaceId>,
    pub cursor_presentation: LiveProductionCursorPresentation,
    pub defer_frame: bool,
    pub output_descriptors: &'a [sophia_engine::HeadlessOutput],
    pub native_scanout: Option<&'a mut LiveProductionNativeScanout>,
    pub wm_update: Option<WmTransactionUpdate>,
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
            present_feedback: VecDeque::with_capacity(PRESENT_FEEDBACK_CAPACITY),
            present_feedback_overflowed: false,
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
            cursor_presentation,
            defer_frame,
            output_descriptors,
            native_scanout,
            wm_update,
        } = request;
        self.presentation_feedback
            .observe_authority_resources(batch)?;
        self.layers
            .retain(|surface, _| !batch.removed_surfaces.contains(surface));
        for transaction in &batch.transactions {
            self.layers.insert(transaction.surface, transaction.clone());
        }
        self.rebuild_input_layers();
        let preserve_gpu_scanout = native_scanout.is_some()
            && (self
                .production
                .committed_surfaces()
                .iter()
                .any(|surface| matches!(surface.buffer, BufferSource::DmaBuf { .. }))
                || self
                    .layers
                    .values()
                    .any(|surface| matches!(surface.target_buffer, BufferSource::DmaBuf { .. })));
        let defer_frame = defer_frame || preserve_gpu_scanout;
        let native_scanout = if preserve_gpu_scanout {
            None
        } else {
            native_scanout
        };
        let active_transactions = self.layers.values().cloned().collect::<Vec<_>>();
        let intake = AuthorityTransactionIntake::new(batch.transaction, batch.transactions.clone())
            .with_surface_removals(batch.removed_surfaces.clone());
        let (production, outputs) = (&mut self.production, &mut self.outputs);
        let output_count = outputs.output_count();
        let event_count = authority_transaction_count(&batch.transactions);
        let mut native_scanout = native_scanout;
        let create_native_frames = native_scanout.is_some();
        let mut adapter = LiveProductionCpuCycleAdapter::new(
            scene,
            updates,
            raised_surface,
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
            cursor_presentation,
            defer_frame,
            output_descriptors,
            mut native_scanout,
            wm_update,
        } = request;
        let committed_surfaces = self.committed_surfaces().to_vec();
        scene.apply_updates(updates, &committed_surfaces)?;
        let compose_started = Instant::now();
        let composition = if defer_frame {
            scene
                .last_report()
                .cloned()
                .ok_or("software redraw coalescing has no prior composed frame")?
        } else {
            scene
                .compose(
                    &committed_surfaces,
                    raised_surface,
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
        if let (Some(native_scanout), Some(frames)) =
            (native_scanout.as_deref_mut(), native_frames.as_ref())
        {
            self.initialize_native_scanout(native_scanout, frames)?;
        }
        let tick = self.run_batch(
            batch,
            if defer_frame { None } else { native_scanout },
            native_frames,
            wm_update,
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

    pub fn run_batch(
        &mut self,
        batch: &LiveProductionAuthorityBatch,
        mut native_scanout: Option<&mut LiveProductionNativeScanout>,
        native_frames: Option<Vec<LiveProductionComposedFrame>>,
        wm_update: Option<WmTransactionUpdate>,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        self.presentation_feedback
            .observe_authority_resources(batch)?;
        if !batch.present_submissions.is_empty() {
            let cpu_background = native_frames
                .as_ref()
                .and_then(|frames| frames.first())
                .map(|frame| frame.frame.clone());
            self.present_scheduler.enqueue_batch(
                batch,
                cpu_background,
                self.presentation_feedback.resources_mut(),
                Instant::now(),
            )?;
            self.layers
                .retain(|surface, _| !batch.removed_surfaces.contains(surface));
            for transaction in &batch.transactions {
                self.layers.insert(transaction.surface, transaction.clone());
            }
            self.rebuild_input_layers();
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
        let output_states = self.native_output_service_states(native_scanout)?;
        if reduce_live_production_async_service_observation(&output_states, true)?
            .present_output_blocked
        {
            return self.run_observation_tick();
        }
        let mixed = self.presentation_feedback.resources().build_mixed_frame(
            transaction,
            queued.cpu_background.clone(),
            queued.target,
            None,
            1.0,
        )?;
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
        native_scanout.queue_mixed_frame(primary_index, transaction, mixed);

        let transactions = self.layers.values().cloned().collect::<Vec<_>>();
        let production = &self.production;
        let outputs = &mut self.outputs;
        let mut adapter = crate::LiveProductionOutputRuntimeAdapter::new(
            1,
            |invocation,
             committed: &[CommittedSurfaceState]|
             -> Result<_, Box<dyn std::error::Error>> {
                if invocation != 0 {
                    return Err("GPU presentation invoked more than one primary output".into());
                }
                outputs.run_output(primary_index, committed, |runtime| {
                    native_scanout.run_tick(
                        primary_index,
                        runtime,
                        compositor_tick_input(&transactions, 0, Vec::new(), None),
                    )
                })
            },
        );
        let report = production
            .run_outputs(&mut adapter)?
            .into_iter()
            .next()
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
        let compose_started = Instant::now();
        let composition = scene
            .compose(&committed, raised_surface, cursor_position)?
            .clone();
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

    pub fn reject_gpu_presentation(&mut self, transaction: TransactionId, ust: u64, msc: u64) {
        if let Ok(outcome) = self
            .presentation_feedback
            .reject_skip(transaction, ust, msc)
        {
            self.route_present_feedback(outcome);
        }
    }

    pub fn route_present_feedback(&mut self, outcome: crate::LivePresentFeedbackOutcome) {
        if self.present_feedback.len() == PRESENT_FEEDBACK_CAPACITY {
            self.present_feedback_overflowed = true;
            return;
        }
        self.present_feedback.push_back(outcome);
    }

    pub fn drain_present_feedback_into(
        &mut self,
        outcomes: &mut Vec<crate::LivePresentFeedbackOutcome>,
    ) -> Result<(), &'static str> {
        if self.present_feedback_overflowed {
            return Err("production Present feedback queue overflowed");
        }
        outcomes.extend(self.present_feedback.drain(..));
        Ok(())
    }

    pub fn shutdown_presentations(&mut self) -> crate::LivePresentationDisconnectReport {
        let queued = self.present_scheduler.drain_transactions();
        for transaction in queued {
            self.reject_gpu_presentation(transaction, 0, 0);
        }
        if let Some(submitted) = self.present_scheduler.take_submitted() {
            self.reject_gpu_presentation(submitted.transaction, 0, 0);
        }

        self.presentation_feedback.disconnect()
    }

    pub fn prepare_authority_transactions(
        &mut self,
        transaction_id: TransactionId,
        transactions: &[SurfaceTransaction],
        removed_surfaces: &[SurfaceId],
    ) -> Result<LiveProductionPreparedAuthorityBatch, Box<dyn std::error::Error>> {
        self.layers
            .retain(|surface, _| !removed_surfaces.contains(surface));
        for transaction in transactions {
            self.layers.insert(transaction.surface, transaction.clone());
        }
        self.rebuild_input_layers();
        let intake = AuthorityTransactionIntake::new(transaction_id, transactions.to_vec())
            .with_surface_removals(removed_surfaces.to_vec());
        let active_transactions = self.layers.values().cloned().collect::<Vec<_>>();
        let authority_commits = self
            .production
            .commit_authority_batches(std::slice::from_ref(&intake));
        Ok(LiveProductionPreparedAuthorityBatch {
            authority_commits,
            active_transactions,
        })
    }

    pub fn run_prepared_authority_transactions(
        &mut self,
        prepared: LiveProductionPreparedAuthorityBatch,
        event_count: usize,
        mut native_scanout: Option<&mut LiveProductionNativeScanout>,
        native_frames: Option<Vec<LiveProductionComposedFrame>>,
        wm_update: Option<WmTransactionUpdate>,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let output_count = self.outputs.output_count();
        let production = &self.production;
        let outputs = &mut self.outputs;
        let mut native_frames = native_frames.unwrap_or_default().into_iter();
        let mut adapter = crate::LiveProductionOutputRuntimeAdapter::new(
            output_count,
            |index, committed: &[CommittedSurfaceState]| -> Result<_, Box<dyn std::error::Error>> {
                let output = outputs
                    .values_mut()
                    .nth(index)
                    .ok_or("production output index was not registered")?;
                output
                    .runtime
                    .assembly_mut()
                    .replace_committed_surfaces(committed.to_vec());
                let input = compositor_tick_input(
                    &prepared.active_transactions,
                    event_count,
                    prepared.authority_commits.clone(),
                    wm_update.clone(),
                );
                Ok(match native_scanout.as_deref_mut() {
                    Some(native_scanout) => {
                        if let Some(frame) = native_frames.next() {
                            native_scanout.queue_frame(index, frame);
                        }
                        if output.runtime.rendered_primary_plane_scanout_in_flight() {
                            output.runtime.run_tick(input)?
                        } else {
                            native_scanout.run_tick(index, &mut output.runtime, input)?
                        }
                    }
                    None => output.runtime.run_tick(input)?,
                })
            },
        );
        production
            .run_outputs(&mut adapter)?
            .into_iter()
            .next()
            .ok_or_else(|| "persistent backend runtime has no outputs".into())
    }

    pub fn run_authority_transactions(
        &mut self,
        run: LiveAuthorityTransactionRun<'_>,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let LiveAuthorityTransactionRun {
            transaction_id,
            transactions,
            removed_surfaces,
            event_count,
            native_scanout,
            native_frames,
            wm_update,
        } = run;
        let prepared =
            self.prepare_authority_transactions(transaction_id, transactions, removed_surfaces)?;
        self.run_prepared_authority_transactions(
            prepared,
            event_count,
            native_scanout,
            native_frames,
            wm_update,
        )
    }

    pub fn committed_surfaces(&self) -> &[CommittedSurfaceState] {
        self.production.committed_surfaces()
    }

    fn rebuild_input_layers(&mut self) {
        self.input_layers.clear();
        self.input_layers.extend(
            self.layers
                .values()
                .enumerate()
                .map(|(index, transaction)| LayerSnapshot {
                    surface: transaction.surface,
                    authority_local_id: None,
                    namespace: transaction.namespace,
                    stack_rank: u32::try_from(index).unwrap_or(u32::MAX),
                    geometry: transaction.target_geometry,
                    source: transaction.target_buffer,
                    damage: transaction.damage.clone(),
                    opacity: 1.0,
                    crop: None,
                    transform: Transform::IDENTITY,
                    generation: transaction.previous_committed_generation,
                    resize_sync: ResizeSyncCapability::ImplicitOnly,
                }),
        );
    }

    pub fn input_layers(&self) -> &[LayerSnapshot] {
        &self.input_layers
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
