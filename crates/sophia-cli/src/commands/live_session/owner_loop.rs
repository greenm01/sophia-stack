struct SessionLoopChannels<'a> {
    authority: &'a Receiver<XAuthorityObservedTransactionBatch>,
    input: &'a SyncSender<XAuthorityRoutedInput>,
    control: &'a XServerFrontendControlRouter,
    control_acknowledgements: &'a Receiver<XAuthorityClientControlAck>,
    input_deliveries: &'a Receiver<XAuthorityClientInputDelivery>,
}

struct SessionLoopResources<'a> {
    child: Option<&'a mut Child>,
    secondary_children: &'a mut Vec<ManagedSessionChild>,
    physical_input: &'a mut Option<SessionPhysicalInput>,
    native_scanout: &'a mut Option<LiveProductionNativeScanout>,
    seat_controller: &'a mut Option<sophia_backend_live::LiveSeatController>,
    wm_session: &'a mut Option<LiveWmSession>,
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
        control_acknowledgements: control_ack_receiver,
        input_deliveries: input_delivery_receiver,
    } = channels;
    let SessionLoopResources {
        mut child,
        secondary_children,
        physical_input,
        native_scanout,
        seat_controller,
        wm_session,
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
    let outputs = native_scanout
        .as_ref()
        .map(LiveProductionNativeScanout::outputs)
        .unwrap_or_else(|| vec![sophia_engine::HeadlessOutput::deterministic()]);
    let output = outputs[0];
    let mut scene = LiveProductionCpuScene::new(output.size);
    if initialize_empty_runtime {
        scene.compose(&[], None, None)?;
    }
    let mut layout = PersistentLiveLayout::new(
        LivePolicyMapMode::from_external_wm(wm_session.is_some()),
        output.size,
    );
    let mut pending_wm_update = None;
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
        Some(
            LiveProductionVisualRuntime::new(
                &outputs,
                native_scanout.as_mut(),
                Some(scene.frames_for_outputs(&outputs)?),
            )?
            .with_m4_proof_controls(
                config.m4_first_acquire_delay,
                config.m4_reject_first_present,
                config.m4_diagnose_first_mixed_export,
            )
            .with_surface_chrome_style(initial_border_style),
        )
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
    let mut modifiers = XCoreKeyboardMapper::new();
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
    let mut emergency_chord = EmergencyChordState::armed();
    let mut virtual_terminal_chord = VirtualTerminalChordState::default();
    let mut keyboard_coverage = PhysicalKeyboardCoverage::default();
    let mut pointer = SessionPointerPlacement::default();
    let mut pointer_focus_handoff = PointerFocusHandoffState::default();
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
    let mut resize_proof: Option<(TransactionId, SurfaceId, Size)> = None;
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
    let mut startup_outputs_ready_reported = false;
    let mut pending_authority_batches = VecDeque::new();
    let mut seat_state = sophia_backend_live::LiveSeatState::Active;
    let mut pending_virtual_terminal: Option<(u8, Instant)> = None;
    let mut requested_virtual_terminal = None;
    let mut seat_release_prepared = false;

    include!("owner_loop/session_control.rs")
}
