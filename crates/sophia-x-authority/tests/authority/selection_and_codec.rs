#[test]
fn evdev_keyboard_mapping_preserves_x_modifier_event_order() {
    let mut keyboard = XCoreKeyboardMapper::new();
    assert_eq!(keyboard.map_evdev_key(42, true), Some((50, 0)));
    assert_eq!(keyboard.modifier_mask(), 1);
    assert_eq!(keyboard.map_evdev_key(30, true), Some((38, 1)));
    assert_eq!(keyboard.map_evdev_key(30, false), Some((38, 1)));
    assert_eq!(keyboard.map_evdev_key(42, false), Some((50, 1)));
    assert_eq!(keyboard.modifier_mask(), 0);

    assert_eq!(keyboard.map_evdev_key(58, true), Some((66, 0)));
    assert_eq!(keyboard.modifier_mask(), 2);
    assert_eq!(keyboard.map_evdev_key(103, true), Some((111, 2)));
    assert_eq!(keyboard.map_evdev_key(105, true), Some((113, 2)));
    assert_eq!(keyboard.map_evdev_key(106, true), Some((114, 2)));
    assert_eq!(keyboard.map_evdev_key(108, true), Some((116, 2)));
    assert_eq!(keyboard.map_evdev_key(0, true), None);
    assert_eq!(keyboard.map_evdev_key(u32::MAX, true), None);
}

#[test]
fn xkb_us_shift_semicolon_delivers_colon_state() {
    let mut keyboard = XkbKeyboardState::new(&XkbRmlvoConfig::default())
        .expect("the default US keymap must compile");
    assert_eq!(keyboard.map_evdev_key(42, true), Some((50, 0)));
    assert_eq!(keyboard.map_evdev_key(39, true), Some((47, 1)));
    assert_eq!(keyboard.map_evdev_key(39, false), Some((47, 1)));
    assert_eq!(keyboard.map_evdev_key(42, false), Some((50, 1)));
    assert_eq!(keyboard.modifier_mask(), 0);
}

#[test]
fn repeated_modifier_edges_do_not_leave_core_state_stuck() {
    let mut keyboard = XCoreKeyboardMapper::new();
    assert_eq!(keyboard.map_evdev_key(42, true), Some((50, 0)));
    assert_eq!(keyboard.map_evdev_key(42, true), Some((50, 1)));
    assert_eq!(keyboard.map_evdev_key(42, false), Some((50, 1)));
    assert_eq!(keyboard.modifier_mask(), 0);
}

#[test]
fn default_pc105_us_map_covers_printable_and_function_keys() {
    let keymap = XkbKeymapSnapshot::new(&XkbRmlvoConfig::default())
        .expect("the default US keymap must compile");
    let printable = [
        (10, '1', '!'),
        (11, '2', '@'),
        (12, '3', '#'),
        (13, '4', '$'),
        (14, '5', '%'),
        (15, '6', '^'),
        (16, '7', '&'),
        (17, '8', '*'),
        (18, '9', '('),
        (19, '0', ')'),
        (20, '-', '_'),
        (21, '=', '+'),
        (24, 'q', 'Q'),
        (25, 'w', 'W'),
        (26, 'e', 'E'),
        (27, 'r', 'R'),
        (28, 't', 'T'),
        (29, 'y', 'Y'),
        (30, 'u', 'U'),
        (31, 'i', 'I'),
        (32, 'o', 'O'),
        (33, 'p', 'P'),
        (34, '[', '{'),
        (35, ']', '}'),
        (38, 'a', 'A'),
        (39, 's', 'S'),
        (40, 'd', 'D'),
        (41, 'f', 'F'),
        (42, 'g', 'G'),
        (43, 'h', 'H'),
        (44, 'j', 'J'),
        (45, 'k', 'K'),
        (46, 'l', 'L'),
        (47, ';', ':'),
        (48, '\'', '"'),
        (49, '`', '~'),
        (51, '\\', '|'),
        (52, 'z', 'Z'),
        (53, 'x', 'X'),
        (54, 'c', 'C'),
        (55, 'v', 'V'),
        (56, 'b', 'B'),
        (57, 'n', 'N'),
        (58, 'm', 'M'),
        (59, ',', '<'),
        (60, '.', '>'),
        (61, '/', '?'),
        (65, ' ', ' '),
    ];
    for (keycode, base, shifted) in printable {
        assert_eq!(
            keymap.core_mapping(keycode, 1),
            [base as u32, shifted as u32],
            "wrong US symbols for X keycode {keycode}",
        );
    }

    let function_keysyms = [
        (67, 0xffbe),
        (68, 0xffbf),
        (69, 0xffc0),
        (70, 0xffc1),
        (71, 0xffc2),
        (72, 0xffc3),
        (73, 0xffc4),
        (74, 0xffc5),
        (75, 0xffc6),
        (76, 0xffc7),
        (95, 0xffc8),
        (96, 0xffc9),
    ];
    for (keycode, keysym) in function_keysyms {
        assert_eq!(keymap.core_mapping(keycode, 1)[0], keysym);
    }
}

#[test]
fn evdev_pointer_mapping_preserves_core_button_state_order() {
    let mut pointer = XCorePointerMapper::new();

    assert_eq!(pointer.map_evdev_button(272, true), Some((1, 0)));
    assert_eq!(pointer.state(), 1 << 8);
    assert_eq!(pointer.map_evdev_button(272, false), Some((1, 1 << 8)));
    assert_eq!(pointer.state(), 0);
    assert_eq!(pointer.map_evdev_button(999, true), None);
}

#[test]
fn protocol_neutral_axes_map_to_x11_core_scroll_buttons_at_the_frontend() {
    assert_eq!(XCorePointerMapper::map_axis_to_button(0, -120), Some(4));
    assert_eq!(XCorePointerMapper::map_axis_to_button(0, 120), Some(5));
    assert_eq!(XCorePointerMapper::map_axis_to_button(-120, 0), Some(6));
    assert_eq!(XCorePointerMapper::map_axis_to_button(120, 0), Some(7));
    assert_eq!(XCorePointerMapper::map_axis_to_button(0, 0), None);
}

#[test]
fn drawing_updates_fail_closed_for_cross_namespace_or_unknown_windows() {
    let owner = NamespaceId::from_raw(1);
    let other = NamespaceId::from_raw(2);
    let window = XResourceId::new(0x70, 1);
    let windows = window_table_with_surface(window, owner);

    assert_eq!(
        surface_transaction_from_drawing_update(
            &windows,
            XDrawingUpdate::present_pixmap(
                TransactionId::from_raw(12),
                other,
                window,
                0x901,
                Region::empty(),
                1,
                250,
            ),
        ),
        Err(XAuthorityAccessError::CrossNamespaceDenied)
    );

    assert_eq!(
        surface_transaction_from_drawing_update(
            &windows,
            XDrawingUpdate::present_pixmap(
                TransactionId::from_raw(12),
                owner,
                XResourceId::new(0x71, 1),
                0x901,
                Region::empty(),
                1,
                250,
            ),
        ),
        Err(XAuthorityAccessError::UnknownResource)
    );
}

#[test]
fn selection_owner_events_track_namespace_and_generation() {
    let namespace = NamespaceId::from_raw(11);
    let owner = XResourceId::new(0x80, 1);
    let windows = window_table_with_surface(owner, namespace);
    let mut monitor = XSelectionMonitor::new();

    let first = monitor.apply_event(
        XSelectionEvent {
            selection: 1,
            owner: Some(owner),
            timestamp: 10,
            selection_timestamp: 10,
            kind: XSelectionChangeKind::SetOwner,
        },
        &windows,
    );
    let second = monitor.apply_event(
        XSelectionEvent {
            selection: 1,
            owner: Some(owner),
            timestamp: 11,
            selection_timestamp: 11,
            kind: XSelectionChangeKind::SetOwner,
        },
        &windows,
    );

    assert_eq!(first.current.namespace, Some(namespace));
    assert_eq!(first.current.generation, 1);
    assert_eq!(second.previous, Some(first.current));
    assert_eq!(second.current.generation, 2);
    assert_eq!(
        monitor.current_owner_for_selection(1).unwrap(),
        second.current
    );
}

#[test]
fn selection_request_becomes_portal_prompt_and_native_denial_artifact() {
    let source_namespace = NamespaceId::from_raw(11);
    let target_namespace = NamespaceId::from_raw(12);
    let owner = XResourceId::new(0x90, 1);
    let requestor = XResourceId::new(0x91, 1);
    let windows =
        window_table_with_two_surfaces(owner, source_namespace, requestor, target_namespace);
    let mut monitor = XSelectionMonitor::new();
    monitor.apply_event(
        XSelectionEvent {
            selection: 7,
            owner: Some(owner),
            timestamp: 10,
            selection_timestamp: 10,
            kind: XSelectionChangeKind::SetOwner,
        },
        &windows,
    );

    let transfer = PortalTransferId::from_raw(5);
    let mut portal = ClipboardPortal::new();
    let dispatch = dispatch_clipboard_selection_request(
        XSelectionRequest {
            requestor,
            selection: 7,
            target: 8,
            target_name: "UTF8_STRING".to_owned(),
            property: 9,
            time: 30,
        },
        &monitor,
        &windows,
        transfer,
        &mut portal,
    )
    .unwrap();

    let ClipboardSelectionDispatch::CrossNamespace {
        portal_request,
        command,
    } = dispatch
    else {
        panic!("expected cross-namespace dispatch");
    };
    let PortalCommand::PromptClipboardTransfer(prompt) = &command else {
        panic!("expected clipboard prompt");
    };
    assert_eq!(prompt.transfer, transfer);
    assert_eq!(prompt.source_namespace, source_namespace);
    assert_eq!(prompt.target_namespace, target_namespace);
    assert_eq!(prompt.decision, PortalDecision::Pending);
    assert_eq!(prompt.generation, 1);
    assert_eq!(portal_request.property, 9);

    let PortalCommand::FailSelection { transfer: denied } = portal.deny(transfer).unwrap() else {
        panic!("expected fail-selection command");
    };
    let failure = clipboard_selection_failure_notify(portal_request.failure);

    assert_eq!(denied, transfer);
    assert_eq!(failure.transfer, transfer);
    assert!(failure.failed_normally());
    assert_eq!(failure.notify.requestor, requestor);
    assert_eq!(failure.notify.selection, 7);
    assert_eq!(failure.notify.target, 8);
    assert_eq!(failure.notify.property, X_ATOM_NONE);
}

#[test]
fn approved_selection_request_becomes_bounded_text_handoff_artifact() {
    let source_namespace = NamespaceId::from_raw(13);
    let target_namespace = NamespaceId::from_raw(14);
    let owner = XResourceId::new(0xa0, 1);
    let requestor = XResourceId::new(0xa1, 1);
    let windows =
        window_table_with_two_surfaces(owner, source_namespace, requestor, target_namespace);
    let mut monitor = XSelectionMonitor::new();
    let update = monitor.apply_event(
        XSelectionEvent {
            selection: 17,
            owner: Some(owner),
            timestamp: 10,
            selection_timestamp: 10,
            kind: XSelectionChangeKind::SetOwner,
        },
        &windows,
    );
    let transfer = PortalTransferId::from_raw(6);
    let mut portal = ClipboardPortal::new();
    let dispatch = dispatch_clipboard_selection_request(
        XSelectionRequest {
            requestor,
            selection: 17,
            target: 18,
            target_name: "text/plain;charset=utf-8".to_owned(),
            property: 19,
            time: 31,
        },
        &monitor,
        &windows,
        transfer,
        &mut portal,
    )
    .unwrap();
    let ClipboardSelectionDispatch::CrossNamespace { portal_request, .. } = dispatch else {
        panic!("expected cross-namespace dispatch");
    };
    let command = portal
        .approve_generation(transfer, update.current.generation)
        .unwrap();
    let handoff =
        clipboard_selection_text_handoff_artifact(&command, &portal_request, "hello").unwrap();

    assert_eq!(handoff.transfer, transfer);
    assert_eq!(handoff.property.requestor, requestor);
    assert_eq!(handoff.property.property, 19);
    assert_eq!(handoff.property.target, 18);
    assert_eq!(handoff.property.bytes, b"hello");
    assert!(handoff.succeeded_normally());
    assert_eq!(handoff.notify.property, 19);
}

#[test]
fn selection_requests_fail_closed_without_cross_namespace_boundary() {
    let namespace = NamespaceId::from_raw(15);
    let owner = XResourceId::new(0xb0, 1);
    let requestor = XResourceId::new(0xb1, 1);
    let windows = window_table_with_two_surfaces(owner, namespace, requestor, namespace);
    let mut monitor = XSelectionMonitor::new();
    monitor.apply_event(
        XSelectionEvent {
            selection: 27,
            owner: Some(owner),
            timestamp: 10,
            selection_timestamp: 10,
            kind: XSelectionChangeKind::SetOwner,
        },
        &windows,
    );

    assert_eq!(
        clipboard_portal_request_from_selection_request(
            XSelectionRequest {
                requestor,
                selection: 27,
                target: 28,
                target_name: "UTF8_STRING".to_owned(),
                property: 29,
                time: 32,
            },
            &monitor,
            &windows,
            PortalTransferId::from_raw(7),
        ),
        Err(ClipboardSelectionRequestError::SameNamespace)
    );
}

#[test]
fn x_authority_request_codec_round_trips_create_window() {
    let request = create_window_request(TransactionId::from_raw(100), NamespaceId::from_raw(21));

    let frame = encode_x_authority_request_frame(&request).unwrap();
    let decoded = decode_x_authority_request_frame(&frame).unwrap();

    assert_eq!(decoded, request);
}

#[test]
fn x_authority_response_codec_round_trips_runtime_outputs() {
    let namespace = NamespaceId::from_raw(22);
    let mut runtime = XAuthorityRuntime::new();
    let create = runtime.apply(create_window_request(
        TransactionId::from_raw(101),
        namespace,
    ));
    let map = runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(102),
        namespace,
        kind: XAuthorityRequestKind::MapWindow {
            window: XResourceId::new(0xc0, 1),
            generation: 2,
        },
    });
    let mut present = runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(103),
        namespace,
        kind: XAuthorityRequestKind::PresentPixmap {
            window: XResourceId::new(0xc0, 1),
            pixmap: 0x777,
            damage: Region::single(Rect {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            }),
            previous_committed_generation: 1,
            timeout_msec: 250,
        },
    });

    assert_eq!(create.surfaces.len(), 1);
    assert_eq!(map.surfaces.len(), 1);
    assert_eq!(present.transactions.len(), 1);
    present.removed_surfaces.push(SurfaceId::new(99, 1));

    let frame = encode_x_authority_response_frame(&present).unwrap();
    let decoded = decode_x_authority_response_frame(&frame).unwrap();

    assert_eq!(decoded, present);
}

#[test]
fn x_authority_codec_round_trips_every_explicit_portal_kind() {
    let kinds = [
        PortalTransferKind::Clipboard,
        PortalTransferKind::DragAndDrop,
        PortalTransferKind::FileHandoff,
        PortalTransferKind::ScreenCapture,
        PortalTransferKind::ScreenRecording,
        PortalTransferKind::UriOpen,
        PortalTransferKind::Notification,
    ];

    for (index, kind) in kinds.into_iter().enumerate() {
        let transaction = TransactionId::from_raw(120 + index as u64);
        let mut response = XAuthorityResponsePacket::accepted(transaction);
        response
            .portal_commands
            .push(XAuthorityPortalCommand::PromptClipboardTransfer(
                PortalTransfer {
                    transfer: PortalTransferId::from_raw(40 + index as u64),
                    source_namespace: NamespaceId::from_raw(30),
                    target_namespace: NamespaceId::from_raw(31),
                    kind,
                    mime_type: None,
                    byte_size: 0,
                    decision: PortalDecision::Pending,
                    generation: 1,
                },
            ));

        let frame = encode_x_authority_response_frame(&response).unwrap();
        assert_eq!(decode_x_authority_response_frame(&frame).unwrap(), response);
    }
}

#[test]
fn x_authority_codec_rejects_wrong_message_kind() {
    let payload = Vec::new();
    let frame = encode_frame(
        IpcMessageKind::WmRequest,
        TransactionId::from_raw(104),
        &payload,
    )
    .unwrap();

    assert_eq!(
        decode_x_authority_request_frame(&frame),
        Err(IpcCodecError::InvalidEnum {
            field: "message_kind",
            value: IpcMessageKind::WmRequest as u32,
        })
    );
}

#[test]
fn x_authority_codec_rejects_bad_magic_and_trailing_bytes() {
    let request = create_window_request(TransactionId::from_raw(105), NamespaceId::from_raw(23));
    let mut bad_magic = encode_x_authority_request_frame(&request).unwrap();
    bad_magic[0..4].copy_from_slice(&(SOPHIA_IPC_MAGIC ^ 0xffff).to_le_bytes());

    assert_eq!(
        decode_x_authority_request_frame(&bad_magic),
        Err(IpcCodecError::BadMagic)
    );

    let mut trailing = encode_x_authority_request_frame(&request).unwrap();
    trailing.push(0);

    assert_eq!(
        decode_x_authority_request_frame(&trailing),
        Err(IpcCodecError::TrailingBytes(1))
    );
}
