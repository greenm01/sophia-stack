#[cfg(unix)]
struct X11InputEventWriter {
    stop: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<Result<(), X11SetupSocketError>>,
}
struct X11ControlWriter {
    stop: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<Result<(), X11SetupSocketError>>,
}

#[cfg(unix)]
struct X11ProtocolEventWriter {
    stop: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<Result<(), X11SetupSocketError>>,
}

#[cfg(unix)]
fn spawn_x11_protocol_event_writer(
    stream: Arc<Mutex<UnixStream>>,
    byte_order: XByteOrder,
    sequence: Arc<AtomicU16>,
    receiver: Receiver<XClientEvent>,
) -> Result<X11ProtocolEventWriter, X11SetupSocketError> {
    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = stop.clone();
    let thread = std::thread::spawn(move || {
        while !writer_stop.load(Ordering::Acquire) {
            let mut event = match receiver.recv_timeout(Duration::from_millis(10)) {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            };
            let mut stream = stream
                .lock()
                .map_err(|_| X11SetupSocketError::new("X11 output socket lock poisoned"))?;
            set_x11_protocol_event_sequence(&mut event, sequence.load(Ordering::Acquire));
            let record = encode_x_client_event(byte_order, event);
            if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some() {
                tracing::trace!(
                    "sophia_x11_socket_write schema=1 writer=protocol bytes={} payload_redacted=true",
                    record.len(),
                );
            }
            if let Err(error) = stream.write_all(&record) {
                if is_x11_client_disconnect(&error) {
                    return Ok(());
                }
                return Err(X11SetupSocketError::new(format!(
                    "failed to write X11 protocol event: {error}"
                )));
            }
            stream.flush().map_err(|error| {
                X11SetupSocketError::new(format!("failed to flush X11 protocol event: {error}"))
            })?;
        }
        Ok(())
    });
    Ok(X11ProtocolEventWriter { stop, thread })
}

#[cfg(unix)]
fn set_x11_protocol_event_sequence(event: &mut XClientEvent, value: u16) {
    match event {
        XClientEvent::SelectionClear { sequence, .. }
        | XClientEvent::SelectionRequest { sequence, .. }
        | XClientEvent::SelectionNotify { sequence, .. }
        | XClientEvent::PropertyNotify { sequence, .. }
        | XClientEvent::RandrScreenChange { sequence, .. }
        | XClientEvent::RandrCrtcChange { sequence, .. }
        | XClientEvent::RandrOutputChange { sequence, .. }
        | XClientEvent::RandrResourceChange { sequence, .. }
        | XClientEvent::PresentConfigureNotify { sequence, .. }
        | XClientEvent::PresentCompleteNotify { sequence, .. }
        | XClientEvent::PresentIdleNotify { sequence, .. } => *sequence = value,
        _ => unreachable!("protocol routing received a non-routable event"),
    }
}

#[cfg(unix)]
#[allow(clippy::too_many_arguments)]
fn spawn_x11_control_writer(
    stream: Arc<Mutex<UnixStream>>,
    byte_order: XByteOrder,
    sequence: Arc<AtomicU16>,
    focused_surface_window: Arc<AtomicU64>,
    surface_windows: Arc<Mutex<BTreeMap<SurfaceId, XResourceId>>>,
    core_event_selections: Arc<Mutex<XCoreEventSelectionState>>,
    atoms: Arc<Mutex<XAtomTable>>,
    properties: Arc<Mutex<XPropertyTable>>,
    runtime: Arc<Mutex<XAuthorityRuntime>>,
    resource_id_range: crate::XWireClientResourceRange,
    namespace: NamespaceId,
    client: XServerFrontendClientId,
    protocol_routing: Option<XServerFrontendRouteRegistry>,
    channels: X11ControlChannels,
) -> Result<X11ControlWriter, X11SetupSocketError> {
    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = stop.clone();
    macro_rules! terminate_client {
        ($kind:expr, $transaction:expr, $surface:expr) => {{
            let stream = stream
                .lock()
                .map_err(|_| X11SetupSocketError::new("X11 output socket lock poisoned"))?;
            stream.shutdown(Shutdown::Both).map_err(|error| {
                X11SetupSocketError::new(format!(
                    "failed to terminate non-cooperating X11 client: {error}"
                ))
            })?;
            drop(stream);
            channels.send_ack(
                client,
                XAuthorityControlAck {
                    kind: $kind,
                    transaction: $transaction,
                    surface: $surface,
                    outcome: XAuthorityControlOutcome::Delivered,
                },
            )?;
            return Ok(());
        }};
    }
    let thread = std::thread::spawn(move || {
        while !writer_stop.load(Ordering::Acquire) {
            let command = match channels.recv_timeout(client) {
                Ok(command) => command,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            };
            let transaction = command.transaction();
            let surface = command.surface();
            let kind = command.kind();
            let window = surface_windows
                .lock()
                .map_err(|_| X11SetupSocketError::new("X11 surface/window map lock poisoned"))?
                .get(&surface)
                .copied();
            let Some(window) = window else {
                channels.send_ack(
                    client,
                    XAuthorityControlAck {
                        kind,
                        transaction,
                        surface,
                        outcome: XAuthorityControlOutcome::UnknownSurface,
                    },
                )?;
                continue;
            };

            let event_sequence = sequence.load(Ordering::Acquire);
            let records = match command {
                XAuthorityControlCommand::ConfigureSurface { size, .. } => {
                    if size.width <= 0
                        || size.height <= 0
                        || size.width > i32::from(u16::MAX)
                        || size.height > i32::from(u16::MAX)
                    {
                        channels.send_ack(
                            client,
                            XAuthorityControlAck {
                                kind,
                                transaction,
                                surface,
                                outcome: XAuthorityControlOutcome::InvalidSize,
                            },
                        )?;
                        continue;
                    }
                    let geometry = match runtime
                        .lock()
                        .map_err(|_| {
                            X11SetupSocketError::new("X11 authority runtime lock poisoned")
                        })?
                        .configure_window_size_from_engine(namespace, window, size)
                    {
                        Ok(geometry) => geometry,
                        Err(_) => {
                            channels.send_ack(
                                client,
                                XAuthorityControlAck {
                                    kind,
                                    transaction,
                                    surface,
                                    outcome: XAuthorityControlOutcome::AuthorityRejected,
                                },
                            )?;
                            continue;
                        }
                    };
                    let width = u16::try_from(geometry.width).expect("validated above");
                    let height = u16::try_from(geometry.height).expect("validated above");
                    let present_event_ids = protocol_routing
                        .as_ref()
                        .map(|routing| routing.present_configure_event_ids(client, window))
                        .transpose()
                        .map_err(|error| {
                            X11SetupSocketError::new(format!(
                                "failed to resolve Present ConfigureNotify subscriptions: {error}"
                            ))
                        })?
                        .unwrap_or_default();
                    let mut records = Vec::with_capacity(present_event_ids.len() + 2);
                    records.push(encode_x_client_event(
                            byte_order,
                            XClientEvent::ConfigureNotify {
                                sequence: event_sequence,
                                event: window,
                                window,
                                above_sibling: None,
                                x: clamp_engine_i16(geometry.x),
                                y: clamp_engine_i16(geometry.y),
                                width,
                                height,
                                border_width: 0,
                                override_redirect: false,
                            },
                        ));
                    records.extend(present_event_ids.into_iter().map(|event_id| {
                        encode_x_client_event(
                            byte_order,
                            XClientEvent::PresentConfigureNotify {
                                sequence: event_sequence,
                                event_id,
                                window,
                                x: clamp_engine_i16(geometry.x),
                                y: clamp_engine_i16(geometry.y),
                                width,
                                height,
                                pixmap_width: width,
                                pixmap_height: height,
                                pixmap_flags: 0,
                            },
                        )
                    }));
                    records.push(encode_x_client_event(
                            byte_order,
                            XClientEvent::Expose {
                                sequence: event_sequence,
                                window,
                                x: 0,
                                y: 0,
                                width,
                                height,
                                count: 0,
                            },
                        ));
                    records
                }
                XAuthorityControlCommand::CloseSurface { .. } => {
                    let atoms = atoms
                        .lock()
                        .map_err(|_| X11SetupSocketError::new("X11 atom table lock poisoned"))?;
                    let Some(protocols) = atoms.atom(X_ATOM_NAME_WM_PROTOCOLS) else {
                        terminate_client!(kind, transaction, surface);
                    };
                    let Some(delete) = atoms.atom(X_ATOM_NAME_WM_DELETE_WINDOW) else {
                        terminate_client!(kind, transaction, surface);
                    };
                    drop(atoms);
                    let properties = properties.lock().map_err(|_| {
                        X11SetupSocketError::new("X11 property table lock poisoned")
                    })?;
                    let protocol_windows = properties.windows_with_property(namespace, protocols);
                    let advertises_delete = |candidate: &XResourceId| {
                        u32::try_from(candidate.local.raw())
                            .is_ok_and(|raw| resource_id_range.owns_new_resource(raw))
                            && properties
                                .get(namespace, *candidate, protocols)
                                .is_some_and(|record| {
                                    record.format == 32
                                        && record
                                            .bytes
                                            .chunks_exact(4)
                                            .any(|bytes| byte_order.u32(bytes) == delete)
                                })
                    };
                    let candidates: Vec<_> = protocol_windows
                        .iter()
                        .map(|candidate| (*candidate, advertises_delete(candidate)))
                        .collect();
                    let ancestors = core_event_selections
                        .lock()
                        .map_err(|_| {
                            X11SetupSocketError::new("X11 core event selection lock poisoned")
                        })?
                        .ancestors(window);
                    let decision = crate::select_x_close_target(window, &ancestors, &candidates);
                    if decision.protocol_window_count == 0 {
                        drop(properties);
                        terminate_client!(kind, transaction, surface);
                    }
                    tracing::debug!(
                        "sophia_x11_close_target schema=1 surface_map_hit=true exact_delete={} fallback_used={} protocol_windows={}",
                        decision.exact_advertises_delete,
                        decision.fallback_used,
                        decision.protocol_window_count,
                    );
                    let window = decision.window;
                    let mut bytes = [0_u8; 32];
                    // ICCCM WM_DELETE_WINDOW is delivered via SendEvent, so
                    // the synthetic-event bit must be set on ClientMessage.
                    bytes[0] = 33 | 0x80;
                    bytes[1] = 32;
                    write_control_u32(byte_order, &mut bytes[4..8], window.local.raw() as u32);
                    write_control_u32(byte_order, &mut bytes[8..12], protocols);
                    write_control_u32(byte_order, &mut bytes[12..16], delete);
                    vec![encode_x_client_event(
                        byte_order,
                        XClientEvent::ClientMessage {
                            sequence: event_sequence,
                            bytes,
                        },
                    )]
                }
                XAuthorityControlCommand::FocusSurface { .. } => {
                    let previous = {
                        let mut runtime = runtime.lock().map_err(|_| {
                            X11SetupSocketError::new("X11 authority runtime lock poisoned")
                        })?;
                        let (previous, _) = runtime.input_focus(namespace);
                        if runtime.set_input_focus(namespace, window, 1).is_err() {
                            channels.send_ack(
                                client,
                                XAuthorityControlAck {
                                    kind,
                                    transaction,
                                    surface,
                                    outcome: XAuthorityControlOutcome::AuthorityRejected,
                                },
                            )?;
                            continue;
                        }
                        previous
                    };
                    let previous_routed = XResourceId::new(
                        focused_surface_window.swap(window.local.raw(), Ordering::AcqRel),
                        1,
                    );
                    if previous == window && previous_routed == window {
                        channels.send_ack(
                            client,
                            XAuthorityControlAck {
                                kind,
                                transaction,
                                surface,
                                outcome: XAuthorityControlOutcome::Delivered,
                            },
                        )?;
                        continue;
                    }
                    let mut records = Vec::with_capacity(2);
                    if previous_routed != window
                        && previous_routed.local.raw() != u64::from(X_SETUP_DEFAULT_ROOT)
                    {
                        records.push(encode_x_client_event(
                            byte_order,
                            XClientEvent::Focus {
                                sequence: event_sequence,
                                focused: false,
                                detail: 3,
                                event: previous_routed,
                                mode: 0,
                            },
                        ));
                    }
                    records.push(encode_x_client_event(
                        byte_order,
                        XClientEvent::Focus {
                            sequence: event_sequence,
                            focused: true,
                            detail: 3,
                            event: window,
                            mode: 0,
                        },
                    ));
                    records
                }
                XAuthorityControlCommand::ClearFocus { .. } => {
                    let root = XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1);
                    {
                        let mut runtime = runtime.lock().map_err(|_| {
                            X11SetupSocketError::new("X11 authority runtime lock poisoned")
                        })?;
                        if runtime.set_input_focus(namespace, root, 1).is_err() {
                            channels.send_ack(
                                client,
                                XAuthorityControlAck {
                                    kind,
                                    transaction,
                                    surface,
                                    outcome: XAuthorityControlOutcome::AuthorityRejected,
                                },
                            )?;
                            continue;
                        }
                    }
                    let previous = XResourceId::new(
                        focused_surface_window.swap(root.local.raw(), Ordering::AcqRel),
                        1,
                    );
                    if previous == root {
                        Vec::new()
                    } else {
                        vec![encode_x_client_event(
                            byte_order,
                            XClientEvent::Focus {
                                sequence: event_sequence,
                                focused: false,
                                detail: 3,
                                event: previous,
                                mode: 0,
                            },
                        )]
                    }
                }
            };

            let mut stream = stream
                .lock()
                .map_err(|_| X11SetupSocketError::new("X11 output socket lock poisoned"))?;
            let event_sequence = sequence.load(Ordering::Acquire);
            for mut record in records {
                write_xi_u16(byte_order, &mut record[2..4], event_sequence);
                if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some() {
                    tracing::trace!(
                        "sophia_x11_socket_write schema=1 writer=control bytes={} payload_redacted=true",
                        record.len(),
                    );
                }
                if let Err(error) = stream.write_all(&record) {
                    if is_x11_client_disconnect(&error) {
                        return Ok(());
                    }
                    return Err(X11SetupSocketError::new(format!(
                        "failed to write X11 control event: {error}"
                    )));
                }
            }
            stream.flush().map_err(|error| {
                X11SetupSocketError::new(format!("failed to flush X11 control event: {error}"))
            })?;
            drop(stream);
            channels.send_ack(
                client,
                XAuthorityControlAck {
                    kind,
                    transaction,
                    surface,
                    outcome: XAuthorityControlOutcome::Delivered,
                },
            )?;
        }
        Ok(())
    });
    Ok(X11ControlWriter { stop, thread })
}

#[cfg(unix)]
fn write_control_u32(byte_order: XByteOrder, out: &mut [u8], value: u32) {
    let bytes = match byte_order {
        XByteOrder::LittleEndian => value.to_le_bytes(),
        XByteOrder::BigEndian => value.to_be_bytes(),
    };
    out.copy_from_slice(&bytes);
}

#[cfg(unix)]
fn clamp_engine_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

#[cfg(unix)]
struct X11InputWriterState {
    stream: Arc<Mutex<UnixStream>>,
    byte_order: XByteOrder,
    sequence: Arc<AtomicU16>,
    focused_surface_window: Arc<AtomicU64>,
    core_event_selections: Arc<Mutex<XCoreEventSelectionState>>,
    xkb_state_details: Arc<AtomicU16>,
    xkb_modifiers: Arc<AtomicU16>,
    surface_windows: Arc<Mutex<BTreeMap<SurfaceId, XResourceId>>>,
    client: XServerFrontendClientId,
}

#[cfg(unix)]
fn spawn_x11_input_event_writer(
    state: X11InputWriterState,
    receiver: X11InputEventReceiver,
) -> Result<X11InputEventWriter, X11SetupSocketError> {
    let X11InputWriterState {
        stream,
        byte_order,
        sequence,
        focused_surface_window,
        core_event_selections,
        xkb_state_details,
        xkb_modifiers,
        surface_windows,
        client,
    } = state;
    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = stop.clone();
    let thread = std::thread::spawn(move || {
        let mut focus_sent_to = None;
        let mut pointer_sent_to = None;
        while !writer_stop.load(Ordering::Acquire) {
            let (event, target_window, xi_event_type, xi_transition_mask, delivery) =
                match receiver.recv_timeout(client) {
                    Ok(event) => event,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => return Ok(()),
                };
            // A mapped GL client can expose its first frame before its event
            // loop installs KeyPress/KeyReleaseMask. Keep physical keys
            // boundedly pending across that startup race instead of writing
            // core events which the client has not selected and will ignore.
            let keyboard_wait_started = std::time::Instant::now();
            let keyboard_deadline = keyboard_wait_started + Duration::from_secs(5);
            let (focused_window, routed_keyboard_window, keyboard_selected) = loop {
                let selections = core_event_selections.lock().map_err(|_| {
                    X11SetupSocketError::new("X11 core event selection lock poisoned")
                })?;
                let focused = XResourceId::new(focused_surface_window.load(Ordering::Acquire), 1);
                let focused_selected = selections.selected_keyboard_target(focused);
                let routed_selected =
                    target_window.and_then(|window| selections.selected_keyboard_target(window));
                let focused_fallback = selections.keyboard_target(focused);
                let routed_fallback =
                    target_window.map(|window| selections.keyboard_target(window));
                drop(selections);
                if !matches!(event, XAuthorityInputEvent::Key(_))
                    || focused_selected.is_some()
                    || routed_selected.is_some()
                    || std::time::Instant::now() >= keyboard_deadline
                {
                    break (
                        focused_selected.unwrap_or(focused_fallback),
                        routed_selected.or(routed_fallback),
                        focused_selected.is_some() || routed_selected.is_some(),
                    );
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some()
                && matches!(event, XAuthorityInputEvent::Key(_))
            {
                tracing::debug!(
                    "sophia_x11_key_delivery schema=2 stage=target_resolved client={} keyboard_selected={} explicit_target={} xi_event={} wait_msec={} input_redacted=true",
                    client.raw(),
                    keyboard_selected,
                    routed_keyboard_window.is_some(),
                    xi_event_type.is_some(),
                    keyboard_wait_started.elapsed().as_millis(),
                );
            }
            if let XAuthorityInputEvent::Key(_) = event
                && routed_keyboard_window.is_some_and(|window| window != focused_window)
            {
                tracing::warn!(
                    "sophia_x11_key_delivery schema=1 target_matches_focus=false explicit_target=true",
                );
            }
            let root = XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1);
            let delivered_window = match event {
                XAuthorityInputEvent::Key(_) => routed_keyboard_window.unwrap_or(focused_window),
                XAuthorityInputEvent::Pointer(pointer) => target_window.unwrap_or(
                    *surface_windows
                        .lock()
                        .map_err(|_| {
                            X11SetupSocketError::new("X11 surface/window map lock poisoned")
                        })?
                        .get(&pointer.surface)
                        .ok_or_else(|| {
                            X11SetupSocketError::new("X11 pointer target surface is unknown")
                        })?,
                ),
            };
            let delivered_focus = delivered_window;
            if matches!(event, XAuthorityInputEvent::Key(_))
                && focused_surface_window.load(Ordering::Acquire) == delivered_focus.local.raw()
            {
                focus_sent_to = Some(delivered_focus);
            }
            let mut record = encode_x_client_event(
                byte_order,
                match event {
                    XAuthorityInputEvent::Key(event) => XClientEvent::Key {
                        sequence: 0,
                        pressed: event.pressed,
                        keycode: event.keycode,
                        time: event.time_msec,
                        root,
                        event: delivered_window,
                        state: event.state,
                    },
                    XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                        kind: XAuthorityPointerEventKind::Motion,
                        surface,
                        root_x,
                        root_y,
                        event_x,
                        event_y,
                        state,
                        time_msec,
                    }) => XClientEvent::PointerMotion {
                        sequence: 0,
                        time: time_msec,
                        root,
                        event: target_window.unwrap_or(
                            *surface_windows
                                .lock()
                                .map_err(|_| {
                                    X11SetupSocketError::new("X11 surface/window map lock poisoned")
                                })?
                                .get(&surface)
                                .ok_or_else(|| {
                                    X11SetupSocketError::new(
                                        "X11 pointer target surface is unknown",
                                    )
                                })?,
                        ),
                        root_x,
                        root_y,
                        event_x,
                        event_y,
                        state,
                    },
                    XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                        kind: XAuthorityPointerEventKind::Button { button, pressed },
                        surface,
                        root_x,
                        root_y,
                        event_x,
                        event_y,
                        state,
                        time_msec,
                    }) => XClientEvent::PointerButton {
                        sequence: 0,
                        pressed,
                        button,
                        time: time_msec,
                        root,
                        event: target_window.unwrap_or(
                            *surface_windows
                                .lock()
                                .map_err(|_| {
                                    X11SetupSocketError::new("X11 surface/window map lock poisoned")
                                })?
                                .get(&surface)
                                .ok_or_else(|| {
                                    X11SetupSocketError::new(
                                        "X11 pointer target surface is unknown",
                                    )
                                })?,
                        ),
                        root_x,
                        root_y,
                        event_x,
                        event_y,
                        state,
                    },
                },
            );
            let write_result = (|| -> Result<(), X11SetupSocketError> {
                let mut stream = stream
                    .lock()
                    .map_err(|_| X11SetupSocketError::new("X11 output socket lock poisoned"))?;
                let sequence = sequence.load(Ordering::Acquire);
                write_xi_u16(byte_order, &mut record[2..4], sequence);
                let transition = match event {
                    XAuthorityInputEvent::Key(_) if focus_sent_to != Some(delivered_window) => {
                        Some((focus_sent_to, 10, 9))
                    }
                    XAuthorityInputEvent::Pointer(_)
                        if pointer_sent_to != Some(delivered_window) =>
                    {
                        Some((pointer_sent_to, 8, 7))
                    }
                    _ => None,
                };
                if let Some((previous, out_type, in_type)) = transition {
                    if let Some(previous) = previous
                        && xi_transition_mask & (1 << out_type) != 0
                    {
                        stream
                            .write_all(&encode_xi_crossing_event(
                                byte_order, sequence, out_type, event, previous,
                            ))
                            .map_err(|error| {
                                X11SetupSocketError::new(format!(
                                    "failed to write XI2 leave/focus-out event: {error}"
                                ))
                            })?;
                    }
                    if xi_transition_mask & (1 << in_type) != 0 {
                        stream
                            .write_all(&encode_xi_crossing_event(
                                byte_order,
                                sequence,
                                in_type,
                                event,
                                delivered_window,
                            ))
                            .map_err(|error| {
                                X11SetupSocketError::new(format!(
                                    "failed to write XI2 enter/focus-in event: {error}"
                                ))
                            })?;
                    }
                    if matches!(event, XAuthorityInputEvent::Pointer(_)) {
                        pointer_sent_to = Some(delivered_window);
                    }
                }
                if matches!(event, XAuthorityInputEvent::Key(_))
                    && focus_sent_to != Some(delivered_focus)
                {
                    let focus = encode_x_client_event(
                        byte_order,
                        XClientEvent::Focus {
                            sequence,
                            focused: true,
                            detail: 3,
                            event: delivered_focus,
                            mode: 0,
                        },
                    );
                    stream.write_all(&focus).map_err(|error| {
                        X11SetupSocketError::new(format!(
                            "failed to write X11 focus event: {error}"
                        ))
                    })?;
                    focus_sent_to = Some(delivered_focus);
                }
                stream.write_all(&record).map_err(|error| {
                    if is_x11_client_disconnect(&error) {
                        X11SetupSocketError::client_disconnect(format!(
                            "X11 client disconnected while writing input: {error}"
                        ))
                    } else {
                        X11SetupSocketError::new(format!(
                            "failed to write X11 input event: {error}"
                        ))
                    }
                })?;
                if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some() {
                    tracing::trace!(
                        "sophia_x11_socket_write schema=1 writer=input bytes={} payload_redacted=true",
                        record.len(),
                    );
                }
                if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some()
                    && matches!(event, XAuthorityInputEvent::Key(_))
                {
                    tracing::debug!(
                        "sophia_x11_key_delivery schema=2 stage=wire_flushed sequence={sequence} input_redacted=true"
                    );
                }
                if let XAuthorityInputEvent::Key(key) = event {
                    let previous = xkb_modifiers
                        .swap(u16::from(key.modifiers_after), Ordering::AcqRel);
                    let changed = previous ^ u16::from(key.modifiers_after);
                    let selected = xkb_state_details.load(Ordering::Acquire);
                    if changed != 0 && selected & 1 != 0 {
                        let state_notify = encode_x_client_event(
                            byte_order,
                            XClientEvent::XkbStateNotify {
                                sequence,
                                time: key.time_msec,
                                modifiers: key.modifiers_after,
                                changed: 1,
                                keycode: key.keycode,
                                event_type: if key.pressed { 2 } else { 3 },
                            },
                        );
                        stream.write_all(&state_notify).map_err(|error| {
                            X11SetupSocketError::new(format!(
                                "failed to write XKB state notification: {error}"
                            ))
                        })?;
                    }
                }
                if let Some(event_type) = xi_event_type {
                    let generic = encode_xi_device_event(
                        byte_order,
                        sequence,
                        event_type,
                        event,
                        delivered_window,
                    );
                    stream.write_all(&generic).map_err(|error| {
                        X11SetupSocketError::new(format!(
                            "failed to write XI2 generic event: {error}"
                        ))
                    })?;
                }
                stream.flush().map_err(|error| {
                    X11SetupSocketError::new(format!("failed to flush X11 input event: {error}"))
                })
            })();
            match write_result {
                Ok(()) => receiver.send_delivery(
                    client,
                    delivery,
                    XAuthorityInputDeliveryOutcome::Flushed,
                )?,
                Err(error) => {
                    if error.client_disconnect {
                        return Ok(());
                    }
                    let _ = receiver.send_delivery(
                        client,
                        delivery,
                        XAuthorityInputDeliveryOutcome::WriteFailed,
                    );
                    return Err(error);
                }
            }
        }
        Ok(())
    });
    Ok(X11InputEventWriter { stop, thread })
}
