#[test]
fn xi_grab_device_installs_only_the_bounded_master_pointer_mask() {
    let namespace = NamespaceId::from_raw(44);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let result = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            1,
            XByteOrder::LittleEndian,
            X_INPUT_MAJOR_OPCODE,
        ),
        XWireRequest::XiGrabDevice {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            time: 0,
            cursor: None,
            device_id: 2,
            pointer_mode: 1,
            keyboard_mode: 1,
            owner_events: false,
            event_mask: vec![0x70],
        },
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(matches!(
        result.outputs.as_slice(),
        [XClientOutput::Reply(XClientReply::GrabStatus { status: 0, .. })]
    ));
    let grab = runtime
        .input_authority_mut()
        .pointer_grab(namespace)
        .unwrap();
    assert_eq!(grab.event_mask, 0);
    assert!(grab.selects_xi_event(4));
    assert!(grab.selects_xi_event(5));
    assert!(grab.selects_xi_event(6));
    assert!(!grab.selects_xi_event(7));
}

#[test]
fn x11_dispatch_advertises_randr_and_replies_to_query_version() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let query = decode_x11_core_request(
        context(namespace, 538, XByteOrder::LittleEndian),
        &query_extension_request(XByteOrder::LittleEndian, X_RANDR_EXTENSION_NAME),
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
    assert_eq!(encoded[0][9], X_RANDR_MAJOR_OPCODE);

    let version = decode_x11_core_request(
        context(namespace, 539, XByteOrder::LittleEndian),
        &randr_query_version_request(XByteOrder::LittleEndian, 1, 5),
    )
    .unwrap();
    assert_eq!(
        version,
        XWireRequest::RandrQueryVersion {
            major_version: 1,
            minor_version: 5,
        }
    );
    let version = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, X_RANDR_MAJOR_OPCODE),
        version,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = version.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][12..16]), 5);

    let select = decode_x11_core_request(
        context(namespace, 540, XByteOrder::LittleEndian),
        &randr_select_input_request(XByteOrder::LittleEndian, X_SETUP_DEFAULT_ROOT, 0x000b),
    )
    .unwrap();
    assert_eq!(
        select,
        XWireRequest::RandrSelectInput {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            enable: 0x000b,
        }
    );
    let select = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, X_RANDR_MAJOR_OPCODE),
        select,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(select.outputs.is_empty());

    let primary = decode_x11_core_request(
        context(namespace, 541, XByteOrder::LittleEndian),
        &randr_window_request(
            XByteOrder::LittleEndian,
            X_RANDR_GET_OUTPUT_PRIMARY_MINOR_OPCODE,
            X_SETUP_DEFAULT_ROOT,
        ),
    )
    .unwrap();
    assert_eq!(
        primary,
        XWireRequest::RandrGetOutputPrimary {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
        }
    );
    let primary = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, X_RANDR_MAJOR_OPCODE),
        primary,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = primary.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]),
        0x2000_0001
    );

    let mut get_providers_request = vec![
        X_RANDR_MAJOR_OPCODE,
        X_RANDR_GET_PROVIDERS_MINOR_OPCODE,
        2,
        0,
    ];
    get_providers_request.extend_from_slice(&X_SETUP_DEFAULT_ROOT.to_le_bytes());
    let get_providers = decode_x11_core_request(
        context(namespace, 542, XByteOrder::LittleEndian),
        &get_providers_request,
    )
    .unwrap();
    assert_eq!(
        get_providers,
        XWireRequest::RandrGetProviders {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
        }
    );
    let get_providers = dispatch_x11_wire_request(
        dispatch_context(namespace, 5, XByteOrder::LittleEndian, X_RANDR_MAJOR_OPCODE),
        get_providers,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = get_providers.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][12..14]), 0);

    let monitors = decode_x11_core_request(
        context(namespace, 542, XByteOrder::LittleEndian),
        &randr_get_monitors_request(XByteOrder::LittleEndian, X_SETUP_DEFAULT_ROOT, true),
    )
    .unwrap();
    assert_eq!(
        monitors,
        XWireRequest::RandrGetMonitors {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            get_active: true,
        }
    );
    let monitors = dispatch_x11_wire_request(
        dispatch_context(namespace, 5, XByteOrder::LittleEndian, X_RANDR_MAJOR_OPCODE),
        monitors,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = monitors.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][12..16]), 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][16..20]), 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 7);
    assert_eq!(encoded[0][36], 1, "the deterministic monitor is primary");
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][38..40]), 1);
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][44..46]),
        1280
    );
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][46..48]), 720);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][56..60]),
        0x2000_0001
    );
}

#[test]
fn randr_output_property_returns_bounded_empty_edid_fallback() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let edid = atoms.intern("EDID", false).unwrap().unwrap();
    let request = decode_x11_core_request(
        context(namespace, 543, XByteOrder::LittleEndian),
        &randr_get_output_property_request(XByteOrder::LittleEndian, 0x2000_0001, edid, 128),
    )
    .unwrap();
    assert!(matches!(
        request,
        XWireRequest::RandrGetOutputProperty {
            output: 0x2000_0001,
            property,
            long_length: 128,
            ..
        } if property == edid
    ));
    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 6, XByteOrder::LittleEndian, X_RANDR_MAJOR_OPCODE),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0].len(), 32);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 0);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 0);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 0);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][12..16]), 0);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][16..20]), 0);
}

#[test]
fn randr_conventional_output_properties_are_valid_across_two_outputs() {
    let namespace = NamespaceId::from_raw(45);
    let topology = OutputTopologySnapshot {
        generation: 1,
        primary: OutputId::from_raw(1),
        outputs: vec![
            OutputTopologyEntry {
                output: OutputId::from_raw(1),
                logical: Rect {
                    x: 0,
                    y: 0,
                    width: 1280,
                    height: 720,
                },
                pixel_size: Size {
                    width: 1280,
                    height: 720,
                },
                scale: 1,
                refresh_millihz: 60_000,
            },
            OutputTopologyEntry {
                output: OutputId::from_raw(2),
                logical: Rect {
                    x: 1280,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                pixel_size: Size {
                    width: 1920,
                    height: 1080,
                },
                scale: 1,
                refresh_millihz: 60_000,
            },
        ],
    };
    let mut runtime = XAuthorityRuntime::with_output_topology(topology).unwrap();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let edid = atoms.atom(X_ATOM_NAME_RANDR_EDID).unwrap();
    let non_desktop = atoms.atom(X_ATOM_NAME_RANDR_NON_DESKTOP).unwrap();

    for (sequence, output) in [0x2000_0001, 0x2000_0002].into_iter().enumerate() {
        let edid_request = decode_x11_core_request(
            context(namespace, 600 + u64::try_from(sequence).unwrap(), XByteOrder::LittleEndian),
            &randr_get_output_property_request(
                XByteOrder::LittleEndian,
                output,
                edid,
                128,
            ),
        )
        .unwrap();
        let edid_result = dispatch_x11_wire_request(
            dispatch_context(
                namespace,
                u16::try_from(sequence + 1).unwrap(),
                XByteOrder::LittleEndian,
                X_RANDR_MAJOR_OPCODE,
            ),
            edid_request,
            &mut runtime,
            &mut atoms,
            &mut properties,
        );
        let encoded = edid_result.encoded_outputs(XByteOrder::LittleEndian);
        assert_eq!(encoded[0][0], 1);
        assert_eq!(encoded[0][1], 0);
        assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 0);

        let non_desktop_request = decode_x11_core_request(
            context(namespace, 610 + u64::try_from(sequence).unwrap(), XByteOrder::LittleEndian),
            &randr_get_output_property_request(
                XByteOrder::LittleEndian,
                output,
                non_desktop,
                1,
            ),
        )
        .unwrap();
        let non_desktop_result = dispatch_x11_wire_request(
            dispatch_context(
                namespace,
                u16::try_from(sequence + 3).unwrap(),
                XByteOrder::LittleEndian,
                X_RANDR_MAJOR_OPCODE,
            ),
            non_desktop_request,
            &mut runtime,
            &mut atoms,
            &mut properties,
        );
        let encoded = non_desktop_result.encoded_outputs(XByteOrder::LittleEndian);
        assert_eq!(encoded[0][0], 1);
        assert_eq!(encoded[0][1], 32);
        assert_eq!(
            read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]),
            X_ATOM_CARDINAL
        );
        assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][16..20]), 1);
        assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][32..36]), 0);
    }

    let invalid_atom = decode_x11_core_request(
        context(namespace, 620, XByteOrder::LittleEndian),
        &randr_get_output_property_request(
            XByteOrder::LittleEndian,
            0x2000_0001,
            0xffff_fffe,
            1,
        ),
    )
    .unwrap();
    let invalid_atom = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            5,
            XByteOrder::LittleEndian,
            X_RANDR_MAJOR_OPCODE,
        ),
        invalid_atom,
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(invalid_atom[0][0], 0);
    assert_eq!(invalid_atom[0][1], 5, "invalid property atom is BadAtom");

    let invalid_output = decode_x11_core_request(
        context(namespace, 621, XByteOrder::LittleEndian),
        &randr_get_output_property_request(
            XByteOrder::LittleEndian,
            0x2fff_ffff,
            edid,
            1,
        ),
    )
    .unwrap();
    let invalid_output = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            6,
            XByteOrder::LittleEndian,
            X_RANDR_MAJOR_OPCODE,
        ),
        invalid_output,
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(invalid_output[0][0], 0);
    assert_eq!(invalid_output[0][1], 2, "invalid output is BadValue");
}

#[test]
fn xfixes_regions_support_create_set_and_destroy_lifecycle() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let region = 0x220100;
    let rectangles = [Rect {
        x: 0,
        y: 0,
        width: 310,
        height: 257,
    }];

    for (sequence, request) in [
        xfixes_create_region_request(XByteOrder::LittleEndian, region, &[]),
        xfixes_set_region_request(XByteOrder::LittleEndian, region, &rectangles),
    ]
    .into_iter()
    .enumerate()
    {
        let request = decode_x11_core_request(
            context(namespace, 540 + sequence as u64, XByteOrder::LittleEndian),
            &request,
        )
        .unwrap();
        if sequence == 1 {
            assert!(matches!(
                request,
                XWireRequest::XfixesSetRegion {
                    rectangles: ref decoded,
                    ..
                } if decoded == &rectangles
            ));
        }
        let result = dispatch_x11_wire_request(
            dispatch_context(
                namespace,
                5 + sequence as u16,
                XByteOrder::LittleEndian,
                X_XFIXES_MAJOR_OPCODE,
            ),
            request,
            &mut runtime,
            &mut atoms,
            &mut properties,
        );
        assert!(result.outputs.is_empty());
    }

    let region_id = XResourceId::new(u64::from(region), 1);
    assert_eq!(
        runtime.validate_xfixes_region_access(namespace, region_id),
        Ok(())
    );
    let destroy = XWireRequest::XfixesDestroyRegion { region: region_id };
    let result = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            7,
            XByteOrder::LittleEndian,
            X_XFIXES_MAJOR_OPCODE,
        ),
        destroy,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(result.outputs.is_empty());
    assert_eq!(
        runtime.validate_xfixes_region_access(namespace, region_id),
        Err(XAuthorityRuntimeError::UnknownResource)
    );
}

#[test]
fn xfixes_selection_subscription_accepts_known_window_atom_and_mask() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let window = 0x220101;
    let create = decode_x11_core_request(
        context(namespace, 543, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, window, 0, 0, 1, 1),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 6, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let request = decode_x11_core_request(
        context(namespace, 544, XByteOrder::LittleEndian),
        &xfixes_select_selection_input_request(
            XByteOrder::LittleEndian,
            window,
            X_ATOM_PRIMARY,
            0b111,
        ),
    )
    .unwrap();
    assert!(matches!(
        request,
        XWireRequest::XfixesSelectSelectionInput {
            selection: X_ATOM_PRIMARY,
            event_mask: 0b111,
            ..
        }
    ));
    let result = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            7,
            XByteOrder::LittleEndian,
            X_XFIXES_MAJOR_OPCODE,
        ),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(result.outputs.is_empty());
}

#[test]
fn x11_dispatch_advertises_probe_backed_xkeyboard_extension() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let query = decode_x11_core_request(
        context(namespace, 545, XByteOrder::LittleEndian),
        &query_extension_request(XByteOrder::LittleEndian, X_KEYBOARD_EXTENSION_NAME),
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
    assert_eq!(encoded[0][9], X_KEYBOARD_MAJOR_OPCODE);
    assert_eq!(encoded[0][10], X_KEYBOARD_FIRST_EVENT);

    let use_extension = decode_x11_core_request(
        context(namespace, 546, XByteOrder::LittleEndian),
        &xkb_use_extension_request(XByteOrder::LittleEndian, 1, 0),
    )
    .unwrap();
    assert_eq!(
        use_extension,
        XWireRequest::XkbUseExtension {
            wanted_major: 1,
            wanted_minor: 0,
        }
    );
    let use_extension = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            2,
            XByteOrder::LittleEndian,
            X_KEYBOARD_MAJOR_OPCODE,
        ),
        use_extension,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = use_extension.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]), 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][10..12]), 0);
}

#[test]
fn extension_event_ranges_do_not_replace_core_or_each_other() {
    let ranges = [
        ("RANDR", X_RANDR_FIRST_EVENT, 2_u8),
        ("XFIXES", X_XFIXES_FIRST_EVENT, 2),
        ("SYNC", X_SYNC_FIRST_EVENT, 2),
        ("XKEYBOARD", X_KEYBOARD_FIRST_EVENT, 1),
        ("GLX", X_GLX_FIRST_EVENT, 17),
        ("XInputExtension", X_INPUT_FIRST_EVENT, 17),
        ("MIT-SHM", X_MIT_SHM_FIRST_EVENT, 1),
    ];
    let mut owners = std::collections::BTreeMap::new();
    for (name, first, count) in ranges {
        assert!(
            first > 35,
            "{name} event base {first} collides with core X11 events"
        );
        for event_type in first..first + count {
            assert!(
                owners.insert(event_type, name).is_none(),
                "{name} event type {event_type} overlaps another extension"
            );
        }

        let namespace = NamespaceId::from_raw(46);
        let query = decode_x11_core_request(
            context(namespace, u64::from(first), XByteOrder::LittleEndian),
            &query_extension_request(XByteOrder::LittleEndian, name),
        )
        .unwrap();
        let encoded = dispatch_x11_wire_request(
            dispatch_context(namespace, 1, XByteOrder::LittleEndian, 98),
            query,
            &mut XAuthorityRuntime::new(),
            &mut XAtomTable::new(),
            &mut XPropertyTable::new(),
        )
        .encoded_outputs(XByteOrder::LittleEndian);
        assert_eq!(
            encoded[0][10], first,
            "{name} did not advertise its allocated event base"
        );
    }
}

#[test]
fn x11_dispatch_advertises_non_core_glx_event_base() {
    let namespace = NamespaceId::from_raw(46);
    let query = decode_x11_core_request(
        context(namespace, 547, XByteOrder::LittleEndian),
        &query_extension_request(XByteOrder::LittleEndian, X_GLX_EXTENSION_NAME),
    )
    .unwrap();
    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 98),
        query,
        &mut XAuthorityRuntime::new(),
        &mut XAtomTable::new(),
        &mut XPropertyTable::new(),
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][9], X_GLX_MAJOR_OPCODE);
    assert_eq!(encoded[0][10], X_GLX_FIRST_EVENT);
}

#[test]
fn xkb_state_names_and_state_subscription_use_standard_wire_layouts() {
    let namespace = NamespaceId::from_raw(45);
    let order = XByteOrder::LittleEndian;
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    let get_state = decode_x11_core_request(
        context(namespace, 1, order),
        &[
            X_KEYBOARD_MAJOR_OPCODE,
            X_KEYBOARD_GET_STATE_MINOR_OPCODE,
            2,
            0,
            3,
            0,
            0,
            0,
        ],
    )
    .unwrap();
    assert_eq!(get_state, XWireRequest::XkbGetState);
    let state = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, order, X_KEYBOARD_MAJOR_OPCODE),
        get_state,
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(order);
    assert_eq!(state[0].len(), 32);
    assert_eq!(state[0][1], 3);

    let names = decode_x11_core_request(
        context(namespace, 2, order),
        &[
            X_KEYBOARD_MAJOR_OPCODE,
            X_KEYBOARD_GET_NAMES_MINOR_OPCODE,
            3,
            0,
            3,
            0,
            0,
            0,
            0x3f,
            0,
            0,
            0,
        ],
    )
    .unwrap();
    assert_eq!(names, XWireRequest::XkbGetNames { which: 0x3f });
    let names = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, order, X_KEYBOARD_MAJOR_OPCODE),
        names,
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(order);
    assert_eq!(read_u32(order, &names[0][8..12]), 0x3f);
    assert_eq!(names[0].len(), 56);
    assert_eq!(names[0][12], 8);
    assert_eq!(names[0][13], u8::MAX);

    let select = decode_x11_core_request(
        context(namespace, 3, order),
        &[
            X_KEYBOARD_MAJOR_OPCODE,
            X_KEYBOARD_SELECT_EVENTS_MINOR_OPCODE,
            5,
            0,
            3,
            0,
            4,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            0,
            1,
            0,
        ],
    )
    .unwrap();
    assert_eq!(
        select,
        XWireRequest::XkbSelectEvents {
            affect_which: 4,
            clear: 0,
            select_all: 0,
            state_details: Some((1, 1)),
        }
    );

    let notify = encode_x_client_event(
        order,
        XClientEvent::XkbStateNotify {
            sequence: 7,
            time: 11,
            modifiers: 1,
            changed: 1,
            keycode: 50,
            event_type: 2,
        },
    );
    assert_eq!(notify[0], X_KEYBOARD_FIRST_EVENT);
    assert_eq!(notify[1], 2);
    assert_eq!(read_u16(order, &notify[24..26]), 1);
    assert_eq!(notify[26], 50);
}

#[test]
fn xge_and_xi2_report_versioned_master_device_classes() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let query = decode_x11_core_request(
        context(namespace, 1, XByteOrder::LittleEndian),
        &query_extension_request(XByteOrder::LittleEndian, X_GENERIC_EVENT_EXTENSION_NAME),
    )
    .unwrap();
    let encoded = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 98),
        query,
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][8], 1);
    assert_eq!(encoded[0][9], X_GENERIC_EVENT_MAJOR_OPCODE);

    let version = decode_x11_core_request(
        context(namespace, 2, XByteOrder::LittleEndian),
        &[X_GENERIC_EVENT_MAJOR_OPCODE, 0, 2, 0, 1, 0, 0, 0],
    )
    .unwrap();
    assert_eq!(
        version,
        XWireRequest::GeQueryVersion {
            major_version: 1,
            minor_version: 0
        }
    );
    let encoded = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            2,
            XByteOrder::LittleEndian,
            X_GENERIC_EVENT_MAJOR_OPCODE,
        ),
        version,
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]), 1);

    let xi_version = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, X_INPUT_MAJOR_OPCODE),
        XWireRequest::XiQueryVersion {
            major_version: 2,
            minor_version: 3,
        },
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &xi_version[0][8..10]),
        2
    );
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &xi_version[0][10..12]),
        1
    );

    let devices = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, X_INPUT_MAJOR_OPCODE),
        XWireRequest::XiQueryDevice { device_id: 0 },
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &devices[0][8..10]), 2);
    let pointer_class_count = read_u16(XByteOrder::LittleEndian, &devices[0][38..40]);
    assert_eq!(pointer_class_count, 7);
    let pointer_name_len = usize::from(read_u16(
        XByteOrder::LittleEndian,
        &devices[0][40..42],
    ));
    let mut class_offset = 44 + pointer_name_len.next_multiple_of(4);
    let mut valuators = Vec::new();
    let mut scrolls = Vec::new();
    for _ in 0..pointer_class_count {
        let class_type = read_u16(
            XByteOrder::LittleEndian,
            &devices[0][class_offset..class_offset + 2],
        );
        let class_len = usize::from(read_u16(
            XByteOrder::LittleEndian,
            &devices[0][class_offset + 2..class_offset + 4],
        )) * 4;
        match class_type {
            2 => valuators.push((
                read_u16(
                    XByteOrder::LittleEndian,
                    &devices[0][class_offset + 6..class_offset + 8],
                ),
                read_u64(
                    XByteOrder::LittleEndian,
                    &devices[0][class_offset + 12..class_offset + 20],
                ) as i64,
                read_u64(
                    XByteOrder::LittleEndian,
                    &devices[0][class_offset + 20..class_offset + 28],
                ) as i64,
                read_u64(
                    XByteOrder::LittleEndian,
                    &devices[0][class_offset + 28..class_offset + 36],
                ) as i64,
            )),
            3 => scrolls.push((
                read_u16(
                    XByteOrder::LittleEndian,
                    &devices[0][class_offset + 6..class_offset + 8],
                ),
                read_u16(
                    XByteOrder::LittleEndian,
                    &devices[0][class_offset + 8..class_offset + 10],
                ),
            )),
            _ => {}
        }
        class_offset += class_len;
    }
    assert_eq!(
        valuators,
        vec![
            (0, 0, i64::from(u16::MAX) << 32, 0),
            (1, 0, i64::from(u16::MAX) << 32, 0),
            (2, 0, 0, 0),
            (3, 0, 0, 0),
        ]
    );
    assert_eq!(scrolls, vec![(2, 2), (3, 1)]);
    assert!(devices[0].len() > 128);
}

#[test]
fn xi_query_pointer_encodes_coordinates_buttons_and_modifiers() {
    let reply = encode_x_client_output(
        XByteOrder::LittleEndian,
        XClientOutput::Reply(XClientReply::XiQueryPointer {
            sequence: 9,
            root: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            child: XResourceId::new(0x220031, 1),
            root_x: 320,
            root_y: 240,
            win_x: -12,
            win_y: 18,
            buttons: (1 << 1) | (1 << 3),
            modifiers: 5,
        }),
    );

    assert_eq!(reply.len(), 60);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &reply[12..16]), 0x220031);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &reply[16..20]), 320 << 16);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &reply[24..28]),
        (-12_i32 << 16) as u32
    );
    assert_eq!(read_u16(XByteOrder::LittleEndian, &reply[34..36]), 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &reply[48..52]), 5);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &reply[56..60]), 10);
}

#[test]
fn xkb_get_map_encodes_schema_aligned_types_symbols_and_modifier_map() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let result = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            4,
            XByteOrder::LittleEndian,
            X_KEYBOARD_MAJOR_OPCODE,
        ),
        XWireRequest::XkbGetMap {
            full: 0x47,
            partial: 0,
        },
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    let reply = &encoded[0];
    assert_eq!(&reply[8..10], &[0, 0]);
    assert_eq!(reply[10], 8);
    assert_eq!(reply[11], u8::MAX);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &reply[12..14]), 0x47);
    assert_eq!(&reply[14..18], &[0, 4, 4, 8]);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &reply[18..20]), 496);
    assert_eq!(reply[20], 248);
    assert_eq!(&reply[31..34], &[8, 248, 10]);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &reply[4..8]) as usize,
        (reply.len() - 32) / 4
    );
    assert_eq!(&reply[40..48], &[1, 1, 0, 0, 2, 1, 0, 0]);
    assert_eq!(&reply[104..112], &[0, 0, 0, 0, 1, 2, 2, 0]);
}

#[test]
fn xkb_state_uses_deterministic_rmlvo_and_tracks_effective_modifiers() {
    let mut keyboard = XkbKeyboardState::new(&XkbRmlvoConfig::default()).unwrap();
    assert_eq!(keyboard.map_evdev_key(42, true), Some((50, 0)));
    assert_eq!(keyboard.map_evdev_key(30, true), Some((38, 1)));
    assert_eq!(keyboard.map_evdev_key(30, false), Some((38, 1)));
    assert_eq!(keyboard.map_evdev_key(42, false), Some((50, 1)));
    assert_eq!(keyboard.modifier_mask(), 0);
}

#[test]
fn xkb_snapshot_drives_core_and_xkb_maps_from_the_same_rmlvo() {
    let us = XkbKeymapSnapshot::new(&XkbRmlvoConfig::default()).unwrap();
    let de_config = XkbRmlvoConfig {
        layout: "de".to_owned(),
        ..XkbRmlvoConfig::default()
    };
    let de = XkbKeymapSnapshot::new(&de_config).unwrap();

    assert_eq!(us.config().layout, "us");
    assert_eq!(de.config().layout, "de");
    assert_eq!(us.core_mapping(8, 248), us.xkb_keysyms().concat());
    assert_eq!(de.core_mapping(8, 248), de.xkb_keysyms().concat());
    assert_ne!(us.core_mapping(29, 1), de.core_mapping(29, 1));
}

#[test]
fn xkb_rmlvo_validation_rejects_empty_and_unbounded_configuration() {
    let mut empty = XkbRmlvoConfig::default();
    empty.layout.clear();
    assert_eq!(
        XkbKeyboardState::new(&empty).unwrap_err(),
        XkbKeyboardError::InvalidConfiguration
    );

    let unbounded = XkbRmlvoConfig {
        options: "x".repeat(XKB_RMLVO_FIELD_MAX_BYTES + 1),
        ..XkbRmlvoConfig::default()
    };
    assert_eq!(
        XkbKeyboardState::new(&unbounded).unwrap_err(),
        XkbKeyboardError::InvalidConfiguration
    );
}
