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
    client: XServerFrontendClientId,
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
            trace_written_selection_event(client, event);
        }
        Ok(())
    });
    Ok(X11ProtocolEventWriter { stop, thread })
}

#[cfg(unix)]
fn trace_written_selection_event(client: XServerFrontendClientId, event: XClientEvent) {
    if std::env::var_os("SOPHIA_LIVE_SESSION_DIAGNOSTIC").is_none() {
        return;
    }
    match event {
        XClientEvent::SelectionClear {
            sequence,
            time,
            owner,
            selection,
        } => tracing::info!(
            "sophia_x11_selection_delivery schema=1 stage=socket_flushed kind=clear client={} sequence={} time={} owner={} selection={} content=redacted",
            client.raw(),
            sequence,
            time,
            owner.local.raw(),
            selection,
        ),
        XClientEvent::SelectionRequest {
            sequence,
            time,
            owner,
            requestor,
            selection,
            target,
            property,
        } => tracing::info!(
            "sophia_x11_selection_delivery schema=1 stage=socket_flushed kind=request client={} sequence={} time={} owner={} requestor={} selection={} target={} property={} content=redacted",
            client.raw(),
            sequence,
            time,
            owner.local.raw(),
            requestor.local.raw(),
            selection,
            target,
            property,
        ),
        XClientEvent::SelectionNotify {
            sequence,
            synthetic,
            time,
            requestor,
            selection,
            target,
            property,
        } => tracing::info!(
            "sophia_x11_selection_delivery schema=1 stage=socket_flushed kind=notify client={} sequence={} synthetic={} time={} requestor={} selection={} target={} property={} property_present={} content=redacted",
            client.raw(),
            sequence,
            synthetic,
            time,
            requestor.local.raw(),
            selection,
            target,
            property,
            property != crate::X_ATOM_NONE,
        ),
        _ => {}
    }
}

#[cfg(unix)]
fn set_x11_protocol_event_sequence(event: &mut XClientEvent, value: u16) {
    match event {
        XClientEvent::SelectionClear { sequence, .. }
        | XClientEvent::SelectionRequest { sequence, .. }
        | XClientEvent::SelectionNotify { sequence, .. }
        | XClientEvent::PropertyNotify { sequence, .. }
        | XClientEvent::CreateNotify { sequence, .. }
        | XClientEvent::MapNotify { sequence, .. }
        | XClientEvent::UnmapNotify { sequence, .. }
        | XClientEvent::ConfigureNotify { sequence, .. }
        | XClientEvent::VisibilityNotify { sequence, .. }
        | XClientEvent::Expose { sequence, .. }
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
    map_transition: Option<&XCoreMapTransition>,
    present_configure: bool,
    selections: &XCoreEventSelectionState,
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
    let mut records = Vec::with_capacity(
        present_events.len()
            + if admit {
                4 + map_transition.map_or(0, |transition| {
                    transition.promoted_descendants.len().saturating_mul(2)
                })
            } else {
                1
            },
    );
    records.extend(
        present_events
            .into_iter()
            .map(|event| encode_x_client_event(byte_order, event)),
    );
    let mut core_events = Vec::new();
    const EXPOSURE_MASK: u32 = 1 << 15;
    const VISIBILITY_CHANGE_MASK: u32 = 1 << 16;
    const STRUCTURE_NOTIFY_MASK: u32 = 1 << 17;
    if protocol_routing.is_some() || selections.selects(window, STRUCTURE_NOTIFY_MASK) {
        core_events.push(XClientEvent::ConfigureNotify {
            sequence: event_sequence,
            synthetic: false,
            event: window,
            window,
            above_sibling: None,
            x: clamp_engine_i16(geometry.x),
            y: clamp_engine_i16(geometry.y),
            width,
            height,
            border_width: 0,
            override_redirect: false,
        });
    }
    if admit {
        if protocol_routing.is_some() || selections.selects(window, STRUCTURE_NOTIFY_MASK) {
            core_events.push(XClientEvent::MapNotify {
                sequence: event_sequence,
                event: window,
                window,
                override_redirect: false,
            });
        }
        let viewable_windows = std::iter::once(window).chain(
            map_transition
                .into_iter()
                .flat_map(|transition| transition.promoted_descendants.iter().copied()),
        );
        for candidate in viewable_windows.clone() {
            if protocol_routing.is_some() || selections.selects(candidate, VISIBILITY_CHANGE_MASK) {
                core_events.push(XClientEvent::VisibilityNotify {
                    sequence: event_sequence,
                    window: candidate,
                    state: 0,
                });
            }
        }
        for candidate in viewable_windows {
            if protocol_routing.is_none() && !selections.selects(candidate, EXPOSURE_MASK) {
                continue;
            }
            let candidate_geometry = selections.geometry(candidate).unwrap_or(geometry);
            core_events.push(XClientEvent::Expose {
                sequence: event_sequence,
                window: candidate,
                x: 0,
                y: 0,
                width: crate::dispatch::clamp_u16(candidate_geometry.width),
                height: crate::dispatch::clamp_u16(candidate_geometry.height),
                count: 0,
            });
        }
    }
    if let Some(routing) = protocol_routing {
        let mut output = crate::XDispatchResult {
            response: None,
            outputs: core_events
                .into_iter()
                .map(crate::XClientOutput::Event)
                .collect(),
            metadata_candidates: Vec::new(),
        };
        route_core_lifecycle_events(routing, client, &mut output)?;
        core_events = output
            .outputs
            .into_iter()
            .filter_map(|output| match output {
                crate::XClientOutput::Event(event) => Some(event),
                _ => None,
            })
            .collect();
    }
    records.extend(
        core_events
            .into_iter()
            .map(|event| encode_x_client_event(byte_order, event)),
    );
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
    xkb_modifiers: Arc<AtomicU16>,
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
                X11RoutedControl::FocusOut { window, time_msec } => {
                    focused_surface_window.store(
                        u64::from(X_SETUP_DEFAULT_ROOT),
                        Ordering::Release,
                    );
                    let records = x11_focus_records(
                        byte_order,
                        sequence.load(Ordering::Acquire),
                        namespace,
                        client,
                        &core_event_selections,
                        protocol_routing
                            .as_ref()
                            .map(|routing| &routing.input_authority),
                        xkb_modifiers.load(Ordering::Acquire),
                        X11FocusRecordRequest::Event {
                            window,
                            focused: false,
                            time_msec,
                        },
                    )?;
                    write_x11_control_records(
                        &stream,
                        byte_order,
                        &sequence,
                        records,
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
                    let map_transition = selections.observe_mapped(window);
                    if std::env::var_os("SOPHIA_LIVE_SESSION_DIAGNOSTIC").is_some() {
                        tracing::debug!(
                            "sophia_x11_viewability schema=1 status=admitted viewable={} promoted_descendants={}",
                            map_transition.viewable,
                            map_transition.promoted_descendants.len(),
                        );
                    }
                    x11_surface_geometry_records(
                        byte_order,
                        event_sequence,
                        client,
                        window,
                        geometry,
                        true,
                        Some(&map_transition),
                        true,
                        &selections,
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
                    let mut selections = core_event_selections
                        .lock()
                        .map_err(|_| {
                            X11SetupSocketError::new("X11 core event selection lock poisoned")
                        })?;
                    selections.update_geometry(window, geometry);
                    x11_surface_geometry_records(
                        byte_order,
                        event_sequence,
                        client,
                        window,
                        geometry,
                        false,
                        None,
                        previous_geometry != Some(geometry),
                        &selections,
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
                    x11_focus_records(
                        byte_order,
                        event_sequence,
                        namespace,
                        client,
                        &core_event_selections,
                        protocol_routing
                            .as_ref()
                            .map(|routing| &routing.input_authority),
                        xkb_modifiers.load(Ordering::Acquire),
                        X11FocusRecordRequest::Surface {
                            window,
                            previous_authority: previous,
                            previous_routed,
                            transition: focus_transition,
                        },
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
                    x11_focus_records(
                        byte_order,
                        event_sequence,
                        namespace,
                        client,
                        &core_event_selections,
                        protocol_routing
                            .as_ref()
                            .map(|routing| &routing.input_authority),
                        xkb_modifiers.load(Ordering::Acquire),
                        X11FocusRecordRequest::Clear {
                            root,
                            previous_routed,
                            transition: focus_transition,
                        },
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
fn lock_x11_request_runtime<'a>(
    runtime: &'a Mutex<XAuthorityRuntime>,
    control_runtime_pending: &AtomicUsize,
) -> Result<std::sync::MutexGuard<'a, XAuthorityRuntime>, X11SetupSocketError> {
    loop {
        wait_for_x11_control_runtime(control_runtime_pending);
        let runtime = runtime
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?;
        // A control can become pending between the pre-lock check and mutex
        // acquisition. Recheck while holding the lock so that request work
        // cannot overtake an already-waiting focus or configure command.
        if control_runtime_pending.load(Ordering::Acquire) == 0 {
            return Ok(runtime);
        }
        drop(runtime);
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
include!("writers/input.rs");
