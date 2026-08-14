#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
mod persistent_native_scanout {
    use crate::*;
    use sophia_engine::{CompositorBackendTickInput, OutputFramePresentationState};
    use sophia_protocol::{OutputId, TransactionId};
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
    use std::time::{Duration, Instant};

    mod cursor;
    mod frame_damage;
    mod output_capabilities;
    mod renderer_images;
    mod state;
    use frame_damage::trace_presented_output_damage;
    pub use renderer_images::LiveProductionRendererImageHandoff;
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
        /// How a group's frame is placed on a head whose mode differs from the
        /// scene. One policy for the session, from configuration.
        mirror_fit: crate::NativeMirrorFit,
        /// One scanout buffer exporter per head, parallel to `heads`.
        ///
        /// Per head because each connector scans out its own buffer at its own
        /// mode. A group's heads show one *scene*, not one buffer: sharing a buffer
        /// would force every head onto a single mode, which is the design this
        /// replaced -- it could not mirror displays of different resolutions
        /// without degrading the better one.
        ///
        /// `LiveProductionNativeGroup` is a *card session*, not a mirror group. The
        /// exporter belongs to neither: it belongs to a head.
        exporters: Vec<
            crate::NativeGbmRenderedScanoutBufferDiscoveryExporter<
                crate::RealAtomicScanoutRenderDeviceDiscovery,
            >,
        >,
        /// One callback channel per logical output, taken once when its runtime is
        /// built. Per output rather than per head because a mirror group's heads
        /// feed one runtime: two queues would make the group's flips arrive as two
        /// unrelated streams that nothing joins back up.
        output_callbacks: BTreeMap<OutputId, Receiver<crate::LivePageFlipCallback>>,
        next_frame_id: u64,
        pub production_page_flips: crate::LiveProductionPageFlipTracker,
        pub presentation_started: Instant,
        pub kernel_page_flip_timestamps: usize,
        pub kernel_page_flip_timestamp_missing: usize,
        kernel_page_flip_ust: BTreeMap<(OutputId, u64), u64>,
        pub vsync_overlap_rejections: usize,
        pub page_flip_phase_rejections: usize,
        pub cursor_updates: usize,
        pub cursor_hidden_updates: usize,
        pub cursor_initialization_deferrals: usize,
        pub cursor_updates_primary_in_flight: usize,
        pub cursor_update_failures: usize,
        pub max_cursor_initialization: Duration,
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
        /// Feeds this head's logical output. Every head of a mirror group holds a
        /// clone of the same sender.
        pub sender: SyncSender<crate::LivePageFlipCallback>,
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
        pub presented_page_flip_ust_usec: u64,
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
        pub snapshot_captures: usize,
        pub snapshot_promotions: usize,
        pub snapshot_rollbacks: usize,
        pub snapshot_evictions: usize,
        pub snapshot_live_entries: usize,
        pub snapshot_live_bytes: u64,
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
            Self::new_with_selection(
                crate::select_real_atomic_scanout_cards(),
                &crate::NativeMirrorGrouping::none(),
            )
        }

        #[cfg(feature = "seat-control")]
        pub fn new_with_seat(
            opener: &crate::LiveSeatDeviceOpener,
        ) -> Result<Self, Box<dyn std::error::Error>> {
            Self::new_with_seat_and_mirroring(opener, &crate::NativeMirrorGrouping::none())
        }

        /// Builds the scanout with connectors grouped into logical outputs.
        ///
        /// The grouping comes from configuration and is the only thing that makes
        /// mirroring happen: without it every connector is its own logical output,
        /// which is the ordinary desktop and was the only shape reachable before.
        #[cfg(feature = "seat-control")]
        pub fn new_with_seat_and_mirroring(
            opener: &crate::LiveSeatDeviceOpener,
            grouping: &crate::NativeMirrorGrouping,
        ) -> Result<Self, Box<dyn std::error::Error>> {
            Self::new_with_selection(
                crate::select_real_atomic_scanout_cards_with_seat(opener),
                grouping,
            )
        }

        fn new_with_selection(
            selection: crate::RealAtomicScanoutSelectionSet,
            grouping: &crate::NativeMirrorGrouping,
        ) -> Result<Self, Box<dyn std::error::Error>> {
            let authority = crate::RealAtomicScanoutSmokeConfig::default_primary_output()
                .ok_or("persistent native scanout config is invalid")?
                .authority;
            let mut sessions =
                selection.into_page_flip_sessions_with_mirroring(authority, grouping);
            if sessions.status != crate::RealAtomicScanoutPageFlipSessionSetStatus::Ready {
                return Err(format!(
                    "persistent native scanout could not open all KMS outputs: {:?}",
                    sessions.status
                )
                .into());
            }
            let outputs = sophia_engine::discover_drm_kms_outputs_from_sysfs("/sys/class/drm")?;
            // Ownership is complete when every discovered connector has a head, not
            // when the logical-output count matches. A mirror group is several heads
            // behind one logical output, so comparing logical outputs to connectors
            // would call a correctly mirrored desktop partial.
            let head_count: usize = sessions
                .sessions
                .iter()
                .map(|session| session.selections().len())
                .sum();
            if head_count != outputs.len() {
                return Err(format!(
                    "persistent native ownership is partial: discovered={} heads={}",
                    outputs.len(),
                    head_count
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
            let mut output_senders: BTreeMap<OutputId, SyncSender<crate::LivePageFlipCallback>> =
                BTreeMap::new();
            let mut output_callbacks = BTreeMap::new();
            let mut exporters = Vec::new();
            for session in sessions.sessions.drain(..) {
                let group = groups.len();
                for (selection, output_id) in session
                    .selections()
                    .iter()
                    .copied()
                    .zip(session.outputs().iter().copied())
                {
                    let size = selection.size();
                    // This head's own exporter, against this head's own plane
                    // formats. The group-wide modifier intersection went with the
                    // shared buffer that needed it: a head scanning out its own
                    // buffer is constrained only by its own plane.
                    let discovery = session.render_device_discovery()?;
                    exporters.push(
                        crate::NativeGbmRenderedScanoutBufferDiscoveryExporter::new(discovery)
                            .with_preferred_modifiers(
                                session
                                    .preferred_xrgb8888_scanout_modifiers_for_selection(selection),
                            ),
                    );
                    let sender = output_senders
                        .entry(output_id)
                        .or_insert_with(|| {
                            let (sender, receiver) = sync_channel(64);
                            output_callbacks.insert(output_id, receiver);
                            sender
                        })
                        .clone();
                    heads.push(LiveProductionNativeHead {
                        group,
                        selection,
                        sender,
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
                        presented_page_flip_ust_usec: 0,
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
                mirror_fit: crate::NativeMirrorFit::default(),
                exporters,
                output_callbacks,
                next_frame_id: 1,
                production_page_flips,
                presentation_started: Instant::now(),
                kernel_page_flip_timestamps: 0,
                kernel_page_flip_timestamp_missing: 0,
                kernel_page_flip_ust: BTreeMap::new(),
                vsync_overlap_rejections: 0,
                page_flip_phase_rejections: 0,
                cursor_updates: 0,
                cursor_hidden_updates: 0,
                cursor_initialization_deferrals: 0,
                cursor_updates_primary_in_flight: 0,
                cursor_update_failures: 0,
                max_cursor_initialization: Duration::ZERO,
                max_cursor_update: Duration::ZERO,
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

        /// The desktop's logical outputs, one per `OutputId`.
        ///
        /// Heads are per connector and a mirror group has several sharing one
        /// logical output, so returning one entry per head would present a group as
        /// two outputs side by side. Everything above this is a topology, and a
        /// topology counts screens rather than cables.
        pub fn outputs(&self) -> Vec<sophia_engine::HeadlessOutput> {
            let mut seen = BTreeSet::new();
            self.heads
                .iter()
                .filter(|head| seen.insert(head.output.id))
                .map(|head| head.output)
                .collect()
        }

        /// The first head driving a logical output.
        ///
        /// Named for what it returns. It was `output_index`, which read like a
        /// position in the output list and was passed one by four callers -- a
        /// coincidence that holds only while every output has exactly one head.
        ///
        /// Correct only where any head of a group will do: reading a card, or a
        /// selection's connector and CRTC. A caller that submits, retires, or
        /// releases per head must use `head_indices` instead, or it will address
        /// head zero and silently ignore the rest of a mirror group.
        pub fn primary_head_index(&self, output: OutputId) -> Option<usize> {
            self.heads.iter().position(|head| head.output.id == output)
        }

        /// The head driving a named connector.
        ///
        /// The one lookup that is exact for a mirror group: every head has its own
        /// connector even when several share a logical output, so a caller that must
        /// address one specific head asks by connector rather than by output.
        pub fn head_index_for_connector_id(&self, connector_id: u32) -> Option<usize> {
            self.heads
                .iter()
                .position(|head| head.selection.connector_id() == connector_id)
        }

        /// Every head driving a logical output, in head order.
        pub fn head_indices(&self, output: OutputId) -> Vec<usize> {
            self.heads
                .iter()
                .enumerate()
                .filter(|(_, head)| head.output.id == output)
                .map(|(index, _)| index)
                .collect()
        }

        /// How many connectors drive each logical output, in output order.
        ///
        /// The topology owner compares this beside the output list: losing one head
        /// of a mirror group leaves the logical outputs unchanged, so a comparison
        /// on outputs alone would call that no change at all.
        pub fn head_fingerprint(&self) -> Vec<(OutputId, usize)> {
            let mut counts: BTreeMap<OutputId, usize> = BTreeMap::new();
            for head in &self.heads {
                *counts.entry(head.output.id).or_default() += 1;
            }
            counts.into_iter().collect()
        }

        fn allocate_frame_id(&mut self) -> LiveProductionNativeFrameId {
            let frame = LiveProductionNativeFrameId::from_raw(self.next_frame_id);
            self.next_frame_id = self
                .next_frame_id
                .checked_add(1)
                .expect("native frame ID space exhausted");
            frame
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

        /// One head and its output's exporter together.
        ///
        /// They live in different tables now, and most work touches both. Handing
        /// out the pair from one place keeps every caller from having to spell out
        /// the disjoint borrow itself.
        fn head_and_exporter(
            &mut self,
            index: usize,
            output: OutputId,
        ) -> (
            &mut LiveProductionNativeHead,
            &mut crate::NativeGbmRenderedScanoutBufferDiscoveryExporter<
                crate::RealAtomicScanoutRenderDeviceDiscovery,
            >,
        ) {
            let _ = output;
            (
                &mut self.heads[index],
                self.exporters
                    .get_mut(index)
                    .expect("a registered head has an exporter"),
            )
        }

        /// The exporter backing an output's first head, for reads.
        ///
        /// Addressing an output rather than a head is correct only where any head
        /// of a group will do. A caller that composes or exports per head must
        /// resolve the head first, or a group's other connectors get nothing.
        fn exporter(
            &self,
            output: OutputId,
        ) -> Option<
            &crate::NativeGbmRenderedScanoutBufferDiscoveryExporter<
                crate::RealAtomicScanoutRenderDeviceDiscovery,
            >,
        > {
            self.exporters.get(self.primary_head_index(output)?)
        }

        /// The exporter backing an output's first head.
        fn exporter_mut(
            &mut self,
            output: OutputId,
        ) -> Result<
            &mut crate::NativeGbmRenderedScanoutBufferDiscoveryExporter<
                crate::RealAtomicScanoutRenderDeviceDiscovery,
            >,
            Box<dyn std::error::Error>,
        > {
            let index = self.primary_head(output)?;
            self.exporters
                .get_mut(index)
                .ok_or_else(|| format!("native output {} has no exporter", output.raw()).into())
        }

        /// The head this logical output is addressed through.
        ///
        /// Every per-head entry point below resolves through this rather than
        /// taking a position, because the caller's position is an index into
        /// *outputs* and the two stop agreeing the moment a mirror group exists.
        fn primary_head(&self, output: OutputId) -> Result<usize, Box<dyn std::error::Error>> {
            self.primary_head_index(output)
                .ok_or_else(|| format!("native output {} has no head", output.raw()).into())
        }

        /// The card driving a logical output.
        pub fn card_for_output(&self, output: OutputId) -> Option<&crate::RealAtomicScanoutCard> {
            self.primary_head_index(output)
                .map(|index| self.card(index))
        }

        /// The primary connector selection of a logical output.
        pub fn selection_for_output(
            &self,
            output: OutputId,
        ) -> Option<crate::LibdrmNativePrimaryPlaneSelection> {
            self.primary_head_index(output)
                .map(|index| self.heads[index].selection)
        }

        pub fn take_output_receiver(
            &mut self,
            output: OutputId,
        ) -> Receiver<crate::LivePageFlipCallback> {
            self.output_callbacks
                .remove(&output)
                .expect("native page-flip receiver must attach once per logical output")
        }

        pub fn run_tick(
            &mut self,
            output: OutputId,
            runtime: &mut crate::LiveBackendRuntimeAssembly,
            input: CompositorBackendTickInput,
        ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
            let index = self.primary_head(output)?;
            self.ensure_page_flip_progress()?;
            if !self.exporter_mut(output)?.pending_frame() {
                self.retire_ready_and_retry_cleanup(output, runtime)?;
                return Ok(runtime.run_tick(input)?);
            }
            let group = self.heads[index].group;
            self.poll_group_callbacks(group)?;
            let (report, exported_nonzero, worker_was_in_flight) = {
                let groups = &mut self.groups;
                let head = &mut self.heads[index];
                let exporter = self
                    .exporters
                    .get_mut(index)
                    .ok_or_else(|| format!("native output {} has no exporter", output.raw()))?;
                let worker_was_in_flight = exporter.worker_in_flight();
                let export_attempts_before = exporter.cpu_frame_export_attempts();
                let report = runtime
                    .run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_with(
                        input,
                        groups[group].session.card(),
                        exporter,
                    )?;
                let exported_nonzero = exporter.cpu_frame_export_attempts()
                    > export_attempts_before
                    && head.pending_nonzero_pixel_bytes > 0;
                if !exporter.pending_cpu_frame() {
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
                let worker_is_in_flight = self.exporter(output).is_some_and(
                    crate::NativeGbmRenderedScanoutBufferDiscoveryExporter::worker_in_flight,
                );
                match submit.status {
                    Status::SubmittedWaitingForPageFlip => {
                        let content = if worker_was_in_flight {
                            self.heads[index].rendering_content.take()
                        } else {
                            self.heads[index].pending_content.take()
                        };
                        let content = content.map(|content| {
                            content.with_nonzero_rgb_pixels(
                                self.exporter(output).map_or(0, |exporter| {
                                    exporter.composition_nonzero_rgb_pixels()
                                }),
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
                                    ..
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
                        let frame = content.map_or(0, |content| content.frame().raw());
                        tracing::info!(
                            "sophia_live_native_page_flip schema=1 status=submitted output={} submission={} content={:?} frame={}",
                            output.raw(),
                            cycle,
                            content,
                            frame,
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
            output: OutputId,
            runtime: &mut crate::LiveBackendRuntimeAssembly,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let index = self.primary_head(output)?;
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
            output: OutputId,
            runtime: &mut crate::LiveBackendRuntimeAssembly,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let index = self.primary_head(output)?;
            self.retire_ready(output, runtime)?;
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
            output: OutputId,
            runtime: &mut crate::LiveBackendRuntimeAssembly,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let index = self.primary_head(output)?;
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
                    let frame = self.heads[index]
                        .submitted_content
                        .or(self.heads[index].presented_content)
                        .map_or(0, |content| content.frame().raw());
                    tracing::info!(
                        "sophia_live_native_page_flip schema=1 status=retired output={} submission={} frame={}",
                        self.heads[index].output.id.raw(),
                        self.heads[index]
                            .submitted_sequence
                            .unwrap_or(self.heads[index].submissions),
                        frame,
                    );
                    self.retirements = self.retirements.saturating_add(1);
                    self.heads[index].retirements = self.heads[index].retirements.saturating_add(1);
                }
                Status::HeadLost => {
                    trace_live_native_lifecycle("kms_buffer_released_after_head_loss");
                    tracing::warn!(
                        "sophia_live_native_page_flip schema=1 status=head_lost output={}",
                        self.heads[index].output.id.raw(),
                    );
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
                    self.heads[index].presented_page_flip_ust_usec = ust;
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
            output: OutputId,
            runtime: &mut crate::LiveBackendRuntimeAssembly,
            frame: LiveProductionComposedFrame,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let index = self.primary_head(output)?;
            self.queue_frame(output, frame);
            let group = self.heads[index].group;
            let groups = &mut self.groups;
            let head = &mut self.heads[index];
            let exporter = self
                .exporters
                .get_mut(index)
                .ok_or_else(|| format!("native output {} has no exporter", output.raw()))?;
            let export_attempts_before = exporter.cpu_frame_export_attempts();
            groups[group]
                .session
                .initialize_persistent_native_gbm_scanout_for_selection(
                    runtime,
                    exporter,
                    head.selection,
                )
                .map_err(|evidence| {
                    format!("persistent native initial modeset failed: {evidence:?}")
                })?;
            exporter.enable_worker()?;
            if exporter.cpu_frame_export_attempts() > export_attempts_before
                && head.pending_nonzero_pixel_bytes > 0
            {
                self.nonzero_exports = self.nonzero_exports.saturating_add(1);
                head.nonzero_exports = head.nonzero_exports.saturating_add(1);
            }
            if !exporter.pending_cpu_frame() {
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
            output: OutputId,
            frame: LiveProductionComposedFrame,
        ) -> LiveProductionCpuFrameQueueStatus {
            let Some(index) = self.primary_head_index(output) else {
                return LiveProductionCpuFrameQueueStatus::NoHead;
            };
            // A group's heads need the frame placed for each of their modes, which
            // the pure-CPU path below cannot express -- it uploads at the frame's
            // own size. An output with one head keeps that path exactly, so no
            // ordinary desktop changes.
            if self.head_indices(output).len() > 1 {
                let projected = self.queue_projected_frame(output, &frame.frame, self.mirror_fit);
                return if projected > 0 {
                    LiveProductionCpuFrameQueueStatus::Queued
                } else {
                    LiveProductionCpuFrameQueueStatus::NoHead
                };
            }
            let status = {
                let head = &self.heads[index];
                reduce_live_production_cpu_frame_queue(
                    head.pending_content,
                    head.submitted_content,
                    head.presented_content,
                    self.exporter(output).is_some_and(
                        crate::NativeGbmRenderedScanoutBufferDiscoveryExporter::worker_in_flight,
                    ),
                    head.callback_accepted != 0 || head.initial_modeset_submission.is_some(),
                    frame.checksum,
                )
            };
            if !matches!(
                status,
                LiveProductionCpuFrameQueueStatus::Queued
                    | LiveProductionCpuFrameQueueStatus::BaselineRequired
            ) {
                return status;
            }
            let frame_id = self.allocate_frame_id();
            let (head, exporter) = self.head_and_exporter(index, output);
            head.pending_nonzero_pixel_bytes = frame.nonzero_pixel_bytes;
            head.last_checksum = frame.checksum;
            head.queue_output_damage_snapshot(frame.output_damage_snapshot.clone());
            head.pending_content = Some(LiveProductionScanoutContent::Cpu {
                frame: frame_id,
                checksum: frame.checksum,
            });
            exporter.set_pending_cpu_frame_with_damage(
                frame.frame,
                frame.checksum,
                frame.output_damage_snapshot,
            );
            status
        }

        pub fn take_presentation_feedback(
            &mut self,
            output: OutputId,
        ) -> Option<LiveProductionNativeFrameRetirement> {
            let retirement = self.production_page_flips.take_retirement(output)?;
            let index = self.primary_head_index(output)?;
            let content = self.heads[index].presented_content?;
            Some(LiveProductionNativeFrameRetirement {
                output,
                frame: content.frame(),
                submission: retirement.cycle,
                content,
                ust: retirement.retirement.ust,
                msc: retirement.retirement.msc,
            })
        }

        pub fn pending_kernel_page_flip_timestamps(&self) -> usize {
            self.kernel_page_flip_ust.len()
        }

        pub fn discard_presentation_feedback(&mut self, output: Option<OutputId>) {
            self.production_page_flips.discard_retirements(output);
        }

        pub fn pending_frame(&self, output: OutputId) -> bool {
            self.primary_head_index(output)
                .and_then(|_| self.exporter(output))
                .is_some_and(crate::NativeGbmRenderedScanoutBufferDiscoveryExporter::pending_frame)
        }

        pub fn submitted_content(&self, output: OutputId) -> Option<LiveProductionScanoutContent> {
            self.heads[self.primary_head_index(output)?].submitted_content
        }

        /// Returns the immutable scene snapshot retired by the latest accepted
        /// page flip for this output. Pending, rendering, and submitted work is
        /// intentionally invisible here.
        pub fn presented_output_frame(
            &self,
            output: OutputId,
        ) -> Option<&sophia_engine::OutputFrameDamageSnapshot> {
            self.heads[self.primary_head_index(output)?]
                .output_frames
                .presented()
        }

        pub fn stable_present(&self, output: OutputId, transaction: TransactionId) -> bool {
            let Some(index) = self.primary_head_index(output) else {
                return false;
            };
            let head = &self.heads[index];
            live_production_scanout_is_stable_present(
                head.presented_content,
                head.submitted_content,
                self.exporter(output).is_some_and(
                    crate::NativeGbmRenderedScanoutBufferDiscoveryExporter::pending_frame,
                ),
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
                        ..
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
                // By connector, not by output. Two heads of a mirror group share an
                // output, so matching on it delivered both flips to whichever head
                // came first and left the sibling looking like it never flipped.
                let Some(head) = self
                    .heads
                    .iter()
                    .find(|head| head.selection.connector_id() == callback.connector_id)
                else {
                    return Err("native callback referenced an unknown connector".into());
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
    LiveProductionCpuFrameQueueStatus, LiveProductionNativeFrameId,
    LiveProductionNativeFrameRetirement, LiveProductionNativeHead, LiveProductionNativeScanout,
    LiveProductionPageFlipWatchdogStatus, LiveProductionRendererImageHandoff,
    LiveProductionScanoutContent, live_production_scanout_is_stable_present,
    reduce_live_production_cpu_frame_queue, reduce_live_production_page_flip_watchdog,
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
