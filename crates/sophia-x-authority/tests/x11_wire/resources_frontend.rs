#[test]
fn x11_dispatch_accepts_open_and_close_font_resources() {
    let namespace = NamespaceId::from_raw(46);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    let open = decode_x11_core_request(
        context(namespace, 631, XByteOrder::LittleEndian),
        &open_font_request(XByteOrder::LittleEndian, 0x220131, "fixed"),
    )
    .unwrap();
    let open = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 45),
        open,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(open.outputs.is_empty());

    let query = decode_x11_core_request(
        context(namespace, 632, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 47, 0x220131),
    )
    .unwrap();
    let query = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 47),
        query,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = query.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 7);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][10..12]), 6);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][12..14]), 6);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][14..16]), 11);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][16..18]), 2);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][26..28]), 6);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][28..30]), 6);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][30..32]), 11);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][32..34]), 2);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][52..54]), 11);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][54..56]), 2);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][56..60]), 0);

    let list = decode_x11_core_request(
        context(namespace, 634, XByteOrder::LittleEndian),
        &list_fonts_request(XByteOrder::LittleEndian, 5, "*"),
    )
    .unwrap();
    let list = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 49),
        list,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = list.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]), 1);
    assert_eq!(encoded[0][32], 5);
    assert_eq!(&encoded[0][33..38], b"fixed");

    let list = decode_x11_core_request(
        context(namespace, 635, XByteOrder::LittleEndian),
        &list_fonts_with_info_request(XByteOrder::LittleEndian, 5, "*"),
    )
    .unwrap();
    let list = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, 50),
        list,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = list.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 5);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 9);
    assert_eq!(&encoded[0][60..65], b"fixed");
    assert_eq!(encoded[0][68], 1);
    assert_eq!(encoded[0][69], 0);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][72..76]), 7);

    let close = decode_x11_core_request(
        context(namespace, 636, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 46, 0x220131),
    )
    .unwrap();
    let close = dispatch_x11_wire_request(
        dispatch_context(namespace, 5, XByteOrder::LittleEndian, 46),
        close,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(close.outputs.is_empty());
}

#[test]
fn x11_dispatch_accepts_glyph_cursor_lifecycle() {
    let namespace = NamespaceId::from_raw(46);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    for (sequence, font) in [(1u16, 0x220141), (2u16, 0x220142)] {
        let open = decode_x11_core_request(
            context(
                namespace,
                640 + u64::from(sequence),
                XByteOrder::LittleEndian,
            ),
            &open_font_request(XByteOrder::LittleEndian, font, "cursor"),
        )
        .unwrap();
        let open = dispatch_x11_wire_request(
            dispatch_context(namespace, sequence, XByteOrder::LittleEndian, 45),
            open,
            &mut runtime,
            &mut atoms,
            &mut properties,
        );
        assert!(open.outputs.is_empty());
    }

    let cursor = decode_x11_core_request(
        context(namespace, 643, XByteOrder::LittleEndian),
        &create_glyph_cursor_request(XByteOrder::LittleEndian, 0x220143, 0x220141, 0x220142),
    )
    .unwrap();
    let cursor = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 94),
        cursor,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(cursor.outputs.is_empty());

    let recolor = decode_x11_core_request(
        context(namespace, 644, XByteOrder::LittleEndian),
        &recolor_cursor_request(XByteOrder::LittleEndian, 0x220143),
    )
    .unwrap();
    let recolor = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, 96),
        recolor,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(recolor.outputs.is_empty());

    let free = decode_x11_core_request(
        context(namespace, 645, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 95, 0x220143),
    )
    .unwrap();
    let free = dispatch_x11_wire_request(
        dispatch_context(namespace, 5, XByteOrder::LittleEndian, 95),
        free,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(free.outputs.is_empty());
}

#[test]
fn x11_dispatch_accepts_xterm_nil2_compatibility_font() {
    let namespace = NamespaceId::from_raw(46);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    let open = decode_x11_core_request(
        context(namespace, 646, XByteOrder::LittleEndian),
        &open_font_request(XByteOrder::LittleEndian, 0x220149, "nil2"),
    )
    .unwrap();
    let open = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 45),
        open,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(open.outputs.is_empty());
}

#[test]
fn x11_core_resource_ids_are_global_and_collisions_preserve_the_original() {
    let namespace = NamespaceId::from_raw(46);
    let window = 0x22014a;
    let gc = 0x22014b;
    let font = 0x22014c;
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    let create_window = decode_x11_core_request(
        context(namespace, 647, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, window, 3, 4, 64, 48),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 1),
        create_window,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    for (sequence, opcode, request) in [
        (
            2,
            45,
            open_font_request(XByteOrder::LittleEndian, window, "fixed"),
        ),
        (
            3,
            53,
            create_pixmap_request(
                XByteOrder::LittleEndian,
                24,
                window,
                window,
                16,
                16,
            ),
        ),
        (
            4,
            55,
            create_gc_request(XByteOrder::LittleEndian, window, window),
        ),
    ] {
        let request = decode_x11_core_request(
            context(namespace, 647 + u64::from(sequence), XByteOrder::LittleEndian),
            &request,
        )
        .unwrap();
        let result = dispatch_x11_wire_request(
            dispatch_context(namespace, sequence, XByteOrder::LittleEndian, opcode),
            request,
            &mut runtime,
            &mut atoms,
            &mut properties,
        );
        assert!(matches!(
            result.outputs.as_slice(),
            [XClientOutput::Error(XClientError {
                code: XErrorCode::BadIdChoice,
                resource_id,
                ..
            })] if *resource_id == window
        ));
    }
    assert_eq!(
        runtime
            .window_geometry(namespace, XResourceId::new(window.into(), 1))
            .unwrap(),
        Rect {
            x: 3,
            y: 4,
            width: 64,
            height: 48,
        }
    );

    let create_gc = decode_x11_core_request(
        context(namespace, 652, XByteOrder::LittleEndian),
        &create_gc_request(XByteOrder::LittleEndian, gc, window),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 5, XByteOrder::LittleEndian, 55),
        create_gc,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let collide_gc = decode_x11_core_request(
        context(namespace, 653, XByteOrder::LittleEndian),
        &open_font_request(XByteOrder::LittleEndian, gc, "fixed"),
    )
    .unwrap();
    let collide_gc = dispatch_x11_wire_request(
        dispatch_context(namespace, 6, XByteOrder::LittleEndian, 45),
        collide_gc,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(matches!(
        collide_gc.outputs.as_slice(),
        [XClientOutput::Error(XClientError {
            code: XErrorCode::BadIdChoice,
            resource_id,
            ..
        })] if *resource_id == gc
    ));
    assert!(runtime.graphics_context_values(namespace, XResourceId::new(gc.into(), 1)).is_ok());

    let open_font = decode_x11_core_request(
        context(namespace, 654, XByteOrder::LittleEndian),
        &open_font_request(XByteOrder::LittleEndian, font, "fixed"),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 7, XByteOrder::LittleEndian, 45),
        open_font,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let collide_font = decode_x11_core_request(
        context(namespace, 655, XByteOrder::LittleEndian),
        &create_pixmap_request(XByteOrder::LittleEndian, 24, font, window, 8, 8),
    )
    .unwrap();
    let collide_font = dispatch_x11_wire_request(
        dispatch_context(namespace, 8, XByteOrder::LittleEndian, 53),
        collide_font,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(matches!(
        collide_font.outputs.as_slice(),
        [XClientOutput::Error(XClientError {
            code: XErrorCode::BadIdChoice,
            resource_id,
            ..
        })] if *resource_id == font
    ));
    assert!(runtime.validate_font_access(namespace, XResourceId::new(font.into(), 1)).is_ok());
}

#[test]
fn x11_pixmap_and_graphics_context_requests_report_resource_specific_errors() {
    let namespace = NamespaceId::from_raw(46);
    let window = 0x22014d;
    let missing = 0x22014e;
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    let create_window = decode_x11_core_request(
        context(namespace, 656, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, window, 0, 0, 64, 48),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 1),
        create_window,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    for (sequence, opcode, request, expected_code) in [
        (
            2,
            53,
            create_pixmap_request(
                XByteOrder::LittleEndian,
                24,
                0x22014f,
                window,
                0,
                8,
            ),
            XErrorCode::BadValue,
        ),
        (
            3,
            54,
            free_pixmap_request(XByteOrder::LittleEndian, missing),
            XErrorCode::BadPixmap,
        ),
        (
            4,
            59,
            set_clip_rectangles_request(XByteOrder::LittleEndian, missing, &[]),
            XErrorCode::BadGraphicsContext,
        ),
        (
            5,
            60,
            free_graphics_context_request(XByteOrder::LittleEndian, missing),
            XErrorCode::BadGraphicsContext,
        ),
    ] {
        let request = decode_x11_core_request(
            context(namespace, 656 + u64::from(sequence), XByteOrder::LittleEndian),
            &request,
        )
        .unwrap();
        let result = dispatch_x11_wire_request(
            dispatch_context(namespace, sequence, XByteOrder::LittleEndian, opcode),
            request,
            &mut runtime,
            &mut atoms,
            &mut properties,
        );
        assert!(matches!(
            result.outputs.as_slice(),
            [XClientOutput::Error(XClientError { code, .. })] if *code == expected_code
        ));
    }

    assert_eq!(
        runtime
            .window_geometry(namespace, XResourceId::new(window.into(), 1))
            .unwrap(),
        Rect {
            x: 0,
            y: 0,
            width: 64,
            height: 48,
        }
    );
}

#[test]
fn x11_dispatch_text8_emits_exact_fixed_6x13_damage() {
    let namespace = NamespaceId::from_raw(46);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let window = 0x220151;

    let create = decode_x11_core_request(
        context(namespace, 646, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, window, 0, 0, 300, 200),
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
        context(namespace, 647, XByteOrder::LittleEndian),
        &create_gc_request(XByteOrder::LittleEndian, 0x220152, window),
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
        context(namespace, 648, XByteOrder::LittleEndian),
        &poly_text8_request(XByteOrder::LittleEndian, window, 0x220152, 5, 16, b"Hi"),
    )
    .unwrap();
    let text = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 74),
        text,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let response = text.response.unwrap();
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].damage,
        Region::single(Rect {
            x: 5,
            y: 5,
            width: 12,
            height: 13,
        })
    );

    let image_text = decode_x11_core_request(
        context(namespace, 649, XByteOrder::LittleEndian),
        &image_text8_request(XByteOrder::LittleEndian, window, 0x220152, 9, 20, b"OK"),
    )
    .unwrap();
    let image_text = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, 76),
        image_text,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let response = image_text.response.unwrap();
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].damage,
        Region::single(Rect {
            x: 9,
            y: 9,
            width: 12,
            height: 13,
        })
    );
}

#[test]
fn x11_dispatch_sophia_present_emits_xpixmap_surface_transaction() {
    let namespace = NamespaceId::from_raw(46);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let create = decode_x11_core_request(
        context(namespace, 621, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, 0x220121, 10, 20, 640, 480),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let present = decode_x11_core_request(
        context(namespace, 622, XByteOrder::LittleEndian),
        &sophia_present_pixmap_request(
            XByteOrder::LittleEndian,
            0x220121,
            0x990,
            (3, 5, 32, 24),
            1,
            250,
        ),
    )
    .unwrap();
    let present = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            2,
            XByteOrder::LittleEndian,
            X_SOPHIA_PRESENT_MAJOR_OPCODE,
        ),
        present,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(present.outputs.is_empty());
    let response = present.response.unwrap();
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].surface,
        SurfaceId::new(0x220121, 1)
    );
    assert_eq!(
        response.transactions[0].target_buffer,
        BufferSource::XPixmap { pixmap: 0x990 }
    );
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
fn x_authority_transaction_emitter_sends_bounded_batches() {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let result = present_dispatch_result(TransactionId::from_raw(610));

    let emitted = try_emit_x_authority_transactions(&sender, &result)
        .unwrap()
        .unwrap();
    let received = receiver.try_recv().unwrap();

    assert_eq!(emitted.transaction, TransactionId::from_raw(610));
    assert_eq!(emitted.transactions.len(), 1);
    assert_eq!(received, emitted);
}

#[test]
fn x_authority_transaction_emitter_reports_backpressure() {
    let (sender, _receiver) = std::sync::mpsc::sync_channel(0);
    let result = present_dispatch_result(TransactionId::from_raw(611));

    assert_eq!(
        try_emit_x_authority_transactions(&sender, &result),
        Err(XAuthorityTransportError::Backpressure {
            transaction: TransactionId::from_raw(611)
        })
    );
}
#[test]
fn protocol_router_remains_usable_after_route_broker_moves_or_drops() {
    use std::num::NonZeroUsize;

    let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(4).unwrap());
    let router = broker.protocol_router();
    let second = router.clone();
    drop(broker);

    assert_eq!(
        router.route_present_complete(
            TransactionId::from_raw(91),
            10,
            20,
            XPresentCompletionMode::Flip,
        ),
        Ok(false)
    );
    assert_eq!(
        second.route_present_idle(TransactionId::from_raw(91)),
        Ok(false)
    );
}

#[cfg(unix)]
#[test]
fn x_server_frontend_config_requires_a_socket_path_and_namespace() {
    assert!(XServerFrontendConfig::new("", NamespaceId::from_raw(1)).is_err());
    assert!(XServerFrontendConfig::new("/tmp/sophia-x11.sock", NamespaceId::INVALID).is_err());

    let config =
        XServerFrontendConfig::new("/tmp/sophia-x11.sock", NamespaceId::from_raw(812)).unwrap();
    assert_eq!(
        config.socket_path(),
        std::path::Path::new("/tmp/sophia-x11.sock")
    );
    assert_eq!(config.namespace(), NamespaceId::from_raw(812));
    assert_eq!(
        config.namespace_context().profile,
        NamespaceProfile::ClassicShared
    );
    assert_eq!(config.max_concurrent_clients().get(), 16);
}

#[cfg(unix)]
#[test]
fn x_server_frontend_config_accepts_a_session_namespace_context() {
    let namespace = NamespaceContext::new(
        NamespaceId::from_raw(821),
        NamespaceProfile::Confined,
        NamespaceCapabilities::NONE
            .with_request(NamespacePortalCapability::Clipboard)
            .with_publish(NamespacePortalCapability::Clipboard),
    )
    .unwrap();

    let config = XServerFrontendConfig::new_with_namespace_context(
        "/tmp/sophia-x11-confined.sock",
        namespace,
    )
    .unwrap();

    assert_eq!(config.namespace(), namespace.id);
    assert_eq!(config.namespace_context(), namespace);
}

#[cfg(unix)]
#[test]
fn x_server_frontend_dri3_open_sends_backend_owned_render_device_fd() {
    use std::fs::File;
    use std::io::{IoSliceMut, Write};
    use std::mem::MaybeUninit;
    use std::os::fd::OwnedFd;
    use std::sync::Arc;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestRenderDeviceProvider;

    impl XServerFrontendRenderDeviceProvider for TestRenderDeviceProvider {
        fn open_render_device_fd(&self) -> Result<OwnedFd, XServerFrontendRenderDeviceError> {
            File::open("/dev/null")
                .map(OwnedFd::from)
                .map_err(|_| XServerFrontendRenderDeviceError::Unavailable)
        }
    }

    let path = std::env::temp_dir().join(format!(
        "sophia-x-server-dri3-open-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = XServerFrontendConfig::new(&path, NamespaceId::from_raw(822))
        .unwrap()
        .with_render_device_provider(Arc::new(TestRenderDeviceProvider));
    let mut frontend = XServerFrontend::bind(config).unwrap();
    let server = thread::spawn(move || frontend.serve_next());

    wait_for_socket(&path);
    let mut stream = connect_x_socket(&path);
    stream
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut stream, XByteOrder::LittleEndian);
    stream
        .write_all(&dri3_open_request(
            XByteOrder::LittleEndian,
            X_SETUP_DEFAULT_ROOT,
            0,
        ))
        .unwrap();

    let mut reply = [0; X_CLIENT_OUTPUT_RECORD_LEN];
    let mut iov = [IoSliceMut::new(&mut reply)];
    let mut ancillary_space = [MaybeUninit::uninit();
        rustix::cmsg_space!(ScmRights(sophia_protocol::DMA_BUF_MAX_PLANES))];
    let mut ancillary = rustix::net::RecvAncillaryBuffer::new(&mut ancillary_space);
    let received = rustix::net::recvmsg(
        &stream,
        &mut iov,
        &mut ancillary,
        rustix::net::RecvFlags::CMSG_CLOEXEC,
    )
    .unwrap();
    assert_eq!(received.bytes, X_CLIENT_OUTPUT_RECORD_LEN);
    assert_eq!(reply[0], 1);
    assert_eq!(reply[1], 1);
    let received_fds = ancillary
        .drain()
        .flat_map(|message| match message {
            rustix::net::RecvAncillaryMessage::ScmRights(fds) => fds.collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    assert_eq!(received_fds.len(), 1);
    File::from(received_fds.into_iter().next().unwrap())
        .metadata()
        .unwrap();

    drop(stream);
    server.join().unwrap().unwrap();
    std::fs::remove_file(path).unwrap();
}

#[cfg(unix)]
#[test]
fn x_server_frontend_assigns_batched_scm_rights_to_fd_bearing_requests() {
    use std::fs::File;
    use std::io::{IoSlice, Write};
    use std::mem::MaybeUninit;
    use std::net::Shutdown;
    use std::os::fd::AsFd;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let path = std::env::temp_dir().join(format!(
        "sophia-x-server-batched-rights-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = XServerFrontendConfig::new(&path, NamespaceId::from_raw(823)).unwrap();
    let mut frontend = XServerFrontend::bind(config).unwrap();
    let server = thread::spawn(move || frontend.serve_next());

    wait_for_socket(&path);
    let mut stream = connect_x_socket(&path);
    stream
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut stream, XByteOrder::LittleEndian);

    let mut requests = xfixes_create_region_request(XByteOrder::LittleEndian, 0x220810, &[]);
    requests.extend_from_slice(&dri3_pixmap_from_buffer_request(
        XByteOrder::LittleEndian,
        0x220811,
        X_SETUP_DEFAULT_ROOT,
        64 * 48 * 4,
        64,
        48,
        256,
        24,
        32,
    ));
    requests.extend_from_slice(&dri3_fence_from_fd_request(
        XByteOrder::LittleEndian,
        X_SETUP_DEFAULT_ROOT,
        0x220812,
        false,
    ));
    let pixmap_fd = File::open("/dev/null").unwrap();
    let fence_fd = File::open("/dev/null").unwrap();
    let borrowed = [pixmap_fd.as_fd(), fence_fd.as_fd()];
    let mut space = [MaybeUninit::uninit();
        rustix::cmsg_space!(ScmRights(sophia_protocol::DMA_BUF_MAX_PLANES))];
    let mut ancillary = rustix::net::SendAncillaryBuffer::new(&mut space);
    assert!(ancillary.push(rustix::net::SendAncillaryMessage::ScmRights(&borrowed)));
    let sent = rustix::net::sendmsg(
        &stream,
        &[IoSlice::new(&requests)],
        &mut ancillary,
        rustix::net::SendFlags::empty(),
    )
    .unwrap();
    assert_eq!(sent, requests.len());
    stream.shutdown(Shutdown::Write).unwrap();

    server.join().unwrap().unwrap();
    std::fs::remove_file(path).unwrap();
}

#[cfg(unix)]
#[test]
fn x_server_frontend_binds_an_owner_only_socket_and_preserves_regular_files() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    let path = std::env::temp_dir().join(format!(
        "sophia-x-server-frontend-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = XServerFrontendConfig::new(&path, NamespaceId::from_raw(813)).unwrap();
    let frontend = XServerFrontend::bind(config).unwrap();
    assert_eq!(frontend.config().socket_path(), path.as_path());
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    drop(frontend);
    std::fs::remove_file(&path).unwrap();

    std::fs::write(&path, b"do not replace regular files").unwrap();
    let config = XServerFrontendConfig::new(&path, NamespaceId::from_raw(814)).unwrap();
    let error = match XServerFrontend::bind(config) {
        Ok(_) => panic!("frontend must not replace a regular file"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("refusing to replace non-socket"));
    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"do not replace regular files"
    );
    std::fs::remove_file(&path).unwrap();
}

#[cfg(unix)]
#[test]
fn x_server_frontend_rejects_bad_cookie_then_accepts_the_configured_cookie() {
    use std::io::Write;
    use std::sync::Arc;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x-server-frontend-cookie-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let cookie = [0x3c; 16];
    let namespace = NamespaceContext::new(
        NamespaceId::from_raw(815),
        NamespaceProfile::ClassicShared,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let policy = Arc::new(TestXAdmissionPolicy::new(namespace, false));
    let config = XServerFrontendConfig::new_with_namespace_context(&socket_path, namespace)
        .unwrap()
        .with_setup_authorization(XServerFrontendSetupAuthorization::MitMagicCookie(cookie))
        .with_admission_policy(policy.clone());
    assert_eq!(
        format!("{:?}", config.setup_authorization()),
        "MitMagicCookie([redacted])"
    );
    let server = thread::spawn(move || {
        let mut frontend = XServerFrontend::bind(config).unwrap();
        frontend.serve_next().unwrap();
        frontend.serve_next().unwrap();
    });

    wait_for_socket(&socket_path);
    let mut rejected = connect_x_socket(&socket_path);
    rejected
        .write_all(&setup_request(
            XByteOrder::LittleEndian,
            11,
            0,
            b"MIT-MAGIC-COOKIE-1",
            b"wrong-cookie-data",
        ))
        .unwrap();
    let mut rejected_prefix = [0; X_SETUP_REPLY_PREFIX_LEN];
    fill_from_socket(&mut rejected, &mut rejected_prefix);
    assert_eq!(rejected_prefix[0], 0);
    let rejected_body_len =
        usize::from(read_u16(XByteOrder::LittleEndian, &rejected_prefix[6..8])) * 4;
    let mut rejected_body = vec![0; rejected_body_len];
    fill_from_socket(&mut rejected, &mut rejected_body);
    assert!(String::from_utf8_lossy(&rejected_body).contains("authorization failed"));
    drop(rejected);

    let mut accepted = connect_x_socket(&socket_path);
    accepted
        .write_all(&setup_request(
            XByteOrder::LittleEndian,
            11,
            0,
            b"MIT-MAGIC-COOKIE-1",
            &cookie,
        ))
        .unwrap();
    read_setup_success(&mut accepted, XByteOrder::LittleEndian);
    drop(accepted);

    server.join().unwrap();
    let requests = policy.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].setup_authentication,
        ClientAuthenticationMethod::MitMagicCookie1
    );
    assert_eq!(policy.revoked.lock().unwrap().len(), 1);
    std::fs::remove_file(&socket_path).unwrap();
}
