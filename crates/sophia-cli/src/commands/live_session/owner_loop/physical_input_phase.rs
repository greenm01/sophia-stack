{
macro_rules! drain_physical_input {
    ($routing_mode:expr) => {{
        synchronize_wm_pointer_epoch!();
        let emergency_exit = false;
        let lease_updates = drain_application_route_lease_updates(
            route_lease_update_receiver,
            &mut application_route_leases,
        );
        if lease_updates.confirmed != 0
            || lease_updates.rejected != 0
            || lease_updates.released != 0
            || lease_updates.stale != 0
        {
            println!(
                "sophia_live_input_lease schema=1 confirmed={} rejected={} released={} stale={}",
                lease_updates.confirmed,
                lease_updates.rejected,
                lease_updates.released,
                lease_updates.stale,
            );
        }
        if let sophia_engine::ApplicationRouteLeaseTimeout::Quarantine(lease) =
            application_route_leases.observe_timeout(
                seat,
                u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            )
        {
            frontend_service_sender.try_send(XServerFrontendServiceCommand::RevokeAdmission {
                admission: lease.admission,
            })?;
            eprintln!(
                "sophia_live_input_lease schema=1 status=quarantined reason=release_timeout admission={}",
                lease.admission.raw(),
            );
        }
        if let Some(poller) = physical_input.as_mut() {
            let empty_committed = [];
            let committed_surfaces = runtime
                .as_ref()
                .map_or(&empty_committed[..], |runtime| runtime.committed_surfaces());
            let empty_layers = [];
            let input_output = runtime.as_ref().and_then(|runtime| runtime.input_output());
            let input_presentation_epoch = runtime
                .as_ref()
                .map_or(0, |runtime| runtime.input_presentation_epoch());
            let input_layers = runtime
                .as_ref()
                .map_or(&empty_layers[..], |runtime| runtime.input_layers());
            let empty_projections = [];
            let input_projections = runtime.as_ref().map_or(
                &empty_projections[..],
                |runtime| runtime.input_projections(),
            );
            let report = route_physical_input(
                poller,
                PhysicalInputRoutingContext {
                    focus: &focus,
                    committed_surfaces,
                    input_layers,
                    input_projections,
                    pointer_outputs: &outputs,
                    surface_roles: &layout.presentation_roles,
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
                    keyboard_coverage: &mut keyboard_coverage,
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
                    keyboard_focus_handoff: &mut keyboard_focus_handoff,
                    pointer_focus_handoff: &mut pointer_focus_handoff,
                    applied_client_focus,
                    floating_gesture: &mut floating_pointer_gesture,
                    application_route_leases: &mut application_route_leases,
                    route_lease_release_sender,
                    input_output,
                    input_presentation_epoch,
                },
            )?;
            let event_timings = poller.drain_event_timings();
            if report.keyboard_focus_handoff_expired
                || report.keyboard_focus_handoff_stale_drops != 0
                || report.keyboard_focus_handoff_capacity_drops != 0
            {
                deferred_physical_key_timings.clear();
            }
            if physical_input_ready_at.is_some() && input_proof_started_at.is_none() {
                for (serial, event_time_msec) in &report.deferred_key_presses {
                    let timing = event_timings
                        .iter()
                        .find(|timing| timing.serial == *serial)
                        .copied()
                        .ok_or("deferred physical key had no libinput timing sidecar")?;
                    if timing.event_time_msec != *event_time_msec {
                        return Err(
                            "deferred physical key timing sidecar did not match event".into()
                        );
                    }
                    deferred_physical_key_timings.insert(*serial, timing);
                }
                if deferred_physical_key_timings.len()
                    > sophia_engine::KEYBOARD_FOCUS_HANDOFF_CAPACITY
                {
                    return Err("deferred physical key timing capacity exhausted".into());
                }
            }
            if physical_input_ready_at.is_some()
                && input_proof_started_at.is_none()
                && let Some((serial, event_time_msec)) = report.routed_key_presses.last().copied()
                && let Some(timing) = event_timings
                    .iter()
                    .find(|timing| timing.serial == serial)
                    .copied()
                    .or_else(|| deferred_physical_key_timings.remove(&serial))
            {
                if timing.event_time_msec != event_time_msec {
                    return Err("physical input timing sidecar did not match routed event".into());
                }
                input_raw_ingress_msec = Some(event_time_msec);
                input_queue_dwell = Some(Duration::from_millis(
                    u64::try_from(timing.queue_dwell_msec).unwrap_or(u64::MAX),
                ));
                println!(
                    "sophia_live_input_latency schema=1 status=ingress source=libinput_kernel event_serial={} ingress_msec={} queue_dwell_msec={}",
                    serial,
                    event_time_msec,
                    timing.queue_dwell_msec,
                );
                std::io::stdout().flush()?;
                deferred_physical_key_timings.clear();
            }
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
            if report.keyboard_focus_handoff_expired {
                eprintln!(
                    "sophia_live_session_keyboard schema=1 status=focus_handoff_dropped reason=timeout"
                );
            }
            if report.keyboard_focus_handoff_stale_drops != 0 {
                eprintln!(
                    "sophia_live_session_keyboard schema=1 status=focus_handoff_dropped reason=stale_target count={}",
                    report.keyboard_focus_handoff_stale_drops,
                );
            }
            if report.keyboard_focus_handoff_capacity_drops != 0 {
                eprintln!(
                    "sophia_live_session_keyboard schema=1 status=focus_handoff_dropped reason=capacity count={}",
                    report.keyboard_focus_handoff_capacity_drops,
                );
            }
            if let Some((surface, count)) = report.keyboard_focus_handoff_released {
                println!(
                    "sophia_live_session_keyboard schema=1 status=focus_handoff_released surface={} count={count}",
                    surface.index(),
                );
            }
            if report.pointer_focus_handoff_stale_drops != 0 {
                eprintln!(
                    "sophia_live_session_pointer schema=5 status=focus_handoff_dropped reason=stale_target count={}",
                    report.pointer_focus_handoff_stale_drops,
                );
            }
            if report.pointer_focus_handoff_capacity_drops != 0 {
                eprintln!(
                    "sophia_live_session_pointer schema=5 status=focus_handoff_dropped reason=capacity count={}",
                    report.pointer_focus_handoff_capacity_drops,
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
            match report.floating_outline {
                FloatingPointerOutlineUpdate::Unchanged => {}
                FloatingPointerOutlineUpdate::Set(outline) => {
                    let outline = clamp_floating_pointer_outline(
                        outline,
                        &wm_output_bounds(&outputs),
                    )
                    .ok_or("floating outline started outside every Engine output")?;
                    if let Some(runtime) = runtime.as_mut()
                        && runtime.set_floating_outline(
                            Some(sophia_backend_live::LiveFloatingOutline {
                                surface: outline.surface,
                                geometry: outline.geometry,
                            }),
                            &scene,
                            native_scanout.as_mut(),
                        )?
                    {
                        println!(
                            "sophia_live_wm_pointer schema=1 status=outline_presented surface={} geometry={}x{}_{}_{}",
                            outline.surface.index(),
                            outline.geometry.width,
                            outline.geometry.height,
                            outline.geometry.x,
                            outline.geometry.y,
                        );
                    }
                }
                FloatingPointerOutlineUpdate::Clear => {
                    if let Some(runtime) = runtime.as_mut()
                        && runtime.set_floating_outline(
                            None,
                            &scene,
                            native_scanout.as_mut(),
                        )?
                    {
                        println!(
                            "sophia_live_wm_pointer schema=1 status=outline_retired atomic_request=true"
                        );
                    }
                }
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
                match wm.enqueue_action(action, &layout, output)? {
                    LiveOrderedWmActionAdmission::Admitted => {
                        println!(
                            "sophia_live_wm schema=1 status=physical_action_admitted action={}",
                            action.raw(),
                        );
                    }
                    LiveOrderedWmActionAdmission::RejectedCapacity { report } => {
                        if report {
                            eprintln!(
                                "sophia_live_wm schema=2 status=request_rejected source=action reason=capacity action={}",
                                action.raw(),
                            );
                        }
                    }
                }
            }
            for interaction in report.wm_pointer_interactions.iter().copied() {
                let wm = wm_session
                    .as_mut()
                    .ok_or("WM pointer interaction activated without a live WM session")?;
                match LivePhysicalWmActionDisposition::from(
                    wm.enqueue_pointer_interaction(interaction, &layout)?,
                ) {
                    LivePhysicalWmActionDisposition::Admitted => {
                        println!(
                            "sophia_live_wm_pointer schema=2 status=interaction_admitted phase={:?} mode={:?} surface={}",
                            interaction.phase,
                            interaction.mode,
                            interaction.surface.index(),
                        );
                    }
                    LivePhysicalWmActionDisposition::RejectedCapacity => {
                        eprintln!(
                            "sophia_live_wm_pointer schema=2 status=request_rejected reason=capacity phase={:?} surface={}",
                            interaction.phase,
                            interaction.surface.index(),
                        );
                    }
                    LivePhysicalWmActionDisposition::Coalesced => {}
                }
            }
            for gesture in report.wm_pointer_gestures.iter().copied() {
                let wm = wm_session
                    .as_mut()
                    .ok_or("WM pointer gesture activated without a live WM session")?;
                match LivePhysicalWmActionDisposition::from(wm.enqueue_pointer_gesture(
                    gesture,
                    &layout,
                )?) {
                    LivePhysicalWmActionDisposition::Admitted => {
                        println!(
                            "sophia_live_wm_pointer schema=1 status=gesture_released atomic_request=true mode={:?} surface={} start_x={} start_y={} end_x={} end_y={}",
                            gesture.mode,
                            gesture.surface.index(),
                            gesture.start.x,
                            gesture.start.y,
                            gesture.end.x,
                            gesture.end.y,
                        );
                    }
                    LivePhysicalWmActionDisposition::RejectedCapacity => {
                        eprintln!(
                            "sophia_live_wm_pointer schema=1 status=request_rejected reason=capacity surface={}",
                            gesture.surface.index(),
                        );
                    }
                    LivePhysicalWmActionDisposition::Coalesced => {}
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
                flush_all_client_keys!("emergency");
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
            if config.firefox_m10_dialog_proof && report.pointer_buttons_routed > 0 {
                println!(
                    "sophia_firefox_dialog schema=1 status=pointer_batch routed={} total={} content=redacted",
                    report.pointer_buttons_routed,
                    metrics.physical_pointer_buttons_routed,
                );
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
            if report.pointer_axes_observed > 0 || report.pointer_axes_routed > 0 {
                println!(
                    "sophia_live_session_pointer schema=9 status=axis_batch observed={} routed={}",
                    report.pointer_axes_observed, report.pointer_axes_routed,
                );
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

macro_rules! schedule_output_topology_rebuild {
    ($reason:literal, $security_epoch_already_advanced:expr) => {{
        let notice_sequence = output_topology_owner
            .notice_sequence
            .checked_add(1)
            .ok_or("synthetic output topology notice sequence exhausted")?;
        let advance_security_epoch =
            output_topology_owner.begin_rescan(notice_sequence)?;
        if advance_security_epoch && !$security_epoch_already_advanced {
            let revoked_input_leases = advance_application_input_security_epoch(
                &mut application_route_leases,
                input_sender,
                &layout.client_routes,
                route_lease_release_sender,
            )?;
            revoke_floating_pointer_interaction!("output_topology");
            pointer_focus_handoff = PointerFocusHandoffState::default();
            keyboard_focus_handoff = KeyboardFocusHandoffState::default();
            key_repeat.cancel_seat(seat);
            println!(
                "sophia_live_input_epoch schema=1 reason=output_topology transition={} epoch={} revoked_leases={revoked_input_leases}",
                output_topology_owner.transition,
                application_route_leases.control_epoch(),
            );
        }
        output_topology_retry_at = Some(Instant::now());
        tracing::warn!(
            "sophia_live_output_topology schema=1 status=deferred transition={} source={} security_epoch_already_advanced={}",
            output_topology_owner.transition,
            $reason,
            $security_epoch_already_advanced,
        );
    }};
}

macro_rules! publish_resumed_topology_transport {
    ($native:expr) => {{
        if output_topology_owner.phase == LiveOutputTopologyPhase::Quarantined {
            let rebuild = output_topology_owner
                .observe_rebuild(outputs.clone(), $native.head_fingerprint())?;
            debug_assert_eq!(rebuild, LiveOutputTopologyRebuild::TransportReplaced);
            output_topology_owner.mark_published($native.retirements, false)?;
            output_topology_retry_at = None;
            tracing::info!(
                "sophia_live_output_topology schema=1 status=published transition={} outputs={} changed=false source=seat_resume input=quarantined",
                output_topology_owner.transition,
                outputs.len(),
            );
        }
    }};
}

let mut native_frame_service_preempted_previous_cycle = false;
let mut native_frame_control_priority_cycles = 0_u8;
let mut last_native_frame_service = Instant::now();
let mut native_frame_service_deadline_armed = false;
let mut native_frame_idle_service_cycles = 0_u8;
loop {
    if let Some(wm) = wm_session.as_mut() {
        wm.service_policy_update()?;
    }
    service_core_config_reload!();
    service_session_controls!();
    include!("topology_phase.rs");
    include!("lifecycle.rs");
    include!("wm_phase.rs");
    include!("authority.rs");
    include!("input_proof.rs");
    service_session_controls!();
}

include!("completion.rs")
}
