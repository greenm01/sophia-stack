mod support;
use sophia_engine::confine_pointer_to_outputs;
use support::*;

#[test]
fn physical_pointer_is_confined_to_the_nearest_visible_output() {
    let outputs = [
        Rect {
            x: 0,
            y: 0,
            width: 2560,
            height: 1440,
        },
        Rect {
            x: 2560,
            y: 0,
            width: 1920,
            height: 1080,
        },
    ];

    assert_eq!(
        confine_pointer_to_outputs(
            Point {
                x: -500.0,
                y: 900.0
            },
            &outputs
        ),
        Some(Point { x: 0.0, y: 900.0 }),
    );
    assert_eq!(
        confine_pointer_to_outputs(
            Point {
                x: 6000.0,
                y: 1300.0,
            },
            &outputs,
        ),
        Some(Point {
            x: 4479.0,
            y: 1079.0,
        }),
    );
    assert_eq!(
        confine_pointer_to_outputs(
            Point {
                x: 3000.0,
                y: 1300.0,
            },
            &outputs,
        ),
        Some(Point {
            x: 3000.0,
            y: 1079.0,
        }),
    );

    let raw_at_right_edge = Point {
        x: 6000.0,
        y: 900.0,
    };
    let confined =
        confine_pointer_to_outputs(raw_at_right_edge, &outputs).expect("outputs are valid");
    let corrected_offset = Point {
        x: confined.x - raw_at_right_edge.x,
        y: confined.y - raw_at_right_edge.y,
    };
    let reversed_raw = Point {
        x: raw_at_right_edge.x - 10.0,
        y: raw_at_right_edge.y,
    };
    assert_eq!(
        confine_pointer_to_outputs(
            Point {
                x: reversed_raw.x + corrected_offset.x,
                y: reversed_raw.y + corrected_offset.y,
            },
            &outputs,
        ),
        Some(Point {
            x: 4469.0,
            y: 900.0,
        }),
    );
}

#[test]
fn physical_pointer_confinement_rejects_invalid_or_missing_geometry() {
    assert_eq!(
        confine_pointer_to_outputs(
            Point {
                x: f64::NAN,
                y: 10.0,
            },
            &[Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            }],
        ),
        None,
    );
    assert_eq!(
        confine_pointer_to_outputs(Point { x: 10.0, y: 10.0 }, &[]),
        None,
    );
}

#[test]
fn routed_input_coalescer_keeps_latest_stable_motion_until_frame() {
    let mut coalescer = RoutedInputCoalescer::new();

    assert_eq!(
        coalescer.push(motion_event(1, 10.0, 10.0), route(1, 0x30, 10.0, 10.0)),
        RoutedInputQueueAction::BufferedMotion
    );
    assert_eq!(
        coalescer.push(motion_event(2, 20.0, 20.0), route(2, 0x30, 20.0, 20.0)),
        RoutedInputQueueAction::BufferedMotion
    );

    let flush = coalescer.flush_frame().unwrap();

    assert_eq!(flush.reason, RoutedInputFlushReason::FrameBoundary);
    assert_eq!(flush.inputs.len(), 1);
    assert_eq!(flush.inputs[0].event.serial, 2);
    assert!(!coalescer.has_pending_motion());
}

#[test]
fn routed_input_coalescer_flushes_on_target_crossing() {
    let mut coalescer = RoutedInputCoalescer::new();
    coalescer.push(motion_event(1, 10.0, 10.0), route(1, 0x30, 10.0, 10.0));

    let action = coalescer.push(motion_event(2, 11.0, 11.0), route(2, 0x40, 1.0, 1.0));

    let RoutedInputQueueAction::Flushed(flush) = action else {
        panic!("expected target crossing flush");
    };
    assert_eq!(flush.reason, RoutedInputFlushReason::TargetCrossing);
    assert_eq!(
        flush
            .inputs
            .iter()
            .map(|input| input.event.serial)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert!(!coalescer.has_pending_motion());
}

#[test]
fn routed_input_coalescer_flushes_for_button_and_key_events() {
    let mut coalescer = RoutedInputCoalescer::new();
    coalescer.push(motion_event(1, 10.0, 10.0), route(1, 0x30, 10.0, 10.0));

    let button = input_event(
        2,
        InputEventKind::PointerButton {
            button: 1,
            pressed: true,
        },
        10.0,
        10.0,
    );
    let action = coalescer.push(button, route(2, 0x30, 10.0, 10.0));

    let RoutedInputQueueAction::Flushed(flush) = action else {
        panic!("expected button flush");
    };
    assert_eq!(flush.reason, RoutedInputFlushReason::StateChangingInput);
    assert_eq!(flush.inputs.len(), 2);
    assert!(!coalescer.has_pending_motion());

    let key = input_event(
        3,
        InputEventKind::Key {
            keycode: 38,
            pressed: true,
        },
        0.0,
        0.0,
    );
    let action = coalescer.push(key, route(3, 0x30, 0.0, 0.0));

    let RoutedInputQueueAction::Flushed(flush) = action else {
        panic!("expected key flush");
    };
    assert_eq!(flush.reason, RoutedInputFlushReason::StateChangingInput);
    assert_eq!(flush.inputs.len(), 1);
    assert_eq!(flush.inputs[0].event.serial, 3);
}

#[test]
fn routed_input_coalescer_flushes_for_drag_grab_and_focus_barriers() {
    for reason in [
        RoutedInputFlushReason::DragStateChanged,
        RoutedInputFlushReason::GrabChanged,
        RoutedInputFlushReason::FocusChanged,
    ] {
        let mut coalescer = RoutedInputCoalescer::new();
        coalescer.push(motion_event(1, 10.0, 10.0), route(1, 0x30, 10.0, 10.0));

        let flush = coalescer.flush_barrier(reason).unwrap();

        assert_eq!(flush.reason, reason);
        assert_eq!(flush.inputs.len(), 1);
        assert_eq!(flush.inputs[0].event.serial, 1);
        assert!(!coalescer.has_pending_motion());
    }
}

#[test]
fn transformed_scene_hit_test_routes_to_topmost_layer_local_coordinates() {
    let mut lower = test_layer(0, 0, 0, Region::empty());
    lower.authority_local_id = Some(AuthorityLocalId::new(0x20, 1));
    let mut upper = test_layer(1, 10, 0, Region::empty());
    upper.authority_local_id = Some(AuthorityLocalId::new(0x30, 1));
    upper.transform = scale_translate_transform(2.0, 30.0, 40.0);
    let event = motion_event(70, 50.0, 60.0);

    let route = hit_test_scene_for_input(&event, &[lower, upper]);

    assert_eq!(route.outcome, InputRouteOutcome::Routed);
    assert_eq!(route.target_surface, Some(SurfaceId::new(1, 1)));
    assert_eq!(route.global_position, Point { x: 50.0, y: 60.0 });
    assert_eq!(route.local_position, Some(Point { x: 10.0, y: 10.0 }));
    assert_eq!(route.transform, scale_translate_transform(2.0, 30.0, 40.0));
}

#[test]
fn transformed_scene_hit_test_reports_no_target_for_miss() {
    let mut layer = test_layer(0, 0, 0, Region::empty());
    layer.authority_local_id = Some(AuthorityLocalId::new(0x20, 1));
    layer.transform = scale_translate_transform(2.0, 30.0, 40.0);
    let event = motion_event(71, 10.0, 10.0);

    let route = hit_test_scene_for_input(&event, &[layer]);

    assert_eq!(route.outcome, InputRouteOutcome::NoTarget);
    assert_eq!(route.target_surface, None);
    assert_eq!(route.local_position, None);
}

#[test]
fn surface_hit_test_routes_without_exposing_authority_window_identity() {
    let layer = test_layer(0, 0, 0, Region::empty());
    let event = motion_event(73, 10.0, 10.0);

    let route = hit_test_scene_surface_for_input(&event, &[layer]);

    assert_eq!(route.outcome, InputRouteOutcome::Routed);
    assert_eq!(route.target_surface, Some(SurfaceId::new(0, 1)));
    assert_eq!(route.local_position, Some(Point { x: 10.0, y: 10.0 }));
}

#[test]
fn transformed_scene_hit_test_feeds_routed_input_request_generation() {
    let mut layer = test_layer(0, 0, 0, Region::empty());
    layer.authority_local_id = Some(AuthorityLocalId::new(0x30, 1));
    layer.transform = scale_translate_transform(2.0, 30.0, 40.0);
    let event = motion_event(72, 54.0, 64.0);

    let route = hit_test_scene_for_input(&event, &[layer]);
    let request = routed_input_request_from_physical_event(&event, &route).unwrap();

    assert_eq!(request.serial, 72);
    assert_eq!(request.target_surface, SurfaceId::new(0, 1));
    assert_eq!(request.local_position, Point { x: 12.0, y: 12.0 });
    assert_eq!(request.kind, InputEventKind::PointerMotion);
}

#[test]
fn physical_input_route_becomes_authority_request() {
    let event = motion_event(77, 25.0, 35.0);
    let route = route(77, 0x44, 5.0, 6.0);

    let request = routed_input_request_from_physical_event(&event, &route).unwrap();

    assert_eq!(request.serial, 77);
    assert_eq!(request.seat, event.seat);
    assert_eq!(request.device, event.device);
    assert_eq!(request.time_msec, event.time_msec);
    assert_eq!(request.target_surface, SurfaceId::new(0x44, 1));
    assert_eq!(request.local_position, Point { x: 5.0, y: 6.0 });
    assert_eq!(request.kind, InputEventKind::PointerMotion);
}

#[test]
fn selected_surface_route_preserves_drag_outside_its_geometry() {
    let target = SurfaceId::new(0x44, 1);
    let layer = test_layer(0x44, 1, 10, Region::empty());
    let event = motion_event(78, 150.0, 120.0);

    let route = route_scene_surface_for_input(&event, &[layer], target);

    assert_eq!(route.target_surface, Some(target));
    assert_eq!(route.global_position, Point { x: 150.0, y: 120.0 });
    assert_eq!(route.local_position, Some(Point { x: 140.0, y: 120.0 }));
}

#[test]
fn physical_input_flush_becomes_authority_requests_after_state_change() {
    let mut coalescer = RoutedInputCoalescer::new();
    coalescer.push(motion_event(1, 10.0, 10.0), route(1, 0x30, 10.0, 10.0));
    let button = input_event(
        2,
        InputEventKind::PointerButton {
            button: 1,
            pressed: true,
        },
        10.0,
        10.0,
    );

    let RoutedInputQueueAction::Flushed(flush) = coalescer.push(button, route(2, 0x30, 10.0, 10.0))
    else {
        panic!("expected state-changing flush");
    };
    let requests = routed_input_requests_from_flush(&flush).unwrap();

    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].serial, 1);
    assert_eq!(requests[1].serial, 2);
    assert_eq!(
        requests[1].kind,
        InputEventKind::PointerButton {
            button: 1,
            pressed: true
        }
    );
}

#[test]
fn physical_input_route_rejects_malformed_routes() {
    let event = motion_event(1, 10.0, 10.0);
    let mut mismatched = route(2, 0x30, 10.0, 10.0);
    assert_eq!(
        routed_input_request_from_physical_event(&event, &mismatched),
        Err(RoutedInputRequestError::SerialMismatch)
    );

    mismatched.input_serial = 1;
    mismatched.outcome = InputRouteOutcome::NoTarget;
    assert_eq!(
        routed_input_request_from_physical_event(&event, &mismatched),
        Err(RoutedInputRequestError::RouteNotAccepted)
    );

    mismatched.outcome = InputRouteOutcome::Routed;
    mismatched.target_surface = None;
    assert_eq!(
        routed_input_request_from_physical_event(&event, &mismatched),
        Err(RoutedInputRequestError::MissingTargetSurface)
    );

    mismatched.target_surface = Some(SurfaceId::new(0x30, 1));
    mismatched.local_position = None;
    assert_eq!(
        routed_input_request_from_physical_event(&event, &mismatched),
        Err(RoutedInputRequestError::MissingLocalPosition)
    );
}

#[test]
fn pointer_focus_handoff_releases_ordered_input_only_after_focus_applies() {
    let target = SurfaceId::new(8, 1);
    let mut handoff = PointerFocusHandoffState::default();
    let press = handoff_request(
        1,
        target,
        InputEventKind::PointerButton {
            button: 0x110,
            pressed: true,
        },
    );
    let motion = handoff_request(2, target, InputEventKind::PointerMotion);
    let release = handoff_request(
        3,
        target,
        InputEventKind::PointerButton {
            button: 0x110,
            pressed: false,
        },
    );

    handoff.begin(target, 100, press).unwrap();
    handoff.defer(motion).unwrap();
    handoff.defer(release).unwrap();

    assert!(handoff.take_ready(Some(SurfaceId::new(9, 1))).is_none());
    let ready = handoff.take_ready(Some(target)).unwrap();
    assert_eq!(
        ready
            .into_iter()
            .map(|request| request.serial)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(handoff.target(), None);
}

#[test]
fn pointer_focus_handoff_expires_without_frontend_acknowledgment() {
    let target = SurfaceId::new(8, 1);
    let mut handoff = PointerFocusHandoffState::default();
    handoff
        .begin(
            target,
            100,
            handoff_request(1, target, InputEventKind::PointerMotion),
        )
        .unwrap();

    assert!(!handoff.expire(2_099));
    assert!(handoff.expire(2_100));
    assert_eq!(handoff.target(), None);
    assert!(handoff.take_ready(Some(target)).is_none());
}

fn handoff_request(
    serial: u64,
    target_surface: SurfaceId,
    kind: InputEventKind,
) -> RoutedInputRequest {
    RoutedInputRequest {
        serial,
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        time_msec: serial,
        target_surface,
        global_position: Point { x: 5.0, y: 6.0 },
        local_position: Point { x: 5.0, y: 6.0 },
        kind,
    }
}
