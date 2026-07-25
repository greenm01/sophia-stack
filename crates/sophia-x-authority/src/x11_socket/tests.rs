#![cfg(all(test, unix))]

use super::*;
use sophia_protocol::{DeviceId, Point};
use std::sync::mpsc::sync_channel;

#[test]
fn listener_transaction_ids_are_global_across_client_workers() {
    let state = X11CoreSocketServerState::new();
    let first_worker = state.clone();
    let second_worker = state.clone();

    let first = first_worker.allocate_transaction().unwrap();
    let second = second_worker.allocate_transaction().unwrap();
    let third = first_worker.allocate_transaction().unwrap();

    assert_ne!(first, second);
    assert_ne!(second, third);
    assert_eq!(first.raw() + 1, second.raw());
    assert_eq!(second.raw() + 1, third.raw());
}

#[test]
fn routed_input_discards_another_clients_event() {
    let first = XServerFrontendClientId(1);
    let second = XServerFrontendClientId(2);
    let (sender, receiver) = sync_channel(2);
    sender
        .send(XAuthorityClientInputEvent {
            client: second,
            event: XAuthorityKeyEvent {
                keycode: 24,
                pressed: true,
                state: 0,
                modifiers_after: 0,
                time_msec: 1,
            }
            .into(),
            target_window: None,
            xi_event_type: None,
            xi_transition_mask: 0,
            delivery: None,
        })
        .unwrap();
    sender
        .send(XAuthorityClientInputEvent {
            client: first,
            event: XAuthorityKeyEvent {
                keycode: 25,
                pressed: true,
                state: 0,
                modifiers_after: 0,
                time_msec: 2,
            }
            .into(),
            target_window: None,
            xi_event_type: None,
            xi_transition_mask: 0,
            delivery: None,
        })
        .unwrap();

    let receiver = X11InputEventReceiver::Routed {
        receiver,
        deliveries: None,
    };
    assert_eq!(receiver.recv_timeout(first), Err(RecvTimeoutError::Timeout));
    assert_eq!(
        receiver.recv_timeout(first).unwrap(),
        (
            XAuthorityInputEvent::Key(XAuthorityKeyEvent {
                keycode: 25,
                pressed: true,
                state: 0,
                modifiers_after: 0,
                time_msec: 2,
            }),
            None,
            None,
            0,
            None,
        )
    );
}

#[test]
fn routed_control_discards_another_clients_command_and_labels_its_ack() {
    let first = XServerFrontendClientId(1);
    let second = XServerFrontendClientId(2);
    let surface = SurfaceId::new(44, 1);
    let (command_sender, command_receiver) = sync_channel(2);
    let (ack_sender, ack_receiver) = sync_channel(1);
    let command = XAuthorityControlCommand::FocusSurface {
        transaction: TransactionId::from_raw(7),
        surface,
    };
    command_sender
        .send(XAuthorityClientControlCommand {
            client: second,
            command,
        })
        .unwrap();
    command_sender
        .send(XAuthorityClientControlCommand {
            client: first,
            command,
        })
        .unwrap();

    let channels = X11ControlChannels::Routed {
        receiver: command_receiver,
        acknowledgements: ack_sender,
    };
    assert_eq!(channels.recv_timeout(first), Err(RecvTimeoutError::Timeout));
    assert_eq!(channels.recv_timeout(first).unwrap(), command);
    let acknowledgement = XAuthorityControlAck {
        transaction: command.transaction(),
        surface: command.surface(),
        outcome: XAuthorityControlOutcome::Delivered,
    };
    channels.send_ack(first, acknowledgement).unwrap();
    assert_eq!(
        ack_receiver.recv().unwrap(),
        XAuthorityClientControlAck {
            client: first,
            acknowledgement,
        }
    );
}

#[test]
fn route_broker_delivers_to_the_registered_client_only() {
    let client = XServerFrontendClientId(9);
    let mut broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(2).unwrap());
    let (registration, channels) = broker.registry.register_client(client).unwrap();
    let input = XAuthorityInputEvent::Key(XAuthorityKeyEvent {
        keycode: 38,
        pressed: true,
        state: 0,
        modifiers_after: 0,
        time_msec: 3,
    });
    let command = XAuthorityControlCommand::FocusSurface {
        transaction: TransactionId::from_raw(8),
        surface: SurfaceId::new(45, 1),
    };

    broker
        .input_sender()
        .send(XAuthorityClientInputEvent {
            client,
            event: input,
            target_window: None,
            xi_event_type: None,
            xi_transition_mask: 0,
            delivery: None,
        })
        .unwrap();
    broker
        .control_sender()
        .send(XAuthorityClientControlCommand { client, command })
        .unwrap();

    assert_eq!(broker.route_pending(), Ok(2));
    assert_eq!(
        channels.input.recv().unwrap(),
        XAuthorityClientInputEvent {
            client,
            event: input,
            target_window: None,
            xi_event_type: None,
            xi_transition_mask: 0,
            delivery: None,
        }
    );
    assert_eq!(channels.control.recv().unwrap(), command);
    let acknowledgement = XAuthorityControlAck {
        transaction: command.transaction(),
        surface: command.surface(),
        outcome: XAuthorityControlOutcome::Delivered,
    };
    let channels = X11ControlChannels::ClientBound {
        receiver: channels.control,
        acknowledgements: broker.registry.acknowledgement_sender.clone(),
    };
    channels.send_ack(client, acknowledgement).unwrap();
    assert_eq!(
        broker
            .recv_control_ack_timeout(Duration::from_millis(1))
            .unwrap(),
        XAuthorityClientControlAck {
            client,
            acknowledgement,
        }
    );
    assert_eq!(broker.registered_client_count(), 1);

    drop(registration);
    assert_eq!(broker.registered_client_count(), 0);
    broker
        .input_sender()
        .send(XAuthorityClientInputEvent {
            client,
            event: input,
            target_window: None,
            xi_event_type: None,
            xi_transition_mask: 0,
            delivery: None,
        })
        .unwrap();
    assert_eq!(
        broker.route_pending(),
        Err(XServerFrontendRouteError::UnknownClient { client })
    );
}

#[test]
fn clearing_old_present_selection_preserves_active_window_feedback() {
    let namespace = NamespaceId::from_raw(10);
    let client = XServerFrontendClientId(9);
    let surface = SurfaceId::new(11, 1);
    let bootstrap_window = XResourceId::new(0x200009, 1);
    let bootstrap_event = XResourceId::new(0x20000d, 1);
    let main_window = XResourceId::new(0x200010, 1);
    let main_event = XResourceId::new(0x200014, 1);
    let pixmap = XResourceId::new(0x200015, 1);
    let idle_fence = XResourceId::new(0x200016, 1);
    let transaction = TransactionId::from_raw(202);
    let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(8).unwrap());
    let (_registration, channels) = broker.registry.register_client(client).unwrap();
    broker
        .registry
        .register_surface(client, namespace, surface, main_window)
        .unwrap();

    broker
        .registry
        .select_present_input(client, bootstrap_event, bootstrap_window, 7)
        .unwrap();
    broker
        .registry
        .select_present_input(client, main_event, main_window, 7)
        .unwrap();
    broker
        .registry
        .select_present_input(client, bootstrap_event, bootstrap_window, 0)
        .unwrap();
    broker
        .registry
        .queue_present(
            transaction,
            client,
            main_window,
            pixmap,
            1,
            Some(idle_fence),
        )
        .unwrap();

    assert_eq!(
        broker.route_present_complete(
            transaction,
            1_188_203,
            7_668_086,
            XPresentCompletionMode::Flip,
        ),
        Ok(true)
    );
    assert_eq!(
        channels.protocol.recv().unwrap(),
        XClientEvent::PresentCompleteNotify {
            sequence: 0,
            event_id: main_event,
            window: main_window,
            serial: 1,
            ust: 1_188_203,
            msc: 7_668_086,
            mode: XPresentCompletionMode::Flip as u8,
        }
    );
    assert_eq!(broker.route_present_idle(transaction), Ok(true));
    assert_eq!(
        channels.protocol.recv().unwrap(),
        XClientEvent::PresentIdleNotify {
            sequence: 0,
            event_id: main_event,
            window: main_window,
            serial: 1,
            pixmap,
            idle_fence: Some(idle_fence),
        }
    );
}

#[test]
fn present_feedback_reaches_every_matching_event_selection() {
    let namespace = NamespaceId::from_raw(10);
    let client = XServerFrontendClientId(10);
    let surface = SurfaceId::new(12, 1);
    let window = XResourceId::new(0x300010, 1);
    let first_event = XResourceId::new(0x300014, 1);
    let second_event = XResourceId::new(0x300015, 1);
    let pixmap = XResourceId::new(0x300016, 1);
    let transaction = TransactionId::from_raw(203);
    let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(8).unwrap());
    let (registration, channels) = broker.registry.register_client(client).unwrap();
    broker
        .registry
        .register_surface(client, namespace, surface, window)
        .unwrap();
    for event_id in [first_event, second_event] {
        broker
            .registry
            .select_present_input(client, event_id, window, 7)
            .unwrap();
    }
    broker
        .registry
        .queue_present(transaction, client, window, pixmap, 2, None)
        .unwrap();

    assert_eq!(
        broker.route_present_complete(transaction, 10, 20, XPresentCompletionMode::Flip),
        Ok(true)
    );
    for event_id in [first_event, second_event] {
        assert!(matches!(
            channels.protocol.recv().unwrap(),
            XClientEvent::PresentCompleteNotify {
                event_id: routed_event,
                ..
            } if routed_event == event_id
        ));
    }
    assert_eq!(broker.route_present_idle(transaction), Ok(true));
    for event_id in [first_event, second_event] {
        assert!(matches!(
            channels.protocol.recv().unwrap(),
            XClientEvent::PresentIdleNotify {
                event_id: routed_event,
                ..
            } if routed_event == event_id
        ));
    }

    let disconnected = TransactionId::from_raw(204);
    broker
        .registry
        .queue_present(disconnected, client, window, pixmap, 3, None)
        .unwrap();
    drop(registration);
    assert_eq!(
        broker.route_present_complete(disconnected, 30, 40, XPresentCompletionMode::Flip,),
        Ok(false)
    );
}

#[test]
fn route_broker_fails_closed_when_a_client_queue_is_backpressured() {
    let client = XServerFrontendClientId(10);
    let mut broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(1).unwrap());
    let (_registration, _channels) = broker.registry.register_client(client).unwrap();
    for time_msec in [4, 5] {
        broker
            .input_sender()
            .send(XAuthorityClientInputEvent {
                client,
                event: XAuthorityKeyEvent {
                    keycode: 39,
                    pressed: true,
                    state: 0,
                    modifiers_after: 0,
                    time_msec,
                }
                .into(),
                target_window: None,
                xi_event_type: None,
                xi_transition_mask: 0,
                delivery: None,
            })
            .unwrap();
        if time_msec == 4 {
            assert_eq!(broker.route_pending(), Ok(1));
        }
    }

    assert_eq!(
        broker.route_pending(),
        Err(XServerFrontendRouteError::ClientQueueFull { client })
    );
}

#[test]
fn route_broker_reports_rejected_delivery_for_an_unknown_client() {
    let client = XServerFrontendClientId(12);
    let (control_ack_sender, _control_ack_receiver) = sync_channel(1);
    let (delivery_sender, delivery_receiver) = sync_channel(1);
    let mut broker = XServerFrontendRouteBroker::with_control_and_input_delivery_senders(
        NonZeroUsize::new(1).unwrap(),
        control_ack_sender,
        delivery_sender,
    );
    let delivery = XAuthorityInputDeliveryId::from_raw(7);
    broker
        .input_sender()
        .send(XAuthorityClientInputEvent {
            client,
            event: XAuthorityKeyEvent {
                keycode: 38,
                pressed: true,
                state: 0,
                modifiers_after: 0,
                time_msec: 1,
            }
            .into(),
            target_window: None,
            xi_event_type: None,
            xi_transition_mask: 0,
            delivery: Some(delivery),
        })
        .unwrap();

    assert_eq!(
        broker.route_pending(),
        Err(XServerFrontendRouteError::UnknownClient { client })
    );
    assert_eq!(
        delivery_receiver.recv().unwrap(),
        XAuthorityClientInputDelivery {
            client,
            delivery,
            outcome: XAuthorityInputDeliveryOutcome::RouteRejected,
        }
    );
}

#[test]
fn active_keyboard_grab_redirects_engine_routed_input_and_window() {
    let namespace = NamespaceId::from_raw(9);
    let focused = XServerFrontendClientId(1);
    let grabber = XServerFrontendClientId(2);
    let surface = SurfaceId::new(10, 1);
    let grab_window = XResourceId::new(0x400001, 1);
    let mut broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(4).unwrap());
    let (_focused_registration, focused_channels) =
        broker.registry.register_client(focused).unwrap();
    let (_grab_registration, grab_channels) = broker.registry.register_client(grabber).unwrap();
    broker
        .registry
        .register_surface(focused, namespace, surface, XResourceId::new(0x200001, 1))
        .unwrap();
    broker
        .registry
        .input_authority
        .lock()
        .unwrap()
        .grab_keyboard(
            namespace,
            crate::XActiveInputGrab {
                owner: grabber.raw(),
                window: grab_window,
                owner_events: false,
                pointer_mode: 1,
                keyboard_mode: 1,
                event_mask: 0,
            },
        )
        .unwrap();
    broker
        .registry
        .input_authority
        .lock()
        .unwrap()
        .select_xi_events(namespace, grabber.raw(), grab_window, &[(1, vec![1 << 2])]);
    broker
        .routed_input_sender()
        .send(XAuthorityRoutedInput {
            request: RoutedInputRequest {
                serial: 1,
                seat: SeatId::from_raw(1),
                device: DeviceId::from_raw(1),
                time_msec: 1,
                target_surface: surface,
                global_position: Point::default(),
                local_position: Point::default(),
                kind: InputEventKind::Key {
                    keycode: 30,
                    pressed: true,
                },
            },
            delivery: None,
        })
        .unwrap();
    assert_eq!(broker.route_pending(), Ok(1));
    assert!(matches!(
        focused_channels.input.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    let routed = grab_channels.input.recv().unwrap();
    assert_eq!(routed.client, grabber);
    assert_eq!(routed.target_window, Some(grab_window));
    assert_eq!(routed.xi_event_type, Some(2));
}

#[test]
fn synchronous_keyboard_grab_queues_until_allow_events() {
    let namespace = NamespaceId::from_raw(10);
    let client = XServerFrontendClientId(3);
    let surface = SurfaceId::new(11, 1);
    let mut broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(2).unwrap());
    let (_registration, channels) = broker.registry.register_client(client).unwrap();
    broker
        .registry
        .register_surface(client, namespace, surface, XResourceId::new(0x200001, 1))
        .unwrap();
    broker
        .registry
        .input_authority
        .lock()
        .unwrap()
        .grab_keyboard(
            namespace,
            crate::XActiveInputGrab {
                owner: client.raw(),
                window: XResourceId::new(0x200001, 1),
                owner_events: false,
                pointer_mode: 1,
                keyboard_mode: 0,
                event_mask: 0,
            },
        )
        .unwrap();
    broker
        .routed_input_sender()
        .send(XAuthorityRoutedInput {
            request: RoutedInputRequest {
                serial: 2,
                seat: SeatId::from_raw(1),
                device: DeviceId::from_raw(1),
                time_msec: 2,
                target_surface: surface,
                global_position: Point::default(),
                local_position: Point::default(),
                kind: InputEventKind::Key {
                    keycode: 30,
                    pressed: true,
                },
            },
            delivery: None,
        })
        .unwrap();
    assert_eq!(broker.route_pending(), Ok(1));
    assert!(matches!(
        channels.input.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    broker
        .registry
        .input_authority
        .lock()
        .unwrap()
        .allow_events(namespace, client.raw(), 3)
        .unwrap();
    assert_eq!(broker.route_pending(), Ok(1));
    assert_eq!(channels.input.recv().unwrap().client, client);
}

#[test]
fn xi2_device_event_uses_xge_header_and_fp1616_local_coordinates() {
    let bytes = encode_xi_device_event(
        XByteOrder::LittleEndian,
        7,
        6,
        XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
            kind: XAuthorityPointerEventKind::Motion,
            surface: SurfaceId::new(1, 1),
            root_x: 11,
            root_y: 12,
            event_x: 3,
            event_y: -4,
            state: 5,
            time_msec: 9,
        }),
        XResourceId::new(0x200001, 1),
    );
    assert_eq!(bytes.len(), 80);
    assert_eq!(bytes[0], 35);
    assert_eq!(bytes[1], crate::X_INPUT_MAJOR_OPCODE);
    assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 6);
    assert_eq!(u16::from_le_bytes([bytes[10], bytes[11]]), 2);
    assert_eq!(
        i32::from_le_bytes(bytes[40..44].try_into().unwrap()),
        3 << 16
    );
    assert_eq!(
        i32::from_le_bytes(bytes[44..48].try_into().unwrap()),
        -4 << 16
    );
    let crossing = encode_xi_crossing_event(
        XByteOrder::LittleEndian,
        8,
        7,
        XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
            kind: XAuthorityPointerEventKind::Motion,
            surface: SurfaceId::new(1, 1),
            root_x: 11,
            root_y: 12,
            event_x: 3,
            event_y: -4,
            state: 5,
            time_msec: 9,
        }),
        XResourceId::new(0x200001, 1),
    );
    assert_eq!(crossing.len(), 72);
    assert_eq!(u16::from_le_bytes([crossing[8], crossing[9]]), 7);
    assert_eq!(crossing[48], 1);
}

#[test]
fn keyboard_focus_propagates_only_through_its_ancestor_chain() {
    let mut selections = XCoreEventSelectionState::default();
    let parent = XResourceId::new(0x200007, 1);
    let child = XResourceId::new(0x200001, 1);
    selections.register(child, parent);
    assert_eq!(selections.selected_keyboard_target(child), None);
    selections.update(parent, Some(1), None);

    assert_eq!(selections.keyboard_target(child), parent);
    assert_eq!(selections.selected_keyboard_target(child), Some(parent));

    assert_eq!(
        selections.keyboard_target(XResourceId::new(0x200009, 1)),
        XResourceId::new(0x200009, 1)
    );
}

#[test]
fn keyboard_delivery_falls_back_to_engine_focused_surface() {
    let selections = XCoreEventSelectionState::default();

    assert_eq!(
        selections.keyboard_target(XResourceId::new(0x200001, 1)),
        XResourceId::new(0x200001, 1)
    );
}

#[test]
fn root_focus_uses_mapped_stacking_order_and_restacking() {
    let root = XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1);
    let lower = XResourceId::new(0x200001, 1);
    let upper = XResourceId::new(0x200002, 1);
    let mut selections = XCoreEventSelectionState::default();
    for window in [lower, upper] {
        selections.register(window, root);
        selections.update(window, Some(1), None);
        selections.observe_mapped(window);
    }
    assert_eq!(selections.keyboard_target(root), upper);

    selections.restack(lower, Some(upper), Some(0));
    assert_eq!(selections.keyboard_target(root), lower);

    selections.observe_unmapped(lower);
    assert_eq!(selections.keyboard_target(root), upper);
}
