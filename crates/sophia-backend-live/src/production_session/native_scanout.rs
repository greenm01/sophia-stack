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
        /// scene, from configuration.
        pub mirror_fit: crate::NativeMirrorFit,
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
        /// Physical-head joins for logical mirror generations.
        output_lifecycles: BTreeMap<OutputId, LiveProductionMirrorGroupLifecycle>,
        next_frame_id: u64,
        pub production_page_flips: crate::LiveProductionPageFlipTracker,
        pub presentation_started: Instant,
        pub kernel_page_flip_timestamps: usize,
        pub kernel_page_flip_timestamp_missing: usize,
        kernel_page_flip_ust: BTreeMap<(OutputId, u32, u64), u64>,
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
        /// This physical head's synchronously displayed baseline. Single-head
        /// outputs keep that owner in the logical runtime; mirror groups transfer
        /// every connector's owner here after initialization.
        pub(crate) displayed_scanout: Option<crate::BoxedRenderedPrimaryPlaneScanoutSubmission>,
        pub(crate) scanout_submission: Option<crate::BoxedRenderedPrimaryPlaneScanoutSubmission>,
        pub(crate) scanout_cleanup: Option<crate::BoxedRenderedPrimaryPlaneScanoutCleanup>,
        pub(crate) scanout_in_flight_ticks: u64,
        pub(crate) last_callback_serial: Option<u64>,
        pub(crate) submitted_group_frame: Option<LiveProductionNativeFrameId>,
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
            Self::new_with_mirroring(&crate::NativeMirrorGrouping::none())
        }

        /// Builds without a seat, with connectors grouped into logical outputs.
        ///
        /// The standalone topology commands need this: they read the operator's
        /// profile to reconcile against, so building the card set without the
        /// grouping that profile asks for would validate a different desktop than
        /// the one configured -- two outputs where the operator asked for one.
        pub fn new_with_mirroring(
            grouping: &crate::NativeMirrorGrouping,
        ) -> Result<Self, Box<dyn std::error::Error>> {
            Self::new_with_selection(crate::select_real_atomic_scanout_cards(), grouping)
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
                        displayed_scanout: None,
                        scanout_submission: None,
                        scanout_cleanup: None,
                        scanout_in_flight_ticks: 0,
                        last_callback_serial: None,
                        submitted_group_frame: None,
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
            // A head and its exporter are one physical scanout slot. Keep them
            // together while ordering logical outputs; sorting only `heads`
            // silently retargets exporters whenever discovery order differs from
            // logical-output order.
            let mut head_exporters = heads.into_iter().zip(exporters).collect::<Vec<_>>();
            head_exporters.sort_by_key(|(head, _)| (head.output.id, head.selection.connector_id()));
            let mut sorted_heads = Vec::with_capacity(head_exporters.len());
            let mut sorted_exporters = Vec::with_capacity(head_exporters.len());
            for (head, exporter) in head_exporters {
                sorted_heads.push(head);
                sorted_exporters.push(exporter);
            }
            let heads = sorted_heads;
            let exporters = sorted_exporters;
            let mut output_lifecycles = BTreeMap::new();
            for output in heads
                .iter()
                .map(|head| head.output.id)
                .collect::<BTreeSet<_>>()
            {
                let connectors = heads
                    .iter()
                    .filter(|head| head.output.id == output)
                    .map(|head| head.selection.connector_id());
                let lifecycle = LiveProductionMirrorGroupLifecycle::new(output, connectors)
                    .expect("a native logical output has at least one physical head");
                output_lifecycles.insert(output, lifecycle);
            }
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
                output_lifecycles,
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
        pub fn head_index_for_output_connector_id(
            &self,
            output: OutputId,
            connector_id: u32,
        ) -> Option<usize> {
            self.heads.iter().position(|head| {
                head.output.id == output && head.selection.connector_id() == connector_id
            })
        }

        /// Resolves a connector id when the caller has already established that
        /// the capability namespace is unambiguous.
        ///
        /// DRM connector ids are card-local, so callback and presentation paths
        /// must use the output-qualified lookup above. Startup topology mapping
        /// retains this facade because its named capability set is validated
        /// before it reaches this point.
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
            if self.head_indices(output).len() > 1 {
                let report = self.run_mirror_group_tick(output, runtime, input)?;
                if self.mirror_poison_drained(output) {
                    return Err("mirror generation failed after physical ownership drained".into());
                }
                return Ok(report);
            }
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
            self.observe_callbacks(index, report.page_flip_callbacks.clone());
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
                        tracing::info!(
                            "sophia_live_native_head_page_flip schema=1 status=submitted output={} connector_id={} submission={} content={:?} frame={}",
                            output.raw(),
                            self.heads[index].selection.connector_id(),
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

        fn run_mirror_group_tick(
            &mut self,
            output: OutputId,
            runtime: &mut crate::LiveBackendRuntimeAssembly,
            input: CompositorBackendTickInput,
        ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
            let indices = self.head_indices(output);
            self.ensure_page_flip_progress()?;
            if indices.is_empty() {
                return Err("mirror group has no head".into());
            }
            let groups = indices
                .iter()
                .map(|index| self.heads[*index].group)
                .collect::<BTreeSet<_>>();
            for group in groups {
                self.poll_group_callbacks(group)?;
            }

            // Admit physical callbacks before the logical tick, but leave the
            // logical page-flip event untouched until the group join below.
            // `runtime.run_tick` then sees an empty queue and cannot expose a
            // partially flipped mirror group to the Engine.
            let page_flip_callbacks = runtime.drain_mirror_page_flip_callback_queue();
            // Advance the logical Engine exactly once. Physical render/export/KMS
            // owners are serviced below without re-running authority or scene work.
            let mut tick = runtime.run_tick(input)?;
            tick.page_flip_callbacks = page_flip_callbacks;
            let callbacks = tick.page_flip_callbacks.accepted_callbacks.clone();
            let mut completed_retire = None;
            let mut completed_serial = None;
            for callback in callbacks {
                let Some(head_index) =
                    self.head_index_for_output_connector_id(output, callback.connector_id)
                else {
                    self.callback_rejected = self.callback_rejected.saturating_add(1);
                    continue;
                };
                let Some(submission) = self.heads[head_index].scanout_submission.take() else {
                    continue;
                };
                if self.heads[head_index]
                    .last_callback_serial
                    .is_some_and(|serial| callback.frame_serial <= serial)
                {
                    self.heads[head_index].scanout_submission = Some(submission);
                    self.callback_rejected = self.callback_rejected.saturating_add(1);
                    continue;
                }
                self.heads[head_index].last_callback_serial = Some(callback.frame_serial);
                let callback_ust = if let Some(ust) = self.kernel_page_flip_ust.remove(&(
                    output,
                    callback.connector_id,
                    callback.frame_serial,
                )) {
                    self.kernel_page_flip_timestamps =
                        self.kernel_page_flip_timestamps.saturating_add(1);
                    ust
                } else {
                    self.kernel_page_flip_timestamp_missing =
                        self.kernel_page_flip_timestamp_missing.saturating_add(1);
                    Self::monotonic_ust_usec()
                };
                let submitted_ust_usec = self.heads[head_index].submitted_ust_usec.take();
                let submit_to_page_flip = submitted_ust_usec
                    .and_then(|submitted| callback_ust.checked_sub(submitted))
                    .map(Duration::from_micros)
                    .or_else(|| {
                        self.heads[head_index]
                            .submitted_at
                            .map(|submitted| submitted.elapsed())
                    })
                    .unwrap_or_default();
                self.max_submit_to_page_flip =
                    self.max_submit_to_page_flip.max(submit_to_page_flip);
                self.heads[head_index].presented_submission_ust_usec =
                    submitted_ust_usec.unwrap_or_default();
                self.heads[head_index].presented_page_flip_ust_usec = callback_ust;
                self.heads[head_index].presented_submit_to_page_flip = submit_to_page_flip;
                if let Some(previous) = self.heads[head_index].displayed_scanout.replace(submission)
                {
                    let crate::LiveRenderedPrimaryPlaneScanoutSubmission {
                        scanout_buffer,
                        primary_plane,
                        ..
                    } = previous;
                    let retired = primary_plane.retire(self.card(head_index));
                    if let Some(primary_plane) = retired.cleanup {
                        self.heads[head_index].scanout_cleanup =
                            Some(crate::LiveRenderedPrimaryPlaneScanoutCleanup {
                                scanout_buffer,
                                primary_plane,
                            });
                    }
                }
                let frame = self.heads[head_index]
                    .submitted_group_frame
                    .take()
                    .ok_or("mirror head callback has no logical generation")?;
                self.heads[head_index].submitted_at = None;
                self.heads[head_index].scanout_in_flight_ticks = 0;
                self.heads[head_index].retirements =
                    self.heads[head_index].retirements.saturating_add(1);
                self.heads[head_index].callback_accepted =
                    self.heads[head_index].callback_accepted.saturating_add(1);
                self.retirements = self.retirements.saturating_add(1);
                self.callback_accepted = self.callback_accepted.saturating_add(1);
                self.heads[head_index].presented_content =
                    self.heads[head_index].submitted_content.take();
                self.heads[head_index].presented_checksum = self.heads[head_index]
                    .submitted_checksum
                    .take()
                    .unwrap_or_default();
                if let Some(submission) = self.heads[head_index].submitted_sequence.take() {
                    self.heads[head_index].presented_submissions = submission;
                }
                tracing::info!(
                    "sophia_live_native_head_page_flip schema=1 status=callback_accepted output={} connector_id={} callbacks=1 kernel_sequence={} frame={}",
                    output.raw(),
                    callback.connector_id,
                    callback.frame_serial,
                    frame.raw(),
                );
                tracing::info!(
                    "sophia_live_native_head_page_flip schema=1 status=retired output={} connector_id={} submission={} frame={}",
                    output.raw(),
                    callback.connector_id,
                    self.heads[head_index].presented_submissions,
                    frame.raw(),
                );
                if self
                    .output_lifecycles
                    .get(&output)
                    .is_some_and(LiveProductionMirrorGroupLifecycle::failed)
                {
                    continue;
                }
                let lifecycle = self
                    .output_lifecycles
                    .get_mut(&output)
                    .expect("mirror output has a lifecycle");
                if !lifecycle.observe_flip_timing(frame, callback.frame_serial, callback_ust) {
                    return Err("mirror callback timing named the wrong generation".into());
                }
                let transition = lifecycle.mark_flipped(callback.connector_id, frame);
                match transition {
                    LiveProductionMirrorHeadTransition::GroupReady => {
                        let (logical_serial, logical_ust) = self
                            .output_lifecycles
                            .get(&output)
                            .and_then(LiveProductionMirrorGroupLifecycle::flip_timing)
                            .ok_or("completed mirror generation has no timing evidence")?;
                        if let Err(error) = self.production_page_flips.observe_page_flip(
                            output,
                            logical_serial,
                            logical_ust / 1_000,
                            logical_ust,
                        ) {
                            self.page_flip_phase_rejections =
                                self.page_flip_phase_rejections.saturating_add(1);
                            return Err(format!(
                                "mirror logical page-flip retirement was rejected: {error:?}"
                            )
                            .into());
                        }
                        // Physical heads may flip in either order, but the
                        // logical scene becomes presented only after the join.
                        for joined_index in self.head_indices(output) {
                            if self.heads[joined_index].output_frames.submitted().is_some() {
                                let presented = self.heads[joined_index]
                                    .output_frames
                                    .mark_presented()
                                    .map_err(|error| {
                                        format!(
                                            "mirror display-list presentation transition failed: {error}"
                                        )
                                    })?;
                                trace_presented_output_damage(
                                    "presented",
                                    self.heads[joined_index].output.id,
                                    &presented,
                                );
                            }
                        }
                        completed_retire = Some(crate::LiveTrackedRenderedPrimaryPlaneScanoutRetireReport {
                            status: crate::LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip,
                            destroy: None,
                            runtime_scanout_state: Some(crate::RuntimeScanoutState::Retired),
                            in_flight: false,
                            in_flight_ticks: 0,
                            cleanup_pending: false,
                        });
                        completed_serial = Some(logical_serial);
                    }
                    LiveProductionMirrorHeadTransition::Accepted => {}
                    invalid => {
                        return Err(format!(
                            "mirror-head {} entered invalid flipped transition {invalid:?}",
                            callback.connector_id,
                        )
                        .into());
                    }
                }
            }
            self.callback_rejected = self.callback_rejected.saturating_add(
                tick.page_flip_callbacks.rejected_unexpected_output
                    + tick.page_flip_callbacks.rejected_stale_frame_serial,
            );
            self.callback_queue_saturated = self
                .callback_queue_saturated
                .saturating_add(usize::from(tick.page_flip_callbacks.max_reached));

            for head_index in indices.iter().copied() {
                if let Some(cleanup) = self.heads[head_index].scanout_cleanup.take() {
                    let retried = crate::retry_rendered_primary_plane_scanout_cleanup(
                        self.card(head_index),
                        cleanup,
                    );
                    self.heads[head_index].scanout_cleanup = retried.cleanup;
                    if self.heads[head_index].scanout_cleanup.is_some() {
                        self.retire_failures = self.retire_failures.saturating_add(1);
                        continue;
                    }
                }
                if self
                    .output_lifecycles
                    .get(&output)
                    .is_some_and(LiveProductionMirrorGroupLifecycle::failed)
                {
                    // Poison forbids new commits, not ownership cleanup. Visit
                    // every head so a failed later commit cannot strand the
                    // earlier head's retired framebuffer.
                    continue;
                }
                if self
                    .output_lifecycles
                    .get(&output)
                    .is_some_and(LiveProductionMirrorGroupLifecycle::awaiting_flips)
                {
                    break;
                }
                if self.heads[head_index].scanout_submission.is_some() {
                    self.heads[head_index].scanout_in_flight_ticks = self.heads[head_index]
                        .scanout_in_flight_ticks
                        .saturating_add(1);
                    continue;
                }
                if !self.exporters[head_index].pending_frame() {
                    continue;
                }
                let logical_frame = self
                    .output_lifecycles
                    .get(&output)
                    .and_then(LiveProductionMirrorGroupLifecycle::active_frame)
                    .or_else(|| {
                        self.heads[head_index]
                            .pending_content
                            .or(self.heads[head_index].rendering_content)
                            .map(LiveProductionScanoutContent::frame)
                    })
                    .ok_or("mirror head has pending renderer work without frame identity")?;
                if self
                    .output_lifecycles
                    .get(&output)
                    .and_then(LiveProductionMirrorGroupLifecycle::active_frame)
                    .is_none()
                {
                    let _ = self
                        .output_lifecycles
                        .get_mut(&output)
                        .expect("mirror output has a lifecycle")
                        .begin(logical_frame);
                }
                let selection = self.heads[head_index].selection;
                let size = self.heads[head_index].output.size;
                let worker_was_in_flight = self.exporters[head_index].worker_in_flight();
                let mut runtime_state = None;
                let head_group = self.heads[head_index].group;
                let submit = {
                    let device = self.groups[head_group].session.card();
                    let head = &mut self.heads[head_index];
                    let exporter = &mut self.exporters[head_index];
                    crate::track_rendered_primary_plane_scanout_submit_from_target_and_selection_with(
                        crate::LiveKmsScanoutTargetStatus::Ready,
                        Some(size),
                        Some(crate::LiveGbmEglFrameTargetRecord::new(size)),
                        &mut head.scanout_submission,
                        &mut head.scanout_cleanup,
                        &mut runtime_state,
                        &mut head.scanout_in_flight_ticks,
                        head.last_callback_serial,
                        None,
                        crate::LibdrmNativePrimaryPlaneSelectionResult {
                            status: crate::LibdrmNativePrimaryPlaneSelectionStatus::Selected,
                            selection: Some(selection),
                        },
                        None,
                        device,
                        exporter,
                    )
                };
                self.heads[head_index].last_submit_report = Some(submit);
                use crate::LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus as Status;
                match submit.status {
                    Status::SubmittedWaitingForPageFlip => {
                        let content = if worker_was_in_flight {
                            self.heads[head_index].rendering_content.take()
                        } else {
                            self.heads[head_index].pending_content.take()
                        }
                        .map(|content| {
                            content.with_nonzero_rgb_pixels(
                                self.exporters[head_index].composition_nonzero_rgb_pixels(),
                            )
                        });
                        if worker_was_in_flight
                            && self.heads[head_index].output_frames.rendering().is_some()
                        {
                            self.heads[head_index]
                                .output_frames
                                .promote_rendering_to_submitted()
                                .map_err(|error| {
                                    format!("mirror display-list worker promotion failed: {error}")
                                })?;
                        } else if !worker_was_in_flight
                            && self.heads[head_index].output_frames.pending().is_some()
                        {
                            self.heads[head_index]
                                .output_frames
                                .mark_submitted()
                                .map_err(|error| {
                                    format!("mirror display-list submit failed: {error}")
                                })?;
                        }
                        self.heads[head_index].submitted_content = content;
                        self.heads[head_index].submitted_group_frame = Some(logical_frame);
                        self.heads[head_index].submissions =
                            self.heads[head_index].submissions.saturating_add(1);
                        self.heads[head_index].submitted_sequence =
                            Some(self.heads[head_index].submissions);
                        self.heads[head_index].submitted_checksum =
                            Some(self.heads[head_index].last_checksum);
                        self.heads[head_index].submitted_at = Some(Instant::now());
                        self.heads[head_index].submitted_ust_usec =
                            Some(Self::monotonic_ust_usec());
                        self.submissions = self.submissions.saturating_add(1);
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
                            self.heads[head_index].nonzero_exports =
                                self.heads[head_index].nonzero_exports.saturating_add(1);
                        }
                        tracing::info!(
                            "sophia_live_native_head_page_flip schema=1 status=submitted output={} connector_id={} submission={} content={:?} frame={}",
                            output.raw(),
                            selection.connector_id(),
                            self.heads[head_index].submissions,
                            content,
                            logical_frame.raw(),
                        );
                        let transition = self
                            .output_lifecycles
                            .get_mut(&output)
                            .expect("mirror output has a lifecycle")
                            .mark_submitted(selection.connector_id(), logical_frame);
                        match transition {
                            LiveProductionMirrorHeadTransition::GroupReady => {
                                let cycle = logical_frame.raw();
                                if let Err(error) = self.production_page_flips.submit(output, cycle)
                                {
                                    self.vsync_overlap_rejections =
                                        self.vsync_overlap_rejections.saturating_add(1);
                                    return Err(format!(
                                        "mirror logical page-flip submission was rejected: {error:?}"
                                    )
                                    .into());
                                }
                                tick.rendered_primary_plane_scanout_submit = Some(submit);
                            }
                            LiveProductionMirrorHeadTransition::Accepted => {}
                            invalid => {
                                return Err(format!(
                                    "mirror-head {} entered invalid submitted transition {invalid:?}",
                                    selection.connector_id(),
                                )
                                .into());
                            }
                        }
                    }
                    Status::ScanoutExportPending => {
                        if !worker_was_in_flight && self.exporters[head_index].worker_in_flight() {
                            self.heads[head_index].rendering_content =
                                self.heads[head_index].pending_content.take();
                            if self.heads[head_index].output_frames.pending().is_some() {
                                self.heads[head_index]
                                    .output_frames
                                    .mark_rendering()
                                    .map_err(|error| {
                                        format!(
                                            "mirror display-list render transition failed: {error}"
                                        )
                                    })?;
                            }
                        }
                        self.submit_deferred = self.submit_deferred.saturating_add(1);
                        // The logical Present owns this generation as soon as any
                        // physical exporter starts. Returning `None` leaves the
                        // Present queued and lets the next Ready pass replace the
                        // frame whose worker is still running.
                        if tick.rendered_primary_plane_scanout_submit.is_none() {
                            tick.rendered_primary_plane_scanout_submit = Some(submit);
                        }
                    }
                    Status::AlreadyInFlight | Status::CleanupPending => {
                        self.submit_deferred = self.submit_deferred.saturating_add(1);
                    }
                    _ => {
                        if worker_was_in_flight {
                            self.heads[head_index].output_frames.discard_rendering();
                            self.heads[head_index].rendering_content = None;
                        } else {
                            self.heads[head_index].output_frames.discard_pending();
                            self.heads[head_index].pending_content = None;
                        }
                        self.submit_failures = self.submit_failures.saturating_add(1);
                        tracing::error!(
                            "sophia_live_native_head_page_flip schema=1 status=submit_failed output={} connector_id={} submit_status={:?} action=terminate_session",
                            output.raw(),
                            selection.connector_id(),
                            submit.status,
                        );
                        let aborted = self
                            .output_lifecycles
                            .get_mut(&output)
                            .expect("mirror output has a lifecycle")
                            .abort(logical_frame);
                        if !aborted {
                            return Err(
                                "mirror submit failure could not poison its generation".into()
                            );
                        }
                        break;
                    }
                }
            }
            tick.rendered_primary_plane_scanout_retire = completed_retire;
            if completed_serial.is_some()
                || self
                    .output_lifecycles
                    .get(&output)
                    .and_then(LiveProductionMirrorGroupLifecycle::active_frame)
                    .is_some()
            {
                let logical_page_flip = crate::LivePageFlipEvent {
                    status: if completed_serial.is_some() {
                        crate::LivePageFlipEventStatus::Presented
                    } else {
                        crate::LivePageFlipEventStatus::WaitingForOutput
                    },
                    frame_serial: completed_serial,
                };
                runtime.set_page_flip_observation(logical_page_flip);
                tick.page_flip = logical_page_flip;
            }
            tick.rendered_primary_plane_scanout_cleanup_pending = indices
                .iter()
                .any(|index| self.heads[*index].scanout_cleanup.is_some());
            tick.rendered_primary_plane_scanout_in_flight_ticks = self
                .heads
                .iter()
                .enumerate()
                .filter(|(index, _)| indices.contains(index))
                .map(|(_, head)| head.scanout_in_flight_ticks)
                .max()
                .unwrap_or_default();
            Ok(tick)
        }

        pub fn retire_ready(
            &mut self,
            output: OutputId,
            runtime: &mut crate::LiveBackendRuntimeAssembly,
        ) -> Result<(), Box<dyn std::error::Error>> {
            if self.head_indices(output).len() > 1 {
                let _ = self.run_mirror_group_tick(
                    output,
                    runtime,
                    CompositorBackendTickInput::default(),
                )?;
                if self.mirror_poison_drained(output) {
                    return Err("mirror generation failed after physical ownership drained".into());
                }
                return Ok(());
            }
            let index = self.primary_head(output)?;
            self.ensure_page_flip_progress()?;
            let group = self.heads[index].group;
            self.poll_group_callbacks(group)?;
            let report = runtime.drain_rendered_primary_plane_page_flip_callbacks_with(
                self.groups[group].session.card(),
            );
            self.observe_callbacks(index, report.page_flip_callbacks.clone());
            if let Some(retire) = report.rendered_primary_plane_scanout_retire {
                self.observe_retire(index, retire);
            }
            Ok(())
        }

        pub(crate) fn retire_ready_for_drain(
            &mut self,
            output: OutputId,
            runtime: &mut crate::LiveBackendRuntimeAssembly,
        ) -> Result<(), Box<dyn std::error::Error>> {
            if self.head_indices(output).len() > 1 {
                let _ = self.run_mirror_group_tick(
                    output,
                    runtime,
                    CompositorBackendTickInput::default(),
                )?;
                return Ok(());
            }
            self.retire_ready(output, runtime)
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
            let mut runtime_cleanup_pending = retired.cleanup_pending;
            if retired.cleanup_pending {
                trace_live_native_lifecycle("displayed_scanout_cleanup_retry_started");
                let cleanup =
                    runtime.retry_tracked_rendered_primary_plane_scanout_cleanup(self.card(index));
                runtime_cleanup_pending = cleanup.cleanup_pending;
            }
            let mut mirror_cleanup_pending = false;
            for head_index in self.head_indices(output) {
                if let Some(displayed) = self.heads[head_index].displayed_scanout.take() {
                    let crate::LiveRenderedPrimaryPlaneScanoutSubmission {
                        scanout_buffer,
                        primary_plane,
                        ..
                    } = displayed;
                    let released = primary_plane.retire(self.card(head_index));
                    if let Some(primary_plane) = released.cleanup {
                        self.heads[head_index].scanout_cleanup =
                            Some(crate::LiveRenderedPrimaryPlaneScanoutCleanup {
                                scanout_buffer,
                                primary_plane,
                            });
                    }
                }
                if let Some(cleanup) = self.heads[head_index].scanout_cleanup.take() {
                    let retried = crate::retry_rendered_primary_plane_scanout_cleanup(
                        self.card(head_index),
                        cleanup,
                    );
                    self.heads[head_index].scanout_cleanup = retried.cleanup;
                }
                if self.heads[head_index].scanout_cleanup.is_some() {
                    mirror_cleanup_pending = true;
                }
            }
            if runtime_cleanup_pending || mirror_cleanup_pending {
                return Err(format!(
                    "persistent displayed scanout cleanup remained pending: runtime={} mirror_heads={}",
                    runtime_cleanup_pending, mirror_cleanup_pending,
                )
                .into());
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
                    tracing::info!(
                        "sophia_live_native_head_page_flip schema=1 status=retired output={} connector_id={} submission={} frame={}",
                        self.heads[index].output.id.raw(),
                        self.heads[index].selection.connector_id(),
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
                tracing::info!(
                    "sophia_live_native_head_page_flip schema=1 status=callback_accepted output={} connector_id={} callbacks={} kernel_sequence={}",
                    self.heads[index].output.id.raw(),
                    self.heads[index].selection.connector_id(),
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
                        self.kernel_page_flip_ust.remove(&(
                            output,
                            self.heads[index].selection.connector_id(),
                            kernel_sequence,
                        )) {
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
            let has_head = !self.head_indices(output).is_empty();
            let initialized = self.initialize_transaction(output, runtime, frame);
            finish_live_production_native_initialization(initialized, has_head, || {
                self.release_displayed_output(output, runtime)
            })
        }

        fn initialize_transaction(
            &mut self,
            output: OutputId,
            runtime: &mut crate::LiveBackendRuntimeAssembly,
            frame: LiveProductionComposedFrame,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let indices = self.head_indices(output);
            let Some(&index) = indices.first() else {
                return Err(format!("native output {} has no head", output.raw()).into());
            };
            if indices.len() > 1 {
                self.queue_projected_bootstrap_frame(output, &frame, self.mirror_fit)?;
                for head_index in indices.iter().copied() {
                    self.exporters[head_index].arm_direct_cpu_bootstrap()?;
                }
            } else {
                self.queue_frame(output, frame);
            }
            for (position, head_index) in indices.into_iter().enumerate() {
                let group = self.heads[head_index].group;
                let selection = self.heads[head_index].selection;
                let export_attempts_before = self.exporters[head_index].cpu_frame_export_attempts();
                let direct_attempts_before =
                    self.exporters[head_index].direct_cpu_bootstrap_attempts();
                let direct_exports_before =
                    self.exporters[head_index].direct_cpu_bootstrap_exports();
                let mixed_attempts_before =
                    self.exporters[head_index].mixed_frame_export_attempts();
                if position == 0 {
                    self.groups[group]
                        .session
                        .initialize_persistent_native_gbm_scanout_for_selection(
                            runtime,
                            &mut self.exporters[head_index],
                            selection,
                        )
                        .map_err(|evidence| {
                            format!("persistent native initial modeset failed: {evidence:?}")
                        })?;
                } else {
                    let displayed = self.groups[group]
                        .session
                        .initialize_persistent_native_gbm_scanout_head(
                            &mut self.exporters[head_index],
                            selection,
                        )
                        .map_err(|evidence| {
                            format!("persistent native mirror-head modeset failed: {evidence:?}")
                        })?;
                    self.heads[head_index].displayed_scanout = Some(
                        displayed
                            .map_scanout_buffer(|owner| Box::new(owner) as Box<dyn std::any::Any>),
                    );
                }
                if self.head_indices(output).len() > 1 {
                    if self.exporters[head_index].direct_cpu_bootstrap_attempts()
                        != direct_attempts_before.saturating_add(1)
                        || self.exporters[head_index].direct_cpu_bootstrap_exports()
                            != direct_exports_before.saturating_add(1)
                    {
                        return Err("mirror bootstrap did not export through direct CPU GBM".into());
                    }
                    tracing::info!(
                        "sophia_live_mirror_bootstrap schema=1 status=direct_cpu output={} connector_id={} exports=1",
                        output.raw(),
                        selection.connector_id(),
                    );
                }
                self.exporters[head_index].enable_worker()?;
                if self.head_indices(output).len() > 1 {
                    if !self.exporters[head_index].worker_enabled() {
                        return Err("mirror renderer worker was not established".into());
                    }
                    tracing::info!(
                        "sophia_live_mirror_bootstrap schema=1 status=worker_ready output={} connector_id={} workers=1",
                        output.raw(),
                        selection.connector_id(),
                    );
                }
                let head = &mut self.heads[head_index];
                let exported_nonzero = (self.exporters[head_index].cpu_frame_export_attempts()
                    > export_attempts_before
                    && head.pending_nonzero_pixel_bytes > 0)
                    || (self.exporters[head_index].mixed_frame_export_attempts()
                        > mixed_attempts_before
                        && self.exporters[head_index].composition_nonzero_rgb_pixels() > 0);
                if exported_nonzero {
                    self.nonzero_exports = self.nonzero_exports.saturating_add(1);
                    head.nonzero_exports = head.nonzero_exports.saturating_add(1);
                }
                if !self.exporters[head_index].pending_cpu_frame() {
                    head.pending_nonzero_pixel_bytes = 0;
                }
                self.submissions = self.submissions.saturating_add(1);
                trace_live_native_lifecycle("initial_modeset_complete");
                head.submissions = head.submissions.saturating_add(1);
                head.presented_checksum = head.last_checksum;
                head.presented_submissions = head.submissions;
                head.presented_content = head.pending_content.take();
                if head.output_frames.pending().is_some() {
                    let presented =
                        head.output_frames
                            .mark_initial_presented()
                            .map_err(|error| {
                                format!(
                                    "initial compositor display-list transition failed: {error}"
                                )
                            })?;
                    trace_presented_output_damage("initial_presented", head.output.id, &presented);
                }
                head.initial_modeset_submission = Some(head.submissions);
                let transition = self
                    .output_lifecycles
                    .get_mut(&output)
                    .expect("a registered output has a head lifecycle")
                    .mark_initialized(selection.connector_id());
                debug_assert!(matches!(
                    transition,
                    LiveProductionMirrorHeadTransition::Accepted
                        | LiveProductionMirrorHeadTransition::GroupReady
                ));
            }
            if self.head_indices(output).len() > 1 {
                self.heads[index].displayed_scanout = Some(
                    runtime
                        .take_displayed_rendered_primary_plane_scanout()
                        .ok_or("mirror primary baseline owner was not retained")?,
                );
            }
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
                let indices = self.head_indices(output);
                let statuses = indices
                    .iter()
                    .map(|head_index| {
                        let head = &self.heads[*head_index];
                        reduce_live_production_cpu_frame_queue(
                            head.pending_content,
                            head.submitted_content,
                            head.presented_content,
                            self.exporters[*head_index].worker_in_flight(),
                            head.callback_accepted != 0
                                || head.initial_modeset_submission.is_some(),
                            frame.checksum,
                        )
                    })
                    .collect::<Vec<_>>();
                if statuses
                    .iter()
                    .any(|status| *status == LiveProductionCpuFrameQueueStatus::GpuFrameOwned)
                {
                    return LiveProductionCpuFrameQueueStatus::GpuFrameOwned;
                }
                for unchanged in [
                    LiveProductionCpuFrameQueueStatus::UnchangedPending,
                    LiveProductionCpuFrameQueueStatus::UnchangedSubmitted,
                    LiveProductionCpuFrameQueueStatus::UnchangedPresented,
                ] {
                    if statuses.iter().all(|status| *status == unchanged) {
                        return unchanged;
                    }
                }
                if self
                    .output_lifecycles
                    .get(&output)
                    .and_then(LiveProductionMirrorGroupLifecycle::active_frame)
                    .is_some()
                    || self.scanout_in_flight(output)
                {
                    return LiveProductionCpuFrameQueueStatus::GpuFrameOwned;
                }
                let projected = self.queue_projected_frame(output, &frame, self.mirror_fit);
                return if projected.is_some() {
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
            self.head_indices(output).into_iter().any(|index| {
                self.exporters[index].pending_frame()
                    || self.heads[index].scanout_submission.is_some()
            })
        }

        pub fn scanout_in_flight(&self, output: OutputId) -> bool {
            self.head_indices(output)
                .into_iter()
                .any(|index| self.heads[index].scanout_submission.is_some())
        }

        pub fn scanout_cleanup_pending(&self, output: OutputId) -> bool {
            self.head_indices(output)
                .into_iter()
                .any(|index| self.heads[index].scanout_cleanup.is_some())
        }

        pub fn mirror_generation_failed(&self, output: OutputId) -> bool {
            self.output_lifecycles
                .get(&output)
                .is_some_and(LiveProductionMirrorGroupLifecycle::failed)
        }

        fn mirror_poison_drained(&self, output: OutputId) -> bool {
            self.mirror_generation_failed(output)
                && !self.scanout_in_flight(output)
                && !self.scanout_cleanup_pending(output)
        }

        pub fn any_head_scanout_in_flight(&self) -> bool {
            self.heads
                .iter()
                .any(|head| head.scanout_submission.is_some())
        }

        pub fn head_scanout_in_flight_count(&self) -> usize {
            self.heads
                .iter()
                .filter(|head| head.scanout_submission.is_some())
                .count()
        }

        pub fn any_head_cleanup_pending(&self) -> bool {
            self.heads.iter().any(|head| head.scanout_cleanup.is_some())
        }

        pub fn submitted_content(&self, output: OutputId) -> Option<LiveProductionScanoutContent> {
            if self.head_indices(output).len() > 1
                && !self
                    .output_lifecycles
                    .get(&output)
                    .is_some_and(LiveProductionMirrorGroupLifecycle::awaiting_flips)
            {
                return None;
            }
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
            let indices = self.head_indices(output);
            if indices.is_empty() {
                return false;
            }
            indices.into_iter().all(|index| {
                let head = &self.heads[index];
                live_production_scanout_is_stable_present(
                    head.presented_content,
                    head.submitted_content,
                    self.exporters[index].pending_frame() || head.scanout_submission.is_some(),
                    transaction,
                )
            })
        }

        pub fn presented_mixed_nonzero_rgb_pixels(&self, transaction: TransactionId) -> usize {
            for output in self.outputs() {
                let pixels = self
                    .head_indices(output.id)
                    .into_iter()
                    .map(|index| match self.heads[index].presented_content {
                        Some(LiveProductionScanoutContent::MixedPresent {
                            transaction: presented,
                            nonzero_rgb_pixels,
                            ..
                        }) if presented == transaction => Some(nonzero_rgb_pixels),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>();
                if let Some(pixels) = pixels {
                    return pixels.into_iter().min().unwrap_or(0);
                }
            }
            0
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
                    (
                        timestamp.output,
                        timestamp.connector_id,
                        timestamp.frame_serial,
                    ),
                    timestamp.ust_usec,
                );
            }
            for callback in callbacks {
                // By connector, not by output. Two heads of a mirror group share an
                // output, so matching on it delivered both flips to whichever head
                // came first and left the sibling looking like it never flipped.
                let Some(head) = self.heads.iter().find(|head| {
                    head.group == group
                        && head.selection.connector_id() == callback.connector_id
                        && head.output.id == callback.output
                }) else {
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
    LiveProductionCpuFrameQueueStatus, LiveProductionMirrorGroupBegin,
    LiveProductionMirrorGroupLifecycle, LiveProductionMirrorHeadTransition,
    LiveProductionNativeFrameId, LiveProductionNativeFrameRetirement, LiveProductionNativeHead,
    LiveProductionNativeScanout, LiveProductionPageFlipWatchdogStatus,
    LiveProductionRendererImageHandoff, LiveProductionScanoutContent,
    finish_live_production_native_initialization, live_production_scanout_is_stable_present,
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
