pub fn run_x11_core_socket_server_once(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
) -> Result<(), X11SetupSocketError> {
    run_x11_core_socket_server_once_observed(path, namespace, |_| {})
}

/// Runs one X11 authority listener until its enclosing process is stopped.
///
/// Clients are served sequentially and share one authority state. Concurrent
/// multi-client dispatch and client-specific resource allocation remain a
/// separate milestone.
#[cfg(unix)]
pub fn run_x11_core_socket_server(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
) -> Result<(), X11SetupSocketError> {
    run_x11_core_socket_server_observed(path, namespace, |_| {})
}
#[cfg(unix)]
pub fn run_x11_core_socket_server_observed(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    mut observer: impl FnMut(&XDispatchResult),
) -> Result<(), X11SetupSocketError> {
    run_x11_core_socket_server_traced(path, namespace, move |trace| {
        observer(&trace.result);
        Ok(())
    })
}
pub fn run_x11_core_socket_server_traced(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    observer: impl FnMut(X11DispatchObservation) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let config = XServerFrontendConfig::new(path.as_ref(), namespace)?;
    let mut frontend = XServerFrontend::bind(config)?;
    frontend.serve_forever_traced(observer)
}

#[cfg(unix)]
pub fn run_x11_core_socket_server_channel(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    sender: SyncSender<XAuthorityObservedTransactionBatch>,
) -> Result<(), X11SetupSocketError> {
    run_x11_core_socket_server_traced(path, namespace, move |trace| {
        try_emit_x_authority_observation(&sender, &trace)
            .map_err(|error| X11SetupSocketError::new(error.to_string()))?;
        Ok(())
    })
}

#[cfg(unix)]
pub fn run_x11_core_socket_server_once_observed(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    mut observer: impl FnMut(&XDispatchResult),
) -> Result<(), X11SetupSocketError> {
    run_x11_core_socket_server_once_traced(path, namespace, move |trace| {
        let result = trace.result;
        observer(&result);
        Ok(())
    })
}

#[cfg(unix)]
pub fn run_x11_core_socket_server_once_traced(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    observer: impl FnMut(X11DispatchObservation) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    run_x11_core_socket_server_once_with_trace_observer(path, namespace, None, observer)
}

#[cfg(unix)]
pub fn run_x11_core_socket_server_once_traced_with_idle_timeout(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    idle_timeout: Duration,
    observer: impl FnMut(X11DispatchObservation) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    run_x11_core_socket_server_once_with_trace_observer(
        path,
        namespace,
        Some(idle_timeout),
        observer,
    )
}

/// Runs one bounded client against an explicitly assembled frontend config.
///
/// This retains the external-probe idle timeout while allowing a caller to
/// inject backend-owned capabilities such as the DRI3 render-device provider.
#[cfg(unix)]
pub fn run_x11_core_socket_server_once_config_traced_with_idle_timeout(
    config: XServerFrontendConfig,
    idle_timeout: Duration,
    observer: impl FnMut(X11DispatchObservation) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let listener = bind_x11_core_socket_server(config.socket_path())?;
    let state = X11CoreSocketServerState::with_output_topology_and_xkb_config(
        config.output_topology().clone(),
        config.xkb_config(),
    )?
    .with_optional_render_device_provider(config.render_device_provider());
    serve_x11_core_socket_listener_once_with_setup_authorization(
        &listener,
        config.namespace(),
        &state,
        config.setup_authorization(),
        config.admission_policy(),
        Some(idle_timeout),
        observer,
    )
}
pub fn run_x11_core_socket_server_once_channel(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    sender: SyncSender<XAuthorityObservedTransactionBatch>,
) -> Result<(), X11SetupSocketError> {
    run_x11_core_socket_server_once_with_trace_observer(path, namespace, None, move |trace| {
        try_emit_x_authority_observation(&sender, &trace)
            .map_err(|error| X11SetupSocketError::new(error.to_string()))?;
        Ok(())
    })
}

#[cfg(unix)]
pub fn run_x11_core_socket_server_once_channels(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    transaction_sender: SyncSender<XAuthorityObservedTransactionBatch>,
    input_receiver: Receiver<XAuthorityInputEvent>,
) -> Result<(), X11SetupSocketError> {
    let listener = bind_x11_core_socket_server(path)?;
    let (mut stream, _) = listener.accept().map_err(|error| {
        X11SetupSocketError::new(format!("failed to accept X11 core client: {error}"))
    })?;
    let state = X11CoreSocketServerState::new();
    serve_x11_core_socket_client_with_trace_observer_and_input(
        &mut stream,
        namespace,
        &state,
        X11ClientConnectionInputs {
            input_receiver: Some(X11InputEventReceiver::Plain(input_receiver)),
            control_channels: None,
            client_routing: None,
        },
        X11ClientAdmissionContext {
            authorization: &XServerFrontendSetupAuthorization::default(),
            admission_policy: None,
            worker_admission: None,
        },
        move |trace| {
            try_emit_x_authority_observation(&transaction_sender, &trace)
                .map_err(|error| X11SetupSocketError::new(error.to_string()))?;
            Ok(())
        },
    )
}

#[cfg(unix)]
pub fn run_x11_core_socket_server_once_session_channels(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    transaction_sender: SyncSender<XAuthorityObservedTransactionBatch>,
    input_receiver: Receiver<XAuthorityClientInputEvent>,
    control_receiver: Receiver<XAuthorityClientControlCommand>,
    control_ack_sender: SyncSender<XAuthorityClientControlAck>,
) -> Result<(), X11SetupSocketError> {
    let listener = bind_x11_core_socket_server(path)?;
    let (mut stream, _) = listener.accept().map_err(|error| {
        X11SetupSocketError::new(format!("failed to accept X11 core client: {error}"))
    })?;
    let state = X11CoreSocketServerState::new();
    serve_x11_core_socket_client_with_trace_observer_and_input(
        &mut stream,
        namespace,
        &state,
        X11ClientConnectionInputs {
            input_receiver: Some(X11InputEventReceiver::Routed {
                receiver: input_receiver,
                deliveries: None,
            }),
            control_channels: Some(X11ControlChannels::Routed {
                receiver: control_receiver,
                acknowledgements: control_ack_sender,
            }),
            client_routing: None,
        },
        X11ClientAdmissionContext {
            authorization: &XServerFrontendSetupAuthorization::default(),
            admission_policy: None,
            worker_admission: None,
        },
        move |trace| {
            try_emit_x_authority_observation(&transaction_sender, &trace)
                .map_err(|error| X11SetupSocketError::new(error.to_string()))?;
            Ok(())
        },
    )
}

/// Runs one routed concurrent X11 client until it disconnects.
///
/// The caller owns the broker's input/control senders and must stop producing
/// routes before joining this helper. This is the migration bridge from the
/// single-client live-session transport to the general bounded concurrent
/// frontend service: the connection uses the same private worker queues as a
/// multi-client frontend, while this helper intentionally accepts only one
/// client.
#[cfg(unix)]
pub fn run_x11_core_socket_server_once_routed(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    transaction_sender: SyncSender<XAuthorityObservedTransactionBatch>,
    mut broker: XServerFrontendRouteBroker,
) -> Result<(), X11SetupSocketError> {
    let config = XServerFrontendConfig::new(path.as_ref().to_path_buf(), namespace)?;
    let mut frontend = XServerFrontend::bind(config)?;
    frontend
        .state
        .runtime
        .lock()
        .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?
        .set_input_authority(broker.registry.input_authority.clone());
    let observer: Arc<X11CoreTraceObserver> = Arc::new(move |trace| {
        try_emit_x_authority_observation(&transaction_sender, &trace)
            .map(|_| ())
            .map_err(|error| X11SetupSocketError::new(error.to_string()))
    });
    frontend.serve_next_concurrently_routed_traced(&broker, observer)?;
    while frontend.active_client_worker_count() != 0 {
        let routed = broker
            .route_pending()
            .map_err(|error| X11SetupSocketError::new(error.to_string()))?;
        frontend.poll_client_workers()?;
        if routed == 0 && frontend.active_client_worker_count() != 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    Ok(())
}

/// Runs a bounded routed X11 frontend until supervision stops accepting.
///
/// While accepting, the service starts every ready local connection up to the
/// configured worker limit, routes all pending Engine input/control into the
/// owning worker's private queues, and reaps completed workers. A
/// [`XServerFrontendServiceCommand::StopAccepting`] command closes admission
/// without closing client streams; the service then drains the workers that
/// already exist. The caller remains responsible for its session process
/// policy and should stop producing Engine routes before sending that command.
#[cfg(unix)]
pub fn run_x_server_frontend_routed_until_stopped(
    config: XServerFrontendConfig,
    transaction_sender: SyncSender<XAuthorityObservedTransactionBatch>,
    mut broker: XServerFrontendRouteBroker,
    service_commands: Receiver<XServerFrontendServiceCommand>,
) -> Result<(), X11SetupSocketError> {
    let mut frontend = XServerFrontend::bind(config)?;
    frontend
        .state
        .runtime
        .lock()
        .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?
        .set_input_authority(broker.registry.input_authority.clone());
    let observer: Arc<X11CoreTraceObserver> = Arc::new(move |trace| {
        try_emit_x_authority_observation(&transaction_sender, &trace)
            .map(|_| ())
            .map_err(|error| X11SetupSocketError::new(error.to_string()))
    });
    let mut accepting = true;
    loop {
        let mut progressed = false;
        match service_commands.try_recv() {
            Ok(XServerFrontendServiceCommand::StopAccepting) | Err(TryRecvError::Disconnected) => {
                accepting = false
            }
            Ok(XServerFrontendServiceCommand::RevokeAdmission { admission }) => {
                progressed |= frontend.revoke_admission(admission)?;
            }
            Ok(XServerFrontendServiceCommand::UpdateOutputTopology {
                snapshot,
                acknowledgement,
            }) => {
                let mut outcome = frontend.update_output_topology(snapshot.clone())?;
                if matches!(outcome, XAuthorityOutputUpdateOutcome::Applied { .. }) {
                    let notifications = broker
                        .registry
                        .broadcast_randr_update(&snapshot)
                        .map_err(|error| X11SetupSocketError::new(error.to_string()))?;
                    if let XAuthorityOutputUpdateOutcome::Applied {
                        notifications: delivered,
                        ..
                    } = &mut outcome
                    {
                        *delivered = notifications;
                    }
                }
                acknowledgement.try_send(outcome).map_err(|error| {
                    X11SetupSocketError::new(format!(
                        "failed to return Engine output topology acknowledgement: {error}"
                    ))
                })?;
                progressed = true;
            }
            Err(TryRecvError::Empty) => {}
        }

        if accepting {
            while frontend.active_client_worker_count()
                < frontend.config().max_concurrent_clients().get()
            {
                if !frontend.try_serve_next_concurrently_routed_traced(&broker, observer.clone())? {
                    break;
                }
                progressed = true;
            }
            let routed = broker
                .route_pending()
                .map_err(|error| X11SetupSocketError::new(error.to_string()))?;
            progressed |= routed != 0;
        }
        let workers_before_reap = frontend.active_client_worker_count();
        frontend.poll_client_workers()?;
        progressed |= workers_before_reap != frontend.active_client_worker_count();

        if !accepting && frontend.active_client_worker_count() == 0 {
            return Ok(());
        }
        if !progressed {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

/// Convenience form of [`run_x_server_frontend_routed_until_stopped`] for an
/// unauthenticated local socket using the default frontend configuration.
#[cfg(unix)]
pub fn run_x11_core_socket_server_routed_until_stopped(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    transaction_sender: SyncSender<XAuthorityObservedTransactionBatch>,
    broker: XServerFrontendRouteBroker,
    service_commands: Receiver<XServerFrontendServiceCommand>,
) -> Result<(), X11SetupSocketError> {
    run_x_server_frontend_routed_until_stopped(
        XServerFrontendConfig::new(path.as_ref().to_path_buf(), namespace)?,
        transaction_sender,
        broker,
        service_commands,
    )
}

#[cfg(unix)]
fn run_x11_core_socket_server_once_with_trace_observer(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    idle_timeout: Option<Duration>,
    observer: impl FnMut(X11DispatchObservation) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let listener = bind_x11_core_socket_server(path)?;
    let mut state = X11CoreSocketServerState::new();
    let authorization = XServerFrontendSetupAuthorization::default();
    serve_x11_core_socket_listener_once_with_setup_authorization(
        &listener,
        namespace,
        &mut state,
        &authorization,
        None,
        idle_timeout,
        observer,
    )
}

#[cfg(unix)]
pub fn bind_x11_core_socket_server(
    path: impl AsRef<Path>,
) -> Result<UnixListener, X11SetupSocketError> {
    let path = path.as_ref();
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => {
            std::fs::remove_file(path).map_err(|error| {
                X11SetupSocketError::new(format!(
                    "failed to remove stale X11 core socket {}: {error}",
                    path.display()
                ))
            })?;
        }
        Ok(_) => {
            return Err(X11SetupSocketError::new(format!(
                "refusing to replace non-socket X11 core path {}",
                path.display()
            )));
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(X11SetupSocketError::new(format!(
                "failed to inspect X11 core socket {}: {error}",
                path.display()
            )));
        }
    }

    let listener = UnixListener::bind(path).map_err(|error| {
        X11SetupSocketError::new(format!(
            "failed to bind X11 core socket {}: {error}",
            path.display()
        ))
    })?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|error| {
        X11SetupSocketError::new(format!(
            "failed to restrict X11 core socket {} to its owner: {error}",
            path.display()
        ))
    })?;
    Ok(listener)
}

#[cfg(unix)]
pub fn serve_x11_core_socket_listener_once(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
) -> Result<(), X11SetupSocketError> {
    serve_x11_core_socket_listener_once_traced(listener, namespace, state, |_| Ok(()))
}

#[cfg(unix)]
pub fn serve_x11_core_socket_listener_once_traced(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
    observer: impl FnMut(X11DispatchObservation) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let authorization = XServerFrontendSetupAuthorization::default();
    serve_x11_core_socket_listener_once_with_setup_authorization(
        listener,
        namespace,
        state,
        &authorization,
        None,
        None,
        observer,
    )
}

#[cfg(unix)]
pub fn serve_x11_core_socket_listener(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
) -> Result<(), X11SetupSocketError> {
    serve_x11_core_socket_listener_traced(listener, namespace, state, |_| Ok(()))
}

#[cfg(unix)]
pub fn serve_x11_core_socket_listener_traced(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
    observer: impl FnMut(X11DispatchObservation) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let authorization = XServerFrontendSetupAuthorization::default();
    serve_x11_core_socket_listener_with_setup_authorization(
        listener,
        namespace,
        state,
        &authorization,
        None,
        observer,
    )
}

#[cfg(unix)]
fn serve_x11_core_socket_listener_with_setup_authorization(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
    authorization: &XServerFrontendSetupAuthorization,
    admission_policy: Option<Arc<dyn XServerFrontendAdmissionPolicy>>,
    mut observer: impl FnMut(X11DispatchObservation) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    loop {
        serve_x11_core_socket_listener_once_with_setup_authorization(
            listener,
            namespace,
            state,
            authorization,
            admission_policy.clone(),
            None,
            &mut observer,
        )?;
    }
}

#[cfg(unix)]
fn serve_x11_core_socket_listener_once_with_setup_authorization(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
    authorization: &XServerFrontendSetupAuthorization,
    admission_policy: Option<Arc<dyn XServerFrontendAdmissionPolicy>>,
    idle_timeout: Option<Duration>,
    observer: impl FnMut(X11DispatchObservation) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let (mut stream, _) = listener.accept().map_err(|error| {
        X11SetupSocketError::new(format!("failed to accept X11 core client: {error}"))
    })?;
    if let Some(timeout) = idle_timeout {
        stream.set_read_timeout(Some(timeout)).map_err(|error| {
            X11SetupSocketError::new(format!("failed to set X11 core read timeout: {error}"))
        })?;
    }
    serve_x11_core_socket_client_with_trace_observer_and_setup_authorization(
        &mut stream,
        namespace,
        state,
        authorization,
        admission_policy,
        observer,
    )
}

#[cfg(unix)]
pub fn serve_x11_setup_socket_client(
    stream: &mut UnixStream,
) -> Result<XSetupRequest, X11SetupSocketError> {
    serve_x11_setup_socket_client_with_root_size(
        stream,
        Size {
            width: i32::from(crate::X_SETUP_ROOT_WIDTH),
            height: i32::from(crate::X_SETUP_ROOT_HEIGHT),
        },
    )
}

#[cfg(unix)]
pub fn serve_x11_setup_socket_client_with_root_size(
    stream: &mut UnixStream,
    root_size: Size,
) -> Result<XSetupRequest, X11SetupSocketError> {
    let authorization = XServerFrontendSetupAuthorization::default();
    serve_x11_setup_socket_client_with_setup_authorization(stream, &authorization, |_| {
        let mut success = XSetupSuccess::client_compatible();
        success.root_size = root_size;
        Ok(Some(success))
    })?
    .map(|(request, _)| request)
    .ok_or_else(|| {
        X11SetupSocketError::new("default X11 setup authorization unexpectedly rejected")
    })
}

#[cfg(unix)]
fn serve_x11_setup_socket_client_with_setup_authorization(
    stream: &mut UnixStream,
    authorization: &XServerFrontendSetupAuthorization,
    setup_success: impl FnOnce(&XSetupRequest) -> Result<Option<XSetupSuccess>, X11SetupSocketError>,
) -> Result<Option<(XSetupRequest, XSetupSuccess)>, X11SetupSocketError> {
    let request = read_x11_setup_request(stream)?;
    if !authorization.permits(&request) {
        write_x11_setup_failure(
            stream,
            request.byte_order,
            b"Sophia X11 authorization failed",
        )?;
        return Ok(None);
    }
    let Some(setup_success) = setup_success(&request)? else {
        write_x11_setup_failure(stream, request.byte_order, b"Sophia X11 admission failed")?;
        return Ok(None);
    };
    let response =
        encode_x11_setup_success(request.byte_order, &setup_success).map_err(|error| {
            X11SetupSocketError::new(format!("failed to encode X11 setup success: {error}"))
        })?;
    stream
        .write_all(&response)
        .map_err(|error| X11SetupSocketError::new(format!("failed to write X11 setup: {error}")))?;
    stream
        .flush()
        .map_err(|error| X11SetupSocketError::new(format!("failed to flush X11 setup: {error}")))?;
    Ok(Some((request, setup_success)))
}

#[cfg(unix)]
fn write_x11_setup_failure(
    stream: &mut UnixStream,
    byte_order: XByteOrder,
    reason: &[u8],
) -> Result<(), X11SetupSocketError> {
    let response =
        encode_x11_setup_failure(byte_order, &XSetupFailure::new(reason)).map_err(|error| {
            X11SetupSocketError::new(format!("failed to encode X11 setup failure: {error}"))
        })?;
    stream.write_all(&response).map_err(|error| {
        X11SetupSocketError::new(format!("failed to write X11 setup failure: {error}"))
    })?;
    stream.flush().map_err(|error| {
        X11SetupSocketError::new(format!("failed to flush X11 setup failure: {error}"))
    })
}

#[cfg(unix)]
pub fn serve_x11_core_socket_client(
    stream: &mut UnixStream,
    namespace: NamespaceId,
) -> Result<(), X11SetupSocketError> {
    let mut state = X11CoreSocketServerState::new();
    serve_x11_core_socket_client_with_state(stream, namespace, &mut state)
}

#[cfg(unix)]
pub fn serve_x11_core_socket_client_with_state(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
) -> Result<(), X11SetupSocketError> {
    serve_x11_core_socket_client_with_trace_observer(stream, namespace, state, |_| Ok(()))
}

#[cfg(unix)]
pub fn serve_x11_core_socket_client_observed(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    mut observer: impl FnMut(&XDispatchResult),
) -> Result<(), X11SetupSocketError> {
    let mut state = X11CoreSocketServerState::new();
    serve_x11_core_socket_client_with_state_observed(stream, namespace, &mut state, move |result| {
        observer(result);
        Ok(())
    })
}

#[cfg(unix)]
pub fn serve_x11_core_socket_client_with_state_observed(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
    mut observer: impl FnMut(&XDispatchResult) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    serve_x11_core_socket_client_with_trace_observer(stream, namespace, state, move |trace| {
        observer(&trace.result)
    })
}

#[cfg(unix)]
fn serve_x11_core_socket_client_with_trace_observer(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
    observer: impl FnMut(X11DispatchObservation) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let authorization = XServerFrontendSetupAuthorization::default();
    serve_x11_core_socket_client_with_trace_observer_and_setup_authorization(
        stream,
        namespace,
        state,
        &authorization,
        None,
        observer,
    )
}

#[cfg(unix)]
fn serve_x11_core_socket_client_with_trace_observer_and_setup_authorization(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
    authorization: &XServerFrontendSetupAuthorization,
    admission_policy: Option<Arc<dyn XServerFrontendAdmissionPolicy>>,
    observer: impl FnMut(X11DispatchObservation) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    serve_x11_core_socket_client_with_trace_observer_and_input(
        stream,
        namespace,
        state,
        X11ClientConnectionInputs {
            input_receiver: None,
            control_channels: None,
            client_routing: None,
        },
        X11ClientAdmissionContext {
            authorization,
            admission_policy,
            worker_admission: None,
        },
        observer,
    )
}
