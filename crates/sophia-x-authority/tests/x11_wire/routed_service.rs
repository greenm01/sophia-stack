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
