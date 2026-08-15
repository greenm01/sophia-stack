use crate::*;
use sophia_engine::*;
use sophia_protocol::*;
use sophia_renderer_live::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::{Duration, Instant};

mod authority;
mod compositor_graphics;
mod native;
mod present;
mod projection;
mod service;
mod software_present;
pub use native::*;
pub use service::*;

#[derive(Debug)]
struct LiveDisplayedSurface {
    layer: LiveRetainedRendererImageLayer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveFocusRingObservation {
    pub surface: SurfaceId,
    pub generation: u64,
    pub primitives: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveChromeSetObservation {
    pub generation: u64,
    pub eligible_surfaces: usize,
    pub frames: usize,
    pub focused_frames: usize,
    pub unfocused_frames: usize,
    pub focus_rings: usize,
    pub primitives: usize,
    pub clearance: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveFloatingOutline {
    pub surface: SurfaceId,
    pub geometry: Rect,
}

/// Immutable interaction projection paired with one independently presented
/// output. Its epoch advances only when hit-test meaning changes; buffer-only
/// presentation may replace pixels without invalidating an application lease.
#[derive(Clone, Debug, PartialEq)]
pub struct LivePresentedInputProjection {
    pub output: OutputId,
    pub epoch: u64,
    pub layers: Vec<LayerSnapshot>,
}

fn replace_displayed_surface(
    displayed_surfaces: &mut BTreeMap<SurfaceId, LiveDisplayedSurface>,
    surface: SurfaceId,
    layer: LiveRetainedRendererImageLayer,
) -> Option<LiveDisplayedSurface> {
    displayed_surfaces.insert(surface, LiveDisplayedSurface { layer })
}

pub struct LiveProductionVisualRuntime {
    production: sophia_engine::ProductionSessionCoordinator,
    outputs: LiveProductionOutputRuntimeSet,
    surface_metadata: BTreeMap<SurfaceId, projection::LiveSurfaceProjectionMetadata>,
    input_projections: Vec<LivePresentedInputProjection>,
    presentation_feedback: crate::LiveProductionPresentFeedbackCoordinator,
    present_scheduler: LiveProductionPresentScheduler,
    surface_content_stream: SurfaceContentStream<LiveProductionAuthorityGroup>,
    released_surface_content: VecDeque<LiveProductionAuthorityGroup>,
    superseded_surface_content: VecDeque<LiveProductionAuthorityGroup>,
    deferred_content_dma_buf_releases: BTreeSet<BufferHandle>,
    deferred_content_fence_releases: BTreeSet<FenceHandle>,
    software_present_frames_waiting: VecDeque<software_present::LiveProductionSoftwarePresentFrame>,
    software_present_frames_bound: BTreeMap<
        LiveProductionNativeFrameId,
        software_present::LiveProductionSoftwarePresentBinding,
    >,
    software_presents_unframed: VecDeque<Vec<LiveProductionSoftwarePresentSubmission>>,
    retired_software_presents: VecDeque<LiveProductionRetiredSoftwarePresent>,
    retired_software_presents_overflowed: bool,
    displayed_surfaces: BTreeMap<SurfaceId, LiveDisplayedSurface>,
    presentation_order: Vec<SurfaceId>,
    chrome_surfaces: Vec<SurfaceId>,
    focused_surface: Option<SurfaceId>,
    surface_chrome_style: SurfaceChromeStyle,
    floating_outline: Option<LiveFloatingOutline>,
    pending_focus_ring_observation: Option<LiveFocusRingObservation>,
    last_focus_ring_observation: Option<LiveFocusRingObservation>,
    pending_chrome_set_observation: Option<LiveChromeSetObservation>,
    last_chrome_set_observation: Option<LiveChromeSetObservation>,
    present_feedback: VecDeque<crate::LivePresentFeedbackOutcome>,
    present_feedback_overflowed: bool,
    present_rejections: usize,
    native_suspend_present_rejections: usize,
    shutdown_present_rejections: usize,
    cpu_buffer_residency: Vec<u64>,
    recent_cpu_buffer_updates: VecDeque<u64>,
}

const PRESENT_FEEDBACK_CAPACITY: usize = 8_192;
const RECENT_CPU_BUFFER_UPDATE_CAPACITY: usize = 16;

pub struct LiveProductionCycleRequest<'a> {
    pub batch: &'a LiveProductionAuthorityBatch,
    pub scene: &'a mut LiveProductionCpuScene,
    pub raised_surface: Option<SurfaceId>,
    pub focused_surface: Option<SurfaceId>,
    pub cursor_presentation: LiveProductionCursorPresentation,
    pub defer_frame: bool,
    pub output_descriptors: &'a [sophia_engine::HeadlessOutput],
    pub native_scanout: Option<&'a mut LiveProductionNativeScanout>,
    pub wm_update: Option<WmTransactionUpdate>,
    pub presentation_layout: &'a [LayerSnapshot],
    pub chrome_surfaces: &'a [SurfaceId],
    pub staged_cpu_buffer_handles: &'a [u64],
}

pub struct LiveAuthorityTransactionRun<'a> {
    pub groups: &'a [LiveProductionAuthorityGroup],
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
        let input_projections = (0..output_runtimes.output_count())
            .filter_map(|index| output_runtimes.output_id(index))
            .map(|output| LivePresentedInputProjection {
                output,
                epoch: 0,
                layers: Vec::new(),
            })
            .collect();
        Ok(Self {
            production,
            outputs: output_runtimes,
            surface_metadata: BTreeMap::new(),
            input_projections,
            presentation_feedback: Default::default(),
            present_scheduler: LiveProductionPresentScheduler::default(),
            surface_content_stream: SurfaceContentStream::default(),
            released_surface_content: VecDeque::new(),
            superseded_surface_content: VecDeque::new(),
            deferred_content_dma_buf_releases: BTreeSet::new(),
            deferred_content_fence_releases: BTreeSet::new(),
            software_present_frames_waiting: VecDeque::new(),
            software_present_frames_bound: BTreeMap::new(),
            software_presents_unframed: VecDeque::new(),
            retired_software_presents: VecDeque::with_capacity(PRESENT_FEEDBACK_CAPACITY),
            retired_software_presents_overflowed: false,
            displayed_surfaces: BTreeMap::new(),
            presentation_order: Vec::new(),
            chrome_surfaces: Vec::new(),
            focused_surface: None,
            surface_chrome_style: SurfaceChromeStyle::default(),
            floating_outline: None,
            pending_focus_ring_observation: None,
            last_focus_ring_observation: None,
            pending_chrome_set_observation: None,
            last_chrome_set_observation: None,
            present_feedback: VecDeque::with_capacity(PRESENT_FEEDBACK_CAPACITY),
            present_feedback_overflowed: false,
            present_rejections: 0,
            native_suspend_present_rejections: 0,
            shutdown_present_rejections: 0,
            cpu_buffer_residency: Vec::with_capacity(16),
            recent_cpu_buffer_updates: VecDeque::with_capacity(RECENT_CPU_BUFFER_UPDATE_CAPACITY),
        })
    }

    pub fn initialize_native_scanout(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
        frames: &[LiveProductionComposedFrame],
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.outputs
            .initialize_native_scanout(native_scanout, frames)?;
        // Initial modesets are synchronously established as presented by the
        // native owner and do not generate a later retirement callback.
        self.publish_presented_input_layers(native_scanout);
        Ok(())
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

    pub fn with_surface_chrome_style(mut self, style: SurfaceChromeStyle) -> Self {
        self.surface_chrome_style = style;
        self
    }

    pub fn set_surface_chrome_style(&mut self, style: SurfaceChromeStyle) -> bool {
        if self.surface_chrome_style == style {
            return false;
        }
        self.surface_chrome_style = style;
        self.last_focus_ring_observation = None;
        self.last_chrome_set_observation = None;
        true
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
            raised_surface,
            focused_surface,
            cursor_presentation,
            defer_frame,
            output_descriptors,
            mut native_scanout,
            wm_update,
            presentation_layout,
            chrome_surfaces,
            staged_cpu_buffer_handles,
        } = request;
        let authority_envelope = batch;
        authority_envelope.validate()?;
        self.presentation_feedback
            .observe_authority_resource_registrations(authority_envelope)?;
        let batch = self.ready_surface_content_batch(authority_envelope)?;
        let _ = self.reject_superseded_surface_content()?;
        let mut updates = authority_batch_cpu_buffer_updates(&batch);
        record_recent_cpu_buffer_updates(&mut self.recent_cpu_buffer_updates, &updates);
        write_cpu_buffer_residency(
            &mut self.cpu_buffer_residency,
            self.production.committed_surfaces(),
            &batch,
            self.surface_content_stream
                .deferred_items()
                .chain(self.released_surface_content.iter()),
            staged_cpu_buffer_handles,
            &self.recent_cpu_buffer_updates,
        );
        retain_relevant_cpu_buffer_updates(scene, &mut updates, &self.cpu_buffer_residency);
        let native_enabled = native_scanout.is_some();
        let focus_changed = self.focused_surface != focused_surface;
        self.focused_surface = focused_surface;
        let presentation_order_changed = self.apply_presentation_layout(presentation_layout);
        let chrome_surfaces_changed = self.set_chrome_surfaces(chrome_surfaces);
        let visual_projection_changed =
            presentation_order_changed || chrome_surfaces_changed || focus_changed;
        let committed_projection_requires_gpu =
            live_production_committed_projection_requires_gpu_scanout(
                self.production.committed_surfaces(),
                &self.presentation_order,
            );
        let retained_cpu_layers = scene.presentation_layers(
            self.production.committed_surfaces(),
            &self.presentation_order,
        );
        // A retained CPU layer is a snapshot from before this authority batch is
        // committed. Let a CPU-only scene compose the current updates instead of
        // placing new chrome around stale client pixels. GPU-owned projections
        // still need the retained mixed path to preserve their image ownership.
        let retained_projection_queued = if live_production_retained_projection_admitted(
            visual_projection_changed,
            !updates.is_empty(),
            committed_projection_requires_gpu,
        ) {
            match (
                native_scanout.as_deref_mut(),
                self.retained_mixed_frame(&retained_cpu_layers)?,
            ) {
                (Some(native_scanout), Some(frame)) => {
                    let primary = self
                        .outputs
                        .primary_output()
                        .ok_or("persistent backend runtime has no primary output")?;
                    native_scanout.queue_retained_mixed_frame(primary, frame)?;
                    true
                }
                _ => false,
            }
        } else {
            false
        };
        if visual_projection_changed {
            tracing::debug!(
                "sophia_live_retained_projection schema=1 status={} focus_changed={} order_changed={} chrome_changed={}",
                if retained_projection_queued {
                    "queued"
                } else {
                    "unavailable"
                },
                focus_changed,
                presentation_order_changed,
                chrome_surfaces_changed,
            );
        }
        let removed_surfaces = authority_batch_removed_surfaces(&batch);
        self.release_removed_presentations(&removed_surfaces, native_scanout.as_deref_mut())?;
        let rebased_groups = batch.groups;
        self.enqueue_software_presents(&rebased_groups)?;
        let software_present_frame_required = !self.software_presents_unframed.is_empty();
        for group in &rebased_groups {
            self.observe_surface_metadata(&group.transactions, &group.removed_surfaces);
        }
        self.displayed_surfaces
            .retain(|surface, _| !removed_surfaces.contains(surface));
        let preserve_gpu_scanout = live_production_should_preserve_gpu_output(
            native_scanout.is_some(),
            self.present_scheduler.has_in_flight(),
            retained_projection_queued,
            presentation_order_changed,
            committed_projection_requires_gpu,
        );
        let defer_frame = if software_present_frame_required {
            false
        } else {
            reduce_live_production_frame_defer(
                defer_frame,
                visual_projection_changed,
                preserve_gpu_scanout,
            )
        };
        let native_scanout = if preserve_gpu_scanout || software_present_frame_required {
            None
        } else {
            native_scanout
        };
        let intakes = rebased_groups
            .iter()
            .map(|group| {
                AuthorityTransactionIntake::new(group.transaction, group.transactions.clone())
                    .with_surface_removals(group.removed_surfaces.clone())
            })
            .collect::<Vec<_>>();
        self.observe_content_ordered_resource_releases(authority_envelope);
        let (production, outputs) = (&mut self.production, &mut self.outputs);
        let output_count = outputs.output_count();
        let event_count = authority_transaction_count_for_groups(&rebased_groups);
        let surface_metadata = self.surface_metadata.clone();
        let mut native_scanout = native_scanout;
        let create_native_frames = native_scanout.is_some();
        let mut adapter = LiveProductionCpuCycleAdapter::new(
            scene,
            &self.presentation_order,
            updates,
            raised_surface,
            focused_surface,
            self.surface_chrome_style,
            cursor_presentation.composition_position(),
            defer_frame,
            create_native_frames,
            &self.cpu_buffer_residency,
            output_descriptors,
            move |_cycle,
                  committed: &[CommittedSurfaceState],
                  authority_commits: &[TransactionCommit],
                  native_frames: Option<Vec<LiveProductionComposedFrame>>| {
                // A deferred cycle may service retained native work without
                // producing a new CPU frame; only an actual frame set initializes outputs.
                let initialize_native = native_frames.is_some();
                let native_frames = native_frames.unwrap_or_default();
                if initialize_native && let Some(native_scanout) = native_scanout.as_deref_mut() {
                    outputs.initialize_native_scanout(native_scanout, &native_frames)?;
                }
                let mut native_frames = native_frames.into_iter();
                let mut output_adapter = crate::LiveProductionOutputRuntimeAdapter::new(
                    output_count,
                    |index,
                     snapshot: &[CommittedSurfaceState]|
                     -> Result<_, Box<dyn std::error::Error>> {
                        let output_id = outputs
                            .output_id(index)
                            .ok_or("production output index was not registered")?;
                        outputs.run_output(index, snapshot, |runtime| {
                            let input = compositor_tick_input_for_committed(
                                snapshot,
                                &surface_metadata,
                                event_count,
                                authority_commits.to_vec(),
                                wm_update.clone(),
                            );
                            Ok(match native_scanout.as_deref_mut() {
                                Some(native_scanout) => {
                                    if let Some(next_frame) = native_frames.next() {
                                        native_scanout.queue_frame(output_id, next_frame);
                                    }
                                    if runtime.rendered_primary_plane_scanout_in_flight() {
                                        runtime.run_tick(input)?
                                    } else {
                                        native_scanout.run_tick(output_id, runtime, input)?
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
            .run_cycle(&intakes, &mut adapter)
            .map_err(|error| {
                format!(
                    "production CPU cycle failed in phase {:?}: {}",
                    error.phase, error.source
                )
            })?;
        drop(adapter);
        if software_present_frame_required {
            if !report.submission.composed {
                return Err("software Present did not produce an immutable composed frame".into());
            }
            if native_enabled {
                self.frame_unframed_software_presents(scene, output_descriptors)?;
            } else {
                self.settle_unframed_software_presents_without_native()?;
            }
        }
        if report.submission.composed {
            self.record_focus_ring_observation(&report.committed_surfaces, false)?;
        }
        // Native input advances only when retire_native_scanout_output
        // observes the corresponding accepted page flip.
        if !native_enabled {
            self.publish_committed_input_layers();
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
            raised_surface,
            focused_surface,
            cursor_presentation,
            defer_frame,
            output_descriptors,
            mut native_scanout,
            wm_update,
            presentation_layout,
            chrome_surfaces,
            staged_cpu_buffer_handles,
        } = request;
        let native_enabled = native_scanout.is_some();
        let batch = self.ready_surface_content_batch(batch)?;
        let mut updates = authority_batch_cpu_buffer_updates(&batch);
        record_recent_cpu_buffer_updates(&mut self.recent_cpu_buffer_updates, &updates);
        write_cpu_buffer_residency(
            &mut self.cpu_buffer_residency,
            self.production.committed_surfaces(),
            &batch,
            self.surface_content_stream
                .deferred_items()
                .chain(self.released_surface_content.iter()),
            staged_cpu_buffer_handles,
            &self.recent_cpu_buffer_updates,
        );
        retain_relevant_cpu_buffer_updates(scene, &mut updates, &self.cpu_buffer_residency);
        self.focused_surface = focused_surface;
        let _ = self.apply_presentation_layout(presentation_layout);
        self.set_chrome_surfaces(chrome_surfaces);
        let committed_surfaces = self.committed_surfaces().to_vec();
        scene.apply_production_updates(updates)?;
        scene.reconcile_buffer_residency(&self.cpu_buffer_residency);
        let missing_buffers = scene.missing_committed_buffer_count(&committed_surfaces);
        if missing_buffers != 0 {
            return Err(format!(
                "production GPU scene is missing {missing_buffers} committed CPU buffer(s)"
            )
            .into());
        }
        let compose_started = Instant::now();
        let mut composition = if defer_frame {
            scene
                .last_report()
                .cloned()
                .ok_or("software redraw coalescing has no prior composed frame")?
        } else {
            let presentation_order =
                raised_presentation_order(&self.presentation_order, raised_surface);
            let display_list = self.display_list(&committed_surfaces, &presentation_order)?;
            let output = output_descriptors
                .first()
                .copied()
                .ok_or("software composition has no output descriptor")?;
            scene
                .compose_display_list(
                    output,
                    &committed_surfaces,
                    &display_list,
                    cursor_presentation.composition_position(),
                )?
                .clone()
        };
        // A Present-bearing authority group owns the next native visual
        // candidate. Do not queue a retained CPU frame ahead of it: that can
        // expose new layout/chrome around old or absent client pixels.
        let native_frames = if defer_frame || batch.has_dma_buf_present_submissions() {
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
            &batch,
            presentation_layout,
            if defer_frame { None } else { native_scanout },
            native_frames,
            cpu_layers,
            wm_update,
        )?;
        let software_present_frame_required = !self.software_presents_unframed.is_empty();
        if software_present_frame_required {
            let committed_surfaces = self.committed_surfaces().to_vec();
            let presentation_order =
                raised_presentation_order(&self.presentation_order, raised_surface);
            let display_list = self.display_list(&committed_surfaces, &presentation_order)?;
            let output = output_descriptors
                .first()
                .copied()
                .ok_or("software Present has no output descriptor")?;
            composition = scene
                .compose_display_list(
                    output,
                    &committed_surfaces,
                    &display_list,
                    cursor_presentation.composition_position(),
                )?
                .clone();
            if native_enabled {
                self.frame_unframed_software_presents(scene, output_descriptors)?;
            } else {
                self.settle_unframed_software_presents_without_native()?;
            }
        }
        Ok((
            LiveProductionCpuSubmission {
                tick,
                composition,
                composed: !defer_frame || software_present_frame_required,
                compose_elapsed: if defer_frame && !software_present_frame_required {
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
            if let Some(displayed) = self.displayed_surfaces.get_mut(&layer.surface) {
                displayed.layer.reproject(layer.geometry);
            }
        }
        order_changed
    }

    fn set_chrome_surfaces(&mut self, surfaces: &[SurfaceId]) -> bool {
        if self.chrome_surfaces == surfaces {
            return false;
        }
        self.chrome_surfaces.clear();
        self.chrome_surfaces.extend_from_slice(surfaces);
        true
    }

    pub fn run_batch(
        &mut self,
        batch: &LiveProductionAuthorityBatch,
        presentation_layout: &[LayerSnapshot],
        mut native_scanout: Option<&mut LiveProductionNativeScanout>,
        native_frames: Option<Vec<LiveProductionComposedFrame>>,
        cpu_layers: Vec<LiveCpuPresentationLayer>,
        wm_update: Option<WmTransactionUpdate>,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        batch.validate()?;
        self.presentation_feedback
            .observe_authority_resource_registrations(batch)?;
        let _ = self.reject_superseded_surface_content()?;
        let removed_surfaces = authority_batch_removed_surfaces(batch);
        self.release_removed_presentations(&removed_surfaces, native_scanout.as_deref_mut())?;
        self.displayed_surfaces
            .retain(|surface, _| !removed_surfaces.contains(surface));
        let mut authority_groups = Vec::new();
        let mut has_present_submissions = false;
        for group in &batch.groups {
            if group.present_submissions.is_empty() {
                authority_groups.push(group.clone());
            } else {
                has_present_submissions = true;
                let superseded = self.present_scheduler.enqueue_group(
                    group,
                    presentation_layout,
                    cpu_layers.clone(),
                    self.presentation_feedback.resources_mut(),
                    Instant::now(),
                )?;
                for transaction in superseded {
                    self.reject_gpu_presentation(transaction);
                }
            }
        }
        // Software and DMA-BUF Presents can arrive in separate authority
        // groups in one owner batch. Queue the software feedback before the
        // GPU group drives the shared native frame so both retire on its
        // page-flip clock.
        self.enqueue_software_presents(&authority_groups)?;
        self.observe_content_ordered_resource_releases(batch);
        if has_present_submissions && !authority_groups.is_empty() {
            let prepared = self.prepare_authority_groups(&authority_groups)?;
            let _ = self.run_prepared_authority_transactions(
                prepared,
                authority_transaction_count_for_groups(&authority_groups),
                None,
                None,
                wm_update.clone(),
            )?;
        }
        if has_present_submissions {
            for group in &batch.groups {
                self.observe_surface_metadata(&group.transactions, &group.removed_surfaces);
            }
            if !self.present_scheduler.has_eligible() {
                return self.run_observation_tick();
            }
            return self.drive_gpu_presentation(native_scanout.as_deref_mut());
        }
        if authority_groups.is_empty() {
            return self.run_observation_tick();
        }
        self.run_authority_transactions(LiveAuthorityTransactionRun {
            event_count: authority_transaction_count_for_groups(&authority_groups),
            groups: &authority_groups,
            native_scanout,
            native_frames,
            wm_update,
        })
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
        let output = output_descriptors
            .first()
            .copied()
            .ok_or("software composition has no output descriptor")?;
        let compose_started = Instant::now();
        let composition = scene
            .compose_display_list(output, &committed, &display_list, cursor_position)?
            .clone();
        self.record_focus_ring_observation(&committed, true)?;
        let frames = scene.frames_for_outputs(output_descriptors)?;
        self.initialize_native_scanout(native_scanout, &frames)?;
        let output_count = self.outputs.output_count();
        let production = &self.production;
        let surface_metadata = &self.surface_metadata;
        let outputs = &mut self.outputs;
        let mut frames = frames.into_iter();
        let mut adapter = crate::LiveProductionOutputRuntimeAdapter::new(
            output_count,
            |index, snapshot: &[CommittedSurfaceState]| -> Result<_, Box<dyn std::error::Error>> {
                let output_id = outputs
                    .output_id(index)
                    .ok_or("production output index was not registered")?;
                outputs.run_output(index, snapshot, |runtime| {
                    if let Some(frame) = frames.next() {
                        native_scanout.queue_frame(output_id, frame);
                    }
                    let input = compositor_tick_input_for_committed(
                        snapshot,
                        surface_metadata,
                        0,
                        Vec::new(),
                        None,
                    );
                    Ok(if runtime.rendered_primary_plane_scanout_in_flight() {
                        runtime.run_tick(input)?
                    } else {
                        native_scanout.run_tick(output_id, runtime, input)?
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
        // Both views from one read, and the assembly resynchronised before the
        // tick. This was the one tick that never replaced the committed list, so
        // it paired fresh templates against whatever an earlier cycle had left in
        // the assembly -- a mismatch the engine rejects as an invalid surface,
        // masked until the first client surface ever committed and deterministic
        // from then on. Nine call paths lead here, which is why the failure
        // looked unrelated to any of them.
        let (layer_templates, committed) = self.scene_views();
        let output = self
            .outputs
            .values_mut()
            .next()
            .ok_or("persistent backend runtime has no outputs")?;
        output
            .runtime
            .assembly_mut()
            .replace_committed_surfaces(committed);
        Ok(output
            .runtime
            .run_tick(compositor_tick_input(&layer_templates, 0, Vec::new(), None))?)
    }
}
fn compositor_tick_input(
    layer_templates: &[LayerSnapshot],
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
        layer_templates: layer_templates.to_vec(),
        scanout_submit_state: None,
        scanout_lifecycle_states: Vec::new(),
    }
}

fn compositor_tick_input_for_committed(
    committed: &[CommittedSurfaceState],
    surface_metadata: &BTreeMap<SurfaceId, projection::LiveSurfaceProjectionMetadata>,
    x_event_count: usize,
    authority_commits: Vec<TransactionCommit>,
    wm_update: Option<WmTransactionUpdate>,
) -> CompositorBackendTickInput {
    let layer_templates = projection::committed_layer_snapshots(committed, surface_metadata);
    compositor_tick_input(
        &layer_templates,
        x_event_count,
        authority_commits,
        wm_update,
    )
}

fn authority_transaction_count_for_groups(groups: &[LiveProductionAuthorityGroup]) -> usize {
    groups.iter().map(|group| group.transactions.len()).sum()
}

fn rebase_authority_groups_to_committed(
    groups: Vec<LiveProductionAuthorityGroup>,
    committed: &[CommittedSurfaceState],
) -> Vec<LiveProductionAuthorityGroup> {
    let mut generations = committed
        .iter()
        .map(|state| (state.surface, state.committed_generation))
        .collect::<BTreeMap<_, _>>();
    groups
        .into_iter()
        .map(|mut group| {
            for transaction in &mut group.transactions {
                let generation = generations.get(&transaction.surface).copied().unwrap_or(0);
                transaction.previous_committed_generation = generation;
                generations.insert(transaction.surface, generation.saturating_add(1));
            }
            for surface in &group.removed_surfaces {
                generations.remove(surface);
            }
            group
        })
        .collect()
}

fn write_cpu_buffer_residency<'a>(
    handles: &mut Vec<u64>,
    committed: &[CommittedSurfaceState],
    batch: &LiveProductionAuthorityBatch,
    pending_groups: impl Iterator<Item = &'a LiveProductionAuthorityGroup>,
    staged: &[u64],
    recent_updates: &VecDeque<u64>,
) {
    handles.clear();
    handles.extend(
        committed
            .iter()
            .filter_map(|surface| match surface.buffer() {
                BufferSource::CpuBuffer { handle } => Some(handle),
                _ => None,
            }),
    );
    handles.extend(
        batch
            .groups
            .iter()
            .flat_map(|group| group.transactions.iter())
            .filter_map(|transaction| match transaction.target_buffer() {
                BufferSource::CpuBuffer { handle } => Some(handle),
                _ => None,
            }),
    );
    handles.extend(
        pending_groups
            .flat_map(|group| group.transactions.iter())
            .filter_map(|transaction| match transaction.target_buffer() {
                BufferSource::CpuBuffer { handle } => Some(handle),
                _ => None,
            }),
    );
    handles.extend_from_slice(staged);
    handles.extend(recent_updates);
    handles.sort_unstable();
    handles.dedup();
}

fn authority_batch_cpu_buffer_updates(
    batch: &LiveProductionAuthorityBatch,
) -> Vec<crate::LiveCpuBufferUpdate> {
    batch
        .groups
        .iter()
        .flat_map(|group| group.cpu_buffer_updates.iter().cloned())
        .collect()
}

fn authority_group_present_owners(
    group: &LiveProductionAuthorityGroup,
) -> Result<Vec<SurfaceTransactionKey>, &'static str> {
    let mut owners = group
        .software_present_submissions
        .iter()
        .map(|submission| submission.candidate)
        .collect::<Vec<_>>();
    for submission in &group.present_submissions {
        let mut candidates = group.transactions.iter().filter(|transaction| {
            transaction.transaction == submission.transaction
                && transaction.surface == submission.surface
                && transaction.target_buffer()
                    == BufferSource::DmaBuf {
                        handle: submission.buffer.raw(),
                    }
        });
        let owner = candidates
            .next()
            .ok_or("DMA-BUF Present has no exact content owner")?
            .key();
        if candidates.next().is_some() {
            return Err("DMA-BUF Present has multiple content owners");
        }
        owners.push(owner);
    }
    Ok(owners)
}

fn record_recent_cpu_buffer_updates(
    recent: &mut VecDeque<u64>,
    updates: &[crate::LiveCpuBufferUpdate],
) {
    for handle in updates.iter().map(crate::LiveCpuBufferUpdate::handle) {
        if let Some(index) = recent.iter().position(|candidate| *candidate == handle) {
            recent.remove(index);
        }
        recent.push_back(handle);
    }
    while recent.len() > RECENT_CPU_BUFFER_UPDATE_CAPACITY {
        recent.pop_front();
    }
}

fn retain_relevant_cpu_buffer_updates(
    scene: &LiveProductionCpuScene,
    updates: &mut Vec<crate::LiveCpuBufferUpdate>,
    rooted_handles: &[u64],
) {
    updates.retain(|update| {
        matches!(update, crate::LiveCpuBufferUpdate::Replace(_))
            || scene.contains_buffer(update.handle())
            || rooted_handles.binary_search(&update.handle()).is_ok()
    });
}

fn authority_batch_removed_surfaces(batch: &LiveProductionAuthorityBatch) -> Vec<SurfaceId> {
    batch
        .groups
        .iter()
        .flat_map(|group| group.removed_surfaces.iter().copied())
        .collect()
}

pub fn live_production_transactions_require_gpu_scanout(
    transactions: &[SurfaceTransaction],
) -> bool {
    transactions
        .iter()
        .any(|transaction| matches!(transaction.target_buffer(), BufferSource::DmaBuf { .. }))
}

pub fn live_production_projection_requires_gpu_scanout(
    transactions: &[SurfaceTransaction],
    presentation_order: &[SurfaceId],
) -> bool {
    transactions.iter().any(|transaction| {
        presentation_order.contains(&transaction.surface)
            && matches!(transaction.target_buffer(), BufferSource::DmaBuf { .. })
    })
}

fn live_production_committed_projection_requires_gpu_scanout(
    committed: &[CommittedSurfaceState],
    presentation_order: &[SurfaceId],
) -> bool {
    committed.iter().any(|state| {
        presentation_order.contains(&state.surface)
            && matches!(state.buffer(), BufferSource::DmaBuf { .. })
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

pub const fn live_production_retained_projection_admitted(
    visual_projection_changed: bool,
    current_cpu_updates: bool,
    committed_projection_requires_gpu: bool,
) -> bool {
    visual_projection_changed && (!current_cpu_updates || committed_projection_requires_gpu)
}

pub const fn live_production_should_preserve_gpu_output(
    native_enabled: bool,
    gpu_present_submitted: bool,
    retained_projection_queued: bool,
    presentation_order_changed: bool,
    committed_projection_requires_gpu: bool,
) -> bool {
    native_enabled
        && (gpu_present_submitted
            || retained_projection_queued
            || (!presentation_order_changed && committed_projection_requires_gpu))
}
