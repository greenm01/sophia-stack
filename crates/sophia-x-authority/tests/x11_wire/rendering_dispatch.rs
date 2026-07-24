#[test]
fn input_focus_state_is_scoped_per_namespace() {
    let first = NamespaceId::from_raw(45);
    let second = NamespaceId::from_raw(46);
    let mut runtime = XAuthorityRuntime::new();
    let none = XResourceId::new(0, 1);

    runtime.set_input_focus(first, none, 2).unwrap();
    assert_eq!(runtime.input_focus(first), (none, 2));
    assert_eq!(
        runtime.input_focus(second),
        (XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1), 1)
    );
}

#[test]
fn x11_dispatch_advertises_big_requests_and_replies_to_enable() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let query = decode_x11_core_request(
        context(namespace, 547, XByteOrder::LittleEndian),
        &query_extension_request(XByteOrder::LittleEndian, X_BIG_REQUESTS_EXTENSION_NAME),
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
    assert_eq!(encoded[0][9], X_BIG_REQUESTS_MAJOR_OPCODE);

    let enable = decode_x11_core_request(
        context(namespace, 548, XByteOrder::LittleEndian),
        &[
            X_BIG_REQUESTS_MAJOR_OPCODE,
            X_BIG_REQUESTS_ENABLE_MINOR_OPCODE,
            1,
            0,
        ],
    )
    .unwrap();
    assert_eq!(enable, XWireRequest::BigRequestsEnable);
    let enable = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            2,
            XByteOrder::LittleEndian,
            X_BIG_REQUESTS_MAJOR_OPCODE,
        ),
        enable,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = enable.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]),
        u32::from(X_SETUP_DEFAULT_MAX_REQUEST_UNITS)
    );
}

#[test]
fn x11_dispatch_mit_shm_attach_is_namespace_local_metadata() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let attach = decode_x11_core_request(
        context(namespace, 528, XByteOrder::LittleEndian),
        &mit_shm_attach_request(XByteOrder::LittleEndian, 0x440010, 88, false),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            1,
            XByteOrder::LittleEndian,
            X_MIT_SHM_MAJOR_OPCODE,
        ),
        attach,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(result.outputs.is_empty());
    assert_eq!(runtime.shm_segment_count(), 1);
    assert!(
        runtime
            .validate_shm_segment_access(namespace, XResourceId::new(0x440010, 1))
            .is_ok()
    );
    assert!(
        runtime
            .validate_shm_segment_access(NamespaceId::from_raw(46), XResourceId::new(0x440010, 1))
            .is_err()
    );

    let null_detach = decode_x11_core_request(
        context(namespace, 529, XByteOrder::LittleEndian),
        &mit_shm_detach_request(XByteOrder::LittleEndian, 0),
    )
    .unwrap();
    let null_detach = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            2,
            XByteOrder::LittleEndian,
            X_MIT_SHM_MAJOR_OPCODE,
        ),
        null_detach,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(null_detach.outputs.is_empty());
    assert_eq!(runtime.shm_segment_count(), 1);
}

#[test]
fn x11_dispatch_mit_shm_put_image_emits_bounded_surface_transaction() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    let missing = decode_x11_core_request(
        context(namespace, 529, XByteOrder::LittleEndian),
        &mit_shm_put_image_request(XByteOrder::LittleEndian, 0x220701, 0x220702, 0x440020, 0),
    )
    .unwrap();
    let missing = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            1,
            XByteOrder::LittleEndian,
            X_MIT_SHM_MAJOR_OPCODE,
        ),
        missing,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = missing.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(
        missing.response.unwrap().outcome,
        XAuthorityResponseOutcome::Accepted
    );
    assert_eq!(encoded[0][0], 0);
    assert_eq!(encoded[0][1], XErrorCode::BadAccess.wire_code());
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]),
        0x440020
    );

    runtime
        .attach_shm_segment(namespace, XResourceId::new(0x440020, 1), 88, false, 1)
        .unwrap();
    let create = decode_x11_core_request(
        context(namespace, 530, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, 0x220701, 0, 0, 300, 200),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let attached = decode_x11_core_request(
        context(namespace, 531, XByteOrder::LittleEndian),
        &mit_shm_put_image_request(XByteOrder::LittleEndian, 0x220701, 0x220702, 0x440020, 0),
    )
    .unwrap();
    let attached = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            3,
            XByteOrder::LittleEndian,
            X_MIT_SHM_MAJOR_OPCODE,
        ),
        attached,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(attached.outputs.is_empty());
    let response = attached.response.unwrap();
    assert_eq!(response.outcome, XAuthorityResponseOutcome::Accepted);
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].surface,
        SurfaceId::new(0x220701, 1)
    );
    assert!(matches!(
        response.transactions[0].target_buffer,
        BufferSource::CpuBuffer { .. }
    ));
    assert_eq!(
        response.transactions[0].damage,
        Region::single(Rect {
            x: 3,
            y: 5,
            width: 32,
            height: 24,
        })
    );
}

#[test]
fn x11_image_text_updates_bounded_xrgb_cpu_pixels() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let create = decode_x11_core_request(
        context(namespace, 532, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, 0x220801, 0, 0, 160, 40),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let gc = decode_x11_core_request(
        context(namespace, 533, XByteOrder::LittleEndian),
        &create_gc_request(XByteOrder::LittleEndian, 0x220802, 0x220801),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 55),
        gc,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let text = decode_x11_core_request(
        context(namespace, 534, XByteOrder::LittleEndian),
        &image_text8_request(
            XByteOrder::LittleEndian,
            0x220801,
            0x220802,
            4,
            16,
            b"Sophia",
        ),
    )
    .unwrap();
    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 76),
        text,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let response = result.response.unwrap();
    assert_eq!(response.outcome, XAuthorityResponseOutcome::Accepted);
    assert_eq!(response.transactions.len(), 1);
    let XAuthorityCpuBufferUpdate::Replace(snapshot) = runtime.take_cpu_buffer_update().unwrap()
    else {
        panic!("first CPU buffer update must replace the buffer");
    };
    assert_eq!(snapshot.drawable, XResourceId::new(0x220801, 1));
    assert_eq!(
        snapshot.size,
        Size {
            width: 160,
            height: 40
        }
    );
    assert_eq!(snapshot.stride, 640);
    assert_eq!(snapshot.format, X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888);
    assert_eq!(snapshot.generation, 1);
    assert!(
        snapshot
            .bytes
            .chunks_exact(4)
            .any(|pixel| pixel != [0, 0, 0, 0])
    );
    assert_eq!(
        response.transactions[0].target_buffer,
        BufferSource::CpuBuffer {
            handle: snapshot.handle
        }
    );
}

#[test]
fn x11_dispatch_reports_empty_extension_list() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 523, XByteOrder::LittleEndian),
        &[99, 0, 1, 0],
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 99),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 0);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 0);
}

#[test]
fn x11_dispatch_query_best_size_echoes_requested_dimensions() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let mut bytes = vec![97, 0];
    push_u16(&mut bytes, XByteOrder::LittleEndian, 3);
    push_u32(&mut bytes, XByteOrder::LittleEndian, X_SETUP_DEFAULT_ROOT);
    push_u16(&mut bytes, XByteOrder::LittleEndian, 64);
    push_u16(&mut bytes, XByteOrder::LittleEndian, 32);
    let request =
        decode_x11_core_request(context(namespace, 524, XByteOrder::LittleEndian), &bytes).unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 97),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]), 64);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][10..12]), 32);
}

#[test]
fn x11_dispatch_get_geometry_reports_root_dimensions() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 525, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 14, X_SETUP_DEFAULT_ROOT),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 14),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 24);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]),
        X_SETUP_DEFAULT_ROOT
    );
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][12..14]), 0);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][14..16]), 0);
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][16..18]),
        X_SETUP_ROOT_WIDTH
    );
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][18..20]),
        X_SETUP_ROOT_HEIGHT
    );
}

#[test]
fn x11_dispatch_get_window_attributes_reports_root_visual_state() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 527, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 3, X_SETUP_DEFAULT_ROOT),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 3),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 3);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]),
        X_SETUP_DEFAULT_VISUAL
    );
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][12..14]), 1);
    assert_eq!(encoded[0][26], 2);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][28..32]),
        X_SETUP_DEFAULT_COLORMAP
    );
}

#[test]
fn x11_dispatch_query_tree_reports_root_without_children() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 528, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 15, X_SETUP_DEFAULT_ROOT),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 15),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 0);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]),
        X_SETUP_DEFAULT_ROOT
    );
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][12..16]), 0);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][16..18]), 0);
}

#[test]
fn x11_dispatch_randr_reports_root_screen_size_and_populated_resources() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let range = decode_x11_core_request(
        context(namespace, 540, XByteOrder::LittleEndian),
        &randr_window_request(
            XByteOrder::LittleEndian,
            X_RANDR_GET_SCREEN_SIZE_RANGE_MINOR_OPCODE,
            X_SETUP_DEFAULT_ROOT,
        ),
    )
    .unwrap();
    assert_eq!(
        range,
        XWireRequest::RandrGetScreenSizeRange {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
        }
    );

    let range = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, X_RANDR_MAJOR_OPCODE),
        range,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = range.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]),
        X_SETUP_ROOT_WIDTH
    );
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][10..12]),
        X_SETUP_ROOT_HEIGHT
    );
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][12..14]),
        X_SETUP_ROOT_WIDTH
    );
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][14..16]),
        X_SETUP_ROOT_HEIGHT
    );

    let resources = decode_x11_core_request(
        context(namespace, 541, XByteOrder::LittleEndian),
        &randr_window_request(
            XByteOrder::LittleEndian,
            X_RANDR_GET_SCREEN_RESOURCES_MINOR_OPCODE,
            X_SETUP_DEFAULT_ROOT,
        ),
    )
    .unwrap();
    assert_eq!(
        resources,
        XWireRequest::RandrGetScreenResources {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            current: false,
        }
    );
    let resources = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, X_RANDR_MAJOR_OPCODE),
        resources,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = resources.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]) > 0);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][16..18]), 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][18..20]), 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][20..22]), 1);
    assert!(read_u16(XByteOrder::LittleEndian, &encoded[0][22..24]) > 0);
}

#[test]
fn x11_dispatch_list_properties_reports_window_property_atoms() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let utf8 = atoms
        .intern(X_ATOM_NAME_UTF8_STRING, false)
        .unwrap()
        .unwrap();
    let net_wm_name = atoms
        .intern(X_ATOM_NAME_NET_WM_NAME, false)
        .unwrap()
        .unwrap();
    properties
        .apply_change(
            namespace,
            XPropertyChange {
                mode: XPropertyMode::Replace,
                window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
                property: net_wm_name,
                property_type: utf8,
                format: 8,
                bytes: b"Sophia root".to_vec(),
            },
        )
        .unwrap();
    let request = decode_x11_core_request(
        context(namespace, 529, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 21, X_SETUP_DEFAULT_ROOT),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 21),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]), 1);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][32..36]),
        net_wm_name
    );
}

#[test]
fn x11_dispatch_translate_coordinates_echoes_root_coordinates() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 526, XByteOrder::LittleEndian),
        &translate_coordinates_request(
            XByteOrder::LittleEndian,
            X_SETUP_DEFAULT_ROOT,
            X_SETUP_DEFAULT_ROOT,
            12,
            34,
        ),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 40),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 0);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][12..14]), 12);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][14..16]), 34);
}

#[test]
fn x11_dispatch_query_colors_returns_bounded_color_records() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 525, XByteOrder::LittleEndian),
        &query_colors_request(XByteOrder::LittleEndian, X_SETUP_DEFAULT_ROOT, &[0, 1]),
    )
    .unwrap();

    assert_eq!(
        request,
        XWireRequest::QueryColors {
            colormap: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            pixels: vec![0, 1],
        }
    );

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 91),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 4);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]), 2);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][32..34]), 0);
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][40..42]),
        u16::MAX
    );
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][42..44]),
        u16::MAX
    );
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][44..46]),
        u16::MAX
    );
}

