#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
mod persistent_native_scanout {
    use crate::*;
    use sophia_engine::{CompositorBackendTickInput, OutputFramePresentationState};
    use sophia_protocol::{OutputId, TransactionId};
    use std::collections::BTreeMap;
    use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
    use std::time::{Duration, Instant};

    mod frame_damage;
    mod renderer_images;
    mod state;
    use frame_damage::trace_presented_output_damage;
    pub use state::*;

    pub struct LiveProductionNativeScanout {
        pub groups: Vec<LiveProductionNativeGroup>,
        pub heads: Vec<LiveProductionNativeHead>,
        pub discovered_outputs: usize,
        pub presentation_outputs: usize,
        pub submissions: usize,
        pub submit_deferred: usize,
        pub submit_failures: usize,
        pub retirements: usize,
        pub retire_failures: usize,
        pub max_in_flight_ticks: u64,
        pub max_submit_to_page_flip: Duration,
        pub callback_accepted: usize,
        pub callback_rejected: usize,
        pub callback_queue_saturated: usize,
        pub nonzero_exports: usize,
        pub production_page_flips: crate::LiveProductionPageFlipTracker,
        pub presentation_started: Instant,
        pub kernel_page_flip_timestamps: usize,
        pub kernel_page_flip_timestamp_missing: usize,
        kernel_page_flip_ust: BTreeMap<(OutputId, u64), u64>,
        pub vsync_overlap_rejections: usize,
        pub page_flip_phase_rejections: usize,
        pub cursor_updates: usize,
        pub cursor_hidden_updates: usize,
        pub cursor_deferred_primary_in_flight: usize,
        pub cursor_update_failures: usize,
        pub max_cursor_update: Duration,
    }

    pub struct LiveProductionNativeGroup {
        pub session: crate::RealAtomicScanoutPageFlipSession,
        pub sender: SyncSender<crate::LivePageFlipCallback>,
        pub receiver: Receiver<crate::LivePageFlipCallback>,
    }

    pub struct LiveProductionNativeHead {
        pub group: usize,
        pub selection: crate::LibdrmNativePrimaryPlaneSelection,
        pub exporter: crate::NativeGbmRenderedScanoutBufferDiscoveryExporter<
            crate::RealAtomicScanoutRenderDeviceDiscovery,
        >,
        pub sender: SyncSender<crate::LivePageFlipCallback>,
        pub receiver: Option<Receiver<crate::LivePageFlipCallback>>,
        pub output: sophia_engine::HeadlessOutput,
        pub submitted_at: Option<Instant>,
        pub submitted_ust_usec: Option<u64>,
        pub pending_nonzero_pixel_bytes: usize,
        pub last_checksum: u64,
        pub submitted_checksum: Option<u64>,
        pub submitted_sequence: Option<usize>,
        pub pending_content: Option<LiveProductionScanoutContent>,
        pub rendering_content: Option<LiveProductionScanoutContent>,
        pub submitted_content: Option<LiveProductionScanoutContent>,
        pub presented_content: Option<LiveProductionScanoutContent>,
        pub presented_checksum: u64,
        pub presented_submissions: usize,
        pub presented_submission_ust_usec: u64,
        pub presented_submit_to_page_flip: Duration,
        pub submissions: usize,
        pub retirements: usize,
        pub callback_accepted: usize,
        pub initial_modeset_submission: Option<usize>,
        pub nonzero_exports: usize,
        pub last_submit_report: Option<crate::LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport>,
        pub output_frames: OutputFramePresentationState,
    }

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct LivePersistentRenderMetrics {
        pub target_creations: usize,
        pub target_recreations: usize,
        pub pipeline_creations: usize,
        pub frame_surface_creations: usize,
        pub cpu_target_creations: usize,
        pub dmabuf_target_creations: usize,
        pub composition_target_creations: usize,
        pub composition_target_reuses: usize,
        pub generation_replacements: usize,
        pub recovery_replacements: usize,
        pub uploads: usize,
        pub import_cache_imports: usize,
        pub import_cache_hits: usize,
        pub import_cache_evictions: usize,
        pub import_cache_live_entries: usize,
        pub import_cache_descriptor_mismatches: usize,
        pub import_cache_capacity_rejections: usize,
        pub worker_requests: usize,
        pub worker_completions: usize,
        pub worker_failures: usize,
        pub worker_soft_stalls: usize,
        pub worker_hard_stalls: usize,
        pub worker_release_enqueue_failures: usize,
        pub max_worker_request: Duration,
        pub max_target_create: Duration,
        pub max_frame_surface_create: Duration,
        pub max_render: Duration,
        pub max_upload: Duration,
    }

    impl LiveProductionNativeScanout {
        pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
            Self::new_with_selection(crate::select_real_atomic_scanout_cards())
        }

        #[cfg(feature = "seat-control")]
        pub fn new_with_seat(
            opener: &crate::LiveSeatDeviceOpener,
        ) -> Result<Self, Box<dyn std::error::Error>> {
            Self::new_with_selection(crate::select_real_atomic_scanout_cards_with_seat(opener))
        }

        fn new_with_selection(
            selection: crate::RealAtomicScanoutSelectionSet,
        ) -> Result<Self, Box<dyn std::error::Error>> {
            let authority = crate::RealAtomicScanoutSmokeConfig::default_primary_output()
                .ok_or("persistent native scanout config is invalid")?
                .authority;
            let mut sessions = selection.into_page_flip_sessions(authority);
            if sessions.status != crate::RealAtomicScanoutPageFlipSessionSetStatus::Ready {
                return Err(format!(
                    "persistent native scanout could not open all KMS outputs: {:?}",
                    sessions.status
                )
                .into());
            }
            let outputs = sophia_engine::discover_drm_kms_outputs_from_sysfs("/sys/class/drm")?;
            if sessions.output_count != outputs.len() {
                return Err(format!(
                    "persistent native ownership is partial: discovered={} native={}",
                    outputs.len(),
                    sessions.output_count
                )
                .into());
            }
            let mut presentation_outputs = sophia_engine::DrmKmsOutputRegistry::new();
            for session in &sessions.sessions {
                for (selection, output_id) in session
                    .selections()
                    .iter()
                    .copied()
                    .zip(session.outputs().iter().copied())
                {
                    let Some(descriptor) = outputs
                        .outputs()
                        .find(|descriptor| descriptor.connector_id == selection.connector_id())
                        .copied()
                    else {
                        return Err(format!(
                            "persistent native output has no Engine connector match: connector={}",
                            selection.connector_id(),
                        )
                        .into());
                    };
                    let descriptor = sophia_engine::DrmKmsOutputDescriptor {
                        output: output_id,
                        ..descriptor
                    };
                    if presentation_outputs.upsert(descriptor)
                        == sophia_engine::DrmKmsOutputRegistryUpdate::CapacityExceeded
                    {
                        return Err(
                            "persistent native presentation output capacity exceeded".into()
                        );
                    }
                }
            }
            if presentation_outputs.len() != sessions.output_count {
                return Err(format!(
                    "persistent native connector mapping is incomplete: mapped={} native={}",
                    presentation_outputs.len(),
                    sessions.output_count,
                )
                .into());
            }
            let presentation_output_count = presentation_outputs.len();
            let production_page_flips =
                crate::LiveProductionPageFlipTracker::from_outputs(&presentation_outputs);
            let mut groups = Vec::new();
            let mut heads = Vec::new();
            for session in sessions.sessions.drain(..) {
                let group = groups.len();
                for (selection, output_id) in session
                    .selections()
                    .iter()
                    .copied()
                    .zip(session.outputs().iter().copied())
                {
                    let size = selection.size();
                    let discovery = session.render_device_discovery()?;
                    let exporter =
                        crate::NativeGbmRenderedScanoutBufferDiscoveryExporter::new(discovery)
                            .with_preferred_modifiers(
                                session
                                    .preferred_xrgb8888_scanout_modifiers_for_selection(selection),
                            );
                    let (sender, receiver) = sync_channel(64);
                    heads.push(LiveProductionNativeHead {
                        group,
                        selection,
                        exporter,
                        sender,
                        receiver: Some(receiver),
                        output: sophia_engine::HeadlessOutput {
                            id: output_id,
                            size,
                            scale: 1,
                        },
                        submitted_at: None,
                        submitted_ust_usec: None,
                        pending_nonzero_pixel_bytes: 0,
                        last_checksum: 0,
                        submitted_checksum: None,
                        submitted_sequence: None,
                        pending_content: None,
                        rendering_content: None,
                        submitted_content: None,
                        presented_content: None,
                        presented_checksum: 0,
                        presented_submissions: 0,
                        presented_submission_ust_usec: 0,
                        presented_submit_to_page_flip: Duration::ZERO,
                        submissions: 0,
                        retirements: 0,
                        callback_accepted: 0,
                        initial_modeset_submission: None,
                        nonzero_exports: 0,
                        last_submit_report: None,
                        output_frames: OutputFramePresentationState::new(
                            sophia_engine::HeadlessOutput {
                                id: output_id,
                                size,
                                scale: 1,
                            },
                        )
                        .map_err(|error| {
                            format!(
                                "native output has invalid compositor display-list state: {error}"
                            )
                        })?,
                    });
                }
                let (sender, receiver) = sync_channel(64);
                groups.push(LiveProductionNativeGroup {
                    session,
                    sender,
                    receiver,
                });
            }
            heads.sort_by_key(|head| head.output.id);
            Ok(Self {
                groups,
                heads,
                discovered_outputs: outputs.len(),
                presentation_outputs: presentation_output_count,
                submissions: 0,
                submit_deferred: 0,
                submit_failures: 0,
                retirements: 0,
                retire_failures: 0,
                max_in_flight_ticks: 0,
                max_submit_to_page_flip: Duration::ZERO,
                callback_accepted: 0,
                callback_rejected: 0,
                callback_queue_saturated: 0,
                nonzero_exports: 0,
                production_page_flips,
                presentation_started: Instant::now(),
                kernel_page_flip_timestamps: 0,
                kernel_page_flip_timestamp_missing: 0,
                kernel_page_flip_ust: BTreeMap::new(),
                vsync_overlap_rejections: 0,
                page_flip_phase_rejections: 0,
                cursor_updates: 0,
                cursor_hidden_updates: 0,
                cursor_deferred_primary_in_flight: 0,
                cursor_update_failures: 0,
                max_cursor_update: Duration::ZERO,
            })
        }

        pub fn update_classic_hardware_cursor(
            &mut self,
            position: sophia_protocol::Point,
        ) -> Result<crate::ClassicHardwareCursorUpdate, Box<dyn std::error::Error>> {
            if !position.x.is_finite() || !position.y.is_finite() {
                return Ok(crate::ClassicHardwareCursorUpdate::Hidden);
            }
            if self.heads.iter().any(|head| head.submitted_at.is_some()) {
                self.cursor_deferred_primary_in_flight =
                    self.cursor_deferred_primary_in_flight.saturating_add(1);
                return Ok(crate::ClassicHardwareCursorUpdate::Deferred);
            }
            let update_started = Instant::now();
            let mut offset_x = 0_i32;
            let mut target = None;
            for head in &self.heads {
                let width = head.output.size.width;
                let height = head.output.size.height;
                let x = position.x.floor() as i32;
                let y = position.y.floor() as i32;
                if x >= offset_x && x < offset_x.saturating_add(width) && y >= 0 && y < height {
                    target = Some((head.group, head.selection, x.saturating_sub(offset_x), y));
                    break;
                }
                offset_x = offset_x.saturating_add(width);
            }

            let mut visible = false;
            let mut deferred = false;
            for (group_index, group) in self.groups.iter_mut().enumerate() {
                let group_target = target
                    .filter(|(target_group, _, _, _)| *target_group == group_index)
                    .map(|(_, selection, x, y)| (selection, x, y));
                match group.session.update_classic_hardware_cursor(group_target) {
                    Ok(crate::ClassicHardwareCursorUpdate::Visible) => visible = true,
                    Ok(crate::ClassicHardwareCursorUpdate::Deferred) => deferred = true,
                    Ok(crate::ClassicHardwareCursorUpdate::Hidden) => {}
                    Err(error) => {
                        self.max_cursor_update =
                            self.max_cursor_update.max(update_started.elapsed());
                        self.cursor_update_failures = self.cursor_update_failures.saturating_add(1);
                        return Err(format!("hardware cursor update failed: {error}").into());
                    }
                }
            }
            self.max_cursor_update = self.max_cursor_update.max(update_started.elapsed());
            if visible {
                self.cursor_updates = self.cursor_updates.saturating_add(1);
            }
            Ok(if deferred {
                crate::ClassicHardwareCursorUpdate::Deferred
            } else if visible {
                crate::ClassicHardwareCursorUpdate::Visible
            } else {
                self.cursor_hidden_updates = self.cursor_hidden_updates.saturating_add(1);
                crate::ClassicHardwareCursorUpdate::Hidden
            })
        }

        pub fn clone_render_device_file(&self) -> std::io::Result<std::fs::File> {
            self.groups
                .first()
                .ok_or_else(|| std::io::Error::other("native scanout has no DRM device group"))?
                .session
                .card()
                .try_clone_file()
        }

        pub fn outputs(&self) -> Vec<sophia_engine::HeadlessOutput> {
            self.heads.iter().map(|head| head.output).collect()
        }

        pub fn output_index(&self, output: OutputId) -> Option<usize> {
            self.heads.iter().position(|head| head.output.id == output)
        }

        pub fn page_flip_hard_stall(&self) -> Option<(OutputId, Duration)> {
            self.heads.iter().find_map(|head| {
                let age = head.submitted_at.map(|submitted| submitted.elapsed());
                (reduce_live_production_page_flip_watchdog(
                    age,
                    LIVE_PRODUCTION_PAGE_FLIP_HARD_STALL,
                ) == LiveProductionPageFlipWatchdogStatus::HardStall)
                    .then_some((head.output.id, age.unwrap_or_default()))
            })
        }

        pub fn ensure_page_flip_progress(&self) -> Result<(), Box<dyn std::error::Error>> {
            let Some((output, age)) = self.page_flip_hard_stall() else {
                return Ok(());
            };
            tracing::error!(
                "sophia_live_native_page_flip schema=1 status=hard_stall output={} age_ms={} action=terminate_session",
                output.raw(),
                age.as_millis(),
            );
            Err(format!(
                "native page flip exceeded the {} ms hard-stall boundary",
                LIVE_PRODUCTION_PAGE_FLIP_HARD_STALL.as_millis(),
            )
            .into())
        }

        pub fn selection(&self, index: usize) -> crate::LibdrmNativePrimaryPlaneSelection {
            self.heads[index].selection
        }

        pub fn card(&self, index: usize) -> &crate::RealAtomicScanoutCard {
            self.groups[self.heads[index].group].session.card()
        }

        pub fn take_receiver(&mut self, index: usize) -> Receiver<crate::LivePageFlipCallback> {
            self.heads[index]
                .receiver
                .take()
                .expect("native page-flip receiver must attach once")
        }

        pub fn run_tick(
            &mut self,
            index: usize,
            runtime: &mut crate::LiveBackendRuntimeAssembly,
            input: CompositorBackendTickInput,
        ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
            self.ensure_page_flip_progress()?;
            if !self.heads[index].exporter.pending_frame() {
                self.retire_ready_and_retry_cleanup(index, runtime)?;
                return Ok(runtime.run_tick(input)?);
            }
            let group = self.heads[index].group;
            self.poll_group_callbacks(group)?;
            let (report, exported_nonzero, worker_was_in_flight) = {
                let groups = &mut self.groups;
                let head = &mut self.heads[index];
                let worker_was_in_flight = head.exporter.worker_in_flight();
                let export_attempts_before = head.exporter.cpu_frame_export_attempts();
                let report = runtime
                    .run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_with(
                        input,
                        groups[group].session.card(),
                        &mut head.exporter,
                    )?;
                let exported_nonzero = head.exporter.cpu_frame_export_attempts()
                    > export_attempts_before
                    && head.pending_nonzero_pixel_bytes > 0;
                if !head.exporter.pending_cpu_frame() {
                    head.pending_nonzero_pixel_bytes = 0;
                }
                (report, exported_nonzero, worker_was_in_flight)
            };
            if exported_nonzero {
                self.nonzero_exports = self.nonzero_exports.saturating_add(1);
                self.heads[index].nonzero_exports =
                    self.heads[index].nonzero_exports.saturating_add(1);
            }
            if let Some(retire) = report.rendered_primary_plane_scanout_retire {
                self.observe_retire(index, retire);
            }
            self.observe_callbacks(index, report.page_flip_callbacks);
            if let Some(submit) = report.rendered_primary_plane_scanout_submit {
                self.heads[index].last_submit_report = Some(submit);
                use crate::LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus as Status;
                let worker_is_in_flight = self.heads[index].exporter.worker_in_flight();
                match submit.status {
                    Status::SubmittedWaitingForPageFlip => {
                        let content = if worker_was_in_flight {
                            self.heads[index].rendering_content.take()
                        } else {
                            self.heads[index].pending_content.take()
                        };
                        let content = content.map(|content| {
                            content.with_nonzero_rgb_pixels(
                                self.heads[index].exporter.composition_nonzero_rgb_pixels(),
                            )
                        });
                        if worker_was_in_flight
                            && self.heads[index].output_frames.rendering().is_some()
                        {
                            self.heads[index]
                                .output_frames
                                .promote_rendering_to_submitted()
                                .map_err(|error| {
                                    format!(
                                        "compositor display-list worker promotion failed: {error}"
                                    )
                                })?;
                        } else if !worker_was_in_flight
                            && self.heads[index].output_frames.pending().is_some()
                        {
                            self.heads[index]
                                .output_frames
                                .mark_submitted()
                                .map_err(|error| {
                                    format!(
                                        "compositor display-list submit transition failed: {error}"
                                    )
                                })?;
                        }
                        trace_live_native_lifecycle("kms_submit_accepted");
                        self.submissions = self.submissions.saturating_add(1);
                        self.heads[index].submissions =
                            self.heads[index].submissions.saturating_add(1);
                        self.heads[index].submitted_at = Some(Instant::now());
                        self.heads[index].submitted_ust_usec = Some(Self::monotonic_ust_usec());
                        self.heads[index].submitted_checksum =
                            Some(self.heads[index].last_checksum);
                        self.heads[index].submitted_sequence = Some(self.heads[index].submissions);
                        self.heads[index].submitted_content = content;
                        if matches!(
                            content,
                            Some(
                                LiveProductionScanoutContent::MixedPresent {
                                    nonzero_rgb_pixels: 1..,
                                    ..
                                } | LiveProductionScanoutContent::RetainedMixed {
                                    nonzero_rgb_pixels: 1..,
                                }
                            )
                        ) {
                            self.nonzero_exports = self.nonzero_exports.saturating_add(1);
                            self.heads[index].nonzero_exports =
                                self.heads[index].nonzero_exports.saturating_add(1);
                        }
                        let output = self.heads[index].output.id;
                        let cycle =
                            u64::try_from(self.heads[index].submissions).unwrap_or(u64::MAX);
                        tracing::info!(
                            "sophia_live_native_page_flip schema=1 status=submitted output={} submission={} content={:?}",
                            output.raw(),
                            cycle,
                            content,
                        );
                        if self.production_page_flips.submit(output, cycle).is_err() {
                            self.vsync_overlap_rejections =
                                self.vsync_overlap_rejections.saturating_add(1);
                        }
                    }
                    Status::ScanoutExportPending => {
                        if !worker_was_in_flight && worker_is_in_flight {
                            self.heads[index].rendering_content =
                                self.heads[index].pending_content.take();
                            if self.heads[index].output_frames.pending().is_some() {
                                self.heads[index]
                                    .output_frames
                                    .mark_rendering()
                                    .map_err(|error| {
                                        format!(
                                            "compositor display-list render transition failed: {error}"
                                        )
                                    })?;
                            }
                        }
                        self.submit_deferred = self.submit_deferred.saturating_add(1);
                    }
                    Status::AlreadyInFlight | Status::CleanupPending => {
                        self.submit_deferred = self.submit_deferred.saturating_add(1);
                    }
                    status => {
                        let failed_content = if worker_was_in_flight {
                            self.heads[index].rendering_content.take()
                        } else {
                            self.heads[index].pending_content.take()
                        };
                        if worker_was_in_flight {
                            self.heads[index].output_frames.discard_rendering();
                        } else {
                            self.heads[index].output_frames.discard_pending();
                        }
                        self.submit_failures = self.submit_failures.saturating_add(1);
                        tracing::warn!(
                            "sophia_live_native_submit schema=1 status=failed output={} reason={status:?} content={failed_content:?} export={:?} scanout_buffer={:?} resources={:?} framebuffer={:?} submit={:?} commit={:?}",
                            self.heads[index].output.id.raw(),
                            submit.export,
                            submit.scanout_buffer,
                            submit.resources,
                            submit.framebuffer,
                            submit.submit,
                            submit.commit_submit,
                        );
                    }
                }
            }
            self.max_in_flight_ticks = self
                .max_in_flight_ticks
                .max(report.rendered_primary_plane_scanout_in_flight_ticks);
            Ok(report)
        }

        pub fn retire_ready(
            &mut self,
            index: usize,
            runtime: &mut crate::LiveBackendRuntimeAssembly,
        ) -> Result<(), Box<dyn std::error::Error>> {
            self.ensure_page_flip_progress()?;
            let group = self.heads[index].group;
            self.poll_group_callbacks(group)?;
            let report = runtime.drain_rendered_primary_plane_page_flip_callbacks_with(
                self.groups[group].session.card(),
            );
            self.observe_callbacks(index, report.page_flip_callbacks);
            if let Some(retire) = report.rendered_primary_plane_scanout_retire {
                self.observe_retire(index, retire);
            }
            Ok(())
        }

        pub fn retire_ready_and_retry_cleanup(
            &mut self,
            index: usize,
            runtime: &mut crate::LiveBackendRuntimeAssembly,
        ) -> Result<(), Box<dyn std::error::Error>> {
            self.retire_ready(index, runtime)?;
            if runtime.rendered_primary_plane_scanout_cleanup_pending() {
                let cleanup =
                    runtime.retry_tracked_rendered_primary_plane_scanout_cleanup(self.card(index));
                if !cleanup.cleanup_pending {
                    self.retire_failures = self.retire_failures.saturating_sub(1);
                }
            }
            Ok(())
        }

        pub fn release_displayed_output(
            &mut self,
            index: usize,
            runtime: &mut crate::LiveBackendRuntimeAssembly,
        ) -> Result<(), Box<dyn std::error::Error>> {
            trace_live_native_lifecycle("displayed_scanout_retire_started");
            let retired = runtime.retire_displayed_rendered_primary_plane_scanout(self.card(index));
            if retired.cleanup_pending {
                trace_live_native_lifecycle("displayed_scanout_cleanup_retry_started");
                let cleanup =
                    runtime.retry_tracked_rendered_primary_plane_scanout_cleanup(self.card(index));
                if cleanup.cleanup_pending {
                    return Err("persistent displayed scanout cleanup remained pending".into());
                }
            }
            trace_live_native_lifecycle("displayed_scanout_owner_released");
            Ok(())
        }

        pub fn observe_retire(
            &mut self,
            index: usize,
            retire: crate::LiveTrackedRenderedPrimaryPlaneScanoutRetireReport,
        ) {
            use crate::LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus as Status;
            match retire.status {
                Status::RetiredAfterPageFlip => {
                    trace_live_native_lifecycle("kms_buffer_retired");
                    tracing::info!(
                        "sophia_live_native_page_flip schema=1 status=retired output={} submission={}",
                        self.heads[index].output.id.raw(),
                        self.heads[index]
                            .submitted_sequence
                            .unwrap_or(self.heads[index].submissions),
                    );
                    self.retirements = self.retirements.saturating_add(1);
                    self.heads[index].retirements = self.heads[index].retirements.saturating_add(1);
                }
                Status::NoSubmission | Status::WaitingForAcceptedPageFlip => {}
                Status::ResourceRetireFailed => {
                    self.retire_failures = self.retire_failures.saturating_add(1);
                }
            }
        }

        pub fn observe_callbacks(
            &mut self,
            index: usize,
            report: crate::LivePageFlipCallbackQueueReport,
        ) {
            self.callback_accepted = self.callback_accepted.saturating_add(report.accepted);
            self.heads[index].callback_accepted = self.heads[index]
                .callback_accepted
                .saturating_add(report.accepted);
            if report.accepted > 0 {
                trace_live_native_lifecycle("page_flip_callback_accepted");
                tracing::info!(
                    "sophia_live_native_page_flip schema=1 status=callback_accepted output={} callbacks={} kernel_sequence={}",
                    self.heads[index].output.id.raw(),
                    report.accepted,
                    report
                        .last_accepted
                        .and_then(|accepted| accepted.event.frame_serial)
                        .map_or_else(|| "none".to_owned(), |serial| serial.to_string()),
                );
                if let Some(checksum) = self.heads[index].submitted_checksum.take() {
                    self.heads[index].presented_checksum = checksum;
                }
                if let Some(submission) = self.heads[index].submitted_sequence.take() {
                    self.heads[index].presented_submissions = submission;
                }
                self.heads[index].presented_content = self.heads[index].submitted_content.take();
                if self.heads[index].output_frames.submitted().is_some() {
                    let presented = self.heads[index]
                        .output_frames
                        .mark_presented()
                        .expect("submitted display-list state checked above");
                    trace_presented_output_damage(
                        "presented",
                        self.heads[index].output.id,
                        &presented,
                    );
                }
                let output = self.heads[index].output.id;
                if let Some(kernel_sequence) = report
                    .last_accepted
                    .and_then(|accepted| accepted.event.frame_serial)
                {
                    let (presentation_msec, ust) = if let Some(ust) =
                        self.kernel_page_flip_ust.remove(&(output, kernel_sequence))
                    {
                        self.kernel_page_flip_timestamps =
                            self.kernel_page_flip_timestamps.saturating_add(1);
                        (ust / 1_000, ust)
                    } else {
                        self.kernel_page_flip_timestamp_missing =
                            self.kernel_page_flip_timestamp_missing.saturating_add(1);
                        let elapsed = self.presentation_started.elapsed();
                        (
                            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
                            u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX),
                        )
                    };
                    let submitted_ust_usec = self.heads[index].submitted_ust_usec.take();
                    let submit_to_page_flip = submitted_ust_usec
                        .and_then(|submitted| ust.checked_sub(submitted))
                        .map(Duration::from_micros)
                        .or_else(|| {
                            self.heads[index]
                                .submitted_at
                                .map(|submitted| submitted.elapsed())
                        })
                        .unwrap_or_default();
                    self.heads[index].submitted_at = None;
                    self.max_submit_to_page_flip =
                        self.max_submit_to_page_flip.max(submit_to_page_flip);
                    self.heads[index].presented_submission_ust_usec =
                        submitted_ust_usec.unwrap_or_default();
                    self.heads[index].presented_submit_to_page_flip = submit_to_page_flip;
                    if self
                        .production_page_flips
                        .observe_page_flip(output, kernel_sequence, presentation_msec, ust)
                        .is_err()
                    {
                        self.page_flip_phase_rejections =
                            self.page_flip_phase_rejections.saturating_add(1);
                    }
                }
            }
            self.callback_rejected = self.callback_rejected.saturating_add(
                report.rejected_unexpected_output + report.rejected_stale_frame_serial,
            );
            self.callback_queue_saturated = self
                .callback_queue_saturated
                .saturating_add(usize::from(report.max_reached));
        }

        fn monotonic_ust_usec() -> u64 {
            let now = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
            let seconds = u64::try_from(now.tv_sec).unwrap_or_default();
            let nanos = u64::try_from(now.tv_nsec).unwrap_or_default();
            seconds
                .saturating_mul(1_000_000)
                .saturating_add(nanos / 1_000)
        }

        pub fn initialize(
            &mut self,
            index: usize,
            runtime: &mut crate::LiveBackendRuntimeAssembly,
            frame: LiveProductionComposedFrame,
        ) -> Result<(), Box<dyn std::error::Error>> {
            self.queue_frame(index, frame);
            let group = self.heads[index].group;
            let groups = &mut self.groups;
            let head = &mut self.heads[index];
            let export_attempts_before = head.exporter.cpu_frame_export_attempts();
            groups[group]
                .session
                .initialize_persistent_native_gbm_scanout_for_selection(
                    runtime,
                    &mut head.exporter,
                    head.selection,
                )
                .map_err(|evidence| {
                    format!("persistent native initial modeset failed: {evidence:?}")
                })?;
            head.exporter.enable_worker()?;
            if head.exporter.cpu_frame_export_attempts() > export_attempts_before
                && head.pending_nonzero_pixel_bytes > 0
            {
                self.nonzero_exports = self.nonzero_exports.saturating_add(1);
                head.nonzero_exports = head.nonzero_exports.saturating_add(1);
            }
            if !head.exporter.pending_cpu_frame() {
                head.pending_nonzero_pixel_bytes = 0;
            }
            self.submissions = self.submissions.saturating_add(1);
            trace_live_native_lifecycle("initial_modeset_complete");
            head.submissions = head.submissions.saturating_add(1);
            head.presented_checksum = head.last_checksum;
            head.presented_submissions = head.submissions;
            head.presented_content = head.pending_content.take();
            if head.output_frames.pending().is_some() {
                let presented = head
                    .output_frames
                    .mark_initial_presented()
                    .map_err(|error| {
                        format!("initial compositor display-list transition failed: {error}")
                    })?;
                trace_presented_output_damage("initial_presented", head.output.id, &presented);
            }
            head.initial_modeset_submission = Some(head.submissions);
            Ok(())
        }

        pub fn queue_frame(
            &mut self,
            index: usize,
            frame: LiveProductionComposedFrame,
        ) -> LiveProductionCpuFrameQueueStatus {
            let head = &mut self.heads[index];
            let status = reduce_live_production_cpu_frame_queue(
                head.pending_content,
                head.submitted_content,
                head.presented_content,
                head.exporter.worker_in_flight(),
                head.callback_accepted != 0 || head.initial_modeset_submission.is_some(),
                frame.checksum,
            );
            if !matches!(
                status,
                LiveProductionCpuFrameQueueStatus::Queued
                    | LiveProductionCpuFrameQueueStatus::BaselineRequired
            ) {
                return status;
            }
            head.pending_nonzero_pixel_bytes = frame.nonzero_pixel_bytes;
            head.last_checksum = frame.checksum;
            head.queue_output_damage_snapshot(frame.output_damage_snapshot.clone());
            head.pending_content = Some(LiveProductionScanoutContent::Cpu {
                checksum: frame.checksum,
            });
            head.exporter
                .set_pending_cpu_frame_with_checksum(frame.frame, frame.checksum);
            status
        }

        pub fn take_presentation_feedback(&mut self, output: OutputId) -> Option<(u64, u64)> {
            let retirement = self.production_page_flips.take_retirement(output)?;
            Some((retirement.retirement.ust, retirement.retirement.msc))
        }

        pub fn pending_kernel_page_flip_timestamps(&self) -> usize {
            self.kernel_page_flip_ust.len()
        }

        pub fn discard_presentation_feedback(&mut self, output: Option<OutputId>) {
            self.production_page_flips.discard_retirements(output);
        }

        pub fn pending_frame(&self, index: usize) -> bool {
            self.heads[index].exporter.pending_frame()
        }

        pub fn stable_present(&self, output: OutputId, transaction: TransactionId) -> bool {
            let Some(index) = self.output_index(output) else {
                return false;
            };
            let head = &self.heads[index];
            live_production_scanout_is_stable_present(
                head.presented_content,
                head.submitted_content,
                head.exporter.pending_frame(),
                transaction,
            )
        }

        pub fn presented_mixed_nonzero_rgb_pixels(&self, transaction: TransactionId) -> usize {
            self.heads
                .iter()
                .find_map(|head| match head.presented_content {
                    Some(LiveProductionScanoutContent::MixedPresent {
                        transaction: presented,
                        nonzero_rgb_pixels,
                    }) if presented == transaction => Some(nonzero_rgb_pixels),
                    _ => None,
                })
                .unwrap_or(0)
        }

        pub fn poll_group_callbacks(
            &mut self,
            group: usize,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let (callbacks, timestamps) = {
                let group = &mut self.groups[group];
                let _ = group
                    .session
                    .poll_native_page_flip_events(&group.sender, 64, 64);
                let timestamps = group.session.drain_emitted_kernel_page_flip_timestamps();
                let mut callbacks = Vec::new();
                loop {
                    match group.receiver.try_recv() {
                        Ok(callback) => callbacks.push(callback),
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            return Err("native card callback router disconnected".into());
                        }
                    }
                }
                (callbacks, timestamps)
            };
            for timestamp in timestamps {
                self.kernel_page_flip_ust.insert(
                    (timestamp.output, timestamp.frame_serial),
                    timestamp.ust_usec,
                );
            }
            for callback in callbacks {
                let Some(head) = self
                    .heads
                    .iter()
                    .find(|head| head.output.id == callback.output)
                else {
                    return Err("native callback referenced an unknown output".into());
                };
                head.sender
                    .try_send(callback)
                    .map_err(|error| match error {
                        TrySendError::Full(_) => "native output callback queue is full",
                        TrySendError::Disconnected(_) => {
                            "native output callback queue is disconnected"
                        }
                    })?;
            }
            Ok(())
        }
    }

    fn trace_live_native_lifecycle(stage: &str) {
        if std::env::var_os("SOPHIA_LIVE_SESSION_DIAGNOSTIC").is_some() {
            tracing::info!("sophia_live_native_lifecycle schema=1 stage={stage}");
        }
    }
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
pub use persistent_native_scanout::{
    LIVE_PRODUCTION_PAGE_FLIP_HARD_STALL, LivePersistentRenderMetrics,
    LiveProductionCpuFrameQueueStatus, LiveProductionNativeHead, LiveProductionNativeScanout,
    LiveProductionPageFlipWatchdogStatus, LiveProductionScanoutContent,
    live_production_scanout_is_stable_present, reduce_live_production_cpu_frame_queue,
    reduce_live_production_page_flip_watchdog,
};

#[derive(Debug)]
pub struct LiveNativeMixedDiagnosticComplete {
    pub status: crate::LiveRendererScanoutBufferExportStatus,
    pub detail: crate::LiveRendererScanoutBufferExportDetail,
    pub cpu_layers: usize,
    pub dmabuf_layers: usize,
    pub live_sources: usize,
    pub live_fences: usize,
    pub live_transactions: usize,
}

impl LiveNativeMixedDiagnosticComplete {
    pub fn reduced_log_line(&self, child_outcome: &str) -> String {
        format!(
            "sophia_native_egl_mixed schema=1 case=mixed status={:?} stage={:?} cpu_layers={} dmabuf_layers={} child_outcome={} live_sources={} live_fences={} live_transactions={}",
            self.status,
            self.detail,
            self.cpu_layers,
            self.dmabuf_layers,
            child_outcome,
            self.live_sources,
            self.live_fences,
            self.live_transactions,
        )
    }
}

impl std::fmt::Display for LiveNativeMixedDiagnosticComplete {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.reduced_log_line("completed"))
    }
}

impl std::error::Error for LiveNativeMixedDiagnosticComplete {}
