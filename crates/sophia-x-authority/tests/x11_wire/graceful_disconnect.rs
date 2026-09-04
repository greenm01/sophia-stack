#[cfg(unix)]
#[test]
fn graceful_disconnect_drains_backpressured_work_while_client_stays_open() {
    use std::io::Write;
    use std::num::NonZeroUsize;
    use std::sync::{Arc, mpsc};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let path = std::env::temp_dir().join(format!(
        "sophia-graceful-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = XServerFrontendConfig::new(&path, NamespaceId::from_raw(992)).unwrap();
    // Rendezvous forces the accepted request to wait for the owner. Closing
    // the browser stream must not cancel that request or its removal batch.
    let (tx, rx) = mpsc::sync_channel(0);
    let (commands, command_rx) = mpsc::sync_channel(2);
    let (events, event_rx) = mpsc::channel();
    let (done, done_rx) = mpsc::channel();
    let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(4).unwrap());
    let server = std::thread::spawn(move || {
        done.send(
            run_x_server_frontend_routed_until_stopped_with_backpressure_observer(
                config,
                tx,
                broker,
                command_rx,
                Arc::new(move |event| {
                    let _ = events.send(event);
                }),
            ),
        )
        .unwrap();
    });
    wait_for_socket(&path);
    let mut client = connect_x_socket(&path);
    client
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut client, XByteOrder::LittleEndian);
    client
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x200111,
            0,
            0,
            30,
            20,
        ))
        .unwrap();
    loop {
        let event = event_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        if event.kind == XAuthorityBackpressureTelemetryKind::Wait {
            break;
        }
    }
    commands
        .send(XServerFrontendServiceCommand::DrainAndDisconnect)
        .unwrap();
    let mut transactions = 0;
    let mut removals = 0;
    loop {
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(batch) => {
                transactions += usize::from(batch.transaction.raw() != 0);
                removals += batch.removed_surfaces.len();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(error) => panic!("graceful drain hung with the client socket still open: {error}"),
        }
    }
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    server.join().unwrap();
    assert!(transactions > 0);
    assert_eq!(removals, 1);
    assert!(
        !event_rx
            .try_iter()
            .any(|event| event.failure == Some(XAuthorityBackpressureFailure::Cancelled))
    );
    drop(client);
    std::fs::remove_file(path).unwrap();
}
