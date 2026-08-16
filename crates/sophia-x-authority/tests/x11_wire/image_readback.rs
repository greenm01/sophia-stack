fn create_readback_window(
    runtime: &mut XAuthorityRuntime,
    atoms: &mut XAtomTable,
    properties: &mut XPropertyTable,
    namespace: NamespaceId,
    byte_order: XByteOrder,
    window: u32,
    width: u16,
    height: u16,
    map: bool,
) {
    let create = decode_x11_core_request(
        context(namespace, 700, byte_order),
        &create_window_request(byte_order, window, 10, 20, width, height),
    )
    .unwrap();
    let created = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, byte_order, 1),
        create,
        runtime,
        atoms,
        properties,
    );
    assert_eq!(created.response.unwrap().outcome, XAuthorityResponseOutcome::Accepted);
    if map {
        let request = decode_x11_core_request(
            context(namespace, 701, byte_order),
            &resource_request(byte_order, 8, window),
        )
        .unwrap();
        let mapped = dispatch_x11_wire_request(
            dispatch_context(namespace, 2, byte_order, 8),
            request,
            runtime,
            atoms,
            properties,
        );
        assert_eq!(mapped.response.unwrap().outcome, XAuthorityResponseOutcome::Accepted);
    }
}

#[allow(clippy::too_many_arguments)]
fn dispatch_get_image_for_test(
    runtime: &mut XAuthorityRuntime,
    atoms: &mut XAtomTable,
    properties: &mut XPropertyTable,
    namespace: NamespaceId,
    byte_order: XByteOrder,
    drawable: u32,
    format: u8,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
    plane_mask: u32,
) -> XDispatchResult {
    let request = decode_x11_core_request(
        context(namespace, 702, byte_order),
        &get_image_request(
            byte_order,
            format,
            drawable,
            x,
            y,
            width,
            height,
            plane_mask,
        ),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 3, byte_order, 73),
        request,
        runtime,
        atoms,
        properties,
    )
}

#[test]
fn get_image_returns_large_z_pixmap_pixels_in_both_byte_orders() {
    for (index, byte_order) in [XByteOrder::LittleEndian, XByteOrder::BigEndian]
        .into_iter()
        .enumerate()
    {
        let namespace = NamespaceId::from_raw(110 + index as u64);
        let window = 0x0022_1000 + index as u32;
        let mut runtime = XAuthorityRuntime::new();
        let mut atoms = XAtomTable::new();
        let mut properties = XPropertyTable::new();
        create_readback_window(
            &mut runtime,
            &mut atoms,
            &mut properties,
            namespace,
            byte_order,
            window,
            400,
            200,
            true,
        );
        let marker = [0x11, 0x22, 0x33, 0x00];
        let response = runtime.apply_put_image(
            TransactionId::from_raw(4),
            namespace,
            XResourceId::new(u64::from(window), 1),
            Region::single(Rect {
                x: 3,
                y: 5,
                width: 1,
                height: 1,
            }),
            Some(&marker), None,);
        assert_eq!(response.outcome, XAuthorityResponseOutcome::Accepted);

        let encoded = dispatch_get_image_for_test(
            &mut runtime,
            &mut atoms,
            &mut properties,
            namespace,
            byte_order,
            window,
            2,
            0,
            0,
            400,
            200,
            u32::MAX,
        )
        .encoded_outputs(byte_order);
        assert_eq!(encoded.len(), 1);
        assert_eq!(encoded[0].len(), 32 + 400 * 200 * 4);
        assert_eq!(encoded[0][1], 24);
        assert_eq!(read_u32(byte_order, &encoded[0][4..8]), 400 * 200);
        assert_eq!(
            read_u32(byte_order, &encoded[0][8..12]),
            X_SETUP_DEFAULT_VISUAL
        );
        let marker_offset = 32 + (5 * 400 + 3) * 4;
        let expected = match byte_order {
            XByteOrder::LittleEndian => [0x11, 0x22, 0x33, 0x00],
            XByteOrder::BigEndian => [0x00, 0x33, 0x22, 0x11],
        };
        assert_eq!(encoded[0][marker_offset..marker_offset + 4], expected);
    }
}

#[test]
fn get_image_sizes_xy_planes_and_empty_replies_exactly() {
    for (index, byte_order) in [XByteOrder::LittleEndian, XByteOrder::BigEndian]
        .into_iter()
        .enumerate()
    {
        let namespace = NamespaceId::from_raw(120 + index as u64);
        let window = 0x0022_1100 + index as u32;
        let mut runtime = XAuthorityRuntime::new();
        let mut atoms = XAtomTable::new();
        let mut properties = XPropertyTable::new();
        create_readback_window(
            &mut runtime,
            &mut atoms,
            &mut properties,
            namespace,
            byte_order,
            window,
            8,
            1,
            true,
        );
        assert_eq!(
            runtime
                .apply_put_image(
                    TransactionId::from_raw(5),
                    namespace,
                    XResourceId::new(u64::from(window), 1),
                    Region::single(Rect {
                        x: 0,
                        y: 0,
                        width: 2,
                        height: 1,
                    }),
                    Some(&[5, 0, 0, 0, 1, 0, 0, 0]), None,)
                .outcome,
            XAuthorityResponseOutcome::Accepted
        );
        let xy = dispatch_get_image_for_test(
            &mut runtime,
            &mut atoms,
            &mut properties,
            namespace,
            byte_order,
            window,
            1,
            0,
            0,
            8,
            1,
            0b101,
        )
        .encoded_outputs(byte_order);
        assert_eq!(xy[0].len(), 40);
        assert_eq!(read_u32(byte_order, &xy[0][4..8]), 2);
        let expected = match byte_order {
            XByteOrder::LittleEndian => (0b0000_0001, 0b0000_0011),
            XByteOrder::BigEndian => (0b1000_0000, 0b1100_0000),
        };
        assert_eq!(xy[0][32], expected.0);
        assert_eq!(xy[0][36], expected.1);

        let empty = dispatch_get_image_for_test(
            &mut runtime,
            &mut atoms,
            &mut properties,
            namespace,
            byte_order,
            window,
            2,
            8,
            1,
            0,
            0,
            u32::MAX,
        )
        .encoded_outputs(byte_order);
        assert_eq!(empty[0].len(), 32);
        assert_eq!(read_u32(byte_order, &empty[0][4..8]), 0);
    }
}

#[test]
fn get_image_reports_protocol_specific_validation_errors() {
    let namespace = NamespaceId::from_raw(130);
    let foreign_namespace = NamespaceId::from_raw(131);
    let byte_order = XByteOrder::LittleEndian;
    let window = 0x0022_1200;
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    let invalid_format = decode_x11_core_request(
        context(namespace, 703, byte_order),
        &get_image_request(byte_order, 0, window, 0, 0, 1, 1, u32::MAX),
    );
    assert_eq!(invalid_format, Err(XWireParseError::InvalidValue(0)));

    let missing = dispatch_get_image_for_test(
        &mut runtime,
        &mut atoms,
        &mut properties,
        namespace,
        byte_order,
        window,
        2,
        0,
        0,
        1,
        1,
        u32::MAX,
    )
    .encoded_outputs(byte_order);
    assert_eq!(missing[0][1], XErrorCode::BadDrawable.wire_code());

    create_readback_window(
        &mut runtime,
        &mut atoms,
        &mut properties,
        namespace,
        byte_order,
        window,
        100,
        80,
        false,
    );
    let unviewable = dispatch_get_image_for_test(
        &mut runtime,
        &mut atoms,
        &mut properties,
        namespace,
        byte_order,
        window,
        2,
        0,
        0,
        1,
        1,
        u32::MAX,
    )
    .encoded_outputs(byte_order);
    assert_eq!(unviewable[0][1], XErrorCode::BadMatch.wire_code());

    let map = decode_x11_core_request(
        context(namespace, 704, byte_order),
        &resource_request(byte_order, 8, window),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 4, byte_order, 8),
        map,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let outside = dispatch_get_image_for_test(
        &mut runtime,
        &mut atoms,
        &mut properties,
        namespace,
        byte_order,
        window,
        2,
        99,
        0,
        2,
        1,
        u32::MAX,
    )
    .encoded_outputs(byte_order);
    assert_eq!(outside[0][1], XErrorCode::BadMatch.wire_code());
    let denied = dispatch_get_image_for_test(
        &mut runtime,
        &mut atoms,
        &mut properties,
        foreign_namespace,
        byte_order,
        window,
        2,
        0,
        0,
        1,
        1,
        u32::MAX,
    )
    .encoded_outputs(byte_order);
    assert_eq!(denied[0][1], XErrorCode::BadAccess.wire_code());

    let pixmap = 0x0022_1201;
    let create_pixmap = decode_x11_core_request(
        context(namespace, 705, byte_order),
        &create_pixmap_request(byte_order, 24, pixmap, X_SETUP_DEFAULT_ROOT, 5000, 5000),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 5, byte_order, 53),
        create_pixmap,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let oversized = dispatch_get_image_for_test(
        &mut runtime,
        &mut atoms,
        &mut properties,
        namespace,
        byte_order,
        pixmap,
        2,
        0,
        0,
        5000,
        5000,
        u32::MAX,
    )
    .encoded_outputs(byte_order);
    assert_eq!(oversized[0][1], XErrorCode::BadAlloc.wire_code());
}
