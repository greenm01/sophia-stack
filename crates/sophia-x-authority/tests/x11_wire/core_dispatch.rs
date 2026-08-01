#[test]
fn x11_dispatch_reports_root_input_focus_for_minimal_server() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 522, XByteOrder::LittleEndian),
        &[43, 0, 1, 0],
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 43),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 1);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]),
        X_SETUP_DEFAULT_ROOT
    );
}

#[test]
fn override_redirect_window_is_reported_as_client_positioned() {
    let namespace = NamespaceId::from_raw(45);
    let window = 0x220901;
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let create = decode_x11_core_request(
        context(namespace, 540, XByteOrder::LittleEndian),
        &create_window_override_redirect_request(
            XByteOrder::LittleEndian,
            window,
            0,
            0,
            1920,
            24,
        ),
    )
    .unwrap();
    let created = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(matches!(
        created.response.as_ref().unwrap().surfaces.as_slice(),
        [surface]
            if surface.presentation == SurfacePresentationRole::ClientPositioned
    ));
    assert!(matches!(
        created.outputs.as_slice(),
        [XClientOutput::Event(XClientEvent::ConfigureNotify {
            override_redirect: true,
            ..
        })]
    ));
    let observed = XAuthorityObservedTransactionBatch::from_dispatch_result(&created).unwrap();
    assert!(matches!(
        observed.surface_presentations.as_slice(),
        [presentation]
            if presentation.role == SurfacePresentationRole::ClientPositioned
                && presentation.geometry.width == 1920
                && presentation.geometry.height == 24
    ));

    let attributes = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 3),
        XWireRequest::GetWindowAttributes {
            window: XResourceId::new(u64::from(window), 1),
        },
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(matches!(
        attributes.outputs.as_slice(),
        [XClientOutput::Reply(XClientReply::GetWindowAttributes {
            override_redirect: true,
            map_state: 0,
            ..
        })]
    ));
}

#[test]
fn reparent_reports_policy_role_transition_to_session_observer() {
    let namespace = NamespaceId::from_raw(45);
    let parent = 0x220911;
    let child = 0x220912;
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    for (sequence, window) in [(1, parent), (2, child)] {
        let create = decode_x11_core_request(
            context(namespace, 540, XByteOrder::LittleEndian),
            &create_window_request(
                XByteOrder::LittleEndian,
                window,
                0,
                0,
                640,
                480,
            ),
        )
        .unwrap();
        dispatch_x11_wire_request(
            dispatch_context(namespace, sequence, XByteOrder::LittleEndian, 1),
            create,
            &mut runtime,
            &mut atoms,
            &mut properties,
        );
    }

    let reparented = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 7),
        XWireRequest::ReparentWindow {
            window: XResourceId::new(u64::from(child), 1),
            parent: XResourceId::new(u64::from(parent), 1),
            x: 12,
            y: 24,
        },
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(matches!(
        reparented.response.as_ref().unwrap().surfaces.as_slice(),
        [surface]
            if surface.surface == SurfaceId::new(child, 1)
                && surface.presentation == SurfacePresentationRole::ClientPositioned
                && surface.geometry.x == 12
                && surface.geometry.y == 24
    ));
}

#[test]
fn x11_dispatch_reports_window_lifecycle_map_state() {
    let namespace = NamespaceId::from_raw(45);
    let window = XResourceId::new(0x220902, 1);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let create = XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(1),
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window,
            surface: SurfaceId::new(0x220902, 1),
            geometry: Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 480,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    };
    assert_eq!(
        runtime.apply(create).outcome,
        XAuthorityResponseOutcome::Accepted
    );

    let attributes = |runtime: &mut XAuthorityRuntime,
                      atoms: &mut XAtomTable,
                      properties: &mut XPropertyTable| {
        dispatch_x11_wire_request(
            dispatch_context(namespace, 2, XByteOrder::LittleEndian, 3),
            XWireRequest::GetWindowAttributes { window },
            runtime,
            atoms,
            properties,
        )
    };
    assert!(matches!(
        attributes(&mut runtime, &mut atoms, &mut properties)
            .outputs
            .as_slice(),
        [XClientOutput::Reply(XClientReply::GetWindowAttributes {
            map_state: 0,
            ..
        })]
    ));

    runtime.set_policy_map_deferred(true);
    assert_eq!(
        runtime
            .apply(XAuthorityRequestPacket {
                transaction: TransactionId::from_raw(2),
                namespace,
                kind: XAuthorityRequestKind::MapWindow {
                    window,
                    generation: 2,
                },
            })
            .outcome,
        XAuthorityResponseOutcome::Accepted
    );
    assert!(matches!(
        attributes(&mut runtime, &mut atoms, &mut properties)
            .outputs
            .as_slice(),
        [XClientOutput::Reply(XClientReply::GetWindowAttributes {
            map_state: 0,
            ..
        })]
    ));

    runtime
        .admit_window_from_engine(
            namespace,
            window,
            Rect {
                x: 10,
                y: 20,
                width: 640,
                height: 480,
            },
        )
        .unwrap();
    assert!(matches!(
        attributes(&mut runtime, &mut atoms, &mut properties)
            .outputs
            .as_slice(),
        [XClientOutput::Reply(XClientReply::GetWindowAttributes {
            map_state: 2,
            ..
        })]
    ));
}

#[test]
fn x11_dispatch_reports_core_modifier_mapping() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 523, XByteOrder::LittleEndian),
        &[119, 0, 1, 0],
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 119),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0].len(), 48);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 2);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][2..4]), 2);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 4);
    assert_eq!(&encoded[0][32..36], &[50, 62, 66, 0]);
}

#[test]
fn x11_dispatch_reports_seven_button_identity_pointer_mapping() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 524, XByteOrder::LittleEndian),
        &[117, 0, 1, 0],
    )
    .unwrap();

    let encoded = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 117),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0].len(), 40);
    assert_eq!(encoded[0][1], 7);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 2);
    assert_eq!(&encoded[0][32..39], &[1, 2, 3, 4, 5, 6, 7]);
}

#[test]
fn x11_dispatch_reports_us_keyboard_mapping_for_minimal_server() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 524, XByteOrder::LittleEndian),
        &[101, 0, 2, 0, 8, 4, 0, 0],
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 101),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0].len(), 64);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 2);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][2..4]), 3);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 8);
    let keysyms = encoded[0][32..64]
        .chunks_exact(4)
        .map(|bytes| read_u32(XByteOrder::LittleEndian, bytes))
        .collect::<Vec<_>>();
    assert_eq!(
        keysyms,
        vec![
            0,
            0,
            0xff1b,
            0xff1b,
            b'1' as u32,
            b'!' as u32,
            b'2' as u32,
            b'@' as u32
        ]
    );
}

#[test]
fn x11_dispatch_reports_evdev_navigation_keysyms() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 525, XByteOrder::LittleEndian),
        &[101, 0, 2, 0, 111, 6, 0, 0],
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, 101),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0][1], 2);
    let keysyms = encoded[0][32..]
        .chunks_exact(4)
        .map(|bytes| read_u32(XByteOrder::LittleEndian, bytes))
        .collect::<Vec<_>>();
    assert_eq!(
        keysyms,
        vec![
            0xff52, 0xff52, 0xff55, 0xff55, 0xff51, 0xff51, 0xff53, 0xff53, 0xff57, 0xff57, 0xff54,
            0xff54,
        ]
    );
}

#[test]
fn x11_dispatch_replies_to_atom_requests_and_rejects_unknown_names() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    let intern = decode_x11_core_request(
        context(namespace, 508, XByteOrder::LittleEndian),
        &intern_atom_request(XByteOrder::LittleEndian, false, X_ATOM_NAME_NET_WM_NAME),
    )
    .unwrap();
    let intern = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 16),
        intern,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = intern.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0][0], 1);
    let net_wm_name = read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]);
    assert_ne!(net_wm_name, 0);

    let missing = decode_x11_core_request(
        context(namespace, 509, XByteOrder::LittleEndian),
        &intern_atom_request(XByteOrder::LittleEndian, true, "SOPHIA_MISSING"),
    )
    .unwrap();
    let missing = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 16),
        missing,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = missing.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 0);

    let get_name = decode_x11_core_request(
        context(namespace, 510, XByteOrder::LittleEndian),
        &get_atom_name_request(XByteOrder::LittleEndian, net_wm_name),
    )
    .unwrap();
    let get_name = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 17),
        get_name,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = get_name.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]), 12);
    assert_eq!(&encoded[0][32..44], X_ATOM_NAME_NET_WM_NAME.as_bytes());

    let unknown = decode_x11_core_request(
        context(namespace, 511, XByteOrder::LittleEndian),
        &get_atom_name_request(XByteOrder::LittleEndian, 0x00ff_ffff),
    )
    .unwrap();
    let unknown = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, 17),
        unknown,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = unknown.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 0);
    assert_eq!(encoded[0][1], XErrorCode::BadAtom.wire_code());
}

#[test]
fn x11_dispatch_reports_extensions_absent_until_explicitly_supported() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let query = decode_x11_core_request(
        context(namespace, 521, XByteOrder::LittleEndian),
        &query_extension_request(XByteOrder::LittleEndian, "SOPHIA-UNKNOWN"),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 98),
        query,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][8], 0);
    assert_eq!(encoded[0][9], 0);
}

#[test]
fn x11_dispatch_advertises_sophia_present_extension() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let query = decode_x11_core_request(
        context(namespace, 524, XByteOrder::LittleEndian),
        &query_extension_request(XByteOrder::LittleEndian, X_SOPHIA_PRESENT_EXTENSION_NAME),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 98),
        query,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][8], 1);
    assert_eq!(encoded[0][9], X_SOPHIA_PRESENT_MAJOR_OPCODE);
    assert_eq!(encoded[0][10], 0);
    assert_eq!(encoded[0][11], 0);
}

#[test]
fn x11_dispatch_advertises_mit_shm_and_replies_to_query_version() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let query = decode_x11_core_request(
        context(namespace, 526, XByteOrder::LittleEndian),
        &query_extension_request(XByteOrder::LittleEndian, X_MIT_SHM_EXTENSION_NAME),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 98),
        query,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][8], 1);
    assert_eq!(encoded[0][9], X_MIT_SHM_MAJOR_OPCODE);

    let version = decode_x11_core_request(
        context(namespace, 527, XByteOrder::LittleEndian),
        &mit_shm_query_version_request(XByteOrder::LittleEndian),
    )
    .unwrap();
    let version = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            2,
            XByteOrder::LittleEndian,
            X_MIT_SHM_MAJOR_OPCODE,
        ),
        version,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = version.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 0);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]), 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][10..12]), 2);
}

#[test]
fn x11_dispatch_negotiates_standard_dri3_and_present_1_2() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    for (name, opcode, first_event) in [
        (X_DRI3_EXTENSION_NAME, X_DRI3_MAJOR_OPCODE, 0),
        (
            X_PRESENT_EXTENSION_NAME,
            X_PRESENT_MAJOR_OPCODE,
            X_PRESENT_FIRST_EVENT,
        ),
    ] {
        let query = decode_x11_core_request(
            context(namespace, 528, XByteOrder::LittleEndian),
            &query_extension_request(XByteOrder::LittleEndian, name),
        )
        .unwrap();
        let result = dispatch_x11_wire_request(
            dispatch_context(namespace, 1, XByteOrder::LittleEndian, 98),
            query,
            &mut runtime,
            &mut atoms,
            &mut properties,
        );
        let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
        assert_eq!(encoded[0][8], 1);
        assert_eq!(encoded[0][9], opcode);
        assert_eq!(encoded[0][10], first_event);

        let version = decode_x11_core_request(
            context(namespace, 529, XByteOrder::LittleEndian),
            &extension_query_version_request(XByteOrder::LittleEndian, opcode, 1, 4),
        )
        .unwrap();
        let version = dispatch_x11_wire_request(
            dispatch_context(namespace, 2, XByteOrder::LittleEndian, opcode),
            version,
            &mut runtime,
            &mut atoms,
            &mut properties,
        );
        let encoded = version.encoded_outputs(XByteOrder::LittleEndian);
        assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 1);
        assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][12..16]), 2);
    }
}

#[test]
fn dri3_open_decodes_default_provider_and_encodes_one_fd_reply() {
    let namespace = NamespaceId::from_raw(45);
    let request = decode_x11_core_request(
        context(namespace, 529, XByteOrder::LittleEndian),
        &dri3_open_request(XByteOrder::LittleEndian, X_SETUP_DEFAULT_ROOT, 0),
    )
    .unwrap();
    assert_eq!(request.required_fd_count(), 0);
    assert_eq!(
        request,
        XWireRequest::Dri3Open {
            drawable: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            provider: 0,
        }
    );

    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 7, XByteOrder::LittleEndian, X_DRI3_MAJOR_OPCODE),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][2..4]), 7);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 0);
}

#[test]
fn dri3_open_rejects_nondefault_provider() {
    let namespace = NamespaceId::from_raw(45);
    let request = decode_x11_core_request(
        context(namespace, 529, XByteOrder::LittleEndian),
        &dri3_open_request(XByteOrder::LittleEndian, X_SETUP_DEFAULT_ROOT, 99),
    )
    .unwrap();
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 8, XByteOrder::LittleEndian, X_DRI3_MAJOR_OPCODE),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 0);
    assert_eq!(encoded[0][1], XErrorCode::BadValue.wire_code());
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 99);
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]),
        u16::from(X_DRI3_OPEN_MINOR_OPCODE)
    );
}

#[test]
fn dri3_get_supported_modifiers_reports_linear_and_implicit_screen_layouts() {
    let namespace = NamespaceId::from_raw(45);
    let request = decode_x11_core_request(
        context(namespace, 529, XByteOrder::LittleEndian),
        &dri3_get_supported_modifiers_request(
            XByteOrder::LittleEndian,
            X_SETUP_DEFAULT_ROOT,
            24,
            32,
        ),
    )
    .unwrap();
    assert_eq!(
        request,
        XWireRequest::Dri3GetSupportedModifiers {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            depth: 24,
            bits_per_pixel: 32,
        }
    );
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 9, XByteOrder::LittleEndian, X_DRI3_MAJOR_OPCODE),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0].len(), 48);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 4);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 0);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][12..16]), 2);
    assert_eq!(read_u64(XByteOrder::LittleEndian, &encoded[0][32..40]), 0);
    assert_eq!(
        read_u64(XByteOrder::LittleEndian, &encoded[0][40..48]),
        0x00ff_ffff_ffff_ffff
    );

    let argb = dispatch_x11_wire_request(
        dispatch_context(namespace, 10, XByteOrder::LittleEndian, X_DRI3_MAJOR_OPCODE),
        XWireRequest::Dri3GetSupportedModifiers {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            depth: 32,
            bits_per_pixel: 32,
        },
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(argb[0][0], 1);

    let invalid = decode_x11_core_request(
        context(namespace, 530, XByteOrder::LittleEndian),
        &dri3_get_supported_modifiers_request(
            XByteOrder::LittleEndian,
            X_SETUP_DEFAULT_ROOT,
            16,
            16,
        ),
    )
    .unwrap();
    let invalid = dispatch_x11_wire_request(
        dispatch_context(namespace, 10, XByteOrder::LittleEndian, X_DRI3_MAJOR_OPCODE),
        invalid,
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(invalid[0][0], 0);
    assert_eq!(invalid[0][1], XErrorCode::BadValue.wire_code());
}

#[test]
fn dri3_pixmap_from_buffer_requires_one_fd_and_preserves_bounded_metadata() {
    let namespace = NamespaceId::from_raw(45);
    let request = decode_x11_core_request(
        context(namespace, 530, XByteOrder::LittleEndian),
        &dri3_pixmap_from_buffer_request(
            XByteOrder::LittleEndian,
            0x220801,
            X_SETUP_DEFAULT_ROOT,
            64 * 48 * 4,
            64,
            48,
            256,
            24,
            32,
        ),
    )
    .unwrap();
    assert_eq!(request.required_fd_count(), 1);
    assert_eq!(
        request,
        XWireRequest::Dri3PixmapFromBuffer {
            pixmap: XResourceId::new(0x220801, 1),
            drawable: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            size_bytes: 64 * 48 * 4,
            width: 64,
            height: 48,
            stride: 256,
            depth: 24,
            bits_per_pixel: 32,
        }
    );
}

#[test]
fn dri3_pixmap_from_buffers_preserves_modifier_and_plane_metadata() {
    let namespace = NamespaceId::from_raw(45);
    let pixmap = XResourceId::new(0x220803, 1);
    let request = decode_x11_core_request(
        context(namespace, 531, XByteOrder::LittleEndian),
        &dri3_pixmap_from_buffers_request(
            XByteOrder::LittleEndian,
            0x220803,
            X_SETUP_DEFAULT_ROOT,
            1,
            64,
            48,
            [256, 0, 0, 0],
            [0, 0, 0, 0],
            24,
            32,
            0,
        ),
    )
    .unwrap();
    assert_eq!(request.required_fd_count(), 1);
    assert_eq!(
        request,
        XWireRequest::Dri3PixmapFromBuffers {
            pixmap,
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            num_buffers: 1,
            width: 64,
            height: 48,
            strides: [256, 0, 0, 0],
            offsets: [0, 0, 0, 0],
            depth: 24,
            bits_per_pixel: 32,
            modifier: 0,
        }
    );

    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, X_DRI3_MAJOR_OPCODE),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(result.outputs.is_empty());
    let descriptor = runtime.dri3_pixmap_descriptor(namespace, pixmap).unwrap();
    assert_eq!(
        descriptor.size,
        Size {
            width: 64,
            height: 48,
        }
    );
    assert_eq!(descriptor.format, sophia_protocol::DRM_FORMAT_XRGB8888);
    assert_eq!(descriptor.modifier, 0);
    assert_eq!(descriptor.plane_count, 1);
    assert_eq!(descriptor.planes[0].unwrap().stride, 256);
    assert_eq!(descriptor.planes[0].unwrap().offset, 0);
}

#[test]
fn dri3_fence_from_fd_requires_one_fd_and_registers_authority_identity() {
    let namespace = NamespaceId::from_raw(45);
    let request = decode_x11_core_request(
        context(namespace, 531, XByteOrder::LittleEndian),
        &dri3_fence_from_fd_request(
            XByteOrder::LittleEndian,
            X_SETUP_DEFAULT_ROOT,
            0x220802,
            false,
        ),
    )
    .unwrap();
    assert_eq!(request.required_fd_count(), 1);
    assert_eq!(
        request,
        XWireRequest::Dri3FenceFromFd {
            drawable: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            fence: XResourceId::new(0x220802, 1),
            initially_triggered: false,
        }
    );

    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, X_DRI3_MAJOR_OPCODE),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(result.outputs.is_empty());
    runtime
        .validate_dri3_fence_access(namespace, XResourceId::new(0x220802, 1))
        .unwrap();
}

#[test]
fn standard_present_pixmap_reduces_dri3_pixmap_to_dmabuf_transaction() {
    let namespace = NamespaceId::from_raw(45);
    let window = XResourceId::new(0x220810, 1);
    let pixmap = XResourceId::new(0x220811, 1);
    let mut runtime = XAuthorityRuntime::new();
    runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(1),
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window,
            surface: SurfaceId::new(45, 1),
            geometry: Rect {
                x: 0,
                y: 0,
                width: 64,
                height: 48,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    });
    let descriptor = runtime
        .create_dri3_pixmap(namespace, pixmap, 2, 64 * 48 * 4, 64, 48, 256, 24, 32)
        .unwrap();

    let request = decode_x11_core_request(
        context(namespace, 532, XByteOrder::LittleEndian),
        &present_pixmap_request(XByteOrder::LittleEndian, window, pixmap, 77),
    )
    .unwrap();
    assert_eq!(request.required_fd_count(), 0);
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let result = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            2,
            XByteOrder::LittleEndian,
            X_PRESENT_MAJOR_OPCODE,
        ),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(result.outputs.is_empty());
    let response = result.response.unwrap();
    assert_eq!(response.transaction, TransactionId::from_raw(532));
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].transaction,
        TransactionId::from_raw(532)
    );
    assert_eq!(
        response.transactions[0].target_buffer,
        BufferSource::DmaBuf {
            handle: descriptor.handle.raw()
        }
    );
    assert_eq!(response.transactions[0].damage.rects[0].width, 64);
    assert_eq!(response.transactions[0].damage.rects[0].height, 48);
}

#[test]
fn child_dri3_present_projects_onto_the_managed_toplevel() {
    let namespace = NamespaceId::from_raw(46);
    let parent = XResourceId::new(0x220820, 1);
    let child = XResourceId::new(0x220821, 1);
    let pixmap = XResourceId::new(0x220822, 1);
    let parent_surface = SurfaceId::new(46, 1);
    let mut runtime = XAuthorityRuntime::new();
    for (window, surface, geometry) in [
        (
            parent,
            parent_surface,
            Rect {
                x: 100,
                y: 200,
                width: 64,
                height: 80,
            },
        ),
        (
            child,
            SurfaceId::new(47, 1),
            Rect {
                x: 5,
                y: 7,
                width: 64,
                height: 48,
            },
        ),
    ] {
        runtime.apply(XAuthorityRequestPacket {
            transaction: TransactionId::from_raw(u64::from(surface.index())),
            namespace,
            kind: XAuthorityRequestKind::CreateWindow {
                window,
                surface,
                geometry,
                constraints: SurfaceConstraints {
                    min_size: None,
                    max_size: None,
                },
                generation: 1,
            },
        });
    }
    runtime.set_window_parent(namespace, child, parent).unwrap();
    let descriptor = runtime
        .create_dri3_pixmap(namespace, pixmap, 1, 64 * 48 * 4, 64, 48, 256, 24, 32)
        .unwrap();

    let response = runtime.present_standard_pixmap(
        TransactionId::from_raw(548),
        namespace,
        child,
        pixmap,
        2,
        3,
        None,
        None,
    );

    assert_eq!(response.outcome, XAuthorityResponseOutcome::Accepted);
    assert_eq!(response.transactions.len(), 1);
    let transaction = &response.transactions[0];
    assert_eq!(transaction.surface, parent_surface);
    assert_eq!(transaction.target_geometry.x, 100);
    assert_eq!(transaction.target_geometry.y, 200);
    assert_eq!(transaction.target_geometry.width, 64);
    assert_eq!(transaction.target_geometry.height, 80);
    assert_eq!(
        transaction.target_buffer,
        BufferSource::DmaBuf {
            handle: descriptor.handle.raw()
        }
    );
    assert_eq!(
        transaction.damage,
        Region::single(Rect {
            x: 7,
            y: 10,
            width: 64,
            height: 48,
        })
    );
    assert_eq!(
        runtime
            .window_presentation_root_and_offset(namespace, child)
            .unwrap(),
        (parent, parent_surface, 5, 7)
    );
}
