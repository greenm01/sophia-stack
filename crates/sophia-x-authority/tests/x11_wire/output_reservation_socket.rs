#[cfg(unix)]
#[test]
fn x11_core_socket_channel_emits_complete_strut_replacement_and_clear() {
    use std::io::Write;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-strut-channel-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let server_path = socket_path.clone();
    let (sender, receiver) =
        std::sync::mpsc::sync_channel(X_AUTHORITY_OBSERVED_TRANSACTION_CHANNEL_CAPACITY);
    let server = thread::spawn(move || {
        run_x11_core_socket_server_once_channel(&server_path, NamespaceId::from_raw(54), sender)
            .unwrap();
    });

    wait_for_socket(&socket_path);
    let mut stream = connect_x_socket(&socket_path);
    stream
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut stream, XByteOrder::LittleEndian);
    let window = 0x220801;
    stream
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            window,
            0,
            0,
            1280,
            28,
        ))
        .unwrap();
    stream
        .write_all(&intern_atom_request(
            XByteOrder::LittleEndian,
            false,
            X_ATOM_NAME_NET_WM_STRUT_PARTIAL,
        ))
        .unwrap();
    let property = read_u32(
        XByteOrder::LittleEndian,
        &read_x_record(&mut stream)[8..12],
    );
    let strut = [0, 0, 28, 0, 0, 0, 0, 0, 0, 1279, 0, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    stream
        .write_all(&change_property_request(
            XByteOrder::LittleEndian,
            XPropertyMode::Replace,
            window,
            property,
            X_ATOM_CARDINAL,
            32,
            &strut,
        ))
        .unwrap();
    assert_eq!(read_x_record(&mut stream)[0], 28);
    stream
        .write_all(&delete_property_request(
            XByteOrder::LittleEndian,
            window,
            property,
        ))
        .unwrap();
    assert_eq!(read_x_record(&mut stream)[0], 28);

    drop(stream);
    let _ = std::fs::remove_file(&socket_path);
    server.join().unwrap();
    let snapshots = std::iter::from_fn(|| receiver.try_recv().ok())
        .flat_map(|batch| batch.surface_output_reservations)
        .collect::<Vec<_>>();
    assert_eq!(snapshots.len(), 2);
    assert_eq!(
        snapshots[0].reservations,
        vec![OutputReservation {
            edge: OutputEdge::Top,
            depth: 28,
            span: AxisSpan {
                start: 0,
                end: 1280,
            },
        }]
    );
    assert_eq!(snapshots[1].surface, snapshots[0].surface);
    assert!(snapshots[1].reservations.is_empty());
}
