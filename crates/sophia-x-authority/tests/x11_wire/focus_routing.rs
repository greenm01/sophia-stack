#[cfg(unix)]
#[test]
fn routed_focus_notifies_both_clients_across_repeated_transitions() {
    use std::io::Write;
    use std::num::NonZeroUsize;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-focus-routing-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let (transaction_sender, transaction_receiver) =
        std::sync::mpsc::sync_channel(X_AUTHORITY_OBSERVED_TRANSACTION_CHANNEL_CAPACITY);
    let (acknowledgement_sender, acknowledgement_receiver) = std::sync::mpsc::sync_channel(4);
    let broker = XServerFrontendRouteBroker::with_control_ack_sender(
        NonZeroUsize::new(4).unwrap(),
        acknowledgement_sender,
    );
    let control_sender = broker.control_sender();
    let (service_sender, service_receiver) = std::sync::mpsc::sync_channel(1);
    let config = XServerFrontendConfig::new(&socket_path, NamespaceId::from_raw(856))
        .unwrap()
        .with_max_concurrent_clients(NonZeroUsize::new(2).unwrap());
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
    let mut first = UnixStream::connect(&socket_path).unwrap();
    first
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut first, XByteOrder::LittleEndian);
    let first_window = 0x0020_0a01;
    first
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            first_window,
            0,
            0,
            320,
            240,
        ))
        .unwrap();
    assert_eq!(read_x_record(&mut first)[0], 22);
    first
        .write_all(&sophia_present_pixmap_request(
            XByteOrder::LittleEndian,
            first_window,
            0xa01,
            (0, 0, 16, 16),
            1,
            1,
        ))
        .unwrap();

    let mut second = UnixStream::connect(&socket_path).unwrap();
    second
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut second, XByteOrder::LittleEndian);
    let second_window = 0x0040_0a01;
    second
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            second_window,
            0,
            0,
            320,
            240,
        ))
        .unwrap();
    assert_eq!(read_x_record(&mut second)[0], 22);
    second
        .write_all(&sophia_present_pixmap_request(
            XByteOrder::LittleEndian,
            second_window,
            0xa02,
            (0, 0, 16, 16),
            1,
            1,
        ))
        .unwrap();

    let mut routes = Vec::new();
    while routes.len() < 2 {
        let batch = transaction_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        if let (Some(client), Some(transaction)) = (batch.client, batch.transactions.first()) {
            routes.push((client, transaction.surface));
        }
    }
    routes.sort_by_key(|(client, _)| client.raw());

    let focus = |index: usize, transaction: u64| {
        let (client, surface) = routes[index];
        control_sender
            .send(XAuthorityClientControlCommand {
                client,
                command: XAuthorityControlCommand::FocusSurface {
                    transaction: TransactionId::from_raw(transaction),
                    surface,
                },
            })
            .unwrap();
        let acknowledgement = acknowledgement_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        assert_eq!(acknowledgement.client, client);
        assert_eq!(
            acknowledgement.acknowledgement.outcome,
            XAuthorityControlOutcome::Delivered
        );
    };

    focus(0, 100);
    assert_focus_event(&mut first, true, first_window);

    control_sender
        .send(XAuthorityClientControlCommand {
            client: routes[1].0,
            command: XAuthorityControlCommand::FocusSurface {
                transaction: TransactionId::from_raw(101),
                surface: routes[1].1,
            },
        })
        .unwrap();
    assert_focus_event(&mut first, false, first_window);
    assert_focus_event(&mut second, true, second_window);
    assert_eq!(
        acknowledgement_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .acknowledgement
            .outcome,
        XAuthorityControlOutcome::Delivered
    );

    control_sender
        .send(XAuthorityClientControlCommand {
            client: routes[0].0,
            command: XAuthorityControlCommand::FocusSurface {
                transaction: TransactionId::from_raw(102),
                surface: routes[0].1,
            },
        })
        .unwrap();
    assert_focus_event(&mut second, false, second_window);
    assert_focus_event(&mut first, true, first_window);
    assert_eq!(
        acknowledgement_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .acknowledgement
            .outcome,
        XAuthorityControlOutcome::Delivered
    );

    drop(first);
    drop(second);
    service_sender
        .send(XServerFrontendServiceCommand::StopAccepting)
        .unwrap();
    drop(service_sender);
    drop(control_sender);
    server.join().unwrap();
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
fn assert_focus_event(stream: &mut std::os::unix::net::UnixStream, focused: bool, window: u32) {
    let event = read_x_record(stream);
    assert_eq!(event[0], if focused { 9 } else { 10 });
    assert_eq!(event[1], 3);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &event[4..8]),
        window
    );
    assert_eq!(event[8], 0);
}
