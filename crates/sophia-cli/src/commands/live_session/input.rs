#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PhysicalInputRouteReport {
    events: usize,
    wm_actions: Vec<WmActionId>,
    keys_observed: usize,
    keys_suppressed_no_focus: usize,
    key_targets: Vec<SurfaceId>,
    pointer_buttons_observed: usize,
    pointer_buttons_suppressed_no_target: usize,
    pointer_buttons_suppressed_by_policy: usize,
    pointer_buttons_routed: usize,
    pointer_button_targets: Vec<SurfaceId>,
    pointer_focus_targets: Vec<SurfaceId>,
    pointer_axes_observed: usize,
    pointer_axes_routed: usize,
    pointer_axis_targets: Vec<SurfaceId>,
    keys_routed: usize,
    pointer_events: usize,
    pointer_routed: usize,
    deliveries: Vec<XAuthorityInputDeliveryId>,
    emergency_exit: bool,
    return_suppressed: bool,
    virtual_terminal: Option<u8>,
    virtual_terminal_modifier_releases: usize,
    pointer_focus_handoff_expired: bool,
    pointer_focus_handoff_released: Option<(SurfaceId, usize)>,
    pointer_boundary_entries: Vec<(sophia_engine::PointerBoundaryContact, Option<usize>)>,
    pointer_boundary_reversals: Vec<(sophia_engine::PointerBoundaryContact, Option<usize>)>,
    pointer_output_transitions: Vec<(sophia_engine::PointerOutputTransition, bool)>,
}

type SessionPointerPlacement = sophia_engine::OutputUnionPointerState;

fn place_pointer_event_for_routing(
    event: &mut sophia_protocol::InputEventPacket,
    focused_surface: Option<SurfaceId>,
    input_layers: &[LayerSnapshot],
    pointer: &mut SessionPointerPlacement,
    buttons_only: bool,
) -> (bool, Option<sophia_engine::OutputUnionPointerPlacement>) {
    let placement = if let Some(raw) = event.global_position {
        let geometry = focused_surface.and_then(|surface| {
            input_layers
                .iter()
                .find(|layer| layer.surface == surface)
                .map(|layer| layer.geometry)
        });
        let placement = pointer.place(raw, geometry);
        event.global_position = Some(placement.position);
        Some(placement)
    } else {
        None
    };
    (
        !(buttons_only && matches!(event.kind, sophia_protocol::InputEventKind::PointerMotion)),
        placement,
    )
}

struct PhysicalInputRoutingContext<'a> {
    focus: &'a InputFocusState,
    committed_surfaces: &'a [CommittedSurfaceState],
    input_layers: &'a [LayerSnapshot],
    surface_roles: &'a BTreeMap<SurfaceId, sophia_protocol::SurfacePresentationRole>,
    client_routes: &'a XAuthorityClientSurfaceRoutes,
    shortcuts: Option<&'a mut WmShortcutRouter>,
    input_sender: &'a SyncSender<XAuthorityRoutedInput>,
    modifiers: &'a mut XCoreKeyboardMapper,
    key_repeat: &'a mut KeyRepeatState,
    key_repeat_map: &'a XkbKeymapSnapshot,
    client_keys: &'a mut SessionClientKeyState,
    emergency_chord: &'a mut EmergencyChordState,
    virtual_terminal_chord: &'a mut VirtualTerminalChordState,
    keyboard_coverage: &'a mut PhysicalKeyboardCoverage,
    pointer: &'a mut SessionPointerPlacement,
    pointer_routing_enabled: bool,
    pointer_proof_required: bool,
    pointer_buttons_only: bool,
    routing_mode: PhysicalInputRoutingMode,
    next_input_delivery: &'a mut u64,
    now_msec: u64,
    physical_text_proof: Option<&'a mut PhysicalTextProof>,
    pointer_focus_handoff: &'a mut PointerFocusHandoffState,
    applied_client_focus: Option<SurfaceId>,
}

fn route_physical_input<P: NonBlockingInputPoller>(
    poller: &mut P,
    context: PhysicalInputRoutingContext<'_>,
) -> Result<PhysicalInputRouteReport, Box<dyn std::error::Error>> {
    let events = poller.poll_ready()?;
    let PhysicalInputRoutingContext {
        focus,
        committed_surfaces,
        input_layers,
        surface_roles,
        client_routes,
        shortcuts,
        input_sender,
        modifiers,
        key_repeat,
        key_repeat_map,
        client_keys,
        emergency_chord,
        virtual_terminal_chord,
        keyboard_coverage,
        pointer,
        pointer_routing_enabled,
        pointer_proof_required,
        pointer_buttons_only,
        routing_mode,
        next_input_delivery,
        now_msec,
        physical_text_proof,
        pointer_focus_handoff,
        applied_client_focus,
    } = context;
    route_input_events_with_pointer_focus(
        events,
        focus,
        committed_surfaces,
        input_layers,
        surface_roles,
        client_routes,
        input_sender,
        modifiers,
        key_repeat,
        key_repeat_map,
        client_keys,
        emergency_chord,
        virtual_terminal_chord,
        keyboard_coverage,
        shortcuts,
        pointer,
        pointer_routing_enabled,
        pointer_proof_required,
        pointer_buttons_only,
        routing_mode,
        next_input_delivery,
        now_msec,
        physical_text_proof,
        Some(pointer_focus_handoff),
        applied_client_focus,
    )
}

#[allow(clippy::too_many_arguments)]
fn route_input_events(
    events: Vec<sophia_protocol::InputEventPacket>,
    focus: &InputFocusState,
    committed_surfaces: &[CommittedSurfaceState],
    input_layers: &[LayerSnapshot],
    _client_routes: &XAuthorityClientSurfaceRoutes,
    input_sender: &SyncSender<XAuthorityRoutedInput>,
    modifiers: &mut XCoreKeyboardMapper,
    key_repeat: &mut KeyRepeatState,
    key_repeat_map: &XkbKeymapSnapshot,
    client_keys: &mut SessionClientKeyState,
    emergency_chord: &mut EmergencyChordState,
    virtual_terminal_chord: &mut VirtualTerminalChordState,
    keyboard_coverage: &mut PhysicalKeyboardCoverage,
    shortcuts: Option<&mut WmShortcutRouter>,
    pointer: &mut SessionPointerPlacement,
    pointer_routing_enabled: bool,
    pointer_proof_required: bool,
    pointer_buttons_only: bool,
    routing_mode: PhysicalInputRoutingMode,
    next_input_delivery: &mut u64,
    now_msec: u64,
    physical_text_proof: Option<&mut PhysicalTextProof>,
) -> Result<PhysicalInputRouteReport, Box<dyn std::error::Error>> {
    let surface_roles = BTreeMap::new();
    route_input_events_with_pointer_focus(
        events,
        focus,
        committed_surfaces,
        input_layers,
        &surface_roles,
        _client_routes,
        input_sender,
        modifiers,
        key_repeat,
        key_repeat_map,
        client_keys,
        emergency_chord,
        virtual_terminal_chord,
        keyboard_coverage,
        shortcuts,
        pointer,
        pointer_routing_enabled,
        pointer_proof_required,
        pointer_buttons_only,
        routing_mode,
        next_input_delivery,
        now_msec,
        physical_text_proof,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn route_input_events_with_pointer_focus(
    events: Vec<sophia_protocol::InputEventPacket>,
    focus: &InputFocusState,
    committed_surfaces: &[CommittedSurfaceState],
    input_layers: &[LayerSnapshot],
    surface_roles: &BTreeMap<SurfaceId, sophia_protocol::SurfacePresentationRole>,
    _client_routes: &XAuthorityClientSurfaceRoutes,
    input_sender: &SyncSender<XAuthorityRoutedInput>,
    modifiers: &mut XCoreKeyboardMapper,
    key_repeat: &mut KeyRepeatState,
    key_repeat_map: &XkbKeymapSnapshot,
    client_keys: &mut SessionClientKeyState,
    emergency_chord: &mut EmergencyChordState,
    virtual_terminal_chord: &mut VirtualTerminalChordState,
    keyboard_coverage: &mut PhysicalKeyboardCoverage,
    mut shortcuts: Option<&mut WmShortcutRouter>,
    pointer: &mut SessionPointerPlacement,
    pointer_routing_enabled: bool,
    pointer_proof_required: bool,
    pointer_buttons_only: bool,
    routing_mode: PhysicalInputRoutingMode,
    next_input_delivery: &mut u64,
    now_msec: u64,
    mut physical_text_proof: Option<&mut PhysicalTextProof>,
    mut pointer_focus_handoff: Option<&mut PointerFocusHandoffState>,
    applied_client_focus: Option<SurfaceId>,
) -> Result<PhysicalInputRouteReport, Box<dyn std::error::Error>> {
    let mut report = PhysicalInputRouteReport {
        events: events.len(),
        wm_actions: Vec::new(),
        keys_observed: 0,
        keys_suppressed_no_focus: 0,
        keys_routed: 0,
        key_targets: Vec::new(),
        pointer_events: 0,
        pointer_buttons_observed: 0,
        pointer_buttons_suppressed_no_target: 0,
        pointer_buttons_suppressed_by_policy: 0,
        pointer_axes_observed: 0,
        pointer_routed: 0,
        pointer_buttons_routed: 0,
        pointer_button_targets: Vec::new(),
        pointer_focus_targets: Vec::new(),
        pointer_axes_routed: 0,
        pointer_axis_targets: Vec::new(),
        deliveries: Vec::new(),
        emergency_exit: false,
        return_suppressed: false,
        virtual_terminal: None,
        virtual_terminal_modifier_releases: 0,
        pointer_focus_handoff_expired: false,
        pointer_focus_handoff_released: None,
        pointer_boundary_entries: Vec::new(),
        pointer_boundary_reversals: Vec::new(),
        pointer_output_transitions: Vec::new(),
    };
    if let Some(handoff) = pointer_focus_handoff.as_deref_mut() {
        report.pointer_focus_handoff_expired = handoff.expire(now_msec);
        if let Some(mut ready) = handoff.take_ready(applied_client_focus) {
            let released_target = applied_client_focus;
            let released_count = ready.len();
            while let Some(request) = ready.pop_front() {
                let is_button = matches!(
                    request.kind,
                    sophia_protocol::InputEventKind::PointerButton { .. }
                );
                let is_axis = matches!(
                    request.kind,
                    sophia_protocol::InputEventKind::PointerAxis { .. }
                );
                let target = request.target_surface;
                let delivery = XAuthorityInputDeliveryId::from_raw(*next_input_delivery);
                *next_input_delivery = next_input_delivery
                    .checked_add(1)
                    .ok_or("live-session input delivery ID exhausted")?;
                input_sender.try_send(XAuthorityRoutedInput {
                    request,
                    delivery: Some(delivery),
                    mode: XAuthorityRoutedInputMode::Deliver,
                })?;
                report.pointer_routed = report.pointer_routed.saturating_add(1);
                if is_button {
                    report.pointer_buttons_routed =
                        report.pointer_buttons_routed.saturating_add(1);
                    report.pointer_button_targets.push(target);
                }
                if is_axis {
                    report.pointer_axes_routed = report.pointer_axes_routed.saturating_add(1);
                    report.pointer_axis_targets.push(target);
                }
                report.deliveries.push(delivery);
            }
            report.pointer_focus_handoff_released =
                released_target.map(|surface| (surface, released_count));
        }
    }
    for mut event in events {
        match event.kind {
            sophia_protocol::InputEventKind::Key { keycode, pressed } => {
                report.keys_observed = report.keys_observed.saturating_add(1);
                keyboard_coverage.observe_key(keycode, pressed);
                if !pressed {
                    let _ = key_repeat.release(event.seat, event.device, keycode);
                }
                match virtual_terminal_chord.observe(keycode, pressed) {
                    VirtualTerminalChordAction::Pass => {}
                    VirtualTerminalChordAction::Consume => continue,
                    VirtualTerminalChordAction::Activate(terminal) => {
                        keyboard_coverage.observe_virtual_terminal(terminal);
                        for modifier_keycode in virtual_terminal_chord
                            .pressed_modifier_keycodes()
                            .into_iter()
                            .flatten()
                        {
                            if let Some(shortcuts) = shortcuts.as_deref_mut() {
                                let _ = shortcuts.route_key(event.seat, modifier_keycode, false);
                            }
                            let _ = modifiers.map_evdev_key(modifier_keycode, false);
                            if routing_mode != PhysicalInputRoutingMode::Full {
                                continue;
                            }
                            let mut release = event.clone();
                            release.kind = sophia_protocol::InputEventKind::Key {
                                keycode: modifier_keycode,
                                pressed: false,
                            };
                            let release = match focus
                                .route_keyboard_event(release, committed_surfaces)
                            {
                                FocusedInputRoute::Routed(release) => release,
                                FocusedInputRoute::NoFocus(_)
                                | FocusedInputRoute::StaleFocus(_)
                                | FocusedInputRoute::UnsupportedEvent(_) => continue,
                            };
                            let Some(target_surface) = release.target_surface else {
                                continue;
                            };
                            let delivery =
                                XAuthorityInputDeliveryId::from_raw(*next_input_delivery);
                            *next_input_delivery =
                                next_input_delivery.checked_add(1).ok_or(
                                    "live-session input delivery ID exhausted",
                                )?;
                            input_sender.try_send(XAuthorityRoutedInput {
                                request: sophia_protocol::RoutedInputRequest {
                                    serial: release.serial,
                                    seat: release.seat,
                                    device: release.device,
                                    time_msec: release.time_msec,
                                    target_surface,
                                    global_position: Point::default(),
                                    local_position: Point::default(),
                                    kind: release.kind,
                                },
                                delivery: Some(delivery),
                                mode: XAuthorityRoutedInputMode::Deliver,
                            })?;
                            client_keys.record_routed(
                                SessionClientPressedKey {
                                    surface: target_surface,
                                    seat: release.seat,
                                    device: release.device,
                                    keycode: modifier_keycode,
                                },
                                false,
                            )?;
                            report.keys_routed = report.keys_routed.saturating_add(1);
                            report.key_targets.push(target_surface);
                            report.virtual_terminal_modifier_releases = report
                                .virtual_terminal_modifier_releases
                                .saturating_add(1);
                            report.deliveries.push(delivery);
                        }
                        report.virtual_terminal = Some(terminal);
                        continue;
                    }
                }
                if emergency_chord.observe(keycode, pressed) == EmergencyChordAction::Triggered {
                    report.emergency_exit = true;
                    continue;
                }
                if routing_mode != PhysicalInputRoutingMode::CursorOnly
                    && let Some(shortcuts) = shortcuts.as_deref_mut()
                {
                    let decision = shortcuts.route_key(event.seat, keycode, pressed);
                    if decision.consumed {
                        if pressed && key_repeat_map.evdev_key_repeats(keycode) {
                            key_repeat.cancel_seat(event.seat);
                        }
                        report.wm_actions.extend(decision.action);
                        continue;
                    }
                }
                if routing_mode != PhysicalInputRoutingMode::Full {
                    continue;
                }
                if sophia_cli::input_proof::pointer_proof_suppresses_return(
                    pointer_proof_required,
                    keycode,
                    physical_text_proof
                        .as_deref()
                        .is_some_and(PhysicalTextProof::is_complete),
                ) {
                    report.return_suppressed = true;
                    continue;
                }
                let event = match focus.route_keyboard_event(event, committed_surfaces) {
                    FocusedInputRoute::Routed(event) => event,
                    FocusedInputRoute::NoFocus(_) => {
                        report.keys_suppressed_no_focus =
                            report.keys_suppressed_no_focus.saturating_add(1);
                        continue;
                    }
                    FocusedInputRoute::StaleFocus(_)
                    | FocusedInputRoute::UnsupportedEvent(_) => continue,
                };
                let Some(target_surface) = event.target_surface else {
                    continue;
                };
                let key = SessionClientPressedKey {
                    surface: target_surface,
                    seat: event.seat,
                    device: event.device,
                    keycode,
                };
                if !pressed && !client_keys.release_is_routable(key) {
                    client_keys.record_routed(key, false)?;
                    continue;
                }
                let evdev_keycode = keycode;
                let Some((keycode, state)) = modifiers.map_evdev_key(keycode, pressed) else {
                    continue;
                };
                if let Some(proof) = physical_text_proof.as_deref_mut()
                    && !proof.is_complete() {
                        let observed = PhysicalTextProofEvent {
                            keycode,
                            pressed,
                            state,
                        };
                        if let Err(mismatch) = proof.observe(observed) {
                            return Err(format!(
                            "physical text proof sequence mismatch at event {}: expected keycode={} pressed={} state={} observed keycode={} pressed={} state={}",
                            mismatch.event_index,
                            mismatch.expected.keycode,
                            mismatch.expected.pressed,
                            mismatch.expected.state,
                            mismatch.observed.keycode,
                            mismatch.observed.pressed,
                            mismatch.observed.state,
                        )
                        .into());
                        }
                    }
                let delivery = XAuthorityInputDeliveryId::from_raw(*next_input_delivery);
                *next_input_delivery = next_input_delivery
                    .checked_add(1)
                    .ok_or("live-session input delivery ID exhausted")?;
                input_sender.try_send(XAuthorityRoutedInput {
                    request: sophia_protocol::RoutedInputRequest {
                        serial: event.serial,
                        seat: event.seat,
                        device: event.device,
                        time_msec: event.time_msec,
                        target_surface,
                        global_position: Point::default(),
                        local_position: Point::default(),
                        kind: event.kind,
                    },
                    delivery: Some(delivery),
                    mode: XAuthorityRoutedInputMode::Deliver,
                })?;
                client_keys.record_routed(key, pressed)?;
                if pressed {
                    match key_repeat.arm(
                        KeyRepeatTarget {
                            surface: target_surface,
                            seat: event.seat,
                            device: event.device,
                            keycode: evdev_keycode,
                            source_time_msec: event.time_msec,
                        },
                        now_msec,
                        key_repeat_map.evdev_key_repeats(evdev_keycode),
                    ) {
                        sophia_engine::KeyRepeatArmOutcome::Armed
                        | sophia_engine::KeyRepeatArmOutcome::NotRepeatable => {}
                        sophia_engine::KeyRepeatArmOutcome::SeatCapacityExhausted => {
                            return Err("key repeat seat capacity exhausted".into());
                        }
                    }
                }
                report.keys_routed = report.keys_routed.saturating_add(1);
                report.key_targets.push(target_surface);
                report.deliveries.push(delivery);
            }
            kind @ (sophia_protocol::InputEventKind::PointerMotion
            | sophia_protocol::InputEventKind::PointerButton { .. }
            | sophia_protocol::InputEventKind::PointerAxis { .. }) => {
                let is_button =
                    matches!(kind, sophia_protocol::InputEventKind::PointerButton { .. });
                let is_axis =
                    matches!(kind, sophia_protocol::InputEventKind::PointerAxis { .. });
                if is_button {
                    report.pointer_buttons_observed =
                        report.pointer_buttons_observed.saturating_add(1);
                }
                if is_axis {
                    report.pointer_axes_observed =
                        report.pointer_axes_observed.saturating_add(1);
                }
                report.pointer_events = report.pointer_events.saturating_add(1);
                if matches!(
                    routing_mode,
                    PhysicalInputRoutingMode::Suppressed | PhysicalInputRoutingMode::ShortcutsOnly
                ) {
                    if is_button {
                        report.pointer_buttons_suppressed_by_policy = report
                            .pointer_buttons_suppressed_by_policy
                            .saturating_add(1);
                    }
                    continue;
                }
                if matches!(
                    routing_mode,
                    PhysicalInputRoutingMode::CursorOnly
                        | PhysicalInputRoutingMode::ControlPlaneOnly
                ) {
                    if is_button {
                        report.pointer_buttons_suppressed_by_policy = report
                            .pointer_buttons_suppressed_by_policy
                            .saturating_add(1);
                    }
                    if !is_button {
                        let focused_surface = focus.focused_surface(event.seat);
                        let (_, placement) = place_pointer_event_for_routing(
                            &mut event,
                            focused_surface,
                            input_layers,
                            pointer,
                            false,
                        );
                        record_pointer_boundary_placement(&mut report, kind, placement);
                    }
                    continue;
                }
                let focused_surface = focus.focused_surface(event.seat);
                let (route_event, placement) = place_pointer_event_for_routing(
                    &mut event,
                    focused_surface,
                    input_layers,
                    pointer,
                    pointer_buttons_only,
                );
                record_pointer_boundary_placement(&mut report, kind, placement);
                if !route_event {
                    continue;
                }
                if !pointer_routing_enabled {
                    if is_button {
                        report.pointer_buttons_suppressed_by_policy = report
                            .pointer_buttons_suppressed_by_policy
                            .saturating_add(1);
                    }
                    continue;
                }
                let pending_target = pointer_focus_handoff
                    .as_deref()
                    .and_then(PointerFocusHandoffState::target);
                let route = pending_target.map_or_else(
                    || sophia_engine::hit_test_scene_surface_for_input(&event, input_layers),
                    |target| {
                        sophia_engine::route_scene_surface_for_input(
                            &event,
                            input_layers,
                            target,
                        )
                    },
                );
                if is_button && route.target_surface.is_none() {
                    report.pointer_buttons_suppressed_no_target = report
                        .pointer_buttons_suppressed_no_target
                        .saturating_add(1);
                }
                let (Some(global), Some(local)) = (event.global_position, route.local_position)
                else {
                    continue;
                };
                let Some(surface) = route.target_surface else {
                    continue;
                };
                let request = sophia_protocol::RoutedInputRequest {
                    serial: event.serial,
                    seat: event.seat,
                    device: event.device,
                    time_msec: event.time_msec,
                    target_surface: surface,
                    global_position: global,
                    local_position: local,
                    kind,
                };
                let starts_focus_handoff = pointer_press_starts_focus_handoff(
                    &kind,
                    applied_client_focus,
                    surface,
                    surface_roles.get(&surface).copied(),
                    pointer_focus_handoff
                        .as_deref()
                        .is_some_and(|handoff| handoff.target().is_none()),
                );
                if let Some(handoff) = pointer_focus_handoff.as_deref_mut() {
                    if starts_focus_handoff {
                        handoff.begin(surface, now_msec, request)?;
                        report.pointer_focus_targets.push(surface);
                        continue;
                    }
                    if handoff.target().is_some() {
                        handoff.defer(request)?;
                        continue;
                    }
                }
                let delivery = XAuthorityInputDeliveryId::from_raw(*next_input_delivery);
                *next_input_delivery = next_input_delivery
                    .checked_add(1)
                    .ok_or("live-session input delivery ID exhausted")?;
                input_sender.try_send(XAuthorityRoutedInput {
                    request,
                    delivery: Some(delivery),
                    mode: XAuthorityRoutedInputMode::Deliver,
                })?;
                report.pointer_routed = report.pointer_routed.saturating_add(1);
                if is_button {
                    report.pointer_buttons_routed = report.pointer_buttons_routed.saturating_add(1);
                    report.pointer_button_targets.push(surface);
                }
                if is_axis {
                    report.pointer_axes_routed = report.pointer_axes_routed.saturating_add(1);
                    report.pointer_axis_targets.push(surface);
                }
                report.deliveries.push(delivery);
            }
        }
    }
    Ok(report)
}

fn pointer_press_starts_focus_handoff(
    kind: &sophia_protocol::InputEventKind,
    applied_focus: Option<SurfaceId>,
    target: SurfaceId,
    role: Option<sophia_protocol::SurfacePresentationRole>,
    handoff_idle: bool,
) -> bool {
    matches!(
        kind,
        sophia_protocol::InputEventKind::PointerButton {
            button: 0x110,
            pressed: true
        }
    ) && applied_focus != Some(target)
        && role != Some(sophia_protocol::SurfacePresentationRole::ClientPositioned)
        && handoff_idle
}

fn record_pointer_boundary_placement(
    report: &mut PhysicalInputRouteReport,
    kind: sophia_protocol::InputEventKind,
    placement: Option<sophia_engine::OutputUnionPointerPlacement>,
) {
    if !matches!(kind, sophia_protocol::InputEventKind::PointerMotion) {
        return;
    }
    let Some(placement) = placement else {
        return;
    };
    if !placement.entered.is_empty() {
        report
            .pointer_boundary_entries
            .push((placement.entered, placement.output_index));
    }
    if !placement.reversed.is_empty() {
        report
            .pointer_boundary_reversals
            .push((placement.reversed, placement.output_index));
    }
    if let Some(transition) = placement.transition {
        report
            .pointer_output_transitions
            .push((transition, placement.contact.is_empty()));
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct KeyRepeatRouteReport {
    routed: usize,
    delivery: Option<XAuthorityInputDeliveryId>,
}

#[allow(clippy::too_many_arguments)]
fn route_due_key_repeat(
    key_repeat: &mut KeyRepeatState,
    seat: SeatId,
    now_msec: u64,
    routing_mode: PhysicalInputRoutingMode,
    focus: &InputFocusState,
    committed_surfaces: &[CommittedSurfaceState],
    client_keys: &SessionClientKeyState,
    input_sender: &SyncSender<XAuthorityRoutedInput>,
    next_input_delivery: &mut u64,
) -> Result<KeyRepeatRouteReport, Box<dyn std::error::Error>> {
    let mut report = KeyRepeatRouteReport::default();
    if routing_mode != PhysicalInputRoutingMode::Full {
        return Ok(report);
    }
    let Some(target) = key_repeat.active_target(seat) else {
        return Ok(report);
    };
    let pressed = SessionClientPressedKey {
        surface: target.surface,
        seat: target.seat,
        device: target.device,
        keycode: target.keycode,
    };
    let target_is_current = focus.focused_surface(seat) == Some(target.surface)
        && committed_surfaces
            .iter()
            .any(|committed| committed.surface == target.surface)
        && client_keys.is_pressed(pressed);
    if !target_is_current {
        key_repeat.cancel_seat(seat);
        return Ok(report);
    }
    let Some(pulse) = key_repeat.take_due(seat, now_msec) else {
        return Ok(report);
    };
    let delivery = XAuthorityInputDeliveryId::from_raw(*next_input_delivery);
    *next_input_delivery = next_input_delivery
        .checked_add(1)
        .ok_or("live-session input delivery ID exhausted")?;
    input_sender.try_send(XAuthorityRoutedInput {
        request: sophia_protocol::RoutedInputRequest {
            serial: delivery.raw(),
            seat: pulse.target.seat,
            device: pulse.target.device,
            time_msec: pulse.time_msec,
            target_surface: pulse.target.surface,
            global_position: Point::default(),
            local_position: Point::default(),
            kind: sophia_protocol::InputEventKind::Key {
                keycode: pulse.target.keycode,
                pressed: true,
            },
        },
        delivery: Some(delivery),
        mode: XAuthorityRoutedInputMode::Repeat,
    })?;
    report.routed = 1;
    report.delivery = Some(delivery);
    Ok(report)
}

fn flush_client_pressed_keys(
    surface: SurfaceId,
    client_keys: &mut SessionClientKeyState,
    scratch: &mut Vec<SessionClientPressedKey>,
    deliveries: &mut Vec<XAuthorityInputDeliveryId>,
    input_sender: &SyncSender<XAuthorityRoutedInput>,
    modifiers: &mut XCoreKeyboardMapper,
    next_input_delivery: &mut u64,
    time_msec: u64,
) -> Result<usize, Box<dyn std::error::Error>> {
    client_keys.copy_surface_keys(surface, scratch);
    deliveries.clear();
    for key in scratch.iter().copied() {
        let Some((_x_keycode, _state)) = modifiers.map_evdev_key(key.keycode, false) else {
            return Err("pressed-key ledger contains an unmappable key".into());
        };
        let delivery = XAuthorityInputDeliveryId::from_raw(*next_input_delivery);
        *next_input_delivery = next_input_delivery
            .checked_add(1)
            .ok_or("live-session input delivery ID exhausted")?;
        input_sender.try_send(XAuthorityRoutedInput {
            request: sophia_protocol::RoutedInputRequest {
                serial: delivery.raw(),
                seat: key.seat,
                device: key.device,
                time_msec,
                target_surface: key.surface,
                global_position: Point::default(),
                local_position: Point::default(),
                kind: sophia_protocol::InputEventKind::Key {
                    keycode: key.keycode,
                    pressed: false,
                },
            },
            delivery: Some(delivery),
            mode: XAuthorityRoutedInputMode::Deliver,
        })?;
        deliveries.push(delivery);
        client_keys.record_synthetic_release(key);
    }
    Ok(scratch.len())
}

fn clear_client_pressed_keys_state_only(
    surface: SurfaceId,
    client_keys: &mut SessionClientKeyState,
    scratch: &mut Vec<SessionClientPressedKey>,
    modifiers: &mut XCoreKeyboardMapper,
    input_sender: &SyncSender<XAuthorityRoutedInput>,
    next_input_delivery: &mut u64,
    time_msec: u64,
) -> Result<usize, Box<dyn std::error::Error>> {
    client_keys.copy_surface_keys(surface, scratch);
    for key in scratch.iter().copied() {
        let _ = modifiers.map_evdev_key(key.keycode, false);
        let serial = *next_input_delivery;
        *next_input_delivery = next_input_delivery
            .checked_add(1)
            .ok_or("live-session input delivery ID exhausted")?;
        input_sender.try_send(XAuthorityRoutedInput {
            request: sophia_protocol::RoutedInputRequest {
                serial,
                seat: key.seat,
                device: key.device,
                time_msec,
                target_surface: key.surface,
                global_position: Point::default(),
                local_position: Point::default(),
                kind: sophia_protocol::InputEventKind::Key {
                    keycode: key.keycode,
                    pressed: false,
                },
            },
            delivery: None,
            mode: XAuthorityRoutedInputMode::StateOnly,
        })?;
        client_keys.record_state_only_release(key);
    }
    Ok(scratch.len())
}
