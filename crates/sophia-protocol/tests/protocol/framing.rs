#[test]
fn chrome_action_request_is_surface_scoped() {
    let request = ChromeActionRequest {
        surface: SurfaceId::new(9, 4),
        generation: 12,
        kind: ChromeActionKind::CloseSurfaceRequested,
    };

    assert_eq!(request.surface, SurfaceId::new(9, 4));
    assert_eq!(request.generation, 12);
    assert_eq!(request.kind, ChromeActionKind::CloseSurfaceRequested);
}

#[test]
fn wm_manage_request_contains_only_blind_policy_data() {
    let surface = SurfaceId::new(2, 1);
    let workspace = WorkspaceId::from_raw(1);
    let request = WmRequestPacket {
        transaction: TransactionId::from_raw(5),
        kind: WmRequestKind::ManageSurface(WmManageSurface {
            node: layout_node(surface, workspace),
            output: OutputId::from_raw(1),
            workspace,
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1280,
                height: 720,
            },
        }),
    };

    assert_eq!(request.transaction, TransactionId::from_raw(5));
    let WmRequestKind::ManageSurface(manage) = request.kind else {
        panic!("expected manage request");
    };
    assert_eq!(manage.node.surface, surface);
    assert_eq!(manage.workspace, workspace);
}

#[test]
fn wm_response_converts_to_layout_transaction() {
    let surface = SurfaceId::new(2, 1);
    let workspace = WorkspaceId::from_raw(1);
    let response = WmResponsePacket {
        transaction: TransactionId::from_raw(5),
        commands: vec![
            WmCommand::AssignWorkspace { surface, workspace },
            WmCommand::ConfigureSurface(SurfaceSizeRequest {
                surface,
                size: Size {
                    width: 640,
                    height: 480,
                },
            }),
            WmCommand::FocusSurface(surface),
            WmCommand::RenderSurface(SurfacePlacement {
                surface,
                geometry: Rect {
                    x: 10,
                    y: 20,
                    width: 640,
                    height: 480,
                },
                z_index: 3,
                crop: None,
                transform: Transform::IDENTITY,
            }),
        ],
        timeout_msec: 250,
    };

    let transaction = response.into_layout_transaction();

    assert_eq!(transaction.transaction, TransactionId::from_raw(5));
    assert_eq!(transaction.requested_sizes.len(), 1);
    assert_eq!(transaction.focus, Some(surface));
    assert_eq!(transaction.render_positions.len(), 1);
    assert_eq!(transaction.render_positions[0].z_index, 3);
    assert_eq!(transaction.timeout_msec, 250);
}

#[test]
fn routed_input_request_is_protocol_neutral_and_surface_targeted() {
    let request = RoutedInputRequest {
        serial: 99,
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        time_msec: 1_000,
        target_surface: SurfaceId::new(42, 1),
        global_position: Point { x: 20.0, y: 30.0 },
        local_position: Point { x: 12.5, y: 9.0 },
        kind: InputEventKind::PointerButton {
            button: 1,
            pressed: true,
        },
    };

    assert_eq!(request.serial, 99);
    assert_eq!(request.target_surface, SurfaceId::new(42, 1));
    assert_eq!(request.local_position.x, 12.5);
    assert_eq!(request.device, DeviceId::from_raw(2));
    assert_eq!(
        request.kind,
        InputEventKind::PointerButton {
            button: 1,
            pressed: true,
        }
    );
}

#[test]
fn routed_input_decision_carries_authority_rejection() {
    let decision = RoutedInputDecision {
        serial: 100,
        target_surface: SurfaceId::new(55, 3),
        outcome: RoutedInputOutcome::RejectedDeniedNamespace,
    };

    assert_eq!(decision.serial, 100);
    assert_eq!(
        decision.outcome,
        RoutedInputOutcome::RejectedDeniedNamespace
    );
}

#[test]
fn wm_request_frame_roundtrips() {
    let request = WmRequestPacket {
        transaction: TransactionId::from_raw(42),
        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
            output: OutputId::from_raw(7),
            workspace: WorkspaceId::from_raw(3),
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1280,
                height: 720,
            },
            nodes: vec![node(1), node(2)],
        }),
    };

    let frame = encode_wm_request_frame(&request).unwrap();
    assert_eq!(
        frame.len(),
        SOPHIA_IPC_HEADER_LEN + frame_payload_len(&frame)
    );
    assert_eq!(decode_wm_request_frame(&frame), Ok(request));
}

#[test]
fn wm_response_frame_roundtrips() {
    let surface = SurfaceId::new(4, 9);
    let response = WmResponsePacket {
        transaction: TransactionId::from_raw(77),
        timeout_msec: 250,
        commands: vec![
            WmCommand::AssignWorkspace {
                surface,
                workspace: WorkspaceId::from_raw(5),
            },
            WmCommand::ConfigureSurface(SurfaceSizeRequest {
                surface,
                size: Size {
                    width: 640,
                    height: 480,
                },
            }),
            WmCommand::FocusSurface(surface),
            WmCommand::RenderSurface(SurfacePlacement {
                surface,
                geometry: Rect {
                    x: 10,
                    y: 20,
                    width: 640,
                    height: 480,
                },
                z_index: 2,
                crop: Some(Rect {
                    x: 0,
                    y: 0,
                    width: 320,
                    height: 240,
                }),
                transform: Transform::IDENTITY,
            }),
        ],
    };

    let frame = encode_wm_response_frame(&response).unwrap();
    assert_eq!(decode_wm_response_frame(&frame), Ok(response));
}

#[test]
fn broker_health_frame_roundtrips() {
    let packet = BrokerHealthPacket::new(
        BrokerKind::Portal,
        BrokerHealthState::Ready,
        12,
        Some("ready".to_owned()),
    )
    .unwrap();

    let frame = encode_broker_health_frame(&packet).unwrap();

    assert_eq!(decode_broker_health_frame(&frame), Ok(packet));
    assert_eq!(frame_payload_len(&frame), 12);
}

#[test]
fn portal_broker_and_payload_frames_roundtrip() {
    let transfer = PortalTransferId::from_raw(41);
    let request = PortalBrokerRequestPacket {
        request: PortalRequest {
            transfer: PortalTransfer {
                transfer,
                source_namespace: NamespaceId::from_raw(10),
                target_namespace: NamespaceId::from_raw(20),
                kind: PortalTransferKind::Clipboard,
                mime_type: Some("UTF8_STRING".to_owned()),
                byte_size: 6,
                decision: PortalDecision::Pending,
                generation: 7,
            },
            deadline_msec: 2_000,
        },
        source_may_publish: true,
        target_may_request: true,
    };
    let frame = encode_portal_broker_request_frame(&request).unwrap();
    assert_eq!(decode_portal_broker_request_frame(&frame), Ok(request));

    let response = PortalBrokerResponsePacket {
        transfer,
        decision: PortalBrokerResponseDecision::Allowed(PortalGrant {
            transfer,
            source_namespace: NamespaceId::from_raw(10),
            target_namespace: NamespaceId::from_raw(20),
            kind: PortalTransferKind::Clipboard,
            source_generation: 7,
            broker_generation: 3,
            deadline_msec: 2_000,
            state: PortalGrantState::Active,
        }),
    };
    let frame = encode_portal_broker_response_frame(&response).unwrap();
    assert_eq!(decode_portal_broker_response_frame(&frame), Ok(response));

    let frame = encode_portal_clipboard_payload_frame(transfer, b"sophia").unwrap();
    assert_eq!(
        decode_portal_clipboard_payload_frame(&frame),
        Ok((transfer, b"sophia".to_vec()))
    );
}

#[test]
fn metadata_broker_health_frame_roundtrips_without_message() {
    let packet =
        BrokerHealthPacket::new(BrokerKind::Metadata, BrokerHealthState::Stopped, 13, None)
            .unwrap();

    let frame = encode_broker_health_frame(&packet).unwrap();

    assert_eq!(decode_broker_health_frame(&frame), Ok(packet));
    assert_eq!(frame_payload_len(&frame), 5);
}

#[test]
fn broker_health_frame_rejects_oversized_status_message() {
    let mut payload = Vec::new();
    push_u16(&mut payload, 1);
    push_u16(&mut payload, 3);
    push_u8(&mut payload, 1);
    push_u16(
        &mut payload,
        (SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN as u16) + 1,
    );
    payload.extend(std::iter::repeat_n(
        b'x',
        SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN + 1,
    ));
    let frame = encode_frame(
        IpcMessageKind::BrokerHealth,
        TransactionId::from_raw(4),
        &payload,
    )
    .unwrap();

    assert_eq!(
        decode_broker_health_frame(&frame),
        Err(IpcCodecError::TextTooLarge {
            field: "broker_health_message",
            len: SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN + 1,
            max: SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN,
        })
    );
}

#[test]
fn oversized_payload_is_rejected_before_allocation() {
    let mut frame = Vec::new();
    push_u32(&mut frame, SOPHIA_IPC_MAGIC);
    push_u16(&mut frame, SOPHIA_IPC_VERSION);
    push_u16(&mut frame, IpcMessageKind::WmRequest as u16);
    push_u64(&mut frame, 1);
    push_u32(&mut frame, (SOPHIA_IPC_MAX_PAYLOAD_LEN as u32) + 1);
    push_u32(&mut frame, 0);

    assert_eq!(
        decode_frame(&frame),
        Err(IpcCodecError::PayloadTooLarge(
            SOPHIA_IPC_MAX_PAYLOAD_LEN + 1
        ))
    );
}

#[test]
fn malformed_frames_fail_closed() {
    assert_eq!(decode_frame(&[]), Err(IpcCodecError::Truncated));

    let mut frame = encode_wm_request_frame(&WmRequestPacket {
        transaction: TransactionId::from_raw(1),
        kind: WmRequestKind::SurfaceRemoved {
            surface: SurfaceId::new(1, 1),
            workspace: WorkspaceId::from_raw(1),
        },
    })
    .unwrap();
    frame[0] = 0;
    assert_eq!(decode_frame(&frame), Err(IpcCodecError::BadMagic));

    let mut frame = encode_wm_request_frame(&WmRequestPacket {
        transaction: TransactionId::from_raw(1),
        kind: WmRequestKind::SurfaceRemoved {
            surface: SurfaceId::new(1, 1),
            workspace: WorkspaceId::from_raw(1),
        },
    })
    .unwrap();
    frame.push(0);
    assert_eq!(decode_frame(&frame), Err(IpcCodecError::TrailingBytes(1)));
}

#[test]
fn excessive_item_count_is_rejected() {
    let mut payload = Vec::new();
    push_u16(&mut payload, 2);
    push_u64(&mut payload, 1);
    push_u64(&mut payload, 1);
    encode_rect(
        Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        },
        &mut payload,
    );
    push_u32(&mut payload, (SOPHIA_IPC_MAX_ITEMS as u32) + 1);
    let frame = encode_frame(
        IpcMessageKind::WmRequest,
        TransactionId::from_raw(9),
        &payload,
    )
    .unwrap();

    assert_eq!(
        decode_wm_request_frame(&frame),
        Err(IpcCodecError::CountTooLarge {
            count: SOPHIA_IPC_MAX_ITEMS + 1,
            max: SOPHIA_IPC_MAX_ITEMS,
        })
    );
}

fn layout_node(surface: SurfaceId, workspace: WorkspaceId) -> LayoutNodeSnapshot {
    LayoutNodeSnapshot {
        surface,
        workspace,
        kind: LayoutNodeKind::Toplevel,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        state: LayoutNodeState::NORMAL,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        geometry: Rect {
            x: 0,
            y: 0,
            width: 320,
            height: 200,
        },
        generation: 1,
    }
}

fn node(index: u32) -> LayoutNodeSnapshot {
    LayoutNodeSnapshot {
        surface: SurfaceId::new(index, 1),
        workspace: WorkspaceId::from_raw(3),
        kind: LayoutNodeKind::Toplevel,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        state: LayoutNodeState::NORMAL,
        constraints: SurfaceConstraints {
            min_size: Some(Size {
                width: 100,
                height: 80,
            }),
            max_size: None,
        },
        geometry: Rect {
            x: (index as i32) * 10,
            y: 0,
            width: 320,
            height: 200,
        },
        generation: 11,
    }
}

fn frame_payload_len(frame: &[u8]) -> usize {
    u32::from_le_bytes(frame[16..20].try_into().unwrap()) as usize
}

fn encode_rect(rect: Rect, out: &mut Vec<u8>) {
    push_i32(out, rect.x);
    push_i32(out, rect.y);
    push_i32(out, rect.width);
    push_i32(out, rect.height);
}

fn push_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}
