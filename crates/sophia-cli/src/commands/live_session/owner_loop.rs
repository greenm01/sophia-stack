struct SessionLoopChannels<'a> {
    authority: &'a Receiver<XAuthorityObservedTransactionBatch>,
    input: &'a SyncSender<XAuthorityRoutedInput>,
    control: &'a SyncSender<XAuthorityClientControlCommand>,
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

fn run_session_loop(
    config: &PersistentXtermSessionConfig,
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
        wm_session.is_some(),
        require_startup_focus.then_some(output.size),
    );
    let mut committed_session_actions = VecDeque::new();
    let mut present_observer = XPresentSessionObserver::new(protocol_router);
    let mut present_feedback = Vec::new();
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
            ),
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
    let mut emergency_chord = EmergencyChordState::armed();
    let mut virtual_terminal_chord = VirtualTerminalChordState::default();
    let mut pointer = SessionPointerPlacement::default();
    if native_scanout.is_some() {
        pointer.center_on_primary_output(output.size);
    }
    let seat = SeatId::from_raw(SESSION_SEAT_RAW);
    let mut focus_deadline_started_at = None;
    let mut focus_ready_reported = false;
    let mut applied_client_focus: Option<SurfaceId> = None;
    let mut focused_client_control: Option<(TransactionId, SurfaceId)> = None;
    let mut next_focus_control_transaction = 1_000_000u64;
    let mut resize_proof: Option<(TransactionId, SurfaceId, Size)> = None;
    let mut resize_proof_complete = false;
    let mut input_observations = InputObservationState::default();
    let mut input_delivery = InputDeliveryState::default();
    let mut logout_requested = false;
    let mut post_input_deadline: Option<Instant> = None;
    let mut application_surface_gone_at: Option<Instant> = None;
    let mut input_content_surface: Option<SurfaceId> = None;
    let mut startup_content_ready = blank_normal_session;
    let mut startup_ready_msec = blank_normal_session.then_some(0);
    let mut input_text_match = false;
    let mut primary_child_exited = child.is_none();
    let mut primary_exit_status = None;
    let mut application_surface_missing_since: Option<Instant> = None;
    let mut client_stdout = Vec::new();
    let mut firefox_m8_proof = FirefoxM8StageProof::default();
    let mut firefox_m8_page_ready_reported = false;
    let mut firefox_m8_selection_owner_changes = 0usize;
    let mut firefox_m8_selection_conversions = 0usize;
    let mut first_protocol_error = None;
    let mut emergency_exit_requested = false;
    let mut cursor_updates = CursorUpdateState::new(pointer.position.is_some());
    let startup_ready_deadline = config
        .startup_ready_timeout
        .map(|timeout| started + timeout);
    let mut startup_required_submissions: Option<Vec<usize>> = None;
    let mut retired_present_surfaces = BTreeMap::new();
    let mut startup_ready_reported = false;
    let mut pending_authority_batches = VecDeque::new();
    let mut seat_state = sophia_backend_live::LiveSeatState::Active;
    let mut pending_virtual_terminal: Option<(u8, Instant)> = None;
    let mut requested_virtual_terminal = None;
    let mut seat_release_prepared = false;

    include!("owner_loop/physical_input_phase.rs")
}
