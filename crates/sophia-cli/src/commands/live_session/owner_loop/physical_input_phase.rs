{
macro_rules! drain_physical_input {
    ($routing_mode:expr) => {{
        let emergency_exit = false;
        if let Some(poller) = physical_input.as_mut() {
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
                    next_input_delivery: &mut input_delivery.next,
                    physical_text_proof: physical_text_proof.as_mut(),
                },
            )?;
            metrics.physical_events = metrics.physical_events.saturating_add(report.events);
            metrics.physical_keys_routed = metrics
                .physical_keys_routed
                .saturating_add(report.keys_routed);
            metrics.physical_pointer_events = metrics
                .physical_pointer_events
                .saturating_add(report.pointer_events);
            metrics.physical_pointer_routed = metrics
                .physical_pointer_routed
                .saturating_add(report.pointer_routed);
            metrics.physical_pointer_buttons_routed = metrics
                .physical_pointer_buttons_routed
                .saturating_add(report.pointer_buttons_routed);
            input_delivery.events_expected = input_delivery
                .events_expected
                .saturating_add(report.deliveries.len());
            input_delivery
                .pending
                .extend(report.deliveries.iter().copied());
            if !report.deliveries.is_empty() && input_proof_started_at.is_some() {
                input_delivery
                    .wait_started_at
                    .get_or_insert_with(Instant::now);
            }
            let pointer_motions_observed = report
                .pointer_events
                .saturating_sub(report.pointer_buttons_observed);
            if pointer_motions_observed > 0 && pointer.position.is_some() {
                if cursor_updates.dirty {
                    metrics.cursor_moves_coalesced = metrics
                        .cursor_moves_coalesced
                        .saturating_add(pointer_motions_observed as u64);
                } else {
                    cursor_updates.dirty_since = Some(Instant::now());
                }
                cursor_updates.dirty = true;
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
            if report.return_suppressed && !input_observations.return_suppressed {
                println!("sophia_live_session_input_pipeline schema=1 status=return_suppressed");
                std::io::stdout().flush()?;
                input_observations.return_suppressed = true;
            }
            if !input_observations.key_observed && report.keys_observed > 0 {
                println!("sophia_live_session_input_pipeline schema=1 status=key_observed");
                std::io::stdout().flush()?;
                input_observations.key_observed = true;
            }
            if !input_observations.key_routed && report.keys_routed > 0 {
                println!("sophia_live_session_input_pipeline schema=1 status=key_routed");
                std::io::stdout().flush()?;
                input_observations.key_routed = true;
            }
            if report.emergency_exit {
                println!("sophia_live_session_input_pipeline schema=1 status=emergency_exit");
                std::io::stdout().flush()?;
                emergency_exit_requested = true;
                let requested_at = Instant::now();
                input_delivery.wait_started_at = Some(requested_at);
                input_delivery.source = Some("emergency");
            }
            if physical_sequence_completed_at.is_none()
                && physical_text_proof
                    .as_ref()
                    .is_some_and(|proof| proof.is_complete())
            {
                let completed_at = Instant::now();
                physical_sequence_completed_at = Some(completed_at);
                input_delivery.wait_started_at = Some(completed_at);
                input_delivery.source = Some("physical");
                if physical_input_pixels_already_changed(
                    injection_checksum,
                    scene.last_report().map(|report| report.checksum),
                    input_surface_pixel_change,
                ) {
                    input_pixel_change = true;
                }
            }
            if !input_observations.pointer_motion_observed
                && report.pointer_events > report.pointer_buttons_observed
            {
                println!("sophia_live_session_pointer schema=2 status=motion_observed");
                input_observations.pointer_motion_observed = true;
            }
            if !input_observations.pointer_motion_routed
                && report.pointer_routed > report.pointer_buttons_routed
            {
                println!("sophia_live_session_pointer schema=2 status=motion_routed");
                input_observations.pointer_motion_routed = true;
            }
            if !input_observations.pointer_button_observed
                && report.pointer_buttons_observed > 0
            {
                println!(
                    "sophia_live_session_pointer schema=2 status=button_observed count={}",
                    report.pointer_buttons_observed
                );
                input_observations.pointer_button_observed = true;
            }
            if !input_observations.pointer_button_routed && report.pointer_buttons_routed > 0 {
                println!(
                    "sophia_live_session_pointer schema=2 status=button_routed count={}",
                    metrics.physical_pointer_buttons_routed
                );
                input_observations.pointer_button_routed = true;
            }
            if input_observations.pointer_motion_observed
                || input_observations.pointer_button_observed
                || input_observations.pointer_button_routed
            {
                std::io::stdout().flush()?;
            }
        }
        emergency_exit
    }};
}

loop {
    let input_baseline_presented_before_wait = include!("lifecycle.rs");
    include!("authority.rs");
    include!("input_proof.rs");
}

include!("completion.rs")
}
