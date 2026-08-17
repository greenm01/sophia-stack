struct SessionLoopChannels<'a> {
    authority: &'a Receiver<XAuthorityObservedTransactionBatch>,
    input: &'a XAuthorityRoutedInputSender,
    control: &'a XServerFrontendControlRouter,
    raster: &'a sophia_x_authority::XServerFrontendRasterRouter,
    control_acknowledgements: &'a Receiver<XAuthorityClientControlAck>,
    input_deliveries: &'a Receiver<XAuthorityClientInputDelivery>,
    route_lease_updates: &'a Receiver<XAuthorityRouteLeaseUpdate>,
    route_lease_releases: &'a SyncSender<XAuthorityRouteLeaseRelease>,
    frontend_service: &'a SyncSender<XServerFrontendServiceCommand>,
}

struct SessionLoopResources<'a> {
    child: Option<&'a mut Child>,
    secondary_children: &'a mut Vec<ManagedSessionChild>,
    physical_input: &'a mut Option<SessionPhysicalInput>,
    native_scanout: &'a mut Option<LiveProductionNativeScanout>,
    seat_controller: &'a mut Option<sophia_backend_live::LiveSeatController>,
    wm_session: &'a mut Option<LiveWmSession>,
    /// Which connectors share one logical output, from the profile loaded at
    /// startup. Fixed for the session's life: a rescan that regrouped differently
    /// would change the desktop's identity behind policy's back.
    mirror_grouping: &'a sophia_backend_live::NativeMirrorGrouping,
    /// Neutral initial policy for heads reconstructed after VT/hotplug loss.
    /// Output-authority commits may replace it independently on live heads.
    initial_head_mapping: sophia_protocol::OutputHeadMapping,
}

struct SessionLoopStartup<'a> {
    xauthority: &'a std::path::Path,
    protocol_router: XServerFrontendProtocolRouter,
    input_proof_result: Option<&'a LiveInputProofResult>,
    client_stdout_capture: Option<&'a LiveClientStdoutCapture>,
    require_startup_focus: bool,
    initial_authority_batch: Option<XAuthorityObservedTransactionBatch>,
    output_notifications: usize,
}

fn authority_wait_timeout(
    physical_input_active: bool,
    cursor_update_pending: bool,
    control_pending: bool,
) -> Duration {
    Duration::from_millis(
        if physical_input_active || cursor_update_pending || control_pending {
            1
        } else {
            25
        },
    )
}

fn native_frame_service_requires_owner_progress(request: &OutputFrameServiceRequest) -> bool {
    request.presentation_queued
        || request.outputs.iter().any(|output| {
            output.pending_frame || output.native_phase != OutputNativeFramePhase::Idle
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProductionCycleNativeOwnerPolicy {
    Unavailable,
    Available,
}

const fn production_cycle_native_owner_policy(
    native_enabled: bool,
    cpu_frame_deferred: bool,
) -> ProductionCycleNativeOwnerPolicy {
    // Composition coalescing is advisory. The visual runtime still needs the
    // native owner to preserve retained GPU content or repaint changed chrome.
    match (native_enabled, cpu_frame_deferred) {
        (false, false) | (false, true) => ProductionCycleNativeOwnerPolicy::Unavailable,
        (true, false) | (true, true) => ProductionCycleNativeOwnerPolicy::Available,
    }
}

fn native_frame_service_should_preempt_authority(
    request: &OutputFrameServiceRequest,
    preempted_previous_cycle: bool,
    control_pending: bool,
    control_priority_cycles: u8,
    service_due: bool,
) -> bool {
    // Controls get several owner turns first, but no control class may block
    // renderer polling indefinitely. An armed service deadline also covers
    // work whose instantaneous output hint has gone idle, so worker watchdogs
    // keep making progress through resize-recovery traffic. The following
    // owner turn returns to authority traffic.
    const CONTROL_PRIORITY_CYCLES: u8 = 4;

    (!control_pending || control_priority_cycles >= CONTROL_PRIORITY_CYCLES)
        && !preempted_previous_cycle
        && (service_due || native_frame_service_requires_owner_progress(request))
}

fn synchronize_runtime_surface_chrome_style(
    runtime: &mut LiveProductionVisualRuntime,
    style: sophia_engine::SurfaceChromeStyle,
) -> bool {
    runtime.set_surface_chrome_style(style)
}

fn capture_renderer_image_handoff(
    runtime: &LiveProductionVisualRuntime,
    native_scanout: &mut LiveProductionNativeScanout,
    output: sophia_protocol::OutputId,
) -> Result<sophia_backend_live::LiveProductionRendererImageHandoff, Box<dyn std::error::Error>> {
    let retained = runtime.retained_renderer_image_ids();
    native_scanout.export_renderer_image_handoff(output, &retained)
}

fn resume_native_scanout_from_scene(
    runtime: &mut LiveProductionVisualRuntime,
    native: &mut LiveProductionNativeScanout,
    outputs: &[sophia_engine::HeadlessOutput],
    scene: &mut LiveProductionCpuScene,
    handoff: Option<sophia_backend_live::LiveProductionRendererImageHandoff>,
) -> Result<usize, Box<dyn std::error::Error>> {
    runtime.resume_native_scanout(
        native,
        outputs,
        scene,
        handoff,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveOutputTopologyExecutionPhase {
    WaitingForQuiescence,
    Preparing,
    Applying,
    AwaitingFirstPresentation,
    Reconciling,
    RollingBack,
}

#[derive(Clone, Debug)]
struct LiveOutputTopologyExecution {
    effect: sophia_cli::live_output_authority::LiveOutputAuthorityEffect,
    phase: LiveOutputTopologyExecutionPhase,
    preparation_deadline: Instant,
    first_frames:
        BTreeMap<sophia_protocol::OutputId, sophia_backend_live::LiveProductionNativeFrameId>,
    frontend_candidate_published: bool,
    last_preparation_progress:
        Option<sophia_backend_live::LiveProductionNativeTopologyPreparationReport>,
}

const OUTPUT_TOPOLOGY_QUIESCENCE_TIMEOUT: Duration = Duration::from_secs(2);

fn begin_output_topology_first_presentation_rollback<NativeRollback, PolicyRollback>(
    phase: &mut LiveOutputTopologyExecutionPhase,
    transaction: sophia_protocol::TransactionId,
    failure: &str,
    request_native_rollback: NativeRollback,
    reject_policy_candidate: PolicyRollback,
) -> Result<bool, Box<dyn std::error::Error>>
where
    NativeRollback: FnOnce(String) -> Result<(), Box<dyn std::error::Error>>,
    PolicyRollback:
        FnOnce(sophia_protocol::TransactionId) -> Result<(), Box<dyn std::error::Error>>,
{
    if *phase != LiveOutputTopologyExecutionPhase::AwaitingFirstPresentation {
        return Ok(false);
    }
    request_native_rollback(format!("first topology presentation failed: {failure}"))?;
    // Once the physical owner accepted reverse apply, retain that fact even if
    // policy notification fails. Session completion can then finish rollback
    // without mistaking the candidate for a merely queued transaction.
    *phase = LiveOutputTopologyExecutionPhase::RollingBack;
    reject_policy_candidate(transaction)?;
    Ok(true)
}

fn run_session_loop(
    config: &mut PersistentXtermSessionConfig,
    channels: SessionLoopChannels<'_>,
    resources: SessionLoopResources<'_>,
    startup: SessionLoopStartup<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let SessionLoopChannels {
        authority: authority_receiver,
        input: input_sender,
        control: control_sender,
        raster: raster_sender,
        control_acknowledgements: control_ack_receiver,
        input_deliveries: input_delivery_receiver,
        route_lease_updates: route_lease_update_receiver,
        route_lease_releases: route_lease_release_sender,
        frontend_service: frontend_service_sender,
    } = channels;
    let SessionLoopResources {
        mut child,
        secondary_children,
        physical_input,
        native_scanout,
        seat_controller,
        wm_session,
        mirror_grouping,
        initial_head_mapping,
    } = resources;
    let SessionLoopStartup {
        xauthority,
        protocol_router,
        input_proof_result,
        client_stdout_capture,
        require_startup_focus,
        mut initial_authority_batch,
        output_notifications,
    } = startup;
    let started = Instant::now();
    let config_watcher = config
        .core_config_source
        .path
        .as_deref()
        .map(sophia_config::ConfigWatcher::spawn)
        .transpose()?;
    let mut config_reload_pending = false;
    let deadline = config.max_runtime.map(|duration| started + duration);
    let blank_normal_session = config.normal_session && config.applications.startup.is_empty();
    let initialize_empty_runtime =
        blank_normal_session || (config.normal_session && native_scanout.is_some());
    let mut outputs = native_scanout
        .as_ref()
        .map(LiveProductionNativeScanout::outputs)
        .unwrap_or_else(|| vec![sophia_engine::HeadlessOutput::deterministic()]);
    let mut output = outputs[0];
    // Headless has no connectors, so every output is its own single head. That is
    // the same shape hardware reports for an unmirrored desktop.
    let heads = native_scanout
        .as_ref()
        .map(LiveProductionNativeScanout::head_fingerprint)
        .unwrap_or_else(|| outputs.iter().map(|output| (output.id, 1)).collect());
    let initial_output_publication_generation =
        1u64.saturating_add(u64::from(config.inject_output_size.is_some()));
    let mut output_topology_owner = LiveOutputTopologyOwner::new_at_generation(
        outputs.clone(),
        heads,
        initial_output_publication_generation,
    )?;
    let mut output_topology_monitor = native_scanout
        .is_some()
        .then(sophia_backend_live::LiveDrmTopologyMonitor::open)
        .transpose()?;
    let mut output_topology_retry_at: Option<Instant> = None;
    let mut deferred_output_topology_notice:
        Option<sophia_backend_live::LiveDrmTopologyRescanNotice> = None;
    let mut output_topology_policy_commit_baseline = 0u64;
    let mut scene = LiveProductionCpuScene::new(output.size);
    if initialize_empty_runtime {
        scene.compose(&[], None, None)?;
    }
    let mut layout = PersistentLiveLayout::new(
        LivePolicyMapMode::from_external_wm(wm_session.is_some()),
        output.size,
    );
    let mut pending_wm_update = None;
    let mut active_output_topology_preparation: Option<LiveOutputTopologyExecution> = None;
    let mut pending_hardware_output_publication: Option<(
        sophia_protocol::OutputAuthoritySnapshot,
        Vec<sophia_backend_live::LibdrmNativeOutputCapability>,
    )> = None;
    // Whether the parked hardware snapshot's topology has presented, which is
    // the first of the two conditions its publication waits on.
    let mut hardware_output_publication_presented = false;
    let mut floating_pointer_gesture = FloatingPointerGestureState::default();
    let mut staged_cpu_buffer_handles = Vec::with_capacity(16);
    let mut layout_progress_deferred_reported = false;
    let mut committed_session_actions = VecDeque::new();
    let mut session_launches = SessionLaunchQueue::default();
    let mut launch_admission_started_at: Option<Instant> = None;
    let mut present_observer = XPresentSessionObserver::new(protocol_router);
    let mut present_feedback = Vec::new();
    let initial_border_style = wm_session
        .as_ref()
        .and_then(|wm| wm.surface_chrome_style())
        .unwrap_or(config.surface_chrome_style);
    let mut runtime = if initialize_empty_runtime {
        let mut initialized =
            LiveProductionVisualRuntime::new(&outputs, native_scanout.as_mut())?
        .with_m4_proof_controls(
            config.m4_first_acquire_delay,
            config.m4_reject_first_present,
            config.m4_diagnose_first_mixed_export,
        )
        .with_surface_chrome_style(initial_border_style);
        if let Some(native) = native_scanout.as_mut() {
            let _ = initialized.run_cpu_repaint(
                &mut scene,
                None,
                None,
                &outputs,
                native,
            )?;
        }
        Some(initialized)
    } else {
        None
    };
    let mut last_authority_update = started;
    let mut injection_checksum = None;
    let mut physical_input_ready_at: Option<Instant> = None;
    let mut physical_text_proof = config
        .expect_physical_text
        .as_deref()
        .map(|text| {
            if config.application_proof_requested() {
                PhysicalTextProof::new_without_submit(text)
            } else {
                PhysicalTextProof::new(text)
            }
        })
        .transpose()?;
    let mut physical_sequence_completed_at: Option<Instant> = None;
    let mut physical_input_completion_reported = false;
    let mut input_pixel_change = false;
    let mut input_surface = None;
    let mut input_surface_generation = None;
    let mut input_surface_pixel_change = false;
    let mut input_proof_started_at = None;
    let mut input_change_submission_baseline = None;
    let mut input_presented_latency = None;
    let mut input_raw_ingress_msec: Option<u64> = None;
    let mut deferred_physical_key_timings = BTreeMap::new();
    let mut routed_input_saturation = RoutedInputIngressSaturation::default();
    let mut routed_input_saturation_ledger = sophia_protocol::CapacityReportLedger::default();
    let mut input_queue_dwell: Option<Duration> = None;
    let mut input_presented_ust_usec: Option<u64> = None;
    let mut input_submit_to_page_flip: Option<Duration> = None;
    let mut pointer_checksum = None;
    let mut pointer_cursor_checksum = None;
    let mut pointer_phase_started_at = None;
    let mut cursor_visible_reported = false;
    let mut pointer_pixel_change = false;
    let mut metrics = SessionLoopMetrics::new(initialize_empty_runtime);
    let mut input_batch_baseline = None;
    let mut input_cpu_update_baseline = None;
    let mut focus = InputFocusState::new();
    let mut modifiers = config.keyboard_mapper();
    let key_repeat_map = XkbKeymapSnapshot::new(&config.xkb_config)?;
    let key_repeat_config = KeyRepeatConfig::new(
        config.key_repeat_config.delay_msec,
        config.key_repeat_config.interval_msec,
    )
    .ok_or("X11 key repeat controls must be nonzero")?;
    let mut key_repeat = KeyRepeatState::new(key_repeat_config);
    let mut client_keys = SessionClientKeyState::default();
    let mut client_key_scratch = Vec::with_capacity(SESSION_CLIENT_PRESSED_KEY_CAPACITY);
    let mut client_key_deliveries = Vec::with_capacity(SESSION_CLIENT_PRESSED_KEY_CAPACITY);
    let mut client_key_release_barrier = BTreeSet::new();
    let mut runtime_deadline_key_drain = RuntimeDeadlineKeyDrain::default();
    let mut emergency_chord = EmergencyChordState::armed();
    let mut virtual_terminal_chord = VirtualTerminalChordState::default();
    let mut keyboard_coverage = PhysicalKeyboardCoverage::default();
    let mut pointer = SessionPointerPlacement::default();
    let mut keyboard_focus_handoff = KeyboardFocusHandoffState::default();
    let mut pointer_focus_handoff = PointerFocusHandoffState::default();
    let mut application_route_leases = ApplicationRouteLeaseState::default();
    if native_scanout.is_some() {
        pointer.set_output_bounds(
            wm_output_bounds(&outputs)
                .into_iter()
                .map(|(_, bounds)| bounds)
                .collect(),
        );
        pointer.center_on_primary_output(output.size);
    }
    let seat = SeatId::from_raw(SESSION_SEAT_RAW);
    let mut focus_deadline_started_at = None;
    let mut focus_ready_reported = false;
    let mut applied_client_focus: Option<SurfaceId> = None;
    let mut session_controls = SessionControlQueue::default();
    let mut session_control_completions =
        Vec::with_capacity(SESSION_CONTROL_CAPACITY);
    let mut next_focus_control_transaction = 1_000_000u64;
    let resize_proof_targets = config.surface_resize_targets();
    let mut resize_proof: Option<(TransactionId, SurfaceId, Size)> = None;
    let mut resize_proof_completed = 0usize;
    let mut resize_proof_complete = false;
    let mut input_observations = InputObservationState::default();
    let mut input_delivery = InputDeliveryState::default();
    let mut logout_requested = false;
    let mut post_input_deadline: Option<Instant> = None;
    let mut application_surface_gone_at: Option<Instant> = None;
    let mut input_content_surface: Option<SurfaceId> = None;
    let mut startup_readiness = SessionStartupReadiness::default();
    if blank_normal_session {
        let _ = reduce_session_startup(
            &mut startup_readiness,
            SessionStartupEvent::BlankSessionReady,
        );
    }
    let mut startup_content_ready = blank_normal_session;
    let mut startup_ready_msec = blank_normal_session.then_some(0);
    let mut input_text_match = false;
    let mut primary_child_exited = child.is_none();
    let mut primary_exit_status = None;
    let mut terminal_client_error: Option<(&'static str, String)> = None;
    let mut terminal_runtime_error: Option<String> = None;
    let mut terminal_client_intake_stopped = false;
    let mut terminal_client_cleanup_failures = Vec::new();
    let mut post_startup_exit_pointer_reported = false;
    let mut application_surface_missing_since: Option<Instant> = None;
    let mut client_stdout = Vec::new();
    let mut firefox_m8_proof = if config.firefox_m10_proof {
        FirefoxM8StageProof::promotion()
    } else {
        FirefoxM8StageProof::default()
    };
    let mut firefox_m10_kitty_proof = FirefoxM10KittyProof::default();
    let mut firefox_m10_selection_kitty_proof = FirefoxM10SelectionKittyProof::default();
    let mut firefox_m10_dialog_proof = FirefoxM10DialogProof::default();
    let mut firefox_m10_primary_proof = FirefoxM10PrimaryProof::default();
    let mut firefox_m10_rendering_page_ready = false;
    let mut firefox_m8_page_ready_reported = false;
    let mut firefox_m8_navigation_ready_reported = false;
    let mut firefox_m8_dialog_ready_reported = false;
    let mut selection_owner_changes = 0usize;
    let mut selection_conversions = 0usize;
    let mut first_protocol_error = None;
    let mut emergency_exit_requested = false;
    let mut cursor_updates = CursorUpdateState::new(pointer.position().is_some());
    let startup_ready_deadline = config
        .startup_ready_timeout
        .map(|timeout| started + timeout);
    let mut startup_required_submissions: Option<Vec<usize>> = None;
    let mut retired_present_surfaces = BTreeMap::new();
    let mut startup_surface_presentations = StartupSurfacePresentationEvidence::default();
    let mut startup_ready_reported = false;
    let mut startup_native_recovery_attempted = false;
    let mut startup_topology_recovery_pending = false;
    let mut startup_outputs_ready_reported = false;
    let mut pending_authority_batches = VecDeque::new();
    // Advisory raster demand is latest-wins per protocol-neutral SurfaceId.
    // A busy client must not turn an authority-route stall into an unbounded
    // generation queue.
    let mut pending_surface_raster_requirements = BTreeMap::new();
    let mut seat_state = sophia_backend_live::LiveSeatState::Active;
    let mut pending_virtual_terminal: Option<(u8, Instant)> = None;
    let mut requested_virtual_terminal = None;
    let mut seat_release_prepared = false;
    let mut suspended_renderer_images = None;
    let mut observed_wm_restart_count = wm_session.as_ref().map_or(0, |wm| wm.restarts);

    macro_rules! revoke_floating_pointer_interaction {
        ($reason:literal) => {{
            if let Some(interaction) = floating_pointer_gesture.cancel() {
                if let Some(wm) = wm_session.as_mut()
                    && matches!(
                        wm.enqueue_pointer_interaction(interaction, &layout)?,
                        LiveWmRequestAdmission::RejectedCapacity
                    )
                {
                    return Err(format!(
                        "security cancellation exceeded WM owner capacity: {}",
                        $reason
                    )
                    .into());
                }
                if let Some(runtime) = runtime.as_mut() {
                    let _ = runtime.set_floating_outline(
                        None,
                        &scene,
                        native_scanout.as_mut(),
                    )?;
                }
                println!(
                    "sophia_live_wm_pointer schema=2 status=interaction_cancelled reason={} surface={}",
                    $reason,
                    interaction.surface.index(),
                );
            }
        }};
    }

    macro_rules! synchronize_wm_pointer_epoch {
        () => {{
            let restart_count = wm_session.as_ref().map_or(0, |wm| wm.restarts);
            if restart_count != observed_wm_restart_count {
                observed_wm_restart_count = restart_count;
                revoke_floating_pointer_interaction!("policy_restart");
            }
        }};
    }

include!("owner_loop/session_control.rs")
}
