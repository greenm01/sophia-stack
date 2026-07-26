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
                    key_repeat: &mut key_repeat,
                    key_repeat_map: &key_repeat_map,
                    client_keys: &mut client_keys,
                    emergency_chord: &mut emergency_chord,
                    virtual_terminal_chord: &mut virtual_terminal_chord,
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
                    now_msec: u64::try_from(started.elapsed().as_millis())
                        .unwrap_or(u64::MAX),
                    physical_text_proof: physical_text_proof.as_mut(),
                    pointer_focus_handoff: &mut pointer_focus_handoff,
                    applied_client_focus,
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
            if report.pointer_focus_handoff_expired {
                eprintln!(
                    "sophia_live_session_pointer schema=5 status=focus_handoff_dropped reason=timeout"
                );
            }
            if let Some((surface, count)) = report.pointer_focus_handoff_released {
                println!(
                    "sophia_live_session_pointer schema=5 status=focus_handoff_released surface={} count={count}",
                    surface.index(),
                );
                input_observations.pointer_focus_target = Some(surface);
                input_observations.pointer_focus_key_routed = false;
            }
            if !input_observations.pointer_focus_key_routed
                && let Some(surface) = input_observations.pointer_focus_target
                && report.key_targets.contains(&surface)
            {
                println!(
                    "sophia_live_session_pointer schema=6 status=focused_key_routed surface={}",
                    surface.index(),
                );
                input_observations.pointer_focus_key_routed = true;
            }
            input_delivery.events_expected = input_delivery
                .events_expected
                .saturating_add(report.deliveries.len());
            input_delivery
                .pending
                .extend(report.deliveries.iter().copied());
            let repeat_report = route_due_key_repeat(
                &mut key_repeat,
                seat,
                u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                $routing_mode,
                &focus,
                committed_surfaces,
                &client_keys,
                input_sender,
                &mut input_delivery.next,
            )?;
            metrics.key_repeats_routed = metrics
                .key_repeats_routed
                .saturating_add(repeat_report.routed);
            input_delivery.events_expected = input_delivery
                .events_expected
                .saturating_add(usize::from(repeat_report.delivery.is_some()));
            if let Some(delivery) = repeat_report.delivery {
                input_delivery.pending.insert(delivery);
            }
            if !report.deliveries.is_empty() && input_proof_started_at.is_some() {
                input_delivery
                    .wait_started_at
                    .get_or_insert_with(Instant::now);
            }
            let pointer_motions_observed = report
                .pointer_events
                .saturating_sub(report.pointer_buttons_observed)
                .saturating_sub(report.pointer_axes_observed);
            for (status, contacts) in [
                (
                    "output_edge_confined",
                    &report.pointer_boundary_entries,
                ),
                (
                    "edge_reverse_immediate",
                    &report.pointer_boundary_reversals,
                ),
            ] {
                for (contact, output_index) in contacts {
                    for (axis, side) in [
                        ("horizontal", contact.horizontal),
                        ("vertical", contact.vertical),
                    ] {
                        let Some(side) = side else {
                            continue;
                        };
                        let side = match side {
                            sophia_engine::PointerBoundarySide::Minimum => "minimum",
                            sophia_engine::PointerBoundarySide::Maximum => "maximum",
                        };
                        println!(
                            "sophia_live_session_pointer schema=7 status={status} axis={axis} side={side}"
                        );
                        if let Some(output_slot) = output_index {
                            println!(
                                "sophia_live_session_pointer schema=8 status={status} axis={axis} side={side} output_slot={output_slot}"
                            );
                        }
                    }
                }
            }
            for (transition, boundary_free) in &report.pointer_output_transitions {
                let boundary = if *boundary_free { "free" } else { "projected" };
                println!(
                    "sophia_live_session_pointer schema=8 status=output_transition from_slot={} to_slot={} boundary={boundary}",
                    transition.from, transition.to
                );
            }
            if !post_startup_exit_pointer_reported
                && config.normal_session
                && primary_child_exited
                && focus.focused_surface(seat).is_none()
                && wm_session.is_some()
                && pointer_motions_observed > 0
            {
                println!(
                    "sophia_live_session_input_pipeline schema=1 status=desktop_pointer_active source=post_startup_exit"
                );
                std::io::stdout().flush()?;
                post_startup_exit_pointer_reported = true;
            }
            if pointer_motions_observed > 0 && pointer.position().is_some() {
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
                match wm.enqueue_action(
                    action,
                    focus.focused_surface(seat),
                    &layout,
                    output,
                )? {
                    LiveWmRequestAdmission::Admitted => {}
                    LiveWmRequestAdmission::RejectedCapacity => {
                        eprintln!(
                            "sophia_live_wm schema=2 status=request_rejected source=action reason=capacity action={}",
                            action.raw(),
                        );
                    }
                    LiveWmRequestAdmission::Duplicate => {
                        return Err("WM action request was unexpectedly deduplicated".into());
                    }
                }
            }
            for surface in report.pointer_focus_targets.iter().copied() {
                let wm = wm_session
                    .as_mut()
                    .ok_or("pointer focus requested without a live WM session")?;
                match wm.enqueue_focus(surface, &layout, output)? {
                    LiveWmRequestAdmission::Admitted => {
                        println!(
                            "sophia_live_wm schema=3 status=focus_requested source=pointer surface={}",
                            surface.index(),
                        );
                    }
                    LiveWmRequestAdmission::Duplicate => {}
                    LiveWmRequestAdmission::RejectedCapacity => {
                        eprintln!(
                            "sophia_live_wm schema=3 status=request_rejected source=pointer_focus reason=capacity surface={}",
                            surface.index(),
                        );
                    }
                }
            }
            if let Some(terminal) = report.virtual_terminal {
                if pending_virtual_terminal.is_none() && requested_virtual_terminal.is_none() {
                    if let Some(surface) = applied_client_focus {
                        flush_client_keys!(surface, "virtual_terminal");
                    }
                    pending_virtual_terminal = Some((terminal, Instant::now()));
                    println!(
                        "sophia_live_session_vt schema=4 status=queued target={terminal} modifier_releases={}",
                        report.virtual_terminal_modifier_releases,
                    );
                }
                std::io::stdout().flush()?;
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
            if !input_observations.key_suppressed_no_focus
                && report.keys_suppressed_no_focus > 0
            {
                println!(
                    "sophia_live_session_input_pipeline schema=2 status=key_suppressed reason=no_focus"
                );
                std::io::stdout().flush()?;
                input_observations.key_suppressed_no_focus = true;
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
                && report.pointer_events
                    > report
                        .pointer_buttons_observed
                        .saturating_add(report.pointer_axes_observed)
            {
                println!("sophia_live_session_pointer schema=2 status=motion_observed");
                input_observations.pointer_motion_observed = true;
            }
            if !input_observations.pointer_motion_routed
                && report.pointer_routed
                    > report
                        .pointer_buttons_routed
                        .saturating_add(report.pointer_axes_routed)
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
            if report.pointer_buttons_suppressed_no_target > 0 {
                input_observations.pointer_buttons_suppressed_no_target = input_observations
                    .pointer_buttons_suppressed_no_target
                    .saturating_add(report.pointer_buttons_suppressed_no_target);
                println!(
                    "sophia_live_session_pointer schema=8 status=button_suppressed reason=no_target count={} total={}",
                    report.pointer_buttons_suppressed_no_target,
                    input_observations.pointer_buttons_suppressed_no_target
                );
            }
            if report.pointer_buttons_suppressed_by_policy > 0 {
                println!(
                    "sophia_live_session_pointer schema=8 status=button_suppressed reason=policy mode={} count={}",
                    physical_input_routing_mode_label($routing_mode),
                    report.pointer_buttons_suppressed_by_policy
                );
            }
            if !input_observations.pointer_button_routed && report.pointer_buttons_routed > 0 {
                println!(
                    "sophia_live_session_pointer schema=2 status=button_routed count={}",
                    metrics.physical_pointer_buttons_routed
                );
                input_observations.pointer_button_routed = true;
            }
            if !input_observations.client_positioned_pointer_button_routed
                && report
                    .pointer_button_targets
                    .iter()
                    .copied()
                    .any(|surface| layout.is_client_positioned(surface))
            {
                println!(
                    "sophia_live_session_pointer schema=4 status=target_routed role=client_positioned kind=button"
                );
                input_observations.client_positioned_pointer_button_routed = true;
            }
            if !input_observations.pointer_axis_observed && report.pointer_axes_observed > 0 {
                println!("sophia_live_session_pointer schema=3 status=axis_observed");
                input_observations.pointer_axis_observed = true;
            }
            if !input_observations.pointer_axis_routed && report.pointer_axes_routed > 0 {
                println!("sophia_live_session_pointer schema=3 status=axis_routed");
                input_observations.pointer_axis_routed = true;
            }
            if !input_observations.client_positioned_pointer_axis_routed
                && report
                    .pointer_axis_targets
                    .iter()
                    .copied()
                    .any(|surface| layout.is_client_positioned(surface))
            {
                println!(
                    "sophia_live_session_pointer schema=4 status=target_routed role=client_positioned kind=axis"
                );
                input_observations.client_positioned_pointer_axis_routed = true;
            }
            if input_observations.pointer_motion_observed
                || input_observations.pointer_button_observed
                || input_observations.pointer_button_routed
                || input_observations.pointer_axis_observed
                || input_observations.pointer_axis_routed
            {
                std::io::stdout().flush()?;
            }
        }
        emergency_exit
    }};
}

loop {
    service_core_config_reload!();
    service_session_controls!();
    let input_baseline_presented_before_wait = include!("lifecycle.rs");
    include!("wm_phase.rs");
    include!("authority.rs");
    include!("input_proof.rs");
    service_session_controls!();
}

include!("completion.rs")
}
