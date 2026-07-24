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
    let mut pointer_pixel_change = false;
    let mut metrics = SessionLoopMetrics::new(initialize_empty_runtime);
    let mut input_batch_baseline = None;
    let mut input_cpu_update_baseline = None;
    let mut focus = InputFocusState::new();
    let mut modifiers = XCoreKeyboardMapper::new();
    let mut emergency_chord = EmergencyChordState::armed();
    let mut pointer = SessionPointerPlacement::default();
    if native_scanout.is_some() {
        pointer.center_on_primary_output(output.size);
    }
    let seat = SeatId::from_raw(SESSION_SEAT_RAW);
    let mut focus_deadline_started_at = None;
    let mut focus_ready_reported = false;
    let mut focus_ready_at: Option<Instant> = None;
    let mut focused_client_ready = wm_session.is_some();
    let mut focused_client_control: Option<(TransactionId, SurfaceId)> = None;
    let mut next_focus_control_transaction = 1_000_000u64;
    let mut resize_proof: Option<(TransactionId, SurfaceId, Size)> = None;
    let mut resize_proof_complete = false;
    let mut key_observed_reported = false;
    let mut key_routed_reported = false;
    let mut pointer_motion_observed_reported = false;
    let mut pointer_motion_routed_reported = false;
    let mut pointer_button_observed_reported = false;
    let mut pointer_button_routed_reported = false;
    let mut next_input_delivery = 1u64;
    let mut pending_input_deliveries = BTreeSet::new();
    let mut logout_requested = false;
    let mut input_events_expected = 0usize;
    let mut input_events_flushed = 0usize;
    let mut input_delivery_wait_started_at: Option<Instant> = None;
    let mut input_delivery_source: Option<&'static str> = None;
    let mut input_flush_latency: Option<Duration> = None;
    let mut post_input_deadline: Option<Instant> = None;
    let mut application_surface_gone_at: Option<Instant> = None;
    let mut terminal_content_ready = blank_normal_session;
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
    let mut return_suppressed_reported = false;
    let mut cursor_dirty = pointer.position.is_some();
    let mut cursor_dirty_since = cursor_dirty.then_some(Instant::now());
    let startup_ready_deadline = config
        .startup_ready_timeout
        .map(|timeout| started + timeout);
    let mut startup_required_submissions: Option<Vec<usize>> = None;
    let mut retired_present_surfaces = BTreeMap::new();
    let mut startup_ready_reported = false;
    let mut pending_authority_batches = VecDeque::new();

    macro_rules! drain_physical_input {
        ($routing_mode:expr) => {{
            let emergency_exit = false;
            if let Some(poller) = physical_input.as_mut()
                && (config.expect_physical_text.is_none() || physical_input_ready_at.is_some())
            {
                let empty_committed = [];
                let committed_surfaces = runtime
                    .as_ref()
                    .map_or(&empty_committed[..], |runtime| runtime.committed_surfaces());
                let empty_layers = [];
                let input_layers = runtime
                    .as_ref()
                    .map_or(&empty_layers[..], |runtime| runtime.input_layers());
                let report = route_physical_input(
                    poller,
                    PhysicalInputRoutingContext {
                        focus: &focus,
                        committed_surfaces,
                        input_layers,
                        client_routes: &layout.client_routes,
                        shortcuts: wm_session
                            .as_mut()
                            .and_then(|wm_session| wm_session.shortcuts.as_mut()),
                        input_sender,
                        modifiers: &mut modifiers,
                        emergency_chord: &mut emergency_chord,
                        pointer: &mut pointer,
                        pointer_routing_enabled: !config.expect_physical_pointer
                            || pointer_checksum.is_some(),
                        pointer_proof_required: sophia_cli::input_proof::pointer_selection_pending(
                            config.expect_physical_pointer,
                            metrics.physical_pointer_buttons_routed,
                        ),
                        pointer_buttons_only: false,
                        routing_mode: $routing_mode,
                        next_input_delivery: &mut next_input_delivery,
                        physical_text_proof: physical_text_proof.as_mut(),
                    },
                )?;
                metrics.physical_events = metrics.physical_events.saturating_add(report.events);
                metrics.physical_keys_routed = metrics.physical_keys_routed.saturating_add(report.keys_routed);
                metrics.physical_pointer_events =
                    metrics.physical_pointer_events.saturating_add(report.pointer_events);
                metrics.physical_pointer_routed =
                    metrics.physical_pointer_routed.saturating_add(report.pointer_routed);
                metrics.physical_pointer_buttons_routed =
                    metrics.physical_pointer_buttons_routed.saturating_add(report.pointer_buttons_routed);
                input_events_expected =
                    input_events_expected.saturating_add(report.deliveries.len());
                pending_input_deliveries.extend(report.deliveries.iter().copied());
                if !report.deliveries.is_empty() && input_proof_started_at.is_some() {
                    input_delivery_wait_started_at.get_or_insert_with(Instant::now);
                }
                let pointer_motions_observed = report
                    .pointer_events
                    .saturating_sub(report.pointer_buttons_observed);
                if pointer_motions_observed > 0 && pointer.position.is_some() {
                    if cursor_dirty {
                        metrics.cursor_moves_coalesced =
                            metrics.cursor_moves_coalesced.saturating_add(pointer_motions_observed as u64);
                    } else {
                        cursor_dirty_since = Some(Instant::now());
                    }
                    cursor_dirty = true;
                }
                for action in report.wm_actions.iter().copied() {
                    let wm = wm_session
                        .as_mut()
                        .ok_or("WM shortcut activated without a live WM session")?;
                    let proposal =
                        wm.request_action(action, focus.focused_surface(seat), &layout, output)?;
                    if let Some(mut result) =
                        layout.stage(proposal, control_sender, control_ack_receiver)?
                    {
                        if result.update.commit.outcome == TransactionOutcome::Committed
                            && let Some(effects) = result.effects.take()
                        {
                            wm.workspace_state = effects.workspace_state;
                            if let Some(action) = effects.session_action {
                                committed_session_actions.push_back((
                                    effects.transaction,
                                    action.0,
                                    action.1,
                                ));
                            }
                        }
                        wm.mark_committed();
                    }
                }

                if report.return_suppressed && !return_suppressed_reported {
                    println!(
                        "sophia_live_session_input_pipeline schema=1 status=return_suppressed"
                    );
                    std::io::stdout().flush()?;
                    return_suppressed_reported = true;
                }
                if !key_observed_reported && report.keys_observed > 0 {
                    println!("sophia_live_session_input_pipeline schema=1 status=key_observed");
                    std::io::stdout().flush()?;
                    key_observed_reported = true;
                }
                if !key_routed_reported && report.keys_routed > 0 {
                    println!("sophia_live_session_input_pipeline schema=1 status=key_routed");
                    std::io::stdout().flush()?;
                    key_routed_reported = true;
                }
                if report.emergency_exit {
                    println!("sophia_live_session_input_pipeline schema=1 status=emergency_exit");
                    std::io::stdout().flush()?;
                    emergency_exit_requested = true;
                    let requested_at = Instant::now();
                    input_delivery_wait_started_at = Some(requested_at);
                    input_delivery_source = Some("emergency");
                }
                if physical_sequence_completed_at.is_none()
                    && physical_text_proof
                        .as_ref()
                        .is_some_and(|proof| proof.is_complete())
                {
                    let completed_at = Instant::now();
                    physical_sequence_completed_at = Some(completed_at);
                    input_delivery_wait_started_at = Some(completed_at);
                    input_delivery_source = Some("physical");
                    // Keep the baseline captured immediately before physical
                    // input became ready. Xterm can render the earlier letters
                    // before the poller observes Return; rebasing here discards
                    // that causal pixel evidence and can falsely report a
                    // static terminal after exact text delivery succeeded.
                    if physical_input_pixels_already_changed(
                        injection_checksum,
                        scene.last_report().map(|report| report.checksum),
                        input_surface_pixel_change,
                    ) {
                        input_pixel_change = true;
                    }
                }
                if !pointer_motion_observed_reported
                    && report.pointer_events > report.pointer_buttons_observed
                {
                    println!("sophia_live_session_pointer schema=2 status=motion_observed");
                    pointer_motion_observed_reported = true;
                }
                if !pointer_motion_routed_reported
                    && report.pointer_routed > report.pointer_buttons_routed
                {
                    println!("sophia_live_session_pointer schema=2 status=motion_routed");
                    pointer_motion_routed_reported = true;
                }
                if !pointer_button_observed_reported && report.pointer_buttons_observed > 0 {
                    println!(
                        "sophia_live_session_pointer schema=2 status=button_observed count={}",
                        report.pointer_buttons_observed
                    );
                    pointer_button_observed_reported = true;
                }
                if !pointer_button_routed_reported && report.pointer_buttons_routed > 0 {
                    println!(
                        "sophia_live_session_pointer schema=2 status=button_routed count={}",
                        metrics.physical_pointer_buttons_routed
                    );
                    pointer_button_routed_reported = true;
                }
                if pointer_motion_observed_reported
                    || pointer_button_observed_reported
                    || pointer_button_routed_reported
                {
                    std::io::stdout().flush()?;
                }
            }
            emergency_exit
        }};
    }

    macro_rules! drain_input_deliveries {
        () => {{
            while let Ok(delivery) = input_delivery_receiver.try_recv() {
                if !pending_input_deliveries.remove(&delivery.delivery) {
                    continue;
                }
                match delivery.outcome {
                    XAuthorityInputDeliveryOutcome::Flushed => {
                        input_events_flushed = input_events_flushed.saturating_add(1);
                    }
                    XAuthorityInputDeliveryOutcome::RouteRejected
                    | XAuthorityInputDeliveryOutcome::WriteFailed => {
                        return Err(format!(
                            "persistent live session X11 input delivery failed: outcome={:?} client={}",
                            delivery.outcome,
                            delivery.client.raw(),
                        )
                        .into());
                    }
                }
            }
            if let Some(wait_started) = input_delivery_wait_started_at
                && !pending_input_deliveries.is_empty()
                && wait_started.elapsed() >= Duration::from_millis(SESSION_INPUT_DELIVERY_TIMEOUT_MSEC)
            {
                return Err(format!(
                    "persistent live session timed out waiting for X11 input delivery: expected={input_events_expected} flushed={input_events_flushed} pending={}",
                    pending_input_deliveries.len(),
                )
                .into());
            }
            if let Some(wait_started) = take_settled_input_delivery_wait(
                &mut input_delivery_wait_started_at,
                pending_input_deliveries.is_empty(),
            ) && input_proof_started_at.is_none()
            {
                let flushed_at = Instant::now();
                input_flush_latency =
                    Some(flushed_at.saturating_duration_since(wait_started));
                input_proof_started_at = Some(flushed_at);
                post_input_deadline = Some(
                    flushed_at + Duration::from_millis(SESSION_PHYSICAL_PIXEL_TIMEOUT_MSEC),
                );
                println!(
                    "sophia_live_session_input_pipeline schema=2 status=key_flushed source={} expected={} flushed={}",
                    input_delivery_source.unwrap_or("unknown"),
                    input_events_expected,
                    input_events_flushed,
                );
                std::io::stdout().flush()?;
            }
        }};
    }

    loop {
        let input_baseline_presented_before_wait = include!("owner_loop/lifecycle.rs");
        include!("owner_loop/authority.rs");
        include!("owner_loop/input_proof.rs");
    }

    include!("owner_loop/completion.rs")
}
