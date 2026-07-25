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
    let events = vec![InputEventPacket {
        serial: 1,
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        time_msec: 1,
        kind: InputEventKind::PointerMotion,
        global_position: Some(Point { x: 18.0, y: -5.0 }),
        target_surface: None,
        local_position: None,
    }];
    let (input_sender, input_receiver) = sync_channel(1);
    let mut modifiers = XCoreKeyboardMapper::new();
    let mut emergency = super::super::EmergencyChordState::awaiting_arm();
    let mut virtual_terminal = sophia_cli::session_keyboard::VirtualTerminalChordState::default();
    let mut pointer = SessionPointerPlacement::default();
    pointer.center_on_primary_output(Size {
        width: 2560,
        height: 1440,
    });
    let initial_position = pointer.position;
    let mut next_delivery = 1;

    let report = route_input_events(
        events,
        &InputFocusState::new(),
        &[],
        &[],
        &XAuthorityClientSurfaceRoutes::default(),
        &input_sender,
        &mut modifiers,
        &mut emergency,
        &mut virtual_terminal,
        None,
        &mut pointer,
        false,
        false,
        false,
        PhysicalInputRoutingMode::Full,
        &mut next_delivery,
        None,
    )
    .unwrap();

    assert_eq!(report.pointer_events, 1);
    assert_eq!(report.pointer_routed, 0);
    assert_ne!(pointer.position, initial_position);
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
        &mut emergency,
        &mut virtual_terminal,
        None,
        &mut pointer,
        false,
        false,
        false,
        PhysicalInputRoutingMode::Full,
        &mut next_delivery,
        None,
    )
    .unwrap();

    assert_eq!(report.keys_suppressed_no_focus, 1);
    assert_eq!(report.keys_routed, 0);
    assert!(input_receiver.try_recv().is_err());
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
