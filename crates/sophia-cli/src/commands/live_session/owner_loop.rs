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
    let mut batches = 0usize;
    let mut transactions = 0usize;
    let mut cpu_buffer_updates = 0usize;
    let mut dma_buf_registrations_observed = 0usize;
    let mut fence_registrations_observed = 0usize;
    let mut present_submissions_observed = 0usize;
    let mut cpu_compositions = usize::from(initialize_empty_runtime);
    let mut coalesced_batches = 0usize;
    let mut input_batch_baseline = None;
    let mut input_cpu_update_baseline = None;
    let mut backend_ticks = 0usize;
    let mut runtime_committed = 0u64;
    let mut runtime_surfaces = 0u64;
    let mut focus = InputFocusState::new();
    let mut modifiers = XCoreKeyboardMapper::new();
    let mut emergency_chord = EmergencyChordState::armed();
    let mut pointer = SessionPointerPlacement::default();
    if native_scanout.is_some() {
        pointer.center_on_primary_output(output.size);
    }
    let mut physical_events = 0usize;
    let mut physical_keys_routed = 0usize;
    let mut physical_pointer_events = 0usize;
    let mut physical_pointer_routed = 0usize;
    let mut physical_pointer_buttons_routed = 0usize;
    let mut session_ticks = 0usize;
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
    let mut max_compose = Duration::ZERO;
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
    let mut protocol_error_count = 0usize;
    let mut expected_protocol_error_count = 0usize;
    let mut firefox_m8_proof = FirefoxM8StageProof::default();
    let mut firefox_m8_page_ready_reported = false;
    let mut firefox_m8_selection_owner_changes = 0usize;
    let mut firefox_m8_selection_conversions = 0usize;
    let mut first_protocol_error = None;
    let mut emergency_exit_requested = false;
    let mut return_suppressed_reported = false;
    let mut cursor_dirty = pointer.position.is_some();
    let mut cursor_dirty_since = cursor_dirty.then_some(Instant::now());
    let mut cursor_moves_coalesced = 0_u64;
    let mut cursor_max_motion_to_submit = Duration::ZERO;
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
                            physical_pointer_buttons_routed,
                        ),
                        pointer_buttons_only: false,
                        routing_mode: $routing_mode,
                        next_input_delivery: &mut next_input_delivery,
                        physical_text_proof: physical_text_proof.as_mut(),
                    },
                )?;
                physical_events = physical_events.saturating_add(report.events);
                physical_keys_routed = physical_keys_routed.saturating_add(report.keys_routed);
                physical_pointer_events =
                    physical_pointer_events.saturating_add(report.pointer_events);
                physical_pointer_routed =
                    physical_pointer_routed.saturating_add(report.pointer_routed);
                physical_pointer_buttons_routed =
                    physical_pointer_buttons_routed.saturating_add(report.pointer_buttons_routed);
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
                        cursor_moves_coalesced =
                            cursor_moves_coalesced.saturating_add(pointer_motions_observed as u64);
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
                        physical_pointer_buttons_routed
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
        if !primary_child_exited
            && let Some(primary_child) = child.as_deref_mut()
            && let Some(status) = primary_child.try_wait()?
        {
            primary_exit_status = Some(status);
            if status.success()
                && config.expect_physical_pointer
                && physical_pointer_buttons_routed == 0
            {
                return Err(
                    "session client exited before the required physical pointer selection".into(),
                );
            }
            if config.application_proof_requested() {
                client_stdout = client_stdout_capture
                    .ok_or("application stdout capture is missing")?
                    .read_bounded()?;
                if client_stdout.len() > 4_096 {
                    return Err("application stdout exceeded the 4096-byte evidence bound".into());
                }
                if let (Some(text), Some(expected)) = (
                    config.inject_text.as_deref(),
                    config.expect_client_stdout.as_deref(),
                ) && client_stdout == expected.as_bytes()
                {
                    input_text_match = true;
                    println!(
                        "sophia_live_session_input schema=3 status=semantic_complete source=synthetic text_match=true bytes={}",
                        text.len()
                    );
                }
            }
            if config.normal_session {
                if let Some(id) = config.applications.startup.first() {
                    println!(
                        "sophia_session_app schema=1 status=exited id={id} source=startup exit_status={status}",
                    );
                }
                primary_child_exited = true;
                if config.exit_when_startup_exits {
                    break;
                }
            } else {
                if status.success()
                    && successful_primary_exit_ends_session(config.input_proof_requested())
                {
                    break;
                }
                if !status.success() {
                    return Err(format!(
                        "session client exited during live session with status {status}"
                    )
                    .into());
                }
                // The proof helper intentionally exits after displaying its
                // received text. Keep the session and secondary terminal alive so
                // the final native frame can retire and pointer evidence can run.
                primary_child_exited = true;
            }
        }
        if config.application_proof_requested()
            && !input_text_match
            && physical_text_proof
                .as_ref()
                .is_some_and(PhysicalTextProof::is_complete)
        {
            input_text_match = true;
            println!(
                "sophia_live_session_input schema=3 status=semantic_complete source=physical text_match=true bytes={}",
                config.expect_physical_text.as_ref().map_or(0, String::len)
            );
        }
        let mut secondary_index = 0;
        while secondary_index < secondary_children.len() {
            if let Some(status) = secondary_children[secondary_index].child.try_wait()? {
                if config.normal_session {
                    terminate_session_child(&mut secondary_children[secondary_index].child, true)?;
                    let id = secondary_children[secondary_index]
                        .id
                        .as_deref()
                        .unwrap_or("untracked");
                    println!(
                        "sophia_session_app schema=1 status=exited id={id} source=managed exit_status={status}",
                    );
                    secondary_children.remove(secondary_index);
                } else {
                    return Err(format!(
                        "secondary xterm {} exited during live session with status {status}",
                        secondary_index + 1
                    )
                    .into());
                }
            } else {
                secondary_index += 1;
            }
        }
        drain_input_deliveries!();
        if emergency_exit_requested && pending_input_deliveries.is_empty() {
            break;
        }
        if !input_text_match
            && let (Some(expected), Some(result)) = (
                config
                    .inject_text
                    .as_deref()
                    .or(config.expect_physical_text.as_deref()),
                input_proof_result,
            )
            && let Some(received) = result.received()?
        {
            if received != expected.as_bytes() {
                return Err(format!(
                    "persistent live session terminal received incorrect input: expected_bytes={} received_bytes={}",
                    expected.len(),
                    received.len(),
                )
                .into());
            }
            input_text_match = true;
            println!(
                "sophia_live_session_input schema=3 status=semantic_complete source={} text_match=true bytes={}",
                if config.inject_text.is_some() {
                    "synthetic"
                } else {
                    "physical"
                },
                received.len(),
            );
            std::io::stdout().flush()?;
        }
        if let Some(post_input_deadline) = post_input_deadline
            && Instant::now() >= post_input_deadline
            && !input_text_match
        {
            return Err(
                "persistent live session timed out waiting for the terminal to receive exact text and Return"
                    .into(),
            );
        }
        if input_presented_latency.is_none()
            && let Some(post_input_deadline) = post_input_deadline
            && Instant::now() >= post_input_deadline
        {
            if !input_pixel_change {
                return Err(format!(
                    "persistent live session timed out waiting for pixels after flushed X11 input: expected={input_events_expected} flushed={input_events_flushed} authority_batches_after_input={} cpu_updates_after_input={} baseline_checksum={injection_checksum:?} final_checksum={:?} baseline_generation={input_surface_generation:?} final_generation={:?} input_surface_pixel_change={input_surface_pixel_change} native_submission_baseline={input_change_submission_baseline:?} native_submissions={} native_callbacks={}",
                    batches.saturating_sub(input_batch_baseline.unwrap_or(batches)),
                    cpu_buffer_updates.saturating_sub(input_cpu_update_baseline.unwrap_or(cpu_buffer_updates)),
                    scene.last_report().map(|report| report.checksum),
                    input_surface.and_then(|surface| {
                        runtime.as_ref().and_then(|runtime| {
                            scene.surface_buffer_generation(runtime.committed_surfaces(), surface)
                        })
                    }),
                    native_scanout.as_ref().map_or(0, |native| native.submissions),
                    native_scanout.as_ref().map_or(0, |native| native.callback_accepted),
                )
                .into());
            }
            return Err("persistent live session input pixels were not presented within the post-flush proof window".into());
        }
        if (post_input_deadline.is_none() || input_presented_latency.is_some())
            && deadline.is_some_and(|deadline| Instant::now() >= deadline)
        {
            if config.input_proof_requested() && injection_checksum.is_none() {
                return Err(
                    "persistent live session startup budget elapsed before a focused terminal frame was ready for input proof"
                        .into(),
                );
            }
            // The global runtime budget bounds startup. Once input has been
            // injected, its delivery and pixel/semantic stages own narrower
            // explicit deadlines. Ending here can strand already-routed keys
            // without giving the frontend a chance to acknowledge them.
            if global_runtime_deadline_ends_session(config.input_proof_requested()) {
                break;
            }
        }
        let input_routing_mode = physical_input_routing_mode(
            primary_child_exited,
            focus.focused_surface(seat),
            input_surface,
            wm_session.as_ref().is_some_and(|wm| wm.shortcuts.is_some()),
        );
        if input_routing_mode != PhysicalInputRoutingMode::Suppressed
            && drain_physical_input!(input_routing_mode)
        {
            break;
        }
        if let (Some(runtime), Some(native_scanout)) = (runtime.as_mut(), native_scanout.as_mut()) {
            let service = runtime.service_native(native_scanout)?;
            if let Some(retired) = service.retired_present {
                let stable = runtime.stable_present(native_scanout, retired.transaction);
                retired_present_surfaces.insert(retired.surface, retired.transaction);
                println!(
                    "sophia_live_session_present schema=1 status=retired transaction={} surface={}",
                    retired.transaction.raw(),
                    retired.surface.index(),
                );
                println!(
                    "sophia_live_session_scanout schema=1 status={} kind=mixed transaction={} pending_primary={}",
                    if stable { "stable" } else { "superseded" },
                    retired.transaction.raw(),
                    !stable,
                );
            }
            runtime_surfaces =
                u64::try_from(runtime.committed_surfaces().len()).unwrap_or(u64::MAX);
            reconcile_initial_session_focus(InitialSessionFocusContext {
                runtime,
                focus: &mut focus,
                seat,
                wm_session_present: wm_session.is_some(),
                layout: &layout,
                control_sender,
                next_focus_control_transaction: &mut next_focus_control_transaction,
                focused_client_control: &mut focused_client_control,
            })?;
        }
        if cursor_dirty
            && let (Some(native_scanout), Some(position)) =
                (native_scanout.as_mut(), pointer.position)
        {
            match native_scanout.update_classic_hardware_cursor(position) {
                Ok(ClassicHardwareCursorUpdate::Visible) => {
                    pointer_pixel_change |= physical_pointer_routed > 0;
                    if let Some(started) = cursor_dirty_since.take() {
                        cursor_max_motion_to_submit =
                            cursor_max_motion_to_submit.max(started.elapsed());
                    }
                    cursor_dirty = false;
                    if pointer_checksum.is_none() {
                        pointer_checksum = Some(0);
                        println!(
                            "sophia_live_session_pointer schema=2 status=visible source=hardware_cursor"
                        );
                    }
                }
                Ok(ClassicHardwareCursorUpdate::Hidden) => {
                    cursor_dirty = false;
                }
                Ok(ClassicHardwareCursorUpdate::Deferred) => {}
                Err(error) => {
                    eprintln!(
                        "sophia_live_session_pointer schema=2 status=unavailable source=hardware_cursor error={error}"
                    );
                    return Err(format!(
                        "native session cannot provide an owned atomic cursor: {error}"
                    )
                    .into());
                }
            }
        }
        if let Some(candidate) = pointer_cursor_checksum
            && native_scanout.as_ref().is_none_or(|native| {
                native.heads.first().is_some_and(|head| {
                    head.presented_checksum == candidate && head.nonzero_exports > 0
                })
            })
        {
            pointer_checksum = Some(candidate);
            pointer_cursor_checksum = None;
            pointer_phase_started_at = Some(Instant::now());
            println!(
                "sophia_live_session_pointer schema=1 status=visible source=physical position=center"
            );
            println!(
                "sophia_live_session_pointer schema=1 status=ready source=physical action=select"
            );
            std::io::stdout().flush()?;
        }

        if let (Some(runtime), Some(surface)) = (runtime.as_ref(), focus.focused_surface(seat))
            && let Some(committed) = runtime
                .committed_surfaces()
                .iter()
                .find(|committed| committed.surface == surface)
        {
            let cpu_visual_detail =
                scene.surface_has_visual_detail(runtime.committed_surfaces(), surface);
            let retired_gpu_detail =
                retired_present_surfaces
                    .get(&surface)
                    .is_some_and(|transaction| {
                        matches!(committed.buffer, BufferSource::DmaBuf { .. })
                            && native_scanout
                                .as_ref()
                                .is_some_and(|native| runtime.stable_present(native, *transaction))
                    });
            if !startup_content_ready && (cpu_visual_detail || retired_gpu_detail) {
                startup_content_ready = true;
                println!(
                    "sophia_live_session_startup schema=1 status=content_ready source={}",
                    if retired_gpu_detail {
                        "stable_present_scanout"
                    } else {
                        "cpu_visual_detail"
                    }
                );
                std::io::stdout().flush()?;
            }
        }
        let focused_surface = focus.focused_surface(seat);
        let startup_frame_presented = native_scanout.as_ref().is_none_or(|native| {
            focused_surface.is_some_and(|surface| {
                retired_present_surfaces
                    .get(&surface)
                    .is_some_and(|transaction| {
                        runtime
                            .as_ref()
                            .is_some_and(|runtime| runtime.stable_present(native, *transaction))
                    })
            }) || startup_required_submissions
                .as_ref()
                .is_some_and(|required| {
                    native
                        .heads
                        .iter()
                        .zip(required)
                        .next()
                        .is_some_and(|(head, required)| head.presented_submissions >= *required)
                })
        });
        if !startup_ready_reported
            && config.startup_ready_timeout.is_some()
            && focus.focused_surface(seat).is_some()
            && focused_client_ready
            && startup_content_ready
            && startup_frame_presented
        {
            startup_ready_reported = true;
            startup_ready_msec.get_or_insert_with(|| started.elapsed().as_millis());
            println!(
                "sophia_live_session_startup schema=1 status=ready elapsed_msec={} surface=true visual_detail=true presented=true",
                started.elapsed().as_millis()
            );
            std::io::stdout().flush()?;
        }
        if !startup_ready_reported
            && startup_ready_deadline.is_some_and(|deadline| Instant::now() >= deadline)
        {
            let stage = if layout.layers.is_empty() {
                "no_surface"
            } else if runtime
                .as_ref()
                .is_none_or(|runtime| runtime.committed_surfaces().is_empty())
            {
                "not_committed"
            } else if focus.focused_surface(seat).is_none() {
                "not_focused"
            } else if !focused_client_ready {
                "focus_control_pending"
            } else if !startup_content_ready {
                "no_visual_detail"
            } else {
                "not_presented"
            };
            if let Some(native) = native_scanout.as_ref() {
                for head in &native.heads {
                    if let Some(report) = head.last_submit_report {
                        eprintln!("{}", report.reduced_log_line());
                    }
                }
            }
            eprintln!(
                "sophia_live_session_startup schema=3 status=failed stage={stage} elapsed_msec={} authority_batches={batches} transactions={transactions} layout_surfaces={} runtime_surfaces={runtime_surfaces} focus={} focus_control_ready={focused_client_ready} retired_present_surfaces={} dma_buf_registrations={dma_buf_registrations_observed} fence_registrations={fence_registrations_observed} present_submissions={present_submissions_observed} native_submissions={} native_submit_failures={} native_retirements={} native_callbacks={} native_state={} protocol_errors={protocol_error_count}",
                started.elapsed().as_millis(),
                layout.layers.len(),
                focus.focused_surface(seat).is_some(),
                retired_present_surfaces.len(),
                native_scanout
                    .as_ref()
                    .map_or(0, |native| native.submissions),
                native_scanout
                    .as_ref()
                    .map_or(0, |native| native.submit_failures),
                native_scanout
                    .as_ref()
                    .map_or(0, |native| native.retirements),
                native_scanout
                    .as_ref()
                    .map_or(0, |native| native.callback_accepted),
                runtime.as_ref().map_or_else(
                    || "none".to_owned(),
                    LiveProductionVisualRuntime::native_diagnostic
                ),
            );
            return Err(format!(
                "startup application was not visibly presented within {} milliseconds: stage={stage}",
                config
                    .startup_ready_timeout
                    .expect("startup deadline requires a timeout")
                    .as_millis()
            )
            .into());
        }

        let input_baseline_presented_before_wait = scene.last_report().is_some_and(|report| {
            report.nonzero_pixel_bytes > 0
                && native_scanout.as_ref().is_none_or(|native| {
                    native.heads.first().is_some_and(|head| {
                        head.presented_checksum == report.checksum && head.nonzero_exports > 0
                    })
                })
        });
        if input_presented_latency.is_none()
            && input_pixel_change
            && let Some(started) = input_proof_started_at
            && native_scanout.as_ref().is_none_or(|native| {
                input_change_submission_baseline.is_some_and(|baseline| {
                    native
                        .heads
                        .first()
                        .is_some_and(|head| head.presented_submissions > baseline)
                })
            })
        {
            input_presented_latency = Some(started.elapsed());
        }
        if require_startup_focus
            && focus.focused_surface(seat).is_none()
            && focus_deadline_started_at
                .is_some_and(|started: Instant| started.elapsed() >= Duration::from_secs(5))
        {
            return Err(
                "live-session input focus was not ready within five seconds of the first presented frame"
                    .into(),
            );
        }
        let physical_sequence_complete = physical_text_proof
            .as_ref()
            .is_none_or(PhysicalTextProof::is_complete);
        let waiting_for_keyboard_sequence =
            physical_input_ready_at.is_some() && !physical_sequence_complete;
        let waiting_for_pointer_selection = sophia_cli::input_proof::pointer_selection_waiting(
            config.expect_physical_pointer,
            physical_sequence_complete,
            input_pixel_change,
            pointer_checksum.is_some(),
            physical_pointer_buttons_routed,
            pointer_pixel_change,
        );
        if waiting_for_keyboard_sequence {
            let ready_at = physical_input_ready_at.expect("checked above");
            if ready_at.elapsed() >= Duration::from_millis(SESSION_PHYSICAL_SEQUENCE_TIMEOUT_MSEC) {
                let proof = physical_text_proof.as_ref().expect("checked above");
                return Err(format!(
                    "persistent live session timed out waiting for exact physical input sequence: matched_events={} expected_events={} keyboard_routed={physical_keys_routed}",
                    proof.matched_events(),
                    proof.expected_events(),
                )
                .into());
            }
        } else if waiting_for_pointer_selection {
            let started_at = pointer_phase_started_at.expect("set above");
            if started_at.elapsed() >= Duration::from_millis(SESSION_PHYSICAL_SEQUENCE_TIMEOUT_MSEC)
            {
                return Err(format!(
                    "persistent live session timed out waiting for a routed physical pointer button: pointer_observed={physical_pointer_events} pointer_routed={physical_pointer_routed} pointer_buttons={physical_pointer_buttons_routed} pointer_pixels={pointer_pixel_change}"
                )
                .into());
            }
        } else if input_delivery_wait_started_at.is_none()
            && (input_proof_started_at.is_none() || input_presented_latency.is_some())
        {
            if config
                .max_ticks
                .is_some_and(|max_ticks| session_ticks >= max_ticks)
            {
                break;
            }
            session_ticks = session_ticks.saturating_add(1);
        }

        let authority_batch = initial_authority_batch
            .take()
            .or_else(|| pending_authority_batches.pop_front())
            .map_or_else(
                || {
                    authority_receiver.recv_timeout(if cursor_dirty {
                        Duration::from_millis(1)
                    } else {
                        Duration::from_millis(25)
                    })
                },
                Ok,
            );
        match authority_batch {
            Ok(batch) => {
                let drain_started = Instant::now();
                while pending_authority_batches.len() < 64
                    && drain_started.elapsed() < Duration::from_millis(2)
                {
                    match authority_receiver.try_recv() {
                        Ok(queued) => pending_authority_batches.push_back(queued),
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            return Err(
                                "persistent X authority transaction channel disconnected".into()
                            );
                        }
                    }
                }
                let defer_cpu_frame = runtime.is_some()
                    && software_batch_may_coalesce(&batch)
                    && pending_authority_batches
                        .front()
                        .is_some_and(software_batch_may_coalesce);
                for error in &batch.protocol_errors {
                    protocol_error_count = protocol_error_count.saturating_add(1);
                    first_protocol_error.get_or_insert(*error);
                }
                expected_protocol_error_count = expected_protocol_error_count
                    .saturating_add(batch.expected_protocol_errors.len());
                if config.firefox_m8_proof {
                    if batch.selection_owner_change {
                        firefox_m8_selection_owner_changes =
                            firefox_m8_selection_owner_changes.saturating_add(1);
                        println!(
                            "sophia_firefox_m8 schema=1 status=selection_observed kind=owner_change count={firefox_m8_selection_owner_changes} content=redacted"
                        );
                    }
                    if batch.selection_conversion {
                        firefox_m8_selection_conversions =
                            firefox_m8_selection_conversions.saturating_add(1);
                        println!(
                            "sophia_firefox_m8 schema=1 status=selection_observed kind=conversion count={firefox_m8_selection_conversions} content=redacted"
                        );
                    }
                    for metadata in &batch.metadata {
                        if !firefox_m8_page_ready_reported
                            && metadata.property_name == "_NET_WM_NAME"
                            && metadata.byte_len == 36
                        {
                            firefox_m8_page_ready_reported = true;
                            println!(
                                "sophia_firefox_m8 schema=1 status=page_ready title_bytes=36 content=redacted"
                            );
                        }
                        for (stage, index, title_bytes) in
                            firefox_m8_proof.observe(&metadata.property_name, metadata.byte_len)
                        {
                            println!(
                                "sophia_firefox_m8 schema=1 status=stage_complete stage={stage} index={index} title_bytes={} content=redacted",
                                title_bytes,
                            );
                        }
                    }
                }
                let has_engine_work = !batch.transactions.is_empty()
                    || !batch.removed_surfaces.is_empty()
                    || !batch.cpu_buffer_updates.is_empty()
                    || !batch.dma_buf_registrations.is_empty()
                    || !batch.fence_registrations.is_empty()
                    || !batch.present_submissions.is_empty()
                    || !batch.released_dma_bufs.is_empty()
                    || !batch.released_fences.is_empty();
                if !has_engine_work {
                    continue;
                }
                last_authority_update = Instant::now();
                batches = batches.saturating_add(1);
                transactions =
                    transactions.saturating_add(authority_transaction_count(&batch.transactions));
                cpu_buffer_updates =
                    cpu_buffer_updates.saturating_add(batch.cpu_buffer_updates.len());
                dma_buf_registrations_observed = dma_buf_registrations_observed
                    .saturating_add(batch.dma_buf_registrations.len());
                fence_registrations_observed =
                    fence_registrations_observed.saturating_add(batch.fence_registrations.len());
                present_submissions_observed =
                    present_submissions_observed.saturating_add(batch.present_submissions.len());
                let removed_surfaces = batch.removed_surfaces.clone();
                if let Some(wm_session) = wm_session.as_mut() {
                    for surface in &removed_surfaces {
                        wm_session.notify_surface_removed(*surface)?;
                    }
                }
                let _ = layout.observe_authority_batch(&batch);
                let mut wm_update = layout.resolve_pending();
                if !resize_proof_complete
                    && let Some((transaction, surface, size)) = resize_proof
                    && layout.pending.is_none()
                    && layout.resize.committed_size(surface) == Some(size)
                {
                    println!(
                        "sophia_live_resize schema=1 status=committed transaction={} surface={} width={} height={} configure_ack=true pixels=true",
                        transaction.raw(),
                        surface.index(),
                        size.width,
                        size.height,
                    );
                    resize_proof_complete = true;
                }
                if wm_update.is_none() {
                    wm_update = layout.expire_pending(control_sender, control_ack_receiver)?;
                }
                if layout.pending.is_none()
                    && let Some(wm_session) = wm_session.as_mut()
                {
                    if let Some(proposal) = wm_session.poll_restart(&layout, output)? {
                        wm_update = layout.stage(proposal, control_sender, control_ack_receiver)?;
                    }
                }
                if resize_proof.is_none()
                    && let Some(size) = config.inject_surface_resize
                    && layout.layers.len() >= if config.secondary_terminal { 2 } else { 1 }
                    && layout.pending.is_none()
                {
                    let surface = layout
                        .layers
                        .keys()
                        .next()
                        .copied()
                        .ok_or("surface resize proof has no target")?;
                    let transaction = TransactionId::from_raw(2_000_000);
                    let mut layers = layout.layers.values().cloned().collect::<Vec<_>>();
                    let layer = layers
                        .iter_mut()
                        .find(|layer| layer.surface == surface)
                        .ok_or("surface resize proof lost its target")?;
                    layer.geometry.width = size.width;
                    layer.geometry.height = size.height;
                    let proposal = LiveWmProposal {
                        transaction,
                        layers,
                        requested_sizes: BTreeMap::from([(surface, size)]),
                        focus: None,
                        timeout: Duration::from_secs(2),
                        update: WmTransactionUpdate {
                            commit: TransactionCommit {
                                transaction,
                                outcome: TransactionOutcome::Committed,
                                applied_surfaces: vec![surface],
                            },
                            ipc_error: None,
                        },
                        moved_surfaces: 0,
                        effects: None,
                    };
                    wm_update = layout.stage(proposal, control_sender, control_ack_receiver)?;
                    resize_proof = Some((transaction, surface, size));
                    println!(
                        "sophia_live_resize schema=1 status=requested transaction={} surface={} width={} height={}",
                        transaction.raw(),
                        surface.index(),
                        size.width,
                        size.height,
                    );
                }
                let wm_update = wm_update.map(|mut result| {
                    if result.update.commit.outcome == TransactionOutcome::Committed
                        && let Some(effects) = result.effects.take()
                        && let Some(wm_session) = wm_session.as_mut()
                    {
                        wm_session.workspace_state = effects.workspace_state;
                        wm_session.mark_committed();
                        if let Some(action) = effects.session_action {
                            committed_session_actions.push_back((
                                effects.transaction,
                                action.0,
                                action.1,
                            ));
                        }
                    }
                    result.update
                });
                let batch = layout.projected_batch(&batch);
                let production_batch = production_authority_batch(&batch);
                if runtime.is_none() {
                    runtime = Some(
                        LiveProductionVisualRuntime::new(&outputs, native_scanout.as_mut(), None)?
                            .with_m4_proof_controls(
                                config.m4_first_acquire_delay,
                                config.m4_reject_first_present,
                                config.m4_diagnose_first_mixed_export,
                            ),
                    );
                }
                let runtime = runtime
                    .as_mut()
                    .expect("persistent backend runtime was initialized above");
                let raised_surface = focus.focused_surface(seat);
                let updates = batch
                    .cpu_buffer_updates
                    .iter()
                    .map(renderer_cpu_buffer_update)
                    .collect::<Vec<_>>();
                let cursor_presentation = if native_scanout.is_some() {
                    LiveProductionCursorPresentation::HardwarePlane
                } else {
                    LiveProductionCursorPresentation::Software(pointer.position)
                };
                let (_tick, report, committed_surfaces, composed, compose_elapsed) =
                    if batch.present_submissions.is_empty() {
                        let (submission, committed_surfaces) = runtime.run_cpu_production_cycle(
                            &production_batch,
                            &mut scene,
                            updates,
                            raised_surface,
                            cursor_presentation,
                            defer_cpu_frame,
                            &outputs,
                            if defer_cpu_frame {
                                None
                            } else {
                                native_scanout.as_mut()
                            },
                            wm_update,
                        )?;
                        (
                            submission.tick,
                            submission.composition,
                            committed_surfaces,
                            submission.composed,
                            submission.compose_elapsed,
                        )
                    } else {
                        let (submission, committed_surfaces) = runtime.run_gpu_production_cycle(
                            &production_batch,
                            &mut scene,
                            updates,
                            raised_surface,
                            cursor_presentation,
                            defer_cpu_frame,
                            &outputs,
                            if defer_cpu_frame {
                                None
                            } else {
                                native_scanout.as_mut()
                            },
                            wm_update,
                        )?;
                        (
                            submission.tick,
                            submission.composition,
                            committed_surfaces,
                            submission.composed,
                            submission.compose_elapsed,
                        )
                    };
                if composed {
                    max_compose = max_compose.max(compose_elapsed);
                    cpu_compositions = cpu_compositions.saturating_add(1);
                } else {
                    coalesced_batches = coalesced_batches.saturating_add(1);
                }
                if let (Some(surface), Some(before_surface)) =
                    (input_surface, input_surface_generation)
                    && scene
                        .surface_buffer_generation(&committed_surfaces, surface)
                        .is_some_and(|generation| generation != before_surface)
                {
                    input_surface_pixel_change = true;
                }
                if let Some(before_frame) = injection_checksum
                    && report.checksum != before_frame
                    && (config.expect_physical_text.is_none()
                        || physical_sequence_completed_at.is_some())
                {
                    input_pixel_change = true;
                }
                if let Some(before_frame) = pointer_checksum
                    && report.checksum != before_frame
                    && physical_pointer_routed > 0
                {
                    pointer_pixel_change = true;
                }
                backend_ticks = backend_ticks.saturating_add(1);
                runtime_committed = record_runtime_commits(
                    runtime_committed,
                    authority_transaction_count(&batch.transactions),
                );
                runtime_surfaces =
                    u64::try_from(runtime.committed_surfaces().len()).unwrap_or(u64::MAX);
                for surface in removed_surfaces {
                    if config.application_proof_requested()
                        && physical_pointer_buttons_routed == 0
                        && Some(surface) == input_surface
                    {
                        application_surface_missing_since.get_or_insert_with(Instant::now);
                    }
                    if config.application_proof_requested() && Some(surface) == input_surface {
                        application_surface_gone_at.get_or_insert_with(Instant::now);
                    }
                    focus.clear_surface(surface);
                }
                if let Some(surface) = input_surface
                    && runtime
                        .committed_surfaces()
                        .iter()
                        .any(|committed| committed.surface == surface)
                {
                    application_surface_missing_since = None;
                    application_surface_gone_at = None;
                }
                reconcile_initial_session_focus(InitialSessionFocusContext {
                    runtime,
                    focus: &mut focus,
                    seat,
                    wm_session_present: wm_session.is_some(),
                    layout: &layout,
                    control_sender,
                    next_focus_control_transaction: &mut next_focus_control_transaction,
                    focused_client_control: &mut focused_client_control,
                })?;
                if let Some((transaction, surface)) = layout.focus_to_apply.take() {
                    let decision = focus.focus_surface(seat, surface, runtime.committed_surfaces());
                    if decision == InputFocusDecision::Focused && wm_session.is_some() {
                        let client = layout
                            .client_routes
                            .client_for_surface(surface)
                            .ok_or("WM focus has no X11 client route")?;
                        control_sender.try_send(XAuthorityClientControlCommand {
                            client,
                            command: XAuthorityControlCommand::FocusSurface {
                                transaction,
                                surface,
                            },
                        })?;
                        let acknowledgement =
                            control_ack_receiver.recv_timeout(Duration::from_millis(500))?;
                        if acknowledgement.client != client
                            || acknowledgement.acknowledgement.transaction != transaction
                            || acknowledgement.acknowledgement.surface != surface
                            || acknowledgement.acknowledgement.outcome
                                != XAuthorityControlOutcome::Delivered
                        {
                            return Err("X Authority rejected WM focus reconciliation".into());
                        }
                    }
                    println!(
                        "sophia_live_wm schema=1 status=focus_reconciled transaction={} target=surface surface={surface:?} outcome={decision:?}",
                        transaction.raw()
                    );
                    if decision == InputFocusDecision::Focused {
                        println!(
                            "sophia_live_wm schema=1 status=focus_committed transaction={} target=surface",
                            transaction.raw()
                        );
                    }
                }
                if !focus_ready_reported && focus.focused_surface(seat).is_some() {
                    println!("sophia_live_session_input_pipeline schema=1 status=focus_ready");
                    std::io::stdout().flush()?;
                    focus_ready_reported = true;
                    focus_ready_at = Some(Instant::now());
                }
                if let Some(surface) = focus.focused_surface(seat) {
                    let cpu_visual_detail =
                        scene.surface_has_visual_detail(runtime.committed_surfaces(), surface);
                    if !startup_content_ready && cpu_visual_detail {
                        startup_content_ready = true;
                        startup_required_submissions = native_scanout.as_ref().map(|native| {
                            native
                                .heads
                                .iter()
                                .map(|head| {
                                    head.submissions
                                        .max(head.presented_submissions.saturating_add(1))
                                })
                                .collect()
                        });
                        println!(
                            "sophia_live_session_startup schema=1 status=content_ready source=cpu_visual_detail"
                        );
                        std::io::stdout().flush()?;
                    }
                    if !terminal_content_ready && cpu_visual_detail {
                        terminal_content_ready = true;
                        startup_ready_msec = Some(started.elapsed().as_millis());
                        println!(
                            "sophia_live_session_input_pipeline schema=1 status=terminal_content_ready"
                        );
                        std::io::stdout().flush()?;
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                let _ = layout.expire_pending(control_sender, control_ack_receiver)?;
                if layout.pending.is_none()
                    && let Some(wm_session) = wm_session.as_mut()
                    && let Some(proposal) = wm_session.poll_restart(&layout, output)?
                {
                    let _ = layout.stage(proposal, control_sender, control_ack_receiver)?;
                }
                if layout.pending.is_none()
                    && last_authority_update.elapsed()
                        >= Duration::from_millis(config.input_quiet_msec)
                    && let Some(wm_session) = wm_session.as_mut()
                {
                    if let Some(surface) = layout.take_next_unmanaged_surface() {
                        let proposal = wm_session.request_manage(surface, &layout, output)?;
                        if layout
                            .stage(proposal, control_sender, control_ack_receiver)?
                            .is_some()
                        {
                            wm_session.mark_committed();
                        }
                    }
                }
                if let (Some(runtime), Some(native_scanout)) =
                    (runtime.as_mut(), native_scanout.as_mut())
                {
                    let service = runtime.service_native(native_scanout)?;
                    if let Some(retired) = service.retired_present {
                        let stable = runtime.stable_present(native_scanout, retired.transaction);
                        retired_present_surfaces.insert(retired.surface, retired.transaction);
                        println!(
                            "sophia_live_session_present schema=1 status=retired transaction={} surface={}",
                            retired.transaction.raw(),
                            retired.surface.index(),
                        );
                        println!(
                            "sophia_live_session_scanout schema=1 status={} kind=mixed transaction={} pending_primary={}",
                            if stable { "stable" } else { "superseded" },
                            retired.transaction.raw(),
                            !stable,
                        );
                    }
                    if let Some(tick) = service.tick {
                        backend_ticks = backend_ticks.saturating_add(1);
                        let _ = tick;
                    }
                    runtime_surfaces =
                        u64::try_from(runtime.committed_surfaces().len()).unwrap_or(u64::MAX);
                    reconcile_initial_session_focus(InitialSessionFocusContext {
                        runtime,
                        focus: &mut focus,
                        seat,
                        wm_session_present: wm_session.is_some(),
                        layout: &layout,
                        control_sender,
                        next_focus_control_transaction: &mut next_focus_control_transaction,
                        focused_client_control: &mut focused_client_control,
                    })?;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("persistent X authority transaction channel disconnected".into());
            }
        }

        if !physical_input_completion_reported
            && input_pixel_change
            && input_text_match
            && let (Some(text), Some(proof)) = (
                config.expect_physical_text.as_deref(),
                physical_text_proof.as_ref(),
            )
            && proof.is_complete()
        {
            println!(
                "sophia_live_session_input schema=2 status=complete source=physical text={} expected_events={} matched_events={} pixel_change=true",
                text,
                proof.expected_events(),
                proof.matched_events(),
            );
            std::io::stdout().flush()?;
            physical_input_completion_reported = true;
        }

        if wm_session.is_none() {
            while let Ok(acknowledgement) = control_ack_receiver.try_recv() {
                let Some((transaction, surface)) = focused_client_control else {
                    continue;
                };
                if acknowledgement.acknowledgement.transaction != transaction
                    || acknowledgement.acknowledgement.surface != surface
                {
                    continue;
                }
                if acknowledgement.acknowledgement.outcome != XAuthorityControlOutcome::Delivered {
                    return Err(format!(
                        "initial X11 focus control was rejected: {:?}",
                        acknowledgement.acknowledgement.outcome
                    )
                    .into());
                }
                focused_client_control = None;
                focused_client_ready = true;
                println!(
                    "sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control"
                );
                std::io::stdout().flush()?;
            }
        }

        let input_baseline_presented = input_baseline_presented_before_wait
            || scene.last_report().is_some_and(|report| {
                report.nonzero_pixel_bytes > 0
                    && native_scanout.as_ref().is_none_or(|native| {
                        native.heads.first().is_some_and(|head| {
                            head.presented_checksum != 0 && head.nonzero_exports > 0
                        })
                    })
            });
        let input_start_stable = if config.expect_physical_text.is_some() {
            focus_ready_at.is_some_and(|ready| ready.elapsed() >= Duration::from_secs(2))
        } else {
            last_authority_update.elapsed() >= Duration::from_millis(config.input_quiet_msec)
                || wm_session.as_ref().is_some_and(|wm| {
                    wm.last_committed_at.is_some_and(|committed| {
                        committed.elapsed() >= Duration::from_millis(config.input_quiet_msec)
                    })
                })
        };
        if require_startup_focus
            && physical_input.is_some()
            && input_baseline_presented
            && focus_deadline_started_at.is_none()
        {
            focus_deadline_started_at = Some(Instant::now());
        }
        if injection_checksum.is_none()
            && config.input_proof_requested()
            && input_baseline_presented
            && input_start_stable
            && focused_client_ready
            && terminal_content_ready
        {
            injection_checksum = scene.last_report().map(|report| report.checksum);
            input_change_submission_baseline = native_scanout
                .as_ref()
                .and_then(|native| native.heads.first())
                .map(|head| head.presented_submissions);
            input_surface = focus.focused_surface(seat);
            input_surface_generation = input_surface.and_then(|surface| {
                runtime.as_ref().and_then(|runtime| {
                    scene.surface_buffer_generation(runtime.committed_surfaces(), surface)
                })
            });
            if let Some(text) = config.inject_text.as_deref() {
                let events = synthetic_text_input_events(text)?;
                let expected = events.len();
                let runtime = runtime
                    .as_ref()
                    .ok_or("synthetic routed input requires an initialized runtime")?;
                let report = route_input_events(
                    events,
                    &focus,
                    runtime.committed_surfaces(),
                    runtime.input_layers(),
                    &layout.client_routes,
                    input_sender,
                    &mut modifiers,
                    &mut emergency_chord,
                    None,
                    &mut pointer,
                    false,
                    false,
                    false,
                    PhysicalInputRoutingMode::Full,
                    &mut next_input_delivery,
                    None,
                )?;
                if report.keys_routed != expected {
                    return Err(format!(
                        "synthetic input did not traverse committed Engine focus: expected={expected} routed={}",
                        report.keys_routed
                    )
                    .into());
                }
                input_events_expected =
                    input_events_expected.saturating_add(report.deliveries.len());
                pending_input_deliveries.extend(report.deliveries.iter().copied());
                input_delivery_wait_started_at = Some(Instant::now());
                input_delivery_source = Some("synthetic");
                input_batch_baseline = Some(batches);
                input_cpu_update_baseline = Some(cpu_buffer_updates);
                if !key_routed_reported {
                    println!(
                        "sophia_live_session_input_pipeline schema=1 status=key_routed source=synthetic"
                    );
                    std::io::stdout().flush()?;
                    key_routed_reported = true;
                }
            } else {
                input_batch_baseline = Some(batches);
                input_cpu_update_baseline = Some(cpu_buffer_updates);
                physical_input_ready_at = Some(Instant::now());
                println!(
                    "sophia_live_session_input schema=1 status=ready source=physical text={}",
                    config
                        .expect_physical_text
                        .as_deref()
                        .expect("checked above")
                );
                std::io::stdout().flush()?;
            }
        }
        if config.expect_physical_pointer
            && physical_input_completion_reported
            && input_pixel_change
            && pointer_checksum.is_none()
            && pointer_cursor_checksum.is_none()
        {
            let runtime = runtime
                .as_ref()
                .ok_or("pointer proof became ready before the backend runtime")?;
            pointer
                .arm_at_focused_surface_center(focus.focused_surface(seat), runtime.input_layers())
                .ok_or("pointer proof has no focused application surface to place the cursor")?;
            cursor_dirty_since.get_or_insert_with(Instant::now);
            cursor_dirty = true;
        }
        if application_surface_missing_since
            .is_some_and(|started| started.elapsed() >= Duration::from_millis(500))
        {
            return Err(
                "application proof surface disappeared before the required physical pointer selection"
                    .into(),
            );
        }
        // Once the proof surface is gone, the session owns no narrower
        // deadline and the global runtime budget intentionally stays out of
        // input proofs. A toolkit that destroyed its window but never exits
        // would otherwise leave the loop presenting blank frames forever;
        // bound that wait and fail closed with the exact exit-term states.
        if sophia_cli::input_proof::application_exit_overdue(
            config.application_proof_requested(),
            application_surface_gone_at.is_some(),
            primary_child_exited,
        ) && application_surface_gone_at.is_some_and(|gone_at| {
            gone_at.elapsed() >= Duration::from_millis(SESSION_COMPLETION_TIMEOUT_MSEC)
        }) {
            return Err(format!(
                "persistent live session application surface was removed but the client did not exit: presented_latency={} text_match={} completion_reported={} pointer_pixels={} buttons_routed={} child_exited={}",
                input_presented_latency.is_some(),
                input_text_match,
                physical_input_completion_reported,
                pointer_pixel_change,
                physical_pointer_buttons_routed,
                primary_child_exited,
            )
            .into());
        }
        if execute_committed_session_actions(
            SessionActionExecutionContext {
                config,
                xauthority,
                children: secondary_children,
                layout: &layout,
                focus: &focus,
                seat,
                control_sender,
                control_ack_receiver,
            },
            &mut committed_session_actions,
        )? {
            logout_requested = true;
            let discarded = pending_input_deliveries.len();
            input_events_expected = input_events_expected.saturating_sub(discarded);
            pending_input_deliveries.clear();
            input_delivery_wait_started_at = None;
            if discarded != 0 {
                println!(
                    "sophia_live_session_input_pipeline schema=2 status=logout_discarded pending={discarded}"
                );
                std::io::stdout().flush()?;
            }
        }
        if logout_requested && pending_input_deliveries.is_empty() {
            break;
        }
        if input_presented_latency.is_none()
            && input_pixel_change
            && let Some(started) = input_proof_started_at
            && native_scanout.as_ref().is_none_or(|native| {
                input_change_submission_baseline.is_some_and(|baseline| {
                    native
                        .heads
                        .first()
                        .is_some_and(|head| head.presented_submissions > baseline)
                })
            })
        {
            input_presented_latency = Some(started.elapsed());
        }
        if let Some(runtime) = runtime.as_mut() {
            present_feedback.clear();
            runtime.drain_present_feedback_into(&mut present_feedback)?;
            for outcome in present_feedback.drain(..) {
                present_observer.observe_feedback(outcome);
            }
        }
        if (config.exit_after_input_proof || config.inject_text.is_some())
            && input_presented_latency.is_some()
            && input_text_match
            && (config.expect_physical_text.is_none() || physical_input_completion_reported)
            && (!config.expect_physical_pointer || pointer_pixel_change)
            && (!config.application_proof_requested() || primary_child_exited)
        {
            break;
        }
    }

    if let (Some(runtime), Some(native_scanout)) = (runtime.as_mut(), native_scanout.as_mut()) {
        runtime.drain_native_scanout(native_scanout, Duration::from_secs(2))?;
    }
    if let Some(runtime) = runtime.as_mut() {
        let report = runtime.shutdown_presentations();
        present_feedback.clear();
        runtime.drain_present_feedback_into(&mut present_feedback)?;
        for outcome in present_feedback.drain(..) {
            present_observer.observe_feedback(outcome);
        }
        present_observer.observe_disconnect(report);
    }
    if input_presented_latency.is_none()
        && input_pixel_change
        && let Some(started) = input_proof_started_at
        && native_scanout.as_ref().is_none_or(|native| {
            input_change_submission_baseline.is_some_and(|baseline| {
                native
                    .heads
                    .first()
                    .is_some_and(|head| head.presented_submissions > baseline)
            })
        })
    {
        input_presented_latency = Some(started.elapsed());
    }

    let report = scene
        .last_report()
        .ok_or("persistent live session received no composable X pixels")?;
    if config.input_proof_requested() && input_events_expected != input_events_flushed {
        return Err(format!(
            "persistent live session completed with unflushed X11 input: expected={input_events_expected} flushed={input_events_flushed} pending={}",
            pending_input_deliveries.len(),
        )
        .into());
    }
    if config.input_proof_requested() && input_flush_latency.is_none() {
        return Err("persistent live session input proof never observed flushed X11 input".into());
    }
    if config.input_proof_requested() && !input_pixel_change {
        return Err(format!(
            "persistent live session input did not change composed terminal pixels: baseline={injection_checksum:?} final_frame={} final_buffers={} input_surface={input_surface:?} input_surface_pixel_change={input_surface_pixel_change} batches={batches} transactions={transactions}",
            report.checksum,
            scene.buffer_checksum(),
        )
        .into());
    }
    if config.input_proof_requested() && input_presented_latency.is_none() {
        let native_heads = runtime.as_ref().map_or_else(
            || "none".to_owned(),
            LiveProductionVisualRuntime::native_diagnostic,
        );
        return Err(format!(
            "persistent live session input pixels were not presented: change_submission_baseline={input_change_submission_baseline:?} primary_presented_submissions={} native_submissions={} native_callbacks={} native_heads={native_heads}",
            native_scanout
                .as_ref()
                .and_then(|native| native.heads.first())
                .map_or(0, |head| head.presented_submissions),
            native_scanout.as_ref().map_or(0, |native| native.submissions),
            native_scanout
                .as_ref()
                .map_or(0, |native| native.callback_accepted),
        )
        .into());
    }
    if config.input_proof_requested() && !input_text_match {
        return Err(
            "persistent live session terminal did not receive the expected text and Return".into(),
        );
    }
    if config.expect_physical_text.is_some()
        && (!physical_text_proof
            .as_ref()
            .is_some_and(PhysicalTextProof::is_complete)
            || !physical_input_completion_reported)
    {
        return Err("persistent live session did not complete exact physical text proof".into());
    }
    if config.expect_physical_pointer
        && (!pointer_pixel_change || physical_pointer_buttons_routed == 0)
    {
        return Err(format!(
            "persistent live session pointer input did not change pixels: baseline={pointer_checksum:?} routed={physical_pointer_routed} buttons={physical_pointer_buttons_routed} observed={physical_pointer_events}"
        )
        .into());
    }
    if config.application_proof_requested() {
        let status =
            primary_exit_status.ok_or("application proof ended before the client exited")?;
        if config.require_client_normal_exit && !status.success() {
            return Err(format!("application did not exit normally: {status}").into());
        }
        if let Some(expected) = config.expect_client_stdout.as_deref()
            && client_stdout != expected.as_bytes()
        {
            return Err(format!(
                "application stdout mismatch: expected_bytes={} received_bytes={}",
                expected.len(),
                client_stdout.len()
            )
            .into());
        }
        if session_protocol_errors_are_fatal(false, true, protocol_error_count) {
            return Err(format!("application emitted {protocol_error_count} X protocol errors; first={first_protocol_error:?}").into());
        }
    }
    if session_protocol_errors_are_fatal(
        config.normal_session,
        config.application_proof_requested(),
        protocol_error_count,
    ) {
        return Err(format!(
            "normal session emitted {protocol_error_count} X protocol errors; first={first_protocol_error:?}"
        )
        .into());
    }
    if config.firefox_m8_proof {
        if !firefox_m8_proof.complete()
            || firefox_m8_selection_owner_changes < 2
            || firefox_m8_selection_conversions < 2
        {
            return Err(format!(
                "Firefox M8 proof incomplete: stages={}/{} selection_owner_changes={} selection_conversions={}",
                firefox_m8_proof.completed_stage,
                FirefoxM8StageProof::STAGES.len(),
                firefox_m8_selection_owner_changes,
                firefox_m8_selection_conversions,
            )
            .into());
        }
        println!(
            "sophia_firefox_m8 schema=1 status=complete stages={} selection_owner_changes={} selection_conversions={} content=redacted",
            firefox_m8_proof.completed_stage,
            firefox_m8_selection_owner_changes,
            firefox_m8_selection_conversions,
        );
    }
    if config.inject_surface_resize.is_some() && !resize_proof_complete {
        return Err(
            "persistent live session did not commit configured surface resize pixels".into(),
        );
    }
    if let Some(wm_session) = wm_session.as_ref()
        && wm_session.committed == 0
    {
        return Err("live session ended without a committed external WM layout".into());
    }
    if config.normal_session
        && (layout.pending.is_some()
            || !committed_session_actions.is_empty()
            || !pending_input_deliveries.is_empty()
            || wm_session.as_ref().is_some_and(|wm| wm.degraded))
    {
        return Err(format!(
            "normal session ended with pending work: wm={} actions={} input={} degraded={}",
            usize::from(layout.pending.is_some()),
            committed_session_actions.len(),
            pending_input_deliveries.len(),
            wm_session.as_ref().is_some_and(|wm| wm.degraded),
        )
        .into());
    }
    let input_stats = physical_input
        .as_ref()
        .map_or_else(Default::default, |input| input.stats());
    if let Some(input) = physical_input.as_ref() {
        let policy = input.policy_report();
        println!(
            "sophia_live_session_input_devices schema=1 source={} added={} removed={} active={} keyboards={} pointers={} touch={}",
            if policy.udev_managed { "udev" } else { "paths" },
            policy.devices_added,
            policy.devices_removed,
            policy.active_devices,
            policy.keyboards,
            policy.pointers,
            policy.touch_devices,
        );
    }
    let (
        native_target_creations,
        native_target_recreations,
        native_pipeline_creations,
        native_uploads,
        native_max_upload,
    ) = native_scanout.as_ref().map_or(
        (0, 0, 0, 0, Duration::ZERO),
        LiveProductionNativeScanout::persistent_render_metrics,
    );
    println!(
        "sophia_live_session_scheduler schema=1 authority_batches={batches} cpu_compositions={cpu_compositions} coalesced_batches={coalesced_batches}"
    );
    println!(
        "sophia_live_session_cursor schema=2 moves_coalesced={} max_motion_to_submit_msec={} buttons_routed={} hardware_updates={} hardware_failures={}",
        cursor_moves_coalesced,
        cursor_max_motion_to_submit.as_millis(),
        physical_pointer_buttons_routed,
        native_scanout
            .as_ref()
            .map_or(0, |scanout| scanout.cursor_updates),
        native_scanout
            .as_ref()
            .map_or(0, |scanout| scanout.cursor_update_failures),
    );
    println!(
        "sophia_live_session_health schema=1 status=clean protocol_errors={} pending_wm={} pending_actions={} pending_input={} wm_degraded={}",
        protocol_error_count,
        usize::from(layout.pending.is_some()),
        committed_session_actions.len(),
        pending_input_deliveries.len(),
        wm_session.as_ref().is_some_and(|wm| wm.degraded),
    );
    println!(
        "sophia_live_session_protocol_errors schema=1 expected={} unexpected={}",
        expected_protocol_error_count, protocol_error_count,
    );

    let present_observation = &present_observer;
    println!(
        "sophia_live_session schema=14 status=bounded_complete display={} elapsed_msec={} startup_ready_msec={} session_ticks={} authority_batches={} authority_transactions={} authority_queue_capacity={} authority_batches_dropped=0 backend_ticks={} runtime_committed={} runtime_surfaces={} cpu_layers={} cpu_nonzero_pixel_bytes={} cpu_max_nonzero_pixel_bytes={} cpu_nonzero_frames={} cpu_checksum={} cpu_max_compose_msec={} injected_input={} input_events_expected={} input_events_flushed={} input_flush_latency_msec={} input_pixel_change={} input_text_match={} input_presented_latency_msec={} input_dispatch_max_gap_msec={} input_queue_max_depth={} input_queue_dwell_max_msec={} physical_events={} physical_keys_routed={} pointer_pixel_change={} physical_pointer_events={} physical_pointer_routed={} pointer_proof={} native_presentation={} native_submissions={} native_submit_deferred={} native_submit_failures={} native_retirements={} native_retire_failures={} native_max_in_flight_ticks={} native_max_submit_to_page_flip_msec={} native_max_upload_msec={} native_target_creations={} native_target_recreations={} native_pipeline_creations={} native_frame_uploads={} native_callback_accepted={} native_callback_rejected={} native_callback_queue_saturated={} native_nonzero_exports={} native_mixed_exports={} native_export_attempts={} native_in_flight={} native_cleanup_pending={} physical_input={} wm_policy={} wm_requests={} wm_committed={} wm_restarts={} wm_degraded={} namespace_profile={} output_update={} output_notifications={} surface_resize={} present_complete_flip={} present_complete_skip={} present_idle={} present_idle_fence_triggers={} present_disconnect_sources={} present_disconnect_fences={} present_disconnect_failures={} present_live_sources={} present_live_fences={} present_live_transactions={} present_acquire_waits={} present_controlled_rejections={}",
        config.display,
        started.elapsed().as_millis(),
        startup_ready_msec.ok_or("persistent live session never reached startup readiness")?,
        session_ticks,
        batches,
        transactions,
        SESSION_AUTHORITY_CAPACITY,
        backend_ticks,
        runtime_committed,
        runtime_surfaces,
        report.layers_composed,
        report.nonzero_pixel_bytes,
        scene.max_nonzero_pixel_bytes(),
        scene.nonzero_frames(),
        report.checksum,
        max_compose.as_millis(),
        config.inject_text.is_some(),
        input_events_expected,
        input_events_flushed,
        input_flush_latency.map_or(0, |duration| duration.as_millis()),
        input_pixel_change,
        input_text_match,
        input_presented_latency
            .map(|latency| latency.as_millis().to_string())
            .unwrap_or_else(|| "none".to_owned()),
        input_stats.max_dispatch_gap_msec,
        input_stats.max_queue_depth,
        input_stats.max_queue_dwell_msec,
        physical_events,
        physical_keys_routed,
        pointer_pixel_change,
        physical_pointer_events,
        physical_pointer_routed,
        if config.expect_physical_pointer {
            "enabled"
        } else {
            "disabled"
        },
        if native_scanout.is_some() {
            "enabled"
        } else {
            "disabled"
        },
        native_scanout
            .as_ref()
            .map_or(0, |native| native.submissions),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.submit_deferred),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.submit_failures),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.retirements),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.retire_failures),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.max_in_flight_ticks),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.max_submit_to_page_flip.as_millis()),
        native_max_upload.as_millis(),
        native_target_creations,
        native_target_recreations,
        native_pipeline_creations,
        native_uploads,
        native_scanout
            .as_ref()
            .map_or(0, |native| native.callback_accepted),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.callback_rejected),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.callback_queue_saturated),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.nonzero_exports),
        native_scanout
            .as_ref()
            .map_or(0, LiveProductionNativeScanout::mixed_exports),
        native_scanout
            .as_ref()
            .map_or(0, LiveProductionNativeScanout::export_attempts),
        runtime
            .as_ref()
            .is_some_and(LiveProductionVisualRuntime::native_scanout_in_flight),
        runtime
            .as_ref()
            .is_some_and(LiveProductionVisualRuntime::native_cleanup_pending),
        if physical_input.is_some() {
            "enabled"
        } else {
            "disabled"
        },
        if wm_session.is_some() {
            "external"
        } else {
            "disabled"
        },
        wm_session.as_ref().map_or(0, |wm| wm.requests),
        wm_session.as_ref().map_or(0, |wm| wm.committed),
        wm_session.as_ref().map_or(0, |wm| wm.restarts),
        wm_session.as_ref().is_some_and(|wm| wm.degraded),
        match config.namespace_profile {
            NamespaceProfile::ClassicShared => "classic_shared",
            NamespaceProfile::Confined => "confined",
        },
        if config.inject_output_size.is_some() {
            "applied"
        } else {
            "disabled"
        },
        output_notifications,
        if resize_proof_complete {
            "committed"
        } else {
            "disabled"
        },
        present_observation.complete_flip,
        present_observation.complete_skip,
        present_observation.idle,
        present_observation.idle_fence_triggers,
        present_observation.disconnect_sources,
        present_observation.disconnect_fences,
        present_observation.disconnect_failures,
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.diagnostics().live_sources),
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.diagnostics().live_fences),
        runtime
            .as_ref()
            .map_or(0, |runtime| { runtime.diagnostics().live_presentations }),
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.diagnostics().acquire_waits),
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.diagnostics().controlled_rejections),
    );
    if let Some(runtime) = runtime.as_ref()
        && (present_observation.disconnect_failures != 0
            || runtime.diagnostics().live_sources != 0
            || runtime.diagnostics().live_fences != 0
            || runtime.diagnostics().live_presentations != 0
            || present_observation.idle
                != present_observation
                    .complete_flip
                    .saturating_add(present_observation.complete_skip))
    {
        return Err("persistent Present resources did not retire exactly once".into());
    }
    if let (Some(runtime), Some(native_scanout)) = (runtime.as_ref(), native_scanout.as_ref())
        && (native_scanout.submissions == 0
            || native_scanout.retirements == 0
            || native_scanout.nonzero_exports == 0
            || native_scanout.submit_failures != 0
            || native_scanout.retire_failures != 0
            || native_scanout.callback_rejected != 0
            || native_scanout.callback_queue_saturated != 0
            || native_scanout.vsync_overlap_rejections != 0
            || native_scanout.page_flip_phase_rejections != 0
            || runtime.native_scanout_in_flight()
            || runtime.native_cleanup_pending())
    {
        return Err(format!(
            "persistent native scanout did not submit, retire, and drain cleanly: overlap_rejections={} phase_rejections={}",
            native_scanout.vsync_overlap_rejections,
            native_scanout.page_flip_phase_rejections,
        )
        .into());
    }
    if let Some(native_scanout) = native_scanout.as_ref() {
        println!(
            "sophia_live_vsync schema=1 status=complete outputs={} overlap_rejections={} phase_rejections={} policy=page_flip_paced",
            native_scanout.heads.len(),
            native_scanout.vsync_overlap_rejections,
            native_scanout.page_flip_phase_rejections,
        );
        for head in &native_scanout.heads {
            println!(
                "sophia_live_output schema=1 status=complete output={} checksum={} submissions={} retirements={} callbacks={} nonzero_exports={}",
                head.output.id.raw(),
                head.last_checksum,
                head.submissions,
                head.retirements,
                head.callback_accepted,
                head.nonzero_exports,
            );
        }
        if native_scanout.heads.iter().any(|head| {
            head.submissions == 0
                || head.retirements == 0
                || head.callback_accepted == 0
                || head.nonzero_exports == 0
        }) {
            return Err(
                "one or more native outputs did not present and retire independently".into(),
            );
        }
        let mut checksums = native_scanout
            .heads
            .iter()
            .map(|head| head.last_checksum)
            .collect::<Vec<_>>();
        checksums.sort_unstable();
        checksums.dedup();
        if checksums.len() != native_scanout.heads.len() {
            return Err("native output frames are not independently distinguishable".into());
        }
    }
    if let Some(client) = config.client.as_deref() {
        let client_name = std::path::Path::new(client)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("client");
        println!(
            "sophia_x_application_session schema=1 status=passed class=gtk3_software client={} profile={} child_outcome=normal exit_code=0 stdout_match={} protocol_errors=0 first_error=none physical_text={} pointer_button={} surface_resize={} buffer_path=cpu_shm native_presentation={} cleanup=clean",
            client_name,
            match config.namespace_profile {
                NamespaceProfile::ClassicShared => "classic_shared",
                NamespaceProfile::Confined => "confined",
            },
            config.expect_client_stdout.is_some(),
            physical_text_proof
                .as_ref()
                .is_some_and(PhysicalTextProof::is_complete),
            physical_pointer_buttons_routed > 0,
            if resize_proof_complete {
                "committed"
            } else {
                "disabled"
            },
            if native_scanout.is_some() {
                "enabled"
            } else {
                "disabled"
            },
        );
    }
    Ok(())
}
