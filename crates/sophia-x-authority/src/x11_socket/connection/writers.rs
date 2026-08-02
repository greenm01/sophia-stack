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
fn x11_surface_geometry_records(
    byte_order: XByteOrder,
    event_sequence: u16,
    client: XServerFrontendClientId,
    window: XResourceId,
    geometry: Rect,
    admit: bool,
    present_configure: bool,
    protocol_routing: Option<&XServerFrontendRouteRegistry>,
) -> Result<Vec<Vec<u8>>, X11SetupSocketError> {
    let width = u16::try_from(geometry.width)
        .map_err(|_| X11SetupSocketError::new("X11 control geometry width is invalid"))?;
    let height = u16::try_from(geometry.height)
        .map_err(|_| X11SetupSocketError::new("X11 control geometry height is invalid"))?;
    let present_events = protocol_routing
        .filter(|_| present_configure)
        .map(|routing| route_x11_present_configure(routing, client, event_sequence, window, geometry))
        .transpose()?
        .unwrap_or_default();
    let mut records =
        Vec::with_capacity(present_events.len() + if admit { 4 } else { 2 });
    records.extend(
        present_events
            .into_iter()
            .map(|event| encode_x_client_event(byte_order, event)),
    );
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
    if admit {
        records.push(encode_x_client_event(
            byte_order,
            XClientEvent::MapNotify {
                sequence: event_sequence,
                event: window,
                window,
                override_redirect: false,
            },
        ));
        records.push(encode_x_client_event(
            byte_order,
            XClientEvent::VisibilityNotify {
                sequence: event_sequence,
                window,
                state: 0,
            },
        ));
    }
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
    Ok(records)
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
    control_runtime_pending: Arc<AtomicUsize>,
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
            let routed = match channels.recv_timeout(client) {
                Ok(routed) => routed,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            };
            let (command, focus_transition) = match routed {
                X11RoutedControl::Authority { command, focus } => (command, focus),
                X11RoutedControl::FocusOut { window } => {
                    focused_surface_window.store(
                        u64::from(X_SETUP_DEFAULT_ROOT),
                        Ordering::Release,
                    );
                    write_x11_control_records(
                        &stream,
                        byte_order,
                        &sequence,
                        vec![x11_focus_event_record(
                            byte_order,
                            sequence.load(Ordering::Acquire),
                            window,
                            false,
                        )],
                    )?;
                    continue;
                }
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
                XAuthorityControlCommand::AdmitSurface { geometry, .. } => {
                    let geometry = match lock_x11_control_runtime(
                        &runtime,
                        &control_runtime_pending,
                    )?
                        .admit_window_from_engine(namespace, window, geometry)
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
                    let mut selections = core_event_selections
                        .lock()
                        .map_err(|_| {
                            X11SetupSocketError::new("X11 core event selection lock poisoned")
                        })?;
                    selections.update_geometry(window, geometry);
                    selections.observe_mapped(window);
                    drop(selections);
                    x11_surface_geometry_records(
                        byte_order,
                        event_sequence,
                        client,
                        window,
                        geometry,
                        true,
                        true,
                        protocol_routing.as_ref(),
                    )?
                }
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
                    let mut runtime =
                        lock_x11_control_runtime(&runtime, &control_runtime_pending)?;
                    let previous_geometry = runtime.window_geometry(namespace, window).ok();
                    let geometry = match runtime
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
                    drop(runtime);
                    core_event_selections
                        .lock()
                        .map_err(|_| {
                            X11SetupSocketError::new("X11 core event selection lock poisoned")
                        })?
                        .update_geometry(window, geometry);
                    x11_surface_geometry_records(
                        byte_order,
                        event_sequence,
                        client,
                        window,
                        geometry,
                        false,
                        previous_geometry != Some(geometry),
                        protocol_routing.as_ref(),
                    )?
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
                        let mut runtime =
                            lock_x11_control_runtime(&runtime, &control_runtime_pending)?;
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
                    x11_focus_surface_records(
                        byte_order,
                        event_sequence,
                        window,
                        previous,
                        previous_routed,
                        focus_transition,
                    )?
                }
                XAuthorityControlCommand::ClearFocus { .. } => {
                    let root = XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1);
                    {
                        let mut runtime =
                            lock_x11_control_runtime(&runtime, &control_runtime_pending)?;
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
                    let previous_routed = XResourceId::new(
                        focused_surface_window.swap(root.local.raw(), Ordering::AcqRel),
                        1,
                    );
                    x11_clear_focus_records(
                        byte_order,
                        event_sequence,
                        root,
                        previous_routed,
                        focus_transition,
                    )?
                }
                XAuthorityControlCommand::WithdrawSurface { .. } => {
                    let was_active = match lock_x11_control_runtime(
                        &runtime,
                        &control_runtime_pending,
                    )?
                        .unmap_window(namespace, window)
                    {
                        Ok(surface) => surface.is_some(),
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
                    core_event_selections
                        .lock()
                        .map_err(|_| {
                            X11SetupSocketError::new("X11 core event selection lock poisoned")
                        })?
                        .observe_unmapped(window);
                    if was_active {
                        vec![encode_x_client_event(
                            byte_order,
                            XClientEvent::UnmapNotify {
                                sequence: event_sequence,
                                event: window,
                                window,
                                from_configure: false,
                            },
                        )]
                    } else {
                        Vec::new()
                    }
                }
            };

            write_x11_control_records(&stream, byte_order, &sequence, records)?;
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
fn wait_for_x11_control_runtime(control_runtime_pending: &AtomicUsize) {
    while control_runtime_pending.load(Ordering::Acquire) != 0 {
        std::thread::yield_now();
    }
}

#[cfg(unix)]
fn lock_x11_control_runtime<'a>(
    runtime: &'a Mutex<XAuthorityRuntime>,
    control_runtime_pending: &AtomicUsize,
) -> Result<std::sync::MutexGuard<'a, XAuthorityRuntime>, X11SetupSocketError> {
    control_runtime_pending.fetch_add(1, Ordering::AcqRel);
    let result = runtime.lock();
    control_runtime_pending.fetch_sub(1, Ordering::AcqRel);
    result.map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))
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
fn x11_selected_xi_event_window(
    authority: &crate::XInputAuthorityState,
    namespace: NamespaceId,
    owner: u64,
    ancestry: &[XResourceId],
    device: u16,
    event_type: u16,
) -> Option<XResourceId> {
    ancestry
        .iter()
        .find(|window| {
            authority.xi_event_selected(namespace, owner, **window, device, event_type)
        })
        .copied()
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
struct X11XiPointerDelivery {
    window: XResourceId,
    child: XResourceId,
    event_x: i16,
    event_y: i16,
    ancestry_depth: usize,
}

#[cfg(unix)]
fn x11_xi_pointer_delivery(
    selections: &XCoreEventSelectionState,
    surface_window: XResourceId,
    event_ancestry: &[XResourceId],
    selected_window: Option<XResourceId>,
    event_x: i16,
    event_y: i16,
) -> Option<X11XiPointerDelivery> {
    let window = selected_window?;
    let selected_index = event_ancestry
        .iter()
        .position(|candidate| *candidate == window)?;
    let child = selected_index
        .checked_sub(1)
        .and_then(|index| event_ancestry.get(index).copied())
        .unwrap_or(XResourceId::NONE);
    let (event_x, event_y) = selections.pointer_event_coordinates(
        surface_window,
        window,
        event_x,
        event_y,
    );
    Some(X11XiPointerDelivery {
        window,
        child,
        event_x,
        event_y,
        ancestry_depth: selected_index,
    })
}

#[cfg(unix)]
fn x11_pointer_surface_window(
    target_window: Option<XResourceId>,
    surface: SurfaceId,
    surface_windows: &Mutex<BTreeMap<SurfaceId, XResourceId>>,
) -> Result<Option<XResourceId>, X11SetupSocketError> {
    if target_window.is_some() {
        return Ok(target_window);
    }
    Ok(surface_windows
        .lock()
        .map_err(|_| X11SetupSocketError::new("X11 surface/window map lock poisoned"))?
        .get(&surface)
        .copied())
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
    input_authority: Option<Arc<Mutex<crate::XInputAuthorityState>>>,
    namespace: NamespaceId,
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
        input_authority,
        namespace,
        client,
    } = state;
    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = stop.clone();
    let thread = std::thread::spawn(move || {
        let mut focus_sent_to = None;
        let mut pointer_sent_to = None;
        while !writer_stop.load(Ordering::Acquire) {
            let (
                event,
                target_window,
                mut xi_event_type,
                mut xi_event_window,
                mut xi_emulated_button_type,
                mut xi_emulated_button_window,
                xi_transition_mask,
                delivery,
            ) =
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
                if x11_keyboard_route_ready(
                    matches!(event, XAuthorityInputEvent::Key(_)),
                    xi_event_type.is_some(),
                    focused_selected.is_some() || routed_selected.is_some(),
                    std::time::Instant::now() >= keyboard_deadline,
                ) {
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
            let (delivered_window, pointer_surface_window, pointer_event_ancestry) = match event {
                XAuthorityInputEvent::Key(_) => {
                    (routed_keyboard_window.unwrap_or(focused_window), None, None)
                }
                XAuthorityInputEvent::Pointer(pointer) => {
                    let Some(surface_window) = x11_pointer_surface_window(
                        target_window,
                        pointer.surface,
                        &surface_windows,
                    )? else {
                        receiver.send_delivery(
                            client,
                            delivery,
                            XAuthorityInputDeliveryOutcome::TargetGone,
                        )?;
                        tracing::debug!(
                            "sophia_x11_input_delivery schema=1 status=target_gone event=pointer client={} input_redacted=true",
                            client.raw(),
                        );
                        continue;
                    };
                    let selections = core_event_selections
                        .lock()
                        .map_err(|_| {
                            X11SetupSocketError::new("X11 core event selection lock poisoned")
                        })?;
                    let event_window = selections.pointer_event_target(
                        surface_window,
                        pointer.event_x,
                        pointer.event_y,
                    );
                    let event_ancestry = selections.ancestry_including(event_window);
                    let delivered_window = selections
                        .selected_pointer_target(
                            surface_window,
                            matches!(pointer.kind, XAuthorityPointerEventKind::Motion),
                            pointer.event_x,
                            pointer.event_y,
                        )
                        .unwrap_or(surface_window);
                    (delivered_window, Some(surface_window), Some(event_ancestry))
                }
            };
            if let (Some(input_authority), Some(event_ancestry)) =
                (input_authority.as_ref(), pointer_event_ancestry.as_ref())
            {
                let authority = input_authority.lock().map_err(|_| {
                    X11SetupSocketError::new("X11 input authority lock poisoned")
                })?;
                let (selected_type, emulated_button_selected_type) = match event {
                    XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                        kind: XAuthorityPointerEventKind::Motion,
                        ..
                    }) => (Some(6), None),
                    XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                        kind: XAuthorityPointerEventKind::Button { pressed, .. },
                        ..
                    }) => (Some(if pressed { 4 } else { 5 }), None),
                    XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                        kind:
                            XAuthorityPointerEventKind::Axis {
                                pressed,
                                horizontal_position_v120,
                                vertical_position_v120,
                                ..
                            },
                        ..
                    }) => (
                        (horizontal_position_v120.is_some()
                            || vertical_position_v120.is_some())
                        .then_some(6),
                        Some(if pressed { 4 } else { 5 }),
                    ),
                    XAuthorityInputEvent::Key(_) => (None, None),
                };
                xi_event_window = selected_type.and_then(|event_type| {
                    x11_selected_xi_event_window(
                        &authority,
                        namespace,
                        client.raw(),
                        event_ancestry,
                        2,
                        event_type,
                    )
                });
                xi_event_type = xi_event_window.and(selected_type);
                xi_emulated_button_window =
                    emulated_button_selected_type.and_then(|event_type| {
                        x11_selected_xi_event_window(
                            &authority,
                            namespace,
                            client.raw(),
                            event_ancestry,
                            2,
                            event_type,
                        )
                    });
                xi_emulated_button_type =
                    xi_emulated_button_window.and(emulated_button_selected_type);
            }
            let (wire_event_x, wire_event_y) = match (event, pointer_surface_window) {
                (XAuthorityInputEvent::Pointer(pointer), Some(surface_window)) => {
                    core_event_selections
                        .lock()
                        .map_err(|_| {
                            X11SetupSocketError::new("X11 core event selection lock poisoned")
                        })?
                        .pointer_event_coordinates(
                            surface_window,
                            delivered_window,
                            pointer.event_x,
                            pointer.event_y,
                        )
                }
                _ => (0, 0),
            };
            let (xi_delivery, xi_emulated_button_delivery) = match (
                event,
                pointer_surface_window,
                pointer_event_ancestry.as_deref(),
            ) {
                (
                    XAuthorityInputEvent::Pointer(pointer),
                    Some(surface_window),
                    Some(event_ancestry),
                ) => {
                    let selections = core_event_selections.lock().map_err(|_| {
                        X11SetupSocketError::new("X11 core event selection lock poisoned")
                    })?;
                    (
                        x11_xi_pointer_delivery(
                            &selections,
                            surface_window,
                            event_ancestry,
                            xi_event_window,
                            pointer.event_x,
                            pointer.event_y,
                        ),
                        x11_xi_pointer_delivery(
                            &selections,
                            surface_window,
                            event_ancestry,
                            xi_emulated_button_window,
                            pointer.event_x,
                            pointer.event_y,
                        ),
                    )
                }
                _ => (None, None),
            };
            if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some()
                && let XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                    kind:
                        XAuthorityPointerEventKind::Axis {
                            pressed,
                            horizontal_position_v120,
                            vertical_position_v120,
                            ..
                        },
                    state,
                    ..
                }) = event
            {
                let descendant_target = pointer_event_ancestry
                    .as_ref()
                    .and_then(|ancestry| ancestry.first())
                    .zip(pointer_surface_window)
                    .is_some_and(|(event_window, surface_window)| *event_window != surface_window);
                tracing::info!(
                    "sophia_x11_axis_delivery schema=2 descendant_target={} smooth_selected={} smooth_child={} smooth_depth={} emulated_button_selected={} emulated_button_child={} emulated_button_depth={} pressed={} horizontal_v120={} vertical_v120={} physical_buttons={} input_redacted=true",
                    descendant_target,
                    xi_event_type == Some(6),
                    xi_delivery.is_some_and(|delivery| delivery.child != XResourceId::NONE),
                    xi_delivery.map_or(0, |delivery| delivery.ancestry_depth),
                    xi_emulated_button_type.is_some(),
                    xi_emulated_button_delivery
                        .is_some_and(|delivery| delivery.child != XResourceId::NONE),
                    xi_emulated_button_delivery.map_or(0, |delivery| delivery.ancestry_depth),
                    pressed,
                    horizontal_position_v120.unwrap_or(0),
                    vertical_position_v120.unwrap_or(0),
                    state >> 8,
                );
            }
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
                        surface: _,
                        root_x,
                        root_y,
                        event_x: _,
                        event_y: _,
                        state,
                        time_msec,
                    }) => XClientEvent::PointerMotion {
                        sequence: 0,
                        time: time_msec,
                        root,
                        event: delivered_window,
                        root_x,
                        root_y,
                        event_x: wire_event_x,
                        event_y: wire_event_y,
                        state,
                    },
                    XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                        kind: XAuthorityPointerEventKind::Button { button, pressed },
                        surface: _,
                        root_x,
                        root_y,
                        event_x: _,
                        event_y: _,
                        state,
                        time_msec,
                    }) => XClientEvent::PointerButton {
                        sequence: 0,
                        pressed,
                        button,
                        time: time_msec,
                        root,
                        event: delivered_window,
                        root_x,
                        root_y,
                        event_x: wire_event_x,
                        event_y: wire_event_y,
                        state,
                    },
                    XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                        kind: XAuthorityPointerEventKind::Axis {
                            button, pressed, ..
                        },
                        surface: _,
                        root_x,
                        root_y,
                        event_x: _,
                        event_y: _,
                        state,
                        time_msec,
                    }) => XClientEvent::PointerButton {
                        sequence: 0,
                        pressed,
                        button,
                        time: time_msec,
                        root,
                        event: delivered_window,
                        root_x,
                        root_y,
                        event_x: wire_event_x,
                        event_y: wire_event_y,
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
                    if let XAuthorityInputEvent::Pointer(pointer) = event {
                        let selections = core_event_selections.lock().map_err(|_| {
                            X11SetupSocketError::new("X11 core event selection lock poisoned")
                        })?;
                        if let Some(previous) = previous
                            && selections.crossing_selected(previous, false)
                        {
                            stream
                                .write_all(&encode_x_client_event(
                                    byte_order,
                                    XClientEvent::PointerCrossing {
                                        sequence,
                                        entered: false,
                                        detail: 3,
                                        time: pointer.time_msec,
                                        root,
                                        event: previous,
                                        root_x: pointer.root_x,
                                        root_y: pointer.root_y,
                                        event_x: wire_event_x,
                                        event_y: wire_event_y,
                                        state: pointer.state,
                                        mode: 0,
                                        focus: true,
                                    },
                                ))
                                .map_err(|error| {
                                    X11SetupSocketError::new(format!(
                                        "failed to write X11 LeaveNotify event: {error}"
                                    ))
                                })?;
                        }
                        if selections.crossing_selected(delivered_window, true) {
                            stream
                                .write_all(&encode_x_client_event(
                                    byte_order,
                                    XClientEvent::PointerCrossing {
                                        sequence,
                                        entered: true,
                                        detail: 3,
                                        time: pointer.time_msec,
                                        root,
                                        event: delivered_window,
                                        root_x: pointer.root_x,
                                        root_y: pointer.root_y,
                                        event_x: wire_event_x,
                                        event_y: wire_event_y,
                                        state: pointer.state,
                                        mode: 0,
                                        focus: true,
                                    },
                                ))
                                .map_err(|error| {
                                    X11SetupSocketError::new(format!(
                                        "failed to write X11 EnterNotify event: {error}"
                                    ))
                                })?;
                        }
                        drop(selections);
                    }
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
                if let (
                    Some(surface_window),
                    XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                        kind,
                        root_x,
                        root_y,
                        event_x,
                        event_y,
                        state,
                        ..
                    }),
                ) = (pointer_surface_window, event)
                {
                    let mask = match kind {
                        XAuthorityPointerEventKind::Motion => state,
                        XAuthorityPointerEventKind::Button { button, pressed }
                        | XAuthorityPointerEventKind::Axis {
                            button, pressed, ..
                        } => {
                            let button_mask = (button <= 5)
                                .then_some(1_u16 << (u32::from(button) + 7))
                                .unwrap_or(0);
                            if pressed {
                                state | button_mask
                            } else {
                                state & !button_mask
                            }
                        }
                    };
                    core_event_selections
                        .lock()
                        .map_err(|_| {
                            X11SetupSocketError::new("X11 core event selection lock poisoned")
                        })?
                        .observe_pointer(
                            surface_window,
                            pointer_event_ancestry
                                .as_ref()
                                .and_then(|ancestry| ancestry.first())
                                .copied()
                                .unwrap_or(delivered_window),
                            root_x,
                            root_y,
                            event_x,
                            event_y,
                            mask,
                        );
                }
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
                    let delivery = xi_delivery.unwrap_or(X11XiPointerDelivery {
                        window: xi_event_window.unwrap_or(delivered_window),
                        child: XResourceId::NONE,
                        event_x: wire_event_x,
                        event_y: wire_event_y,
                        ancestry_depth: 0,
                    });
                    // The smooth valuator event is the physical source event. Only
                    // its compatibility button companion is pointer-emulated.
                    let generic = encode_xi_device_event(
                        byte_order,
                        sequence,
                        event_type,
                        event,
                        delivery.window,
                        delivery.child,
                        delivery.event_x,
                        delivery.event_y,
                        0,
                    );
                    stream.write_all(&generic).map_err(|error| {
                        X11SetupSocketError::new(format!(
                            "failed to write XI2 generic event: {error}"
                        ))
                    })?;
                }
                if let Some(event_type) = xi_emulated_button_type {
                    let delivery = xi_emulated_button_delivery.unwrap_or(X11XiPointerDelivery {
                        window: xi_emulated_button_window.unwrap_or(delivered_window),
                        child: XResourceId::NONE,
                        event_x: wire_event_x,
                        event_y: wire_event_y,
                        ancestry_depth: 0,
                    });
                    let generic = encode_xi_device_event(
                        byte_order,
                        sequence,
                        event_type,
                        event,
                        delivery.window,
                        delivery.child,
                        delivery.event_x,
                        delivery.event_y,
                        XI_POINTER_EMULATED,
                    );
                    stream.write_all(&generic).map_err(|error| {
                        X11SetupSocketError::new(format!(
                            "failed to write emulated XI2 wheel-button event: {error}"
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

#[cfg(unix)]
fn x11_keyboard_route_ready(
    is_key: bool,
    xi_selected: bool,
    core_selected: bool,
    deadline_elapsed: bool,
) -> bool {
    !is_key || xi_selected || core_selected || deadline_elapsed
}
