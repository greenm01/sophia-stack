#[cfg(unix)]
#[test]
fn routed_surface_density_requirement_publishes_exact_derived_text_variant() {
    use std::io::Write;
    use std::num::NonZeroUsize;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-raster-route-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let namespace = NamespaceId::from_raw(951);
    let config = XServerFrontendConfig::new(&socket_path, namespace)
        .unwrap()
        .with_max_concurrent_clients(NonZeroUsize::new(1).unwrap());
    let (transaction_sender, transaction_receiver) = std::sync::mpsc::sync_channel(8);
    let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(4).unwrap());
    let raster = broker.raster_router();
    let (service_sender, service_receiver) = std::sync::mpsc::sync_channel(2);
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
    let mut client = connect_x_socket(&socket_path);
    client
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut client, XByteOrder::LittleEndian);
    let window = 0x0020_0d01;
    let gc = 0x0020_0d02;
    client
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            window,
            0,
            0,
            80,
            40,
        ))
        .unwrap();
    client
        .write_all(&create_gc_values_request(
            XByteOrder::LittleEndian,
            gc,
            window,
            3,
            u32::MAX,
            0x00ff_ffff,
            0,
            0,
            0,
        ))
        .unwrap();
    client
        .write_all(&image_text8_request(
            XByteOrder::LittleEndian,
            window,
            gc,
            4,
            16,
            b"AaZz",
        ))
        .unwrap();

    let drawn = loop {
        let batch = transaction_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        if batch.cpu_buffer_updates.len() == 1 && batch.transactions.len() == 1 {
            break batch;
        }
    };
    let surface = drawn.transactions[0].surface;
    raster
        .try_route(SurfaceRasterRequirements {
            surface,
            committed_content_generation: 2,
            requirement_generation: 1,
            logical_extent: Size {
                width: 80,
                height: 40,
            },
            classes: vec![
                SurfaceRasterClass {
                    density_millis: 750,
                    transform: SurfaceRasterTransform::Normal,
                },
                SurfaceRasterClass {
                    density_millis: 1_000,
                    transform: SurfaceRasterTransform::Normal,
                },
            ],
        })
        .unwrap();
    let response = transaction_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    assert_eq!(response.raster_responses.len(), 1);
    assert_eq!(response.raster_responses[0].surface, surface);
    assert_eq!(response.transactions[0].content.variants().len(), 2);
    assert!(response.transactions[0].content.variants().iter().any(|variant| {
        variant.density_millis == 750
            && variant.pixel_size == Size { width: 60, height: 30 }
            && variant.fidelity == sophia_protocol::SurfaceContentFidelity::AuthorityRaster
    }));
    assert_eq!(response.cpu_buffer_updates.len(), 1);

    drop(client);
    service_sender
        .send(XServerFrontendServiceCommand::StopAccepting)
        .unwrap();
    drop(service_sender);
    server.join().unwrap();
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn classic_peer_mutation_preserves_creator_route_and_foreign_destroy_retires_it() {
    use std::io::Write;
    use std::num::NonZeroUsize;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-classic-owner-route-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let namespace = NamespaceContext::new(
        NamespaceId::from_raw(952),
        NamespaceProfile::ClassicShared,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let policy = Arc::new(TestXAdmissionPolicy::new(namespace, false));
    let config = XServerFrontendConfig::new_with_namespace_context(&socket_path, namespace)
        .unwrap()
        .with_admission_policy(policy)
        .with_max_concurrent_clients(NonZeroUsize::new(2).unwrap());
    let (transaction_sender, transaction_receiver) = std::sync::mpsc::sync_channel(16);
    let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(4).unwrap());
    let (service_sender, service_receiver) = std::sync::mpsc::sync_channel(2);
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
    let mut creator = connect_x_socket(&socket_path);
    creator
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut creator, XByteOrder::LittleEndian);
    let window = 0x0020_0e01;
    creator
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            window,
            0,
            0,
            80,
            40,
        ))
        .unwrap();
    let created = loop {
        let batch = transaction_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        if batch
            .surface_presentations
            .iter()
            .any(|presentation| presentation.surface.index() == window)
        {
            break batch;
        }
    };
    let surface = created.surface_presentations[0].surface;
    let creator_admission = created.admission.expect("creator admission");
    assert_eq!(created.client.map(XServerFrontendClientId::raw), Some(1));
    assert!(created.surface_routes.is_empty());

    let mut peer = connect_x_socket(&socket_path);
    peer.write_all(&setup_request(
        XByteOrder::LittleEndian,
        11,
        0,
        b"",
        b"",
    ))
    .unwrap();
    read_setup_success(&mut peer, XByteOrder::LittleEndian);
    peer.write_all(&resource_request(XByteOrder::LittleEndian, 8, window))
        .unwrap();
    let mapped = loop {
        let batch = transaction_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        if batch.client.map(XServerFrontendClientId::raw) == Some(2)
            && batch
                .surface_presentations
                .iter()
                .any(|presentation| presentation.surface == surface)
        {
            break batch;
        }
    };
    assert_ne!(mapped.admission, Some(creator_admission));
    assert!(mapped.surface_routes.is_empty());

    let peer_gc = 0x0040_0e02;
    peer.write_all(&create_gc_values_request(
        XByteOrder::LittleEndian,
        peer_gc,
        window,
        3,
        u32::MAX,
        0x00ff_ffff,
        0,
        0,
        0,
    ))
    .unwrap();
    peer.write_all(&image_text8_request(
        XByteOrder::LittleEndian,
        window,
        peer_gc,
        4,
        16,
        b"peer",
    ))
    .unwrap();
    let drawn = loop {
        let batch = transaction_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        if batch.client.map(XServerFrontendClientId::raw) == Some(2)
            && batch
                .transactions
                .iter()
                .any(|transaction| transaction.surface == surface)
        {
            break batch;
        }
    };
    assert_eq!(
        drawn.surface_routes,
        [XAuthoritySurfaceRouteObservation {
            surface,
            client: XServerFrontendClientId::from_raw(1),
            admission: Some(creator_admission),
        }]
    );
    let mut routes = XAuthorityClientSurfaceRoutes::default();
    routes.observe(&drawn).unwrap();
    assert_eq!(
        routes
            .client_for_surface(surface)
            .map(XServerFrontendClientId::raw),
        Some(1)
    );
    let mut conflicting = drawn.clone();
    conflicting.surface_routes[0].client = XServerFrontendClientId::from_raw(2);
    assert!(matches!(
        routes.observe(&conflicting),
        Err(XAuthorityClientSurfaceRouteError::ConflictingObservation { surface: rejected })
            if rejected == surface
    ));
    assert_eq!(
        routes
            .client_for_surface(surface)
            .map(XServerFrontendClientId::raw),
        Some(1)
    );

    peer.write_all(&resource_request(XByteOrder::LittleEndian, 4, window))
        .unwrap();
    let removed = loop {
        let batch = transaction_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        if batch.removed_surfaces.contains(&surface) {
            break batch;
        }
    };
    assert_eq!(removed.client.map(XServerFrontendClientId::raw), Some(2));
    assert!(removed.surface_routes.is_empty());
    routes.observe(&removed).unwrap();
    assert!(routes.is_empty());
    routes.observe(&drawn).unwrap();
    assert!(routes.is_empty());

    drop(peer);
    drop(creator);
    service_sender
        .send(XServerFrontendServiceCommand::StopAccepting)
        .unwrap();
    drop(service_sender);
    server.join().unwrap();
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn routed_service_revokes_one_live_admission_without_disrupting_its_classic_peer() {
    use std::io::{Read, Write};
    use std::num::NonZeroUsize;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-supervisor-revocation-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let namespace = NamespaceContext::new(
        NamespaceId::from_raw(854),
        NamespaceProfile::ClassicShared,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let policy = Arc::new(TestXAdmissionPolicy::new(namespace, false));
    let config = XServerFrontendConfig::new_with_namespace_context(&socket_path, namespace)
        .unwrap()
        .with_admission_policy(policy.clone())
        .with_max_concurrent_clients(NonZeroUsize::new(2).unwrap());
    let (transaction_sender, transaction_receiver) =
        std::sync::mpsc::sync_channel(X_AUTHORITY_OBSERVED_TRANSACTION_CHANNEL_CAPACITY);
    let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(4).unwrap());
    let (service_command_sender, service_command_receiver) = std::sync::mpsc::sync_channel(2);
    let server = thread::spawn(move || {
        run_x_server_frontend_routed_until_stopped(
            config,
            transaction_sender,
            broker,
            service_command_receiver,
        )
        .unwrap();
    });

    wait_for_socket(&socket_path);
    let mut first = connect_x_socket(&socket_path);
    first
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut first, XByteOrder::LittleEndian);
    first
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x0020_0801,
            1,
            2,
            160,
            90,
        ))
        .unwrap();
    first
        .write_all(&sophia_present_pixmap_request(
            XByteOrder::LittleEndian,
            0x0020_0801,
            0x994,
            (0, 0, 16, 16),
            1,
            1,
        ))
        .unwrap();

    let mut second = connect_x_socket(&socket_path);
    second
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut second, XByteOrder::LittleEndian);
    second
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x0040_0802,
            3,
            4,
            160,
            90,
        ))
        .unwrap();
    second
        .write_all(&sophia_present_pixmap_request(
            XByteOrder::LittleEndian,
            0x0040_0802,
            0x995,
            (0, 0, 16, 16),
            1,
            1,
        ))
        .unwrap();

    let mut initial_batches = Vec::new();
    while initial_batches.len() < 2 {
        let batch = transaction_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        if !batch.transactions.is_empty() {
            initial_batches.push(batch);
        }
    }
    let first_client = initial_batches
        .iter()
        .find_map(|batch| {
            (batch.client.map(XServerFrontendClientId::raw) == Some(1))
                .then(|| batch.client.unwrap())
        })
        .unwrap();

    service_command_sender
        .send(XServerFrontendServiceCommand::RevokeAdmission {
            admission: ClientAdmissionId::from_raw(1),
        })
        .unwrap();
    first
        .set_read_timeout(Some(X_RECORD_READ_TIMEOUT))
        .unwrap();
    let mut disconnected = [0u8; 1];
    match first.read(&mut disconnected) {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::UnexpectedEof
            ) => {}
        outcome => panic!("revoked X11 client remained connected: {outcome:?}"),
    }

    let cleanup = transaction_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    assert_eq!(cleanup.client, Some(first_client));
    assert_eq!(cleanup.removed_surfaces.len(), 1);

    second
        .write_all(&resource_request(XByteOrder::LittleEndian, 8, 0x0020_0801))
        .unwrap();
    let mut error = [0; 32];
    fill_from_socket(&mut second, &mut error);
    assert_eq!(error[0], 0);
    assert_eq!(error[1], XErrorCode::BadWindow.wire_code());
    second
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x0040_0803,
            5,
            6,
            80,
            45,
        ))
        .unwrap();

    drop(first);
    drop(second);
    service_command_sender
        .send(XServerFrontendServiceCommand::StopAccepting)
        .unwrap();
    drop(service_command_sender);
    server.join().unwrap();
    let revoked = policy.revoked.lock().unwrap();
    assert_eq!(revoked.len(), 2);
    assert!(
        revoked
            .iter()
            .any(|context| context.client_id == ClientAdmissionId::from_raw(1))
    );
    assert!(
        revoked
            .iter()
            .any(|context| context.client_id == ClientAdmissionId::from_raw(2))
    );
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn routed_service_retains_revocation_requested_before_admission_attaches() {
    use std::io::{Read, Write};
    use std::num::NonZeroUsize;
    use std::sync::Arc;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-early-revocation-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let namespace = NamespaceContext::new(
        NamespaceId::from_raw(855),
        NamespaceProfile::ClassicShared,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let policy = Arc::new(TestXAdmissionPolicy::new(namespace, false));
    let config = XServerFrontendConfig::new_with_namespace_context(&socket_path, namespace)
        .unwrap()
        .with_admission_policy(policy.clone());
    let (transaction_sender, _transaction_receiver) =
        std::sync::mpsc::sync_channel(X_AUTHORITY_OBSERVED_TRANSACTION_CHANNEL_CAPACITY);
    let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(2).unwrap());
    let (service_command_sender, service_command_receiver) = std::sync::mpsc::sync_channel(2);
    let server = thread::spawn(move || {
        run_x_server_frontend_routed_until_stopped(
            config,
            transaction_sender,
            broker,
            service_command_receiver,
        )
        .unwrap();
    });

    wait_for_socket(&socket_path);
    service_command_sender
        .send(XServerFrontendServiceCommand::RevokeAdmission {
            admission: ClientAdmissionId::from_raw(1),
        })
        .unwrap();
    let mut client = connect_x_socket(&socket_path);
    client
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut client, XByteOrder::LittleEndian);
    client
        .set_read_timeout(Some(X_RECORD_READ_TIMEOUT))
        .unwrap();
    let mut disconnected = [0u8; 1];
    match client.read(&mut disconnected) {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::UnexpectedEof
            ) => {}
        outcome => panic!("early-revoked X11 client remained connected: {outcome:?}"),
    }

    drop(client);
    service_command_sender
        .send(XServerFrontendServiceCommand::StopAccepting)
        .unwrap();
    drop(service_command_sender);
    server.join().unwrap();
    let revoked = policy.revoked.lock().unwrap();
    assert_eq!(revoked.len(), 1);
    assert_eq!(revoked[0].client_id, ClientAdmissionId::from_raw(1));
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn routed_service_backpressure_blocks_without_disconnect_and_drains_in_order() {
    use std::io::Write;
    use std::num::NonZeroUsize;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-bp-order-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let namespace = NamespaceId::from_raw(856);
    let config = XServerFrontendConfig::new(&socket_path, namespace)
        .unwrap()
        .with_max_concurrent_clients(NonZeroUsize::new(1).unwrap());
    let (transaction_sender, transaction_receiver) = std::sync::mpsc::sync_channel(1);
    let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(2).unwrap());
    let (service_sender, service_receiver) = std::sync::mpsc::sync_channel(2);
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
    let mut client = connect_x_socket(&socket_path);
    client
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    client
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut client, XByteOrder::LittleEndian);
    let windows = [0x0020_0a01, 0x0020_0a02, 0x0020_0a03];
    for (index, window) in windows.into_iter().enumerate() {
        client
            .write_all(&create_window_request(
                XByteOrder::LittleEndian,
                window,
                i16::try_from(index + 1).unwrap(),
                i16::try_from(index + 2).unwrap(),
                u16::try_from(300 + index).unwrap(),
                u16::try_from(200 + index).unwrap(),
            ))
            .unwrap();
    }
    client
        .write_all(&resource_request(
            XByteOrder::LittleEndian,
            14,
            windows[2],
        ))
        .unwrap();

    // The blocked worker must not retain the authority runtime lock. A topology
    // update crosses that lock in the service thread and must still settle.
    let snapshot = OutputTopologySnapshot {
        generation: 2,
        primary: OutputId::from_raw(9),
        outputs: vec![OutputTopologyEntry {
            output: OutputId::from_raw(9),
            logical: Rect {
                x: 0,
                y: 0,
                width: 1600,
                height: 900,
            },
            pixel_size: Size {
                width: 1600,
                height: 900,
            },
            scale: 1,
            refresh_millihz: 60_000,
        }],
    };
    let (ack_sender, ack_receiver) = std::sync::mpsc::sync_channel(1);
    service_sender
        .send(XServerFrontendServiceCommand::UpdateOutputTopology {
            snapshot,
            acknowledgement: ack_sender,
        })
        .unwrap();
    assert_eq!(
        ack_receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
        XAuthorityOutputUpdateOutcome::Applied {
            generation: 2,
            notifications: 0,
        }
    );

    let first = transaction_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    let second = transaction_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    let third = transaction_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    assert_eq!(
        first.surface_presentations[0].surface,
        SurfaceId::new(windows[0], 1)
    );
    assert_eq!(
        second.surface_presentations[0].surface,
        SurfaceId::new(windows[1], 1)
    );
    assert_eq!(
        third.surface_presentations[0].surface,
        SurfaceId::new(windows[2], 1)
    );
    assert_eq!(first.surface_presentations[0].geometry.x, 1);
    assert_eq!(first.surface_presentations[0].geometry.y, 2);
    assert_eq!(second.surface_presentations[0].geometry.x, 2);
    assert_eq!(second.surface_presentations[0].geometry.y, 3);
    assert_eq!(third.surface_presentations[0].geometry.width, 302);
    assert_eq!(third.surface_presentations[0].geometry.height, 202);
    assert!(first.transaction.raw() < second.transaction.raw());
    assert!(second.transaction.raw() < third.transaction.raw());

    expect_x_reply(
        &read_x_reply(&mut client, XByteOrder::LittleEndian),
        XByteOrder::LittleEndian,
    );
    drop(client);
    service_sender
        .send(XServerFrontendServiceCommand::StopAccepting)
        .unwrap();
    drop(service_sender);
    server.join().unwrap();
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn routed_service_cancellation_releases_authority_backpressured_worker() {
    use std::io::Write;
    use std::num::NonZeroUsize;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-bp-cancel-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let namespace = NamespaceContext::new(
        NamespaceId::from_raw(857),
        NamespaceProfile::ClassicShared,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let policy = Arc::new(TestXAdmissionPolicy::new(namespace, false));
    let config = XServerFrontendConfig::new_with_namespace_context(&socket_path, namespace)
        .unwrap()
        .with_admission_policy(policy.clone())
        .with_max_concurrent_clients(NonZeroUsize::new(1).unwrap());
    let (transaction_sender, transaction_receiver) = std::sync::mpsc::sync_channel(0);
    let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(2).unwrap());
    let (service_sender, service_receiver) = std::sync::mpsc::sync_channel(1);
    let (done_sender, done_receiver) = std::sync::mpsc::sync_channel(1);
    let (telemetry_sender, telemetry_receiver) = std::sync::mpsc::channel();
    let server = thread::spawn(move || {
        let result = run_x_server_frontend_routed_until_stopped_with_backpressure_observer(
            config,
            transaction_sender,
            broker,
            service_receiver,
            Arc::new(move |event| telemetry_sender.send(event).unwrap()),
        );
        done_sender.send(result).unwrap();
    });

    wait_for_socket(&socket_path);
    let mut client = connect_x_socket(&socket_path);
    client
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut client, XByteOrder::LittleEndian);
    let window = 0x0020_0a11;
    client
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            window,
            1,
            2,
            300,
            200,
        ))
        .unwrap();
    let waiting = loop {
        let event = telemetry_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("client worker never reported authority backpressure");
        if event.kind == XAuthorityBackpressureTelemetryKind::Wait {
            break event;
        }
    };
    assert_eq!(waiting.client, Some(XServerFrontendClientId::from_raw(1)));
    service_sender
        .send(XServerFrontendServiceCommand::StopAndDisconnect)
        .unwrap();
    done_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("service cancellation left the authority observer blocked")
        .unwrap();
    server.join().unwrap();
    let terminal = telemetry_receiver.try_iter().collect::<Vec<_>>();
    assert!(terminal.iter().any(|event| {
        event.client == waiting.client
            && event.transaction == waiting.transaction
            && event.kind == XAuthorityBackpressureTelemetryKind::Shutdown
            && event.failure == Some(XAuthorityBackpressureFailure::Cancelled)
    }));
    assert!(!terminal.iter().any(|event| {
        event.client == waiting.client
            && event.transaction == waiting.transaction
            && event.kind == XAuthorityBackpressureTelemetryKind::Resume
    }));
    assert_eq!(policy.revoked.lock().unwrap().len(), 1);
    // Keep the owner receiver alive until after the worker exits: this proves
    // service cancellation, rather than channel disconnection, released it.
    drop(transaction_receiver);
    drop(client);
    drop(service_sender);
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn routed_service_disconnect_cleanup_follows_backpressured_requests() {
    use std::io::Write;
    use std::num::NonZeroUsize;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-bp-disconnect-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let namespace = NamespaceContext::new(
        NamespaceId::from_raw(858),
        NamespaceProfile::ClassicShared,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let policy = Arc::new(TestXAdmissionPolicy::new(namespace, false));
    let config = XServerFrontendConfig::new_with_namespace_context(&socket_path, namespace)
        .unwrap()
        .with_admission_policy(policy.clone())
        .with_max_concurrent_clients(NonZeroUsize::new(1).unwrap());
    let (transaction_sender, transaction_receiver) = std::sync::mpsc::sync_channel(1);
    let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(2).unwrap());
    let (service_sender, service_receiver) = std::sync::mpsc::sync_channel(1);
    let server = thread::spawn(move || {
        run_x_server_frontend_routed_until_stopped(
            config,
            transaction_sender,
            broker,
            service_receiver,
        )
    });

    wait_for_socket(&socket_path);
    let mut client = connect_x_socket(&socket_path);
    client
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    client
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut client, XByteOrder::LittleEndian);
    let windows = [0x0020_0a21, 0x0020_0a22];
    for (index, window) in windows.into_iter().enumerate() {
        client
            .write_all(&create_window_request(
                XByteOrder::LittleEndian,
                window,
                i16::try_from(index + 1).unwrap(),
                i16::try_from(index + 2).unwrap(),
                300,
                200,
            ))
            .unwrap();
    }
    client
        .write_all(&resource_request(
            XByteOrder::LittleEndian,
            14,
            windows[1],
        ))
        .unwrap();
    drop(client);

    let created = transaction_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    let configured = transaction_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    let cleanup = transaction_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    assert_eq!(
        created.surface_presentations[0].surface,
        SurfaceId::new(windows[0], 1)
    );
    assert_eq!(
        configured.surface_presentations[0].surface,
        SurfaceId::new(windows[1], 1)
    );
    assert!(created.removed_surfaces.is_empty());
    assert!(configured.removed_surfaces.is_empty());
    assert_eq!(
        cleanup.removed_surfaces,
        vec![SurfaceId::new(windows[0], 1), SurfaceId::new(windows[1], 1)]
    );
    assert!(created.transaction.raw() < configured.transaction.raw());
    assert!(configured.transaction.raw() < cleanup.transaction.raw());

    service_sender
        .send(XServerFrontendServiceCommand::StopAccepting)
        .unwrap();
    drop(service_sender);
    server.join().unwrap().unwrap();
    assert_eq!(policy.revoked.lock().unwrap().len(), 1);
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn routed_service_authority_disconnect_releases_backpressured_worker() {
    use std::collections::BTreeSet;
    use std::io::{Read, Write};
    use std::num::NonZeroUsize;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-bp-owner-gone-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let namespace = NamespaceContext::new(
        NamespaceId::from_raw(859),
        NamespaceProfile::ClassicShared,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let policy = Arc::new(TestXAdmissionPolicy::new(namespace, false));
    let config = XServerFrontendConfig::new_with_namespace_context(&socket_path, namespace)
        .unwrap()
        .with_admission_policy(policy.clone())
        .with_max_concurrent_clients(NonZeroUsize::new(2).unwrap());
    let (transaction_sender, transaction_receiver) = std::sync::mpsc::sync_channel(0);
    let (ack_sender, ack_receiver) = std::sync::mpsc::sync_channel(4);
    let broker = XServerFrontendRouteBroker::with_control_ack_sender(
        NonZeroUsize::new(2).unwrap(),
        ack_sender,
    );
    let control_router = broker.control_router();
    let (service_sender, service_receiver) = std::sync::mpsc::sync_channel(1);
    let (done_sender, done_receiver) = std::sync::mpsc::sync_channel(1);
    let (telemetry_sender, telemetry_receiver) = std::sync::mpsc::channel();
    let server = thread::spawn(move || {
        let result = run_x_server_frontend_routed_until_stopped_with_backpressure_observer(
            config,
            transaction_sender,
            broker,
            service_receiver,
            Arc::new(move |event| telemetry_sender.send(event).unwrap()),
        );
        done_sender.send(result).unwrap();
    });

    wait_for_socket(&socket_path);
    let windows = [0x0020_0a31, 0x0040_0a32];
    let mut clients = Vec::new();
    for window in windows {
        let mut client = connect_x_socket(&socket_path);
        client
            .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
            .unwrap();
        read_setup_success(&mut client, XByteOrder::LittleEndian);
        client
            .write_all(&create_window_request(
                XByteOrder::LittleEndian,
                window,
                1,
                2,
                300,
                200,
            ))
            .unwrap();
        clients.push(client);
    }

    let mut waiting_clients = BTreeSet::new();
    let mut telemetry = Vec::new();
    while waiting_clients.len() != 2 {
        let event = telemetry_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("both client workers did not reach authority backpressure");
        if event.kind == XAuthorityBackpressureTelemetryKind::Wait {
            waiting_clients.insert(event.client.expect("routed Wait must identify its client"));
        }
        telemetry.push(event);
    }
    assert_eq!(policy.requests.lock().unwrap().len(), 2);

    // The Engine owner disappearing must release a worker even when no channel
    // slot ever existed. StopAccepting intentionally does not set cancellation,
    // so this path can succeed only by observing sender disconnection.
    drop(transaction_receiver);
    service_sender
        .send(XServerFrontendServiceCommand::StopAccepting)
        .unwrap();
    let error = done_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("authority receiver disconnect left the client worker blocked")
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("observed transaction channel is disconnected")
    );
    server.join().unwrap();
    telemetry.extend(telemetry_receiver.try_iter());
    assert!(telemetry.iter().any(|event| {
        event.kind == XAuthorityBackpressureTelemetryKind::TransportFailure
            && event.failure == Some(XAuthorityBackpressureFailure::Disconnected)
    }));
    for client in &waiting_clients {
        assert!(telemetry.iter().any(|event| {
            event.client == Some(*client)
                && matches!(
                    (event.kind, event.failure),
                    (
                        XAuthorityBackpressureTelemetryKind::TransportFailure,
                        Some(XAuthorityBackpressureFailure::Disconnected)
                    ) | (
                        XAuthorityBackpressureTelemetryKind::Shutdown,
                        Some(XAuthorityBackpressureFailure::Cancelled)
                    )
                )
        }));
    }
    let revoked = policy.revoked.lock().unwrap();
    assert_eq!(revoked.len(), 2);
    assert_eq!(
        revoked
            .iter()
            .map(|context| context.client_id.raw())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([1, 2])
    );
    drop(revoked);

    // Service return is after wait_for_clients: both sockets are closed and
    // retained route handles can report only ClientGone for the old clients.
    for client in &mut clients {
        client
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut byte = [0_u8; 1];
        match client.read(&mut byte) {
            Ok(0) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::BrokenPipe
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::UnexpectedEof
                ) => {}
            outcome => panic!("fatal authority disconnect retained a live client: {outcome:?}"),
        }
    }
    for (index, client) in waiting_clients.iter().copied().enumerate() {
        control_router
            .route_control(XAuthorityClientControlCommand {
                client,
                command: XAuthorityControlCommand::FocusSurface {
                    transaction: TransactionId::from_raw(900 + u64::try_from(index).unwrap()),
                    surface: SurfaceId::new(windows[index], 1),
                },
            })
            .unwrap();
        let ack = ack_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(ack.client, client);
        assert_eq!(ack.acknowledgement.outcome, XAuthorityControlOutcome::ClientGone);
    }
    drop(clients);
    drop(service_sender);
    std::fs::remove_file(&socket_path).unwrap();
}
