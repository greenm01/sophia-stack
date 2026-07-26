use super::*;
use sophia_engine::InputFocusDecision;
use sophia_protocol::TransactionId;

#[test]
fn unknown_surface_keeps_wm_focus_request_pending() {
    let request = (TransactionId::from_raw(7), SurfaceId::new(41, 1));
    assert_eq!(
        pending_wm_focus_after_engine_decision(request, InputFocusDecision::UnknownSurface),
        Some(request),
    );
    assert_eq!(
        pending_wm_focus_after_engine_decision(request, InputFocusDecision::Focused),
        None,
    );
}

#[test]
fn held_application_pointer_delivery_does_not_freeze_cursor() {
    let action = WmActionId::from_raw(7);
    let registry = WmShortcutRegistry::from_hello(&WmHello {
        api_version: WM_API_VERSION,
        capabilities: WmCapabilities::all_supported(),
        policy_generation: 1,
        chrome: sophia_protocol::WmChromeStyle::default(),
        bindings: vec![WmBindingRegistration {
            action,
            keycode: 28,
            modifiers: WmModifierMask {
                bits: WmModifierMask::SUPER,
            },
        }],
    })
    .unwrap();
    let mut shortcuts = WmShortcutRouter::new(registry);
    let events = vec![
        InputEventPacket {
            serial: 1,
            seat: SeatId::from_raw(1),
            device: DeviceId::from_raw(2),
            time_msec: 1,
            kind: InputEventKind::PointerMotion,
            global_position: Some(Point { x: 18.0, y: -5.0 }),
            target_surface: None,
            local_position: None,
        },
        InputEventPacket {
            serial: 2,
            seat: SeatId::from_raw(1),
            device: DeviceId::from_raw(1),
            time_msec: 2,
            kind: InputEventKind::Key {
                keycode: 125,
                pressed: true,
            },
            global_position: None,
            target_surface: None,
            local_position: None,
        },
        InputEventPacket {
            serial: 3,
            seat: SeatId::from_raw(1),
            device: DeviceId::from_raw(1),
            time_msec: 3,
            kind: InputEventKind::Key {
                keycode: 28,
                pressed: true,
            },
            global_position: None,
            target_surface: None,
            local_position: None,
        },
    ];
    let (input_sender, input_receiver) = sync_channel(1);
    let mut modifiers = XCoreKeyboardMapper::new();
    let (mut key_repeat, key_repeat_map) = super::test_key_repeat_parts();
    let mut client_keys = SessionClientKeyState::default();
    let mut emergency = super::super::EmergencyChordState::awaiting_arm();
    let mut virtual_terminal = sophia_cli::session_keyboard::VirtualTerminalChordState::default();
    let mut pointer = SessionPointerPlacement::default();
    pointer.center_on_primary_output(Size {
        width: 2560,
        height: 1440,
    });
    let initial_position = pointer.position();
    let mut next_delivery = 1;

    let report = route_input_events(
        events,
        &InputFocusState::new(),
        &[],
        &[],
        &XAuthorityClientSurfaceRoutes::default(),
        &input_sender,
        &mut modifiers,
        &mut key_repeat,
        &key_repeat_map,
        &mut client_keys,
        &mut emergency,
        &mut virtual_terminal,
        Some(&mut shortcuts),
        &mut pointer,
        false,
        false,
        false,
        PhysicalInputRoutingMode::Full,
        &mut next_delivery,
        0,
        None,
    )
    .unwrap();

    assert_eq!(report.pointer_events, 1);
    assert_eq!(report.pointer_routed, 0);
    assert_eq!(report.wm_actions, [action]);
    assert_eq!(report.keys_routed, 0);
    assert_ne!(pointer.position(), initial_position);
    assert!(input_receiver.try_recv().is_err());
}

#[test]
fn full_routing_suppresses_keyboard_input_when_workspace_focus_is_clear() {
    let events = vec![InputEventPacket {
        serial: 1,
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(1),
        time_msec: 1,
        kind: InputEventKind::Key {
            keycode: 30,
            pressed: true,
        },
        global_position: None,
        target_surface: None,
        local_position: None,
    }];
    let (input_sender, input_receiver) = sync_channel(1);
    let mut modifiers = XCoreKeyboardMapper::new();
    let (mut key_repeat, key_repeat_map) = super::test_key_repeat_parts();
    let mut client_keys = SessionClientKeyState::default();
    let mut emergency = super::super::EmergencyChordState::awaiting_arm();
    let mut virtual_terminal = sophia_cli::session_keyboard::VirtualTerminalChordState::default();
    let mut pointer = SessionPointerPlacement::default();
    let mut next_delivery = 1;

    let report = route_input_events(
        events,
        &InputFocusState::new(),
        &[],
        &[],
        &XAuthorityClientSurfaceRoutes::default(),
        &input_sender,
        &mut modifiers,
        &mut key_repeat,
        &key_repeat_map,
        &mut client_keys,
        &mut emergency,
        &mut virtual_terminal,
        None,
        &mut pointer,
        false,
        false,
        false,
        PhysicalInputRoutingMode::Full,
        &mut next_delivery,
        0,
        None,
    )
    .unwrap();

    assert_eq!(report.keys_suppressed_no_focus, 1);
    assert_eq!(report.keys_routed, 0);
    assert!(input_receiver.try_recv().is_err());
}

#[test]
fn full_routing_suppresses_pointer_buttons_when_workspace_has_no_target() {
    let events = [true, false]
        .into_iter()
        .enumerate()
        .map(|(index, pressed)| InputEventPacket {
            serial: u64::try_from(index + 1).unwrap(),
            seat: SeatId::from_raw(1),
            device: DeviceId::from_raw(2),
            time_msec: u64::try_from(index + 1).unwrap(),
            kind: InputEventKind::PointerButton {
                button: 0x110,
                pressed,
            },
            global_position: Some(Point { x: 64.0, y: 64.0 }),
            target_surface: None,
            local_position: None,
        })
        .collect();
    let (input_sender, input_receiver) = sync_channel(2);
    let mut modifiers = XCoreKeyboardMapper::new();
    let (mut key_repeat, key_repeat_map) = super::test_key_repeat_parts();
    let mut client_keys = SessionClientKeyState::default();
    let mut emergency = super::super::EmergencyChordState::awaiting_arm();
    let mut virtual_terminal = sophia_cli::session_keyboard::VirtualTerminalChordState::default();
    let mut pointer = SessionPointerPlacement::default();
    pointer.center_on_primary_output(Size {
        width: 2560,
        height: 1440,
    });
    let mut next_delivery = 1;

    let report = route_input_events(
        events,
        &InputFocusState::new(),
        &[],
        &[],
        &XAuthorityClientSurfaceRoutes::default(),
        &input_sender,
        &mut modifiers,
        &mut key_repeat,
        &key_repeat_map,
        &mut client_keys,
        &mut emergency,
        &mut virtual_terminal,
        None,
        &mut pointer,
        true,
        false,
        false,
        PhysicalInputRoutingMode::Full,
        &mut next_delivery,
        0,
        None,
    )
    .unwrap();

    assert_eq!(report.pointer_buttons_observed, 2);
    assert_eq!(report.pointer_buttons_suppressed_no_target, 2);
    assert_eq!(report.pointer_buttons_suppressed_by_policy, 0);
    assert_eq!(report.pointer_buttons_routed, 0);
    assert!(report.pointer_focus_targets.is_empty());
    assert!(report.deliveries.is_empty());
    assert!(input_receiver.try_recv().is_err());
}

#[test]
fn routed_keyboard_report_retains_the_opaque_focus_target() {
    let seat = SeatId::from_raw(1);
    let surface = SurfaceId::new(41, 1);
    let geometry = Rect {
        x: 0,
        y: 0,
        width: 640,
        height: 480,
    };
    let committed = [CommittedSurfaceState {
        surface,
        committed_generation: 1,
        geometry,
        buffer: BufferSource::CpuBuffer { handle: 1 },
        damage: Region::single(geometry),
    }];
    let mut focus = InputFocusState::new();
    assert_eq!(
        focus.focus_surface(seat, surface, &committed),
        InputFocusDecision::Focused
    );
    let events = vec![InputEventPacket {
        serial: 1,
        seat,
        device: DeviceId::from_raw(1),
        time_msec: 1,
        kind: InputEventKind::Key {
            keycode: 30,
            pressed: true,
        },
        global_position: None,
        target_surface: None,
        local_position: None,
    }];
    let (input_sender, input_receiver) = sync_channel(1);
    let mut modifiers = XCoreKeyboardMapper::new();
    let (mut key_repeat, key_repeat_map) = super::test_key_repeat_parts();
    let mut client_keys = SessionClientKeyState::default();
    let mut emergency = super::super::EmergencyChordState::awaiting_arm();
    let mut virtual_terminal = sophia_cli::session_keyboard::VirtualTerminalChordState::default();
    let mut pointer = SessionPointerPlacement::default();
    let mut next_delivery = 1;

    let report = route_input_events(
        events,
        &focus,
        &committed,
        &[],
        &XAuthorityClientSurfaceRoutes::default(),
        &input_sender,
        &mut modifiers,
        &mut key_repeat,
        &key_repeat_map,
        &mut client_keys,
        &mut emergency,
        &mut virtual_terminal,
        None,
        &mut pointer,
        false,
        false,
        false,
        PhysicalInputRoutingMode::Full,
        &mut next_delivery,
        0,
        None,
    )
    .unwrap();

    assert_eq!(report.keys_routed, 1);
    assert_eq!(report.key_targets, [surface]);
    assert_eq!(
        input_receiver.try_recv().unwrap().request.target_surface,
        surface
    );
}

#[test]
fn stable_focused_gpu_frame_proves_post_input_pixels() {
    let input_surface = SurfaceId::new(41, 1);
    assert!(stable_gpu_frame_proves_post_input_pixels(
        true,
        Some(input_surface),
        input_surface,
        true,
    ));
    assert!(!stable_gpu_frame_proves_post_input_pixels(
        false,
        Some(input_surface),
        input_surface,
        true,
    ));
    assert!(!stable_gpu_frame_proves_post_input_pixels(
        true,
        Some(input_surface),
        SurfaceId::new(42, 1),
        true,
    ));
    assert!(!stable_gpu_frame_proves_post_input_pixels(
        true,
        Some(input_surface),
        input_surface,
        false,
    ));
}
