#[cfg(unix)]
struct X11ClientConnectionInputs {
    input_receiver: Option<X11InputEventReceiver>,
    control_channels: Option<X11ControlChannels>,
    client_routing: Option<XServerFrontendRouteRegistry>,
}

#[cfg(unix)]
struct X11ClientAdmissionContext<'a> {
    authorization: &'a XServerFrontendSetupAuthorization,
    admission_policy: Option<Arc<dyn XServerFrontendAdmissionPolicy>>,
    worker_admission: Option<(u64, Sender<X11CoreClientWorkerAdmission>)>,
}

#[cfg(unix)]
fn serve_x11_core_socket_client_with_trace_observer_and_input(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
    inputs: X11ClientConnectionInputs,
    admission: X11ClientAdmissionContext<'_>,
    mut observer: impl FnMut(X11DispatchObservation) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let X11ClientConnectionInputs {
        input_receiver,
        control_channels,
        client_routing,
    } = inputs;
    let X11ClientAdmissionContext {
        authorization,
        admission_policy,
        worker_admission,
    } = admission;
    let peer_credentials = if admission_policy.is_some() {
        x11_peer_credentials(stream)?
    } else {
        None
    };
    let mut setup_lease = None;
    let mut admission_lease = None;
    let mut admission_failure = None;
    let Some((setup, setup_success)) = serve_x11_setup_socket_client_with_setup_authorization(
        stream,
        authorization,
        |setup_request| {
            if let Some(policy) = admission_policy.as_ref() {
                let request = XServerFrontendAdmissionRequest {
                    setup_authentication: authorization.authentication_method(),
                    peer_credentials,
                };
                match policy.admit(request) {
                    Ok(context) if context.is_valid() => {
                        admission_lease =
                            Some(XServerFrontendAdmissionLease::new(policy.clone(), context));
                    }
                    Ok(_) => {
                        admission_failure = Some(XServerFrontendAdmissionError::Unavailable);
                        return Ok(None);
                    }
                    Err(error) => {
                        admission_failure = Some(error);
                        return Ok(None);
                    }
                }
            }
            debug_assert!(authorization.permits(setup_request));
            let (lease, setup_success) = state.next_client_setup_success()?;
            setup_lease = Some(lease);
            Ok(Some(setup_success))
        },
    )?
    else {
        if admission_failure == Some(XServerFrontendAdmissionError::Unavailable) {
            return Err(X11SetupSocketError::new(
                "Sophia X Server Frontend admission policy unavailable",
            ));
        }
        return Ok(());
    };
    let namespace = admission_lease
        .as_ref()
        .map(|lease| lease.context().namespace.id)
        .unwrap_or(namespace);
    let client_lease = setup_lease.ok_or_else(|| {
        X11SetupSocketError::new("Sophia X Server Frontend did not retain a setup client lease")
    })?;
    let client = client_lease.client;
    if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some() {
        tracing::debug!(
            "sophia_x11_client_route schema=1 stage=accepted client={}",
            client.raw()
        );
    }
    let resource_id_range = client_lease.resource_id_range;
    let mut sequence = 0u16;
    let event_sequence = Arc::new(AtomicU16::new(0));
    let focused_surface_window = Arc::new(AtomicU64::new(u64::from(X_SETUP_DEFAULT_ROOT)));
    let core_event_selections = Arc::new(Mutex::new(XCoreEventSelectionState::default()));
    let xkb_state_details = Arc::new(AtomicU16::new(0));
    let xkb_modifiers = Arc::new(AtomicU16::new(0));
    let surface_windows = Arc::new(Mutex::new(BTreeMap::new()));
    let output_stream = Arc::new(Mutex::new(stream.try_clone().map_err(|error| {
        X11SetupSocketError::new(format!("failed to clone X11 output socket: {error}"))
    })?));
    let protocol_routing = client_routing.clone();
    let (route_registration, input_receiver, control_channels, protocol_receiver) =
        if let Some(routing) = client_routing {
            let (registration, channels) = match routing.register_client(client) {
                Ok(registration) => registration,
                Err(error) => {
                    let _ = state.release_client(client);
                    return Err(X11SetupSocketError::new(format!(
                        "failed to register X11 client route: {error}"
                    )));
                }
            };
            (
                Some(registration),
                Some(X11InputEventReceiver::Routed {
                    receiver: channels.input,
                    deliveries: routing.input_delivery_sender.clone(),
                }),
                Some(X11ControlChannels::ClientBound {
                    receiver: channels.control,
                    acknowledgements: routing.acknowledgement_sender.clone(),
                }),
                Some(channels.protocol),
            )
        } else {
            (None, input_receiver, control_channels, None)
        };
    let input_writer = input_receiver
        .map(|receiver| {
            spawn_x11_input_event_writer(
                X11InputWriterState {
                    stream: output_stream.clone(),
                    byte_order: setup.byte_order,
                    sequence: event_sequence.clone(),
                    focused_surface_window: focused_surface_window.clone(),
                    core_event_selections: core_event_selections.clone(),
                    xkb_state_details: xkb_state_details.clone(),
                    xkb_modifiers: xkb_modifiers.clone(),
                    surface_windows: surface_windows.clone(),
                    client,
                },
                receiver,
            )
        })
        .transpose()?;
    let control_writer = control_channels
        .map(|channels| {
            spawn_x11_control_writer(
                output_stream.clone(),
                setup.byte_order,
                event_sequence.clone(),
                focused_surface_window.clone(),
                surface_windows.clone(),
                core_event_selections.clone(),
                state.atoms.clone(),
                state.properties.clone(),
                state.runtime.clone(),
                resource_id_range,
                namespace,
                client,
                protocol_routing.clone(),
                channels,
            )
        })
        .transpose()?;
    let protocol_writer = protocol_receiver
        .map(|receiver| {
            spawn_x11_protocol_event_writer(
                output_stream.clone(),
                setup.byte_order,
                event_sequence.clone(),
                receiver,
            )
        })
        .transpose()?;
    state.register_client(client_lease)?;
    if let Some((worker_id, sender)) = worker_admission
        && let Some(lease) = admission_lease.as_ref()
    {
        let _ = sender.send(X11CoreClientWorkerAdmission {
            worker_id,
            admission: lease.context().client_id,
        });
    }

    let result = (|| {
        // SCM_RIGHTS on a Unix stream is an in-band barrier, but recvmsg can
        // return the descriptors alongside bytes that precede the request
        // which consumes them. Retain those descriptors until the decoded X11
        // request declares its FD arity instead of binding them to the first
        // header returned by recvmsg.
        let mut pending_request_fds = Vec::new();
        while let Some(received) = read_x11_core_request(stream, setup.byte_order)? {
            let major_opcode = received.major_opcode;
            let request = received.bytes;
            let request_minor_code = if major_opcode >= 128 {
                u16::from(request[1])
            } else {
                0
            };
            let ancillary_fds = received.fds;
            let mut received_fds = Vec::new();
            loop {
                let server_owner = state
                    .runtime
                    .lock()
                    .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?
                    .input_authority_mut()
                    .server_owner(namespace);
                if server_owner.is_none_or(|owner| owner == client.raw()) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            sequence = sequence.wrapping_add(1);
            let transaction = state.allocate_transaction()?;
            let dispatch_context = XDispatchContext {
                byte_order: setup.byte_order,
                namespace,
                sequence,
                major_opcode,
                client_id: client.raw(),
            };
            let mut parse_failed = false;
            let mut request_stage = X11ObservedRequestStage::Other;
            let (
                mut output,
                cpu_buffer_update,
                dri3_pixmap_import,
                dri3_fence_import,
                present_submission,
                software_present_submission,
                released_dma_bufs,
                released_fences,
                mut server_reply_fds,
                surface_output_reservations,
            ) = match decode_x11_core_request(
                XWireClientContext {
                    byte_order: setup.byte_order,
                    namespace,
                    transaction,
                    resource_id_range: Some(resource_id_range),
                },
                &request,
            ) {
                Ok(request) => {
                    let required_fd_count = request.required_fd_count();
                    pending_request_fds.extend(ancillary_fds);
                    const MAX_PENDING_REQUEST_FDS: usize = sophia_protocol::DMA_BUF_MAX_PLANES * 16;
                    if pending_request_fds.len() > MAX_PENDING_REQUEST_FDS {
                        return Err(X11SetupSocketError::new(
                            "X11 request stream carried too many pending file descriptors",
                        ));
                    }
                    if required_fd_count != 0 {
                        let take = required_fd_count.min(pending_request_fds.len());
                        received_fds.extend(pending_request_fds.drain(..take));
                    }
                    if required_fd_count != received_fds.len() {
                        return Err(X11SetupSocketError::new(format!(
                            "X11 request opcode {major_opcode} required {} file descriptors but received {}",
                            required_fd_count,
                            received_fds.len()
                        )));
                    }
                    let event_selection = x11_core_event_selection_update(&request);
                    let dri3_open = matches!(&request, crate::XWireRequest::Dri3Open { .. });
                    let dri3_query = matches!(
                        &request,
                        crate::XWireRequest::QueryExtension { name }
                            if name == crate::X_DRI3_EXTENSION_NAME
                    );
                    let dri3_pixmap = match &request {
                        crate::XWireRequest::Dri3PixmapFromBuffer { pixmap, .. }
                        | crate::XWireRequest::Dri3PixmapFromBuffers { pixmap, .. } => {
                            Some(*pixmap)
                        }
                        _ => None,
                    };
                    let dri3_fence_request = match &request {
                        crate::XWireRequest::Dri3FenceFromFd {
                            fence,
                            initially_triggered,
                            ..
                        } => Some((*fence, *initially_triggered)),
                        _ => None,
                    };
                    let freed_pixmap = match &request {
                        crate::XWireRequest::FreePixmap { pixmap } => Some(*pixmap),
                        _ => None,
                    };
                    let destroyed_fence = match &request {
                        crate::XWireRequest::SyncDestroyFence { fence } => Some(*fence),
                        _ => None,
                    };
                    let hierarchy_create = match &request {
                        crate::XWireRequest::CreateWindow { packet, parent, .. } => {
                            match &packet.kind {
                                crate::XAuthorityRequestKind::CreateWindow { window, .. } => {
                                    Some((*window, *parent))
                                }
                                _ => None,
                            }
                        }
                        crate::XWireRequest::ReparentWindow { window, parent, .. } => {
                            Some((*window, *parent))
                        }
                        _ => None,
                    };
                    let hierarchy_restack = match &request {
                        crate::XWireRequest::ConfigureWindow {
                            window,
                            sibling,
                            stack_mode,
                            ..
                        } => Some((*window, *sibling, *stack_mode)),
                        _ => None,
                    };
                    let randr_selection = match &request {
                        crate::XWireRequest::RandrSelectInput { window, enable } => {
                            Some((*window, *enable))
                        }
                        _ => None,
                    };
                    let present_selection = match &request {
                        crate::XWireRequest::PresentSelectInput {
                            event_id,
                            window,
                            event_mask,
                        } => Some((*event_id, *window, *event_mask)),
                        _ => None,
                    };
                    let pending_present = match &request {
                        crate::XWireRequest::PresentPixmap {
                            window,
                            pixmap,
                            serial,
                            idle_fence,
                            ..
                        } => Some((*window, *pixmap, *serial, *idle_fence)),
                        _ => None,
                    };
                    let present_request = match &request {
                        crate::XWireRequest::PresentPixmap {
                            wait_fence,
                            idle_fence,
                            x_offset,
                            y_offset,
                            ..
                        } => Some((*wait_fence, *idle_fence, *x_offset, *y_offset)),
                        _ => None,
                    };
                    let xkb_selection = match &request {
                        crate::XWireRequest::XkbSelectEvents {
                            affect_which,
                            clear,
                            select_all,
                            state_details,
                        } => Some((*affect_which, *clear, *select_all, *state_details)),
                        _ => None,
                    };
                    let xkb_get_state = matches!(request, crate::XWireRequest::XkbGetState);
                    let selection_property_read = selection_property_read_trace(&request);
                    let requested_input_focus = match &request {
                        crate::XWireRequest::SetInputFocus { focus, .. } => Some(*focus),
                        _ => None,
                    };
                    let mapped_window = match &request {
                        crate::XWireRequest::Authority(crate::XAuthorityRequestPacket {
                            kind: crate::XAuthorityRequestKind::MapWindow { window, .. },
                            ..
                        }) => Some(*window),
                        _ => None,
                    };
                    let destroyed_window = match &request {
                        crate::XWireRequest::DestroyWindow { window } => Some(*window),
                        _ => None,
                    };
                    let unmapped_window = match &request {
                        crate::XWireRequest::UnmapWindow { window } => Some(*window),
                        _ => None,
                    };
                    let output_reservation_property = match &request {
                        crate::XWireRequest::ChangeProperty(change) => {
                            Some((change.window, change.property))
                        }
                        crate::XWireRequest::DeleteProperty { window, property } => {
                            Some((*window, *property))
                        }
                        _ => None,
                    };
                    let output_reservation_surface =
                        if let Some((window, property)) = output_reservation_property {
                            surface_windows
                                .lock()
                                .map_err(|_| {
                                    X11SetupSocketError::new(
                                        "X11 surface/window map lock poisoned",
                                    )
                                })?
                                .iter()
                                .find_map(|(surface, candidate)| {
                                    (*candidate == window).then_some((*surface, window, property))
                                })
                        } else {
                            None
                        };
                    if let crate::XWireRequest::CreateWindow {
                        packet:
                            crate::XAuthorityRequestPacket {
                                kind:
                                    crate::XAuthorityRequestKind::CreateWindow {
                                        window, surface, ..
                                    },
                                ..
                            },
                        ..
                    } = &request
                    {
                        surface_windows
                            .lock()
                            .map_err(|_| {
                                X11SetupSocketError::new("X11 surface/window map lock poisoned")
                            })?
                            .insert(*surface, *window);
                        if let Some(routing) = protocol_routing.as_ref() {
                            routing
                                .register_surface(client, namespace, *surface, *window)
                                .map_err(|error| {
                                    X11SetupSocketError::new(format!(
                                        "failed to register X11 surface route: {error}"
                                    ))
                                })?;
                        }
                    }
                    request_stage = x11_observed_request_stage(&request);
                    let queued_present = if let Some((window, pixmap, serial, idle_fence)) =
                        pending_present
                        && let Some(routing) = protocol_routing.as_ref()
                    {
                        routing
                            .queue_present(transaction, client, window, pixmap, serial, idle_fence)
                            .map_err(|error| {
                                X11SetupSocketError::client_failure(format!(
                                    "failed to queue Present feedback: {error}"
                                ))
                            })?;
                        true
                    } else {
                        false
                    };
                    let mut runtime = state.runtime.lock().map_err(|_| {
                        X11SetupSocketError::new("X11 authority runtime lock poisoned")
                    })?;
                    let mut atoms = state
                        .atoms
                        .lock()
                        .map_err(|_| X11SetupSocketError::new("X11 atom table lock poisoned"))?;
                    let mut properties = state.properties.lock().map_err(|_| {
                        X11SetupSocketError::new("X11 property table lock poisoned")
                    })?;
                    let released_dma_buf = freed_pixmap.and_then(|pixmap| {
                        runtime
                            .dri3_pixmap_descriptor(namespace, pixmap)
                            .ok()
                            .map(|descriptor| descriptor.handle)
                    });
                    let released_fence = destroyed_fence
                        .and_then(|fence| runtime.dri3_fence_handle(namespace, fence).ok());
                    let mut output = dispatch_x11_wire_request(
                        dispatch_context,
                        request,
                        &mut runtime,
                        &mut atoms,
                        &mut properties,
                    );
                    trace_selection_property_read_result(selection_property_read, &output);
                    if dri3_query && !state.has_render_device_provider() {
                        for client_output in &mut output.outputs {
                            if let crate::XClientOutput::Reply(
                                crate::XClientReply::QueryExtension {
                                    present,
                                    major_opcode,
                                    first_event,
                                    first_error,
                                    ..
                                },
                            ) = client_output
                            {
                                *present = false;
                                *major_opcode = 0;
                                *first_event = 0;
                                *first_error = 0;
                            }
                        }
                    }
                    if xkb_get_state {
                        for client_output in &mut output.outputs {
                            if let crate::XClientOutput::Reply(crate::XClientReply::XkbGetState {
                                modifiers,
                                ..
                            }) = client_output
                            {
                                *modifiers = xkb_modifiers.load(Ordering::Acquire) as u8;
                            }
                        }
                    }
                    if std::env::var_os("SOPHIA_LIVE_SESSION_DIAGNOSTIC").is_some()
                        && request_stage == X11ObservedRequestStage::KeyboardMapping
                    {
                        tracing::debug!(
                            "sophia_x11_keyboard_map schema=1 status=served detail_redacted=true"
                        );
                    }
                    let dispatch_succeeded = !output
                        .outputs
                        .iter()
                        .any(|output| matches!(output, crate::XClientOutput::Error(_)));
                    if dispatch_succeeded {
                        if let Some(focus) = requested_input_focus {
                            focused_surface_window.store(focus.local.raw(), Ordering::Release);
                        }
                        let mut selections = core_event_selections.lock().map_err(|_| {
                            X11SetupSocketError::new("X11 core event selection lock poisoned")
                        })?;
                        if let Some((window, event_mask, do_not_propagate_mask)) = event_selection {
                            selections.update(window, event_mask, do_not_propagate_mask);
                            if let Some(mask) = event_mask
                                && let Some(routing) = protocol_routing.as_ref()
                            {
                                routing.select_core_events(client, window, mask).map_err(
                                    |error| {
                                        X11SetupSocketError::new(format!(
                                            "failed to update core X11 event subscription: {error}"
                                        ))
                                    },
                                )?;
                            }
                        }
                        if let Some((window, parent)) = hierarchy_create {
                            selections.register(window, parent);
                        }
                        if let Some((window, sibling, mode)) = hierarchy_restack {
                            selections.restack(window, sibling, mode);
                        }
                        if let Some(window) = mapped_window
                            && output.response.as_ref().is_some_and(|response| {
                                response.surfaces.iter().any(|surface| surface.mapped)
                            })
                        {
                            selections.observe_mapped(window);
                        }
                        if let Some(window) = unmapped_window {
                            selections.observe_unmapped(window);
                        }
                        if let Some(window) = destroyed_window {
                            selections.remove(window);
                            if let Some(routing) = protocol_routing.as_ref() {
                                routing.remove_core_event_window(window).map_err(|error| {
                                    X11SetupSocketError::new(format!(
                                        "failed to remove core X11 event subscriptions: {error}"
                                    ))
                                })?;
                            }
                        }
                        if let Some((window, mask)) = randr_selection
                            && let Some(routing) = protocol_routing.as_ref()
                        {
                            routing
                                .select_randr_input(client, window, mask)
                                .map_err(|error| {
                                    X11SetupSocketError::new(format!(
                                        "failed to update RandR subscription: {error}"
                                    ))
                                })?;
                        }
                        if let Some((event_id, window, mask)) = present_selection
                            && let Some(routing) = protocol_routing.as_ref()
                        {
                            routing
                                .select_present_input(client, event_id, window, mask)
                                .map_err(|error| {
                                    X11SetupSocketError::new(format!(
                                        "failed to update Present subscription: {error}"
                                    ))
                                })?;
                        }
                        if let Some((affect_which, clear, select_all, state)) = xkb_selection {
                            let mut details = xkb_state_details.load(Ordering::Acquire);
                            if clear & 4 != 0 {
                                details = 0;
                            }
                            if select_all & 4 != 0 {
                                details = u16::MAX;
                            }
                            if affect_which & 4 != 0
                                && let Some((affect, selected)) = state
                            {
                                details = (details & !affect) | (selected & affect);
                            }
                            xkb_state_details.store(details, Ordering::Release);
                        }
                    }
                    if queued_present
                        && !dispatch_succeeded
                        && let Some(routing) = protocol_routing.as_ref()
                    {
                        routing.cancel_present(transaction).map_err(|error| {
                            X11SetupSocketError::new(format!(
                                "failed to cancel rejected Present feedback: {error}"
                            ))
                        })?;
                    }
                    // The CPU update belongs to this dispatch. Keep it under
                    // the runtime lock so a simultaneous client cannot take
                    // an update generated by this request.
                    let cpu_buffer_update = runtime.take_cpu_buffer_update();
                    let dri3_pixmap_import = dri3_pixmap.and_then(|pixmap| {
                        runtime
                            .dri3_pixmap_descriptor(namespace, pixmap)
                            .ok()
                            .map(|descriptor| XAuthorityDri3PixmapImport { pixmap, descriptor })
                    });
                    let dri3_fence_import = dispatch_succeeded
                        .then_some(dri3_fence_request)
                        .flatten()
                        .and_then(|(fence, initially_triggered)| {
                            runtime
                                .dri3_fence_handle(namespace, fence)
                                .ok()
                                .map(|handle| XAuthorityDri3FenceImport {
                                    fence,
                                    handle,
                                    initially_triggered,
                                })
                        });
                    let present_submission = dispatch_succeeded
                        .then_some(present_request)
                        .flatten()
                        .and_then(|(wait_fence, idle_fence, x_offset, y_offset)| {
                            let response = output.response.as_ref()?;
                            let transaction = response.transactions.first()?;
                            let sophia_protocol::BufferSource::DmaBuf { handle } =
                                transaction.target_buffer
                            else {
                                return None;
                            };
                            Some(XAuthorityPresentSubmission {
                                transaction: response.transaction,
                                surface: transaction.surface,
                                buffer: sophia_protocol::BufferHandle::from_raw(handle),
                                x_offset,
                                y_offset,
                                acquire_fence: wait_fence.and_then(|fence| {
                                    runtime.dri3_fence_handle(namespace, fence).ok()
                                }),
                                idle_fence: idle_fence.and_then(|fence| {
                                    runtime.dri3_fence_handle(namespace, fence).ok()
                                }),
                            })
                        });
                    let software_present_submission = dispatch_succeeded
                        .then_some(present_request)
                        .flatten()
                        .and_then(|(wait_fence, idle_fence, _, _)| {
                            let response = output.response.as_ref()?;
                            let transaction = response.transactions.first()?;
                            if !matches!(
                                transaction.target_buffer,
                                sophia_protocol::BufferSource::CpuBuffer { .. }
                            ) {
                                return None;
                            }
                            Some(crate::XAuthoritySoftwarePresentSubmission {
                                transaction: response.transaction,
                                surface: transaction.surface,
                                acquire_fence: wait_fence.and_then(|fence| {
                                    runtime.dri3_fence_handle(namespace, fence).ok()
                                }),
                                idle_fence: idle_fence.and_then(|fence| {
                                    runtime.dri3_fence_handle(namespace, fence).ok()
                                }),
                            })
                        });
                    let mut server_reply_fds = Vec::new();
                    if dispatch_succeeded && dri3_open {
                        match state.open_render_device_fd() {
                            Ok(fd) => server_reply_fds.push(fd),
                            Err(_) => {
                                output.outputs =
                                    vec![crate::XClientOutput::Error(crate::XClientError {
                                        code: crate::XErrorCode::BadImplementation,
                                        sequence,
                                        resource_id: 0,
                                        minor_code: u16::from(crate::X_DRI3_OPEN_MINOR_OPCODE),
                                        major_code: crate::X_DRI3_MAJOR_OPCODE,
                                    })];
                            }
                        }
                    }
                    let surface_output_reservations = dispatch_succeeded
                        .then_some(output_reservation_surface)
                        .flatten()
                        .filter(|(_, _, property)| {
                            matches!(
                                atoms.name(*property),
                                Some(
                                    X_ATOM_NAME_NET_WM_STRUT
                                        | X_ATOM_NAME_NET_WM_STRUT_PARTIAL
                                )
                            )
                        })
                        .map(|(surface, window, _)| SurfaceOutputReservations {
                            surface,
                            reservations: x_output_reservations_for_window(
                                &properties,
                                &atoms,
                                namespace,
                                window,
                                setup.byte_order,
                                Rect {
                                    x: 0,
                                    y: 0,
                                    width: setup_success.root_size.width,
                                    height: setup_success.root_size.height,
                                },
                            ),
                        })
                        .into_iter()
                        .collect();
                    (
                        output,
                        cpu_buffer_update,
                        dri3_pixmap_import,
                        dri3_fence_import,
                        present_submission,
                        software_present_submission,
                        released_dma_buf.into_iter().collect::<Vec<_>>(),
                        released_fence.into_iter().collect::<Vec<_>>(),
                        server_reply_fds,
                        surface_output_reservations,
                    )
                }
                Err(error) => {
                    parse_failed = true;
                    (
                        dispatch_x11_parse_error(dispatch_context, request_minor_code, error),
                        None,
                        None,
                        None,
                        None,
                        None,
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    )
                }
            };
            if let Some(routing) = protocol_routing.as_ref() {
                route_x11_dispatch_protocol_outputs(
                    state,
                    routing,
                    namespace,
                    client,
                    &mut output,
                )?;
            }
            if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some() {
                let replies = output
                    .outputs
                    .iter()
                    .filter(|item| matches!(item, crate::XClientOutput::Reply(_)))
                    .count();
                let errors = output
                    .outputs
                    .iter()
                    .filter(|item| matches!(item, crate::XClientOutput::Error(_)))
                    .count();
                let events = output
                    .outputs
                    .iter()
                    .filter(|item| matches!(item, crate::XClientOutput::Event(_)))
                    .count();
                tracing::debug!(
                    "sophia_x11_dispatch schema=1 sequence={} major={} minor={} request_len={} parse_failed={} detail_redacted={} replies={} errors={} events={} response={}",
                    sequence,
                    major_opcode,
                    request_minor_code,
                    request.len(),
                    parse_failed,
                    request_stage != X11ObservedRequestStage::Other,
                    replies,
                    errors,
                    events,
                    output.response.is_some(),
                );
            }
            let observed_received_fds = received_fds
                .iter()
                .map(OwnedFd::try_clone)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| {
                    X11SetupSocketError::new(format!(
                        "failed to retain received X11 descriptor for observation: {error}"
                    ))
                })?;
            observer(X11DispatchObservation {
                client,
                resource_id_range,
                sequence,
                major_opcode,
                minor_opcode: request_minor_code,
                request_stage,
                failure: parse_failed.then_some(X11ObservedDispatchFailure::ParseRejected),
                result: output.clone(),
                surface_output_reservations,
                cpu_buffer_update: cpu_buffer_update.clone(),
                received_fd_count: received_fds.len(),
                received_fds: observed_received_fds,
                dri3_pixmap_import,
                dri3_fence_import,
                present_submission,
                software_present_submission,
                released_dma_bufs: released_dma_bufs.clone(),
                released_fences: released_fences.clone(),
                server_reply_fd_count: server_reply_fds.len(),
            })?;
            let encoded_outputs = output.encoded_outputs(setup.byte_order);
            {
                let mut output_stream = output_stream
                    .lock()
                    .map_err(|_| X11SetupSocketError::new("X11 output socket lock poisoned"))?;
                if !encoded_outputs.is_empty() || !server_reply_fds.is_empty() {
                    for (index, bytes) in encoded_outputs.into_iter().enumerate() {
                        let fds = if index == 0 {
                            core::mem::take(&mut server_reply_fds)
                        } else {
                            Vec::new()
                        };
                        let record = X11SocketOutputRecord::new(bytes, fds)?;
                        if let Err(error) =
                            write_x11_socket_output_record(&mut output_stream, record)
                        {
                            if is_x11_client_disconnect(&error) {
                                return Ok(());
                            }
                            return Err(X11SetupSocketError::new(format!(
                                "failed to write X11 output: {error}"
                            )));
                        }
                    }
                    debug_assert!(server_reply_fds.is_empty());
                    if let Err(error) = output_stream.flush() {
                        if matches!(
                            error.kind(),
                            ErrorKind::BrokenPipe
                                | ErrorKind::ConnectionReset
                                | ErrorKind::UnexpectedEof
                        ) {
                            return Ok(());
                        }
                        return Err(X11SetupSocketError::new(format!(
                            "failed to flush X11 output: {error}"
                        )));
                    }
                }
                // Publish the request sequence while holding the same lock
                // used by every asynchronous event writer. Otherwise a
                // writer can snapshot the old value, wait behind this reply,
                // and emit a backwards sequence after it.
                event_sequence.store(sequence, Ordering::Release);
            }
        }
        Ok(())
    })();

    let writer_result: Result<(), X11SetupSocketError> = (|| {
        if let Some(writer) = input_writer {
            writer.stop.store(true, Ordering::Release);
            writer.thread.join().map_err(|_| {
                X11SetupSocketError::new("X11 input event writer thread panicked")
            })??;
        }
        if let Some(writer) = control_writer {
            writer.stop.store(true, Ordering::Release);
            writer
                .thread
                .join()
                .map_err(|_| X11SetupSocketError::new("X11 control writer thread panicked"))??;
        }
        if let Some(writer) = protocol_writer {
            writer.stop.store(true, Ordering::Release);
            writer.thread.join().map_err(|_| {
                X11SetupSocketError::new("X11 protocol event writer thread panicked")
            })??;
        }
        Ok(())
    })();
    state
        .runtime
        .lock()
        .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?
        .input_authority_mut()
        .cleanup_owner(client.raw());
    let client_lease = state.release_client(client)?;
    debug_assert_eq!(client_lease.resource_id_range, resource_id_range);
    let release = release_x11_client_lease(state, namespace, client_lease)?;
    if let Some(routing) = protocol_routing.as_ref() {
        for window in &release.destroyed_windows {
            routing
                .remove_core_event_window(*window)
                .map_err(|error| {
                    X11SetupSocketError::new(format!(
                        "failed to remove disconnected X11 event subscriptions: {error}"
                    ))
                })?;
        }
    }
    drop(route_registration);
    let cleanup_observer_result = if release.removed_surfaces.is_empty()
        && release.released_dma_bufs.is_empty()
        && release.released_fences.is_empty()
    {
        Ok(())
    } else {
        sequence = sequence.wrapping_add(1);
        let transaction = state.allocate_transaction()?;
        let mut response = XAuthorityResponsePacket::accepted(transaction);
        response.removed_surfaces = release.removed_surfaces;
        let cleanup = XDispatchResult {
            response: Some(response),
            outputs: Vec::new(),
            metadata_candidates: Vec::new(),
        };
        observer(X11DispatchObservation {
            client,
            resource_id_range,
            sequence,
            major_opcode: 0,
            minor_opcode: 0,
            request_stage: X11ObservedRequestStage::DisconnectCleanup,
            failure: None,
            result: cleanup,
            surface_output_reservations: Vec::new(),
            cpu_buffer_update: None,
            received_fd_count: 0,
            received_fds: Vec::new(),
            dri3_pixmap_import: None,
            dri3_fence_import: None,
            present_submission: None,
            software_present_submission: None,
            released_dma_bufs: release.released_dma_bufs,
            released_fences: release.released_fences,
            server_reply_fd_count: 0,
        })
    };
    let admission_result = admission_lease.as_mut().map_or(Ok(()), |lease| {
        lease.revoke().map_err(|error| {
            X11SetupSocketError::new(format!("failed to revoke X11 client admission: {error}"))
        })
    });
    result?;
    writer_result?;
    cleanup_observer_result?;
    admission_result
}
