#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PhysicalInputRouteReport {
    events: usize,
    wm_actions: Vec<WmActionId>,
    keys_observed: usize,
    keys_suppressed_no_focus: usize,
    pointer_buttons_observed: usize,
    pointer_buttons_routed: usize,
    pointer_axes_observed: usize,
    pointer_axes_routed: usize,
    keys_routed: usize,
    pointer_events: usize,
    pointer_routed: usize,
    deliveries: Vec<XAuthorityInputDeliveryId>,
    emergency_exit: bool,
    return_suppressed: bool,
    virtual_terminal: Option<u8>,
    virtual_terminal_modifier_releases: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct SessionPointerPlacement {
    raw_position: Option<Point>,
    offset: Option<Point>,
    position: Option<Point>,
}

fn pointer_offset_for_geometry(raw: Point, geometry: Rect) -> Point {
    Point {
        x: f64::from(geometry.x) + f64::from(geometry.width) / 2.0 - raw.x,
        y: f64::from(geometry.y) + f64::from(geometry.height) / 2.0 - raw.y,
    }
}

impl SessionPointerPlacement {
    fn center_on_primary_output(&mut self, size: Size) -> Point {
        let center = Point {
            x: f64::from(size.width.max(1)) / 2.0,
            y: f64::from(size.height.max(1)) / 2.0,
        };
        self.raw_position = Some(Point::default());
        self.offset = Some(center);
        self.position = Some(center);
        center
    }

    fn observe_raw(&mut self, raw: Point) {
        self.raw_position = Some(raw);
    }

    fn arm_at_focused_surface_center(
        &mut self,
        focused_surface: Option<SurfaceId>,
        input_layers: &[LayerSnapshot],
    ) -> Option<Point> {
        let geometry = focused_surface.and_then(|surface| {
            input_layers
                .iter()
                .find(|layer| layer.surface == surface)
                .map(|layer| layer.geometry)
        })?;
        let raw = self.raw_position.unwrap_or_default();
        let offset = pointer_offset_for_geometry(raw, geometry);
        let position = Point {
            x: raw.x + offset.x,
            y: raw.y + offset.y,
        };
        self.offset = Some(offset);
        self.position = Some(position);
        Some(position)
    }

    fn place(
        &mut self,
        raw: Point,
        focused_surface: Option<SurfaceId>,
        input_layers: &[LayerSnapshot],
    ) -> Point {
        self.observe_raw(raw);
        let offset = *self.offset.get_or_insert_with(|| {
            let Some(geometry) = focused_surface.and_then(|surface| {
                input_layers
                    .iter()
                    .find(|layer| layer.surface == surface)
                    .map(|layer| layer.geometry)
            }) else {
                return Point::default();
            };
            pointer_offset_for_geometry(raw, geometry)
        });
        let position = Point {
            x: raw.x + offset.x,
            y: raw.y + offset.y,
        };
        self.position = Some(position);
        position
    }
}

fn place_pointer_event_for_routing(
    event: &mut sophia_protocol::InputEventPacket,
    focused_surface: Option<SurfaceId>,
    input_layers: &[LayerSnapshot],
    pointer: &mut SessionPointerPlacement,
    buttons_only: bool,
) -> bool {
    if let Some(raw) = event.global_position {
        event.global_position = Some(pointer.place(raw, focused_surface, input_layers));
    }
    !(buttons_only && matches!(event.kind, sophia_protocol::InputEventKind::PointerMotion))
}

struct PhysicalInputRoutingContext<'a> {
    focus: &'a InputFocusState,
    committed_surfaces: &'a [CommittedSurfaceState],
    input_layers: &'a [LayerSnapshot],
    client_routes: &'a XAuthorityClientSurfaceRoutes,
    shortcuts: Option<&'a mut WmShortcutRouter>,
    input_sender: &'a SyncSender<XAuthorityRoutedInput>,
    modifiers: &'a mut XCoreKeyboardMapper,
    client_keys: &'a mut SessionClientKeyState,
    emergency_chord: &'a mut EmergencyChordState,
    virtual_terminal_chord: &'a mut VirtualTerminalChordState,
    pointer: &'a mut SessionPointerPlacement,
    pointer_routing_enabled: bool,
    pointer_proof_required: bool,
    pointer_buttons_only: bool,
    routing_mode: PhysicalInputRoutingMode,
    next_input_delivery: &'a mut u64,
    physical_text_proof: Option<&'a mut PhysicalTextProof>,
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
        client_routes,
        shortcuts,
        input_sender,
        modifiers,
        client_keys,
        emergency_chord,
        virtual_terminal_chord,
        pointer,
        pointer_routing_enabled,
        pointer_proof_required,
        pointer_buttons_only,
        routing_mode,
        next_input_delivery,
        physical_text_proof,
    } = context;
    route_input_events(
        events,
        focus,
        committed_surfaces,
        input_layers,
        client_routes,
        input_sender,
        modifiers,
        client_keys,
        emergency_chord,
        virtual_terminal_chord,
        shortcuts,
        pointer,
        pointer_routing_enabled,
        pointer_proof_required,
        pointer_buttons_only,
        routing_mode,
        next_input_delivery,
        physical_text_proof,
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
    client_keys: &mut SessionClientKeyState,
    emergency_chord: &mut EmergencyChordState,
    virtual_terminal_chord: &mut VirtualTerminalChordState,
    mut shortcuts: Option<&mut WmShortcutRouter>,
    pointer: &mut SessionPointerPlacement,
    pointer_routing_enabled: bool,
    pointer_proof_required: bool,
    pointer_buttons_only: bool,
    routing_mode: PhysicalInputRoutingMode,
    next_input_delivery: &mut u64,
    mut physical_text_proof: Option<&mut PhysicalTextProof>,
) -> Result<PhysicalInputRouteReport, Box<dyn std::error::Error>> {
    let mut report = PhysicalInputRouteReport {
        events: events.len(),
        wm_actions: Vec::new(),
        keys_observed: 0,
        keys_suppressed_no_focus: 0,
        keys_routed: 0,
        pointer_events: 0,
        pointer_buttons_observed: 0,
        pointer_axes_observed: 0,
        pointer_routed: 0,
        pointer_buttons_routed: 0,
        pointer_axes_routed: 0,
        deliveries: Vec::new(),
        emergency_exit: false,
        return_suppressed: false,
        virtual_terminal: None,
        virtual_terminal_modifier_releases: 0,
    };
    for mut event in events {
        match event.kind {
            sophia_protocol::InputEventKind::Key { keycode, pressed } => {
                report.keys_observed = report.keys_observed.saturating_add(1);
                match virtual_terminal_chord.observe(keycode, pressed) {
                    VirtualTerminalChordAction::Pass => {}
                    VirtualTerminalChordAction::Consume => continue,
                    VirtualTerminalChordAction::Activate(terminal) => {
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
                report.keys_routed = report.keys_routed.saturating_add(1);
                report.deliveries.push(delivery);
            }
            kind @ (sophia_protocol::InputEventKind::PointerMotion
            | sophia_protocol::InputEventKind::PointerButton { .. }
            | sophia_protocol::InputEventKind::PointerAxis { .. }) => {
                if matches!(
                    routing_mode,
                    PhysicalInputRoutingMode::Suppressed | PhysicalInputRoutingMode::ShortcutsOnly
                ) {
                    continue;
                }
                if let Some(raw) = event.global_position {
                    pointer.observe_raw(raw);
                }
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
                    PhysicalInputRoutingMode::CursorOnly
                        | PhysicalInputRoutingMode::ControlPlaneOnly
                ) {
                    if !is_button {
                        let focused_surface = focus.focused_surface(event.seat);
                        let _ = place_pointer_event_for_routing(
                            &mut event,
                            focused_surface,
                            input_layers,
                            pointer,
                            false,
                        );
                    }
                    continue;
                }
                let focused_surface = focus.focused_surface(event.seat);
                if !place_pointer_event_for_routing(
                    &mut event,
                    focused_surface,
                    input_layers,
                    pointer,
                    pointer_buttons_only,
                ) {
                    continue;
                }
                if !pointer_routing_enabled {
                    continue;
                }
                let route = sophia_engine::hit_test_scene_surface_for_input(&event, input_layers);
                let (Some(global), Some(local)) = (event.global_position, route.local_position)
                else {
                    continue;
                };
                let Some(surface) = route.target_surface else {
                    continue;
                };
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
                        target_surface: surface,
                        global_position: global,
                        local_position: local,
                        kind,
                    },
                    delivery: Some(delivery),
                    mode: XAuthorityRoutedInputMode::Deliver,
                })?;
                report.pointer_routed = report.pointer_routed.saturating_add(1);
                if is_button {
                    report.pointer_buttons_routed = report.pointer_buttons_routed.saturating_add(1);
                }
                if is_axis {
                    report.pointer_axes_routed = report.pointer_axes_routed.saturating_add(1);
                }
                report.deliveries.push(delivery);
            }
        }
    }
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

fn clear_removed_surface_keys(
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
    Ok(client_keys.clear_surface(surface))
}
