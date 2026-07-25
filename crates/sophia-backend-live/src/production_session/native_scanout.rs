#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
mod persistent_native_scanout {
    use crate::*;
    use sophia_engine::CompositorBackendTickInput;
    use sophia_protocol::{OutputId, TransactionId};
    use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
    use std::time::{Duration, Instant};

    mod state;
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
        pub vsync_overlap_rejections: usize,
        pub page_flip_phase_rejections: usize,
        pub cursor_updates: usize,
        pub cursor_update_failures: usize,
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
        pub pending_nonzero_pixel_bytes: usize,
        pub last_checksum: u64,
        pub submitted_checksum: Option<u64>,
        pub submitted_sequence: Option<usize>,
        pub pending_content: Option<LiveProductionScanoutContent>,
        pub submitted_content: Option<LiveProductionScanoutContent>,
        pub presented_content: Option<LiveProductionScanoutContent>,
        pub presented_checksum: u64,
        pub presented_submissions: usize,
        pub submissions: usize,
        pub retirements: usize,
        pub callback_accepted: usize,
        pub initial_modeset_submission: Option<usize>,
        pub nonzero_exports: usize,
        pub last_submit_report: Option<crate::LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport>,
    }

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct LivePersistentRenderMetrics {
        pub target_creations: usize,
        pub target_recreations: usize,
        pub pipeline_creations: usize,
        pub cpu_target_creations: usize,
        pub dmabuf_target_creations: usize,
        pub composition_target_creations: usize,
        pub epoch_replacements: usize,
        pub recovery_replacements: usize,
        pub uploads: usize,
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
                        pending_nonzero_pixel_bytes: 0,
                        last_checksum: 0,
                        submitted_checksum: None,
                        submitted_sequence: None,
                        pending_content: None,
                        submitted_content: None,
                        presented_content: None,
                        presented_checksum: 0,
                        presented_submissions: 0,
                        submissions: 0,
                        retirements: 0,
                        callback_accepted: 0,
                        initial_modeset_submission: None,
                        nonzero_exports: 0,
                        last_submit_report: None,
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
                vsync_overlap_rejections: 0,
                page_flip_phase_rejections: 0,
                cursor_updates: 0,
                cursor_update_failures: 0,
            })
        }

        pub fn update_classic_hardware_cursor(
            &mut self,
            position: sophia_protocol::Point,
        ) -> Result<crate::ClassicHardwareCursorUpdate, Box<dyn std::error::Error>> {
            if !position.x.is_finite() || !position.y.is_finite() {
                return Ok(crate::ClassicHardwareCursorUpdate::Hidden);
            }
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
                        self.cursor_update_failures = self.cursor_update_failures.saturating_add(1);
                        return Err(format!("hardware cursor update failed: {error}").into());
                    }
                }
            }
            if visible {
                self.cursor_updates = self.cursor_updates.saturating_add(1);
            }
            Ok(if deferred {
                crate::ClassicHardwareCursorUpdate::Deferred
            } else if visible {
                crate::ClassicHardwareCursorUpdate::Visible
            } else {
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
            if !self.heads[index].exporter.pending_frame() {
                self.retire_ready_and_retry_cleanup(index, runtime)?;
                return Ok(runtime.run_tick(input)?);
            }
            let group = self.heads[index].group;
            self.poll_group_callbacks(group)?;
            let (report, exported_nonzero, queued_content) = {
                let groups = &mut self.groups;
                let head = &mut self.heads[index];
                let queued_content = head.pending_content;
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
                if !head.exporter.pending_frame() {
                    head.pending_content = None;
                }
                let queued_content = queued_content.map(|content| match content {
                    LiveProductionScanoutContent::Mixed { transaction, .. } => {
                        LiveProductionScanoutContent::Mixed {
                            transaction,
                            nonzero_rgb_pixels: head.exporter.composition_nonzero_rgb_pixels(),
                        }
                    }
                    content => content,
                });
                (report, exported_nonzero, queued_content)
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
                match submit.status {
                    Status::SubmittedWaitingForPageFlip => {
                        trace_live_native_lifecycle("kms_submit_accepted");
                        self.submissions = self.submissions.saturating_add(1);
                        self.heads[index].submissions =
                            self.heads[index].submissions.saturating_add(1);
                        self.heads[index].submitted_at = Some(Instant::now());
                        self.heads[index].submitted_checksum =
                            Some(self.heads[index].last_checksum);
                        self.heads[index].submitted_sequence = Some(self.heads[index].submissions);
                        self.heads[index].submitted_content = queued_content;
                        if matches!(
                            queued_content,
                            Some(LiveProductionScanoutContent::Mixed {
                                nonzero_rgb_pixels: 1..,
                                ..
                            })
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
                            queued_content,
                        );
                        if self.production_page_flips.submit(output, cycle).is_err() {
                            self.vsync_overlap_rejections =
                                self.vsync_overlap_rejections.saturating_add(1);
                        }
                    }
                    Status::AlreadyInFlight | Status::CleanupPending => {
                        self.submit_deferred = self.submit_deferred.saturating_add(1);
                    }
                    status => {
                        self.submit_failures = self.submit_failures.saturating_add(1);
                        tracing::warn!(
                            "sophia_live_native_submit schema=1 status=failed output={} reason={status:?} content={queued_content:?} export={:?} scanout_buffer={:?} submit={:?} commit={:?}",
                            self.heads[index].output.id.raw(),
                            submit.export,
                            submit.scanout_buffer,
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
                    if let Some(submitted_at) = self.heads[index].submitted_at.take() {
                        self.max_submit_to_page_flip =
                            self.max_submit_to_page_flip.max(submitted_at.elapsed());
                    }
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
                let output = self.heads[index].output.id;
                if let Some(kernel_sequence) = report
                    .last_accepted
                    .and_then(|accepted| accepted.event.frame_serial)
                {
                    let elapsed = self.presentation_started.elapsed();
                    let presentation_msec = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
                    let ust = u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX);
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
            head.pending_content = Some(LiveProductionScanoutContent::Cpu {
                checksum: frame.checksum,
            });
            head.exporter
                .set_pending_cpu_frame_with_checksum(frame.frame, frame.checksum);
            status
        }

        pub fn queue_mixed_frame(
            &mut self,
            index: usize,
            transaction: TransactionId,
            frame: crate::LiveOwnedMixedCompositionFrame,
        ) {
            let head = &mut self.heads[index];
            if let Some(superseded) = head.pending_content {
                tracing::warn!(
                    "sophia_live_native_scanout schema=1 status=superseded output={} old={superseded:?} new=Mixed({})",
                    head.output.id.raw(),
                    transaction.raw(),
                );
            }
            head.pending_content = Some(LiveProductionScanoutContent::Mixed {
                transaction,
                nonzero_rgb_pixels: 0,
            });
            head.exporter.set_pending_mixed_frame(frame);
        }

        pub fn diagnose_mixed_frame(
            &mut self,
            index: usize,
            frame: crate::LiveOwnedMixedCompositionFrame,
        ) -> (
            crate::LiveRendererScanoutBufferExportStatus,
            crate::LiveRendererScanoutBufferExportDetail,
        ) {
            use crate::LiveRenderedScanoutBufferExporter as _;

            let head = &mut self.heads[index];
            head.exporter.set_pending_mixed_frame(frame);
            let export = head.exporter.export_rendered_scanout_buffer(
                crate::LiveGbmEglFrameTargetRecord::new(head.output.size),
            );
            let status = export.status;
            let detail = export.detail;
            drop(export);
            (status, detail)
        }

        pub fn take_presentation_feedback(&mut self, output: OutputId) -> Option<(u64, u64)> {
            let retirement = self.production_page_flips.take_retirement(output)?;
            Some((retirement.retirement.ust, retirement.retirement.msc))
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
                    Some(LiveProductionScanoutContent::Mixed {
                        transaction: presented,
                        nonzero_rgb_pixels,
                    }) if presented == transaction => Some(nonzero_rgb_pixels),
                    _ => None,
                })
                .unwrap_or(0)
        }

        pub fn export_attempts(&self) -> usize {
            self.heads
                .iter()
                .map(|head| head.exporter.cpu_frame_export_attempts())
                .chain(
                    self.heads
                        .iter()
                        .map(|head| head.exporter.mixed_frame_export_attempts()),
                )
                .sum()
        }

        pub fn mixed_exports(&self) -> usize {
            self.heads
                .iter()
                .map(|head| head.exporter.mixed_frame_exports())
                .sum()
        }

        pub fn persistent_render_metrics(&self) -> LivePersistentRenderMetrics {
            self.heads.iter().fold(
                LivePersistentRenderMetrics::default(),
                |mut metrics, head| {
                    let stats = head.exporter.persistent_render_stats();
                    metrics.target_creations = metrics
                        .target_creations
                        .saturating_add(stats.target_creations);
                    metrics.target_recreations = metrics
                        .target_recreations
                        .saturating_add(stats.target_recreations);
                    metrics.pipeline_creations = metrics
                        .pipeline_creations
                        .saturating_add(stats.gl_pipeline_creations);
                    metrics.cpu_target_creations = metrics
                        .cpu_target_creations
                        .saturating_add(stats.cpu_target_creations);
                    metrics.dmabuf_target_creations = metrics
                        .dmabuf_target_creations
                        .saturating_add(stats.dmabuf_target_creations);
                    metrics.composition_target_creations = metrics
                        .composition_target_creations
                        .saturating_add(stats.composition_target_creations);
                    metrics.epoch_replacements = metrics
                        .epoch_replacements
                        .saturating_add(stats.epoch_replacements);
                    metrics.recovery_replacements = metrics
                        .recovery_replacements
                        .saturating_add(stats.recovery_replacements);
                    metrics.uploads = metrics.uploads.saturating_add(stats.frame_uploads);
                    metrics.max_upload = metrics.max_upload.max(stats.max_upload);
                    metrics
                },
            )
        }

        pub fn poll_group_callbacks(
            &mut self,
            group: usize,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let callbacks = {
                let group = &mut self.groups[group];
                let _ = group
                    .session
                    .poll_native_page_flip_events(&group.sender, 64, 64);
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
                callbacks
            };
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
    LivePersistentRenderMetrics, LiveProductionCpuFrameQueueStatus, LiveProductionNativeHead,
    LiveProductionNativeScanout, LiveProductionScanoutContent,
    live_production_scanout_is_stable_present, reduce_live_production_cpu_frame_queue,
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
