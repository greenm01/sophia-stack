#[cfg(unix)]
#[test]
fn deferred_map_subwindows_maps_children_without_bypassing_toplevel_admission() {
    use std::collections::BTreeMap;
    use std::io::Write;
    use std::num::NonZeroUsize;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-deferred-map-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let server_path = socket_path.clone();
    let (transaction_sender, transaction_receiver) =
        std::sync::mpsc::sync_channel(X_AUTHORITY_OBSERVED_TRANSACTION_CHANNEL_CAPACITY);
    let (acknowledgement_sender, acknowledgement_receiver) = std::sync::mpsc::sync_channel(2);
    let broker = XServerFrontendRouteBroker::with_control_ack_sender(
        NonZeroUsize::new(4).unwrap(),
        acknowledgement_sender,
    );
    let control_sender = broker.control_sender();
    let (service_sender, service_receiver) = std::sync::mpsc::sync_channel(1);
    let config = XServerFrontendConfig::new(&server_path, NamespaceId::from_raw(855))
        .unwrap()
        .with_policy_map_deferred(true);
    let server = thread::spawn(move || {
        run_x_server_frontend_routed_until_stopped(
            config,
            transaction_sender,
            broker,
            service_receiver,
        )
        .unwrap();
    });

    wait_for_socket(&socket_path);
    let mut stream = UnixStream::connect(&socket_path).unwrap();
    stream
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut stream, XByteOrder::LittleEndian);
    stream
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x0020_0801,
            10,
            20,
            500,
            500,
        ))
        .unwrap();
    stream
        .write_all(&create_window_request_with_parent(
            XByteOrder::LittleEndian,
            0x0020_0802,
            0x0020_0801,
            1,
            2,
            100,
            80,
        ))
        .unwrap();
    let lifecycle_mask = (1_u32 << 15) | (1_u32 << 16) | (1_u32 << 17) | (1_u32 << 22);
    for window in [0x0020_0801, 0x0020_0802] {
        stream
            .write_all(&change_window_event_mask_request(
                XByteOrder::LittleEndian,
                window,
                lifecycle_mask,
            ))
            .unwrap();
    }
    stream
        .write_all(&resource_request(
            XByteOrder::LittleEndian,
            9,
            0x0020_0801,
        ))
        .unwrap();

    let child_batch = loop {
        let batch = transaction_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        if batch
            .surface_presentations
            .iter()
            .any(|presentation| presentation.surface == SurfaceId::new(0x0020_0802, 1))
        {
            break batch;
        }
    };
    assert!(child_batch.presentation_intents.is_empty());
    assert_eq!(child_batch.surface_presentations.len(), 1);
    assert_eq!(
        child_batch.surface_presentations[0].surface,
        SurfaceId::new(0x0020_0802, 1)
    );
    assert_eq!(
        child_batch.surface_presentations[0].role,
        sophia_protocol::SurfacePresentationRole::ClientPositioned
    );
    assert!(!child_batch.surface_presentations[0].mapped);
    assert_eq!(read_x_record(&mut stream)[0], 19);
    stream
        .write_all(&create_gc_request(
            XByteOrder::LittleEndian,
            0x0020_0803,
            0x0020_0802,
        ))
        .unwrap();
    stream
        .write_all(&poly_fill_rectangle_request(
            XByteOrder::LittleEndian,
            0x0020_0802,
            0x0020_0803,
            &[(1, 2, 3, 4)],
        ))
        .unwrap();
    let drawing_batch = loop {
        let batch = transaction_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        if !batch.transactions.is_empty() {
            break batch;
        }
    };
    assert_eq!(drawing_batch.transactions.len(), 1);
    assert_eq!(
        drawing_batch.transactions[0].surface,
        SurfaceId::new(0x0020_0801, 1)
    );
    assert_eq!(
        drawing_batch.transactions[0].damage,
        Region::single(Rect {
            x: 2,
            y: 4,
            width: 3,
            height: 4,
        })
    );
    assert_eq!(drawing_batch.cpu_buffer_updates.len(), 1);
    assert!(drawing_batch.presentation_intents.is_empty());

    stream
        .write_all(&resource_request(
            XByteOrder::LittleEndian,
            8,
            0x0020_0801,
        ))
        .unwrap();

    let batch = loop {
        let batch = transaction_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        if !batch.presentation_intents.is_empty() {
            break batch;
        }
    };
    let intent = batch.presentation_intents[0];
    assert_eq!(intent.surface, SurfaceId::new(0x0020_0801, 1));
    assert_eq!(
        intent.kind,
        sophia_protocol::SurfacePresentationIntentKind::Request
    );
    let client = batch.client.unwrap();
    let transaction = TransactionId::from_raw(90);
    let geometry = Rect {
        x: 40,
        y: 50,
        width: 640,
        height: 480,
    };
    control_sender
        .send(XAuthorityClientControlCommand {
            client,
            command: XAuthorityControlCommand::AdmitSurface {
                transaction,
                surface: intent.surface,
                geometry,
            },
        })
        .unwrap();

    assert_eq!(read_x_record(&mut stream)[0], 22);
    assert_eq!(read_x_record(&mut stream)[0], 19);
    assert_eq!(read_x_record(&mut stream)[0], 15);
    let child_visibility = read_x_record(&mut stream);
    assert_eq!(child_visibility[0], 15);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &child_visibility[4..8]),
        0x0020_0802
    );
    assert_eq!(read_x_record(&mut stream)[0], 12);
    let child_expose = read_x_record(&mut stream);
    assert_eq!(child_expose[0], 12);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &child_expose[4..8]),
        0x0020_0802
    );
    assert_eq!(
        acknowledgement_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap(),
        XAuthorityClientControlAck {
            client,
            acknowledgement: XAuthorityControlAck {
                kind: XAuthorityControlKind::AdmitSurface,
                transaction,
                surface: intent.surface,
                outcome: XAuthorityControlOutcome::Delivered,
            },
        }
    );

    let state = PolicyPresentationState {
        fullscreen: true,
        ..PolicyPresentationState::default()
    };
    control_sender
        .send(XAuthorityClientControlCommand {
            client,
            command: XAuthorityControlCommand::SetPresentationState {
                transaction,
                surface: intent.surface,
                state,
            },
        })
        .unwrap();
    let net_notify = read_x_record(&mut stream);
    let wm_notify = read_x_record(&mut stream);
    assert_eq!(net_notify[0], 28);
    assert_eq!(wm_notify[0], 28);
    assert_eq!(
        acknowledgement_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap(),
        XAuthorityClientControlAck {
            client,
            acknowledgement: XAuthorityControlAck {
                kind: XAuthorityControlKind::SetPresentationState,
                transaction,
                surface: intent.surface,
                outcome: XAuthorityControlOutcome::Delivered,
            },
        }
    );

    let mut interned = BTreeMap::new();
    for name in [
        X_ATOM_NAME_NET_WM_STATE,
        X_ATOM_NAME_NET_WM_STATE_FULLSCREEN,
        X_ATOM_NAME_WM_STATE,
    ] {
        stream
            .write_all(&intern_atom_request(XByteOrder::LittleEndian, true, name))
            .unwrap();
        let reply = read_x_reply(&mut stream, XByteOrder::LittleEndian);
        let atom = read_u32(XByteOrder::LittleEndian, &reply[8..12]);
        assert_ne!(atom, 0);
        interned.insert(name, atom);
    }
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &net_notify[8..12]),
        interned[X_ATOM_NAME_NET_WM_STATE]
    );
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &wm_notify[8..12]),
        interned[X_ATOM_NAME_WM_STATE]
    );
    for (property, expected) in [
        (
            interned[X_ATOM_NAME_NET_WM_STATE],
            vec![interned[X_ATOM_NAME_NET_WM_STATE_FULLSCREEN]],
        ),
        (interned[X_ATOM_NAME_WM_STATE], vec![1, 0]),
    ] {
        stream
            .write_all(&get_property_request(
                XByteOrder::LittleEndian,
                false,
                0x0020_0801,
                property,
                X_PROPERTY_ANY_TYPE,
                0,
                8,
            ))
            .unwrap();
        let reply = read_x_reply(&mut stream, XByteOrder::LittleEndian);
        assert_eq!(read_u32(XByteOrder::LittleEndian, &reply[16..20]), expected.len() as u32);
        let values = reply[32..32 + expected.len() * 4]
            .chunks_exact(4)
            .map(|bytes| read_u32(XByteOrder::LittleEndian, bytes))
            .collect::<Vec<_>>();
        assert_eq!(values, expected);
    }
    stream
        .write_all(&change_property_request(
            XByteOrder::LittleEndian,
            XPropertyMode::Replace,
            0x0020_0801,
            interned[X_ATOM_NAME_NET_WM_STATE],
            X_ATOM_ATOM,
            32,
            &[],
        ))
        .unwrap();
    let denied_change = read_x_record(&mut stream);
    assert_eq!(denied_change[0], 0);
    assert_eq!(denied_change[1], 10, "Engine-owned state rejects ChangeProperty");
    stream
        .write_all(&delete_property_request(
            XByteOrder::LittleEndian,
            0x0020_0801,
            interned[X_ATOM_NAME_WM_STATE],
        ))
        .unwrap();
    let denied_delete = read_x_record(&mut stream);
    assert_eq!(denied_delete[0], 0);
    assert_eq!(denied_delete[1], 10, "Engine-owned state rejects DeleteProperty");

    drop(stream);
    service_sender
        .send(XServerFrontendServiceCommand::StopAccepting)
        .unwrap();
    drop(service_sender);
    drop(control_sender);
    server.join().unwrap();
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn routed_lifecycle_events_follow_structure_and_substructure_masks() {
    use std::io::Write;
    use std::num::NonZeroUsize;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-lifecycle-routing-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = XServerFrontendConfig::new(&socket_path, NamespaceId::from_raw(856))
        .unwrap()
        .with_max_concurrent_clients(NonZeroUsize::new(2).unwrap());
    let (transaction_sender, _transaction_receiver) =
        std::sync::mpsc::sync_channel(X_AUTHORITY_OBSERVED_TRANSACTION_CHANNEL_CAPACITY);
    let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(4).unwrap());
    let (service_sender, service_receiver) = std::sync::mpsc::sync_channel(1);
    let server = thread::spawn(move || {
        run_x_server_frontend_routed_until_stopped(
            config,
            transaction_sender,
            broker,
            service_receiver,
        )
        .unwrap();
    });

    wait_for_socket(&socket_path);
    let mut observer = UnixStream::connect(&socket_path).unwrap();
    observer
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut observer, XByteOrder::LittleEndian);
    observer
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    observer
        .write_all(&change_window_event_mask_request(
            XByteOrder::LittleEndian,
            X_SETUP_DEFAULT_ROOT,
            1 << 19,
        ))
        .unwrap();

    let mut owner = UnixStream::connect(&socket_path).unwrap();
    owner
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut owner, XByteOrder::LittleEndian);
    owner
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let window = 0x0040_0801;
    owner
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            window,
            10,
            20,
            320,
            240,
        ))
        .unwrap();
    let created = read_x_record(&mut observer);
    assert_eq!(created[0], 16);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &created[4..8]),
        X_SETUP_DEFAULT_ROOT
    );
    assert_eq!(read_u32(XByteOrder::LittleEndian, &created[8..12]), window);

    owner
        .write_all(&change_window_event_mask_request(
            XByteOrder::LittleEndian,
            window,
            (1 << 15) | (1 << 16) | (1 << 17),
        ))
        .unwrap();
    owner
        .write_all(&configure_window_request(
            XByteOrder::LittleEndian,
            window,
            0x000c,
            &[400, 300],
        ))
        .unwrap();
    let configured = read_x_record(&mut owner);
    assert_eq!(configured[0], 22);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &configured[20..22]), 400);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &configured[22..24]), 300);
    let parent_configured = read_x_record(&mut observer);
    assert_eq!(parent_configured[0], 22);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &parent_configured[4..8]),
        X_SETUP_DEFAULT_ROOT
    );

    owner
        .write_all(&resource_request(XByteOrder::LittleEndian, 8, window))
        .unwrap();
    for expected in [19, 15, 12] {
        assert_eq!(read_x_record(&mut owner)[0], expected);
    }
    let parent_mapped = read_x_record(&mut observer);
    assert_eq!(parent_mapped[0], 19);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &parent_mapped[4..8]),
        X_SETUP_DEFAULT_ROOT
    );
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &parent_mapped[8..12]),
        window
    );

    drop(owner);
    drop(observer);
    service_sender
        .send(XServerFrontendServiceCommand::StopAccepting)
        .unwrap();
    drop(service_sender);
    server.join().unwrap();
    std::fs::remove_file(&socket_path).unwrap();
}
