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
            .map_err(x_authority_observation_client_error)
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
    broker: XServerFrontendRouteBroker,
    service_commands: Receiver<XServerFrontendServiceCommand>,
) -> Result<(), X11SetupSocketError> {
    run_x_server_frontend_routed_until_stopped_with_backpressure_observer(
        config,
        transaction_sender,
        broker,
        service_commands,
        Arc::new(trace_x_authority_backpressure),
    )
}

/// Runs the routed frontend with an additional value-free backpressure observer.
///
/// Production uses this seam for stable tracing. Tests may wait for an exact
/// transition before exercising shutdown without inferring worker state from
/// socket timing.
#[cfg(unix)]
struct XAuthorityEgressEnvelope {
    transaction: TransactionId,
    batch: Option<XAuthorityObservedTransactionBatch>,
}

#[cfg(unix)]
fn send_x_authority_egress_envelope(
    sender: &SyncSender<XAuthorityEgressEnvelope>,
    mut envelope: XAuthorityEgressEnvelope,
    cancellation: &AtomicBool,
) -> Result<(), X11SetupSocketError> {
    loop {
        if cancellation.load(Ordering::Acquire) {
            return Err(X11SetupSocketError::service_shutdown(
                "authority egress enqueue cancelled",
            ));
        }
        match sender.try_send(envelope) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Full(pending)) => envelope = pending,
            Err(TrySendError::Disconnected(_)) => {
                return Err(X11SetupSocketError::new(
                    "authority egress sequencer disconnected",
                ));
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(unix)]
fn run_x_authority_egress_sequencer(
    receiver: Receiver<XAuthorityEgressEnvelope>,
    sender: SyncSender<XAuthorityObservedTransactionBatch>,
    cancellation: Arc<AtomicBool>,
    telemetry: Arc<XAuthorityBackpressureObserver>,
) -> Result<(), X11SetupSocketError> {
    let mut expected = 1_u64;
    let mut input_disconnected = false;
    let mut pending = BTreeMap::<u64, Option<XAuthorityObservedTransactionBatch>>::new();
    let mut output = None::<(XAuthorityObservedTransactionBatch, Instant)>;
    let mut waiting = BTreeSet::<u64>::new();
    loop {
        loop {
            match receiver.try_recv() {
                Ok(envelope) => {
                    let ticket = envelope.transaction.raw();
                    if ticket < expected || pending.insert(ticket, envelope.batch).is_some() {
                        return Err(X11SetupSocketError::new(
                            "authority egress received a duplicate or stale ticket",
                        ));
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    input_disconnected = true;
                    break;
                }
            }
        }

        while output.is_none() {
            let Some(batch) = pending.remove(&expected) else { break };
            expected = expected.saturating_add(1);
            if let Some(batch) = batch {
                output = Some((batch, Instant::now()));
            }
        }

        if cancellation.load(Ordering::Acquire) {
            if let Some((batch, started)) = output.take() {
                telemetry(XAuthorityBackpressureTelemetry {
                    kind: XAuthorityBackpressureTelemetryKind::Shutdown,
                    client: batch.client,
                    transaction: batch.transaction,
                    waited: started.elapsed(),
                    failure: Some(XAuthorityBackpressureFailure::Cancelled),
                });
            }
            for batch in pending.values().flatten() {
                telemetry(XAuthorityBackpressureTelemetry {
                    kind: XAuthorityBackpressureTelemetryKind::Shutdown,
                    client: batch.client,
                    transaction: batch.transaction,
                    waited: Duration::ZERO,
                    failure: Some(XAuthorityBackpressureFailure::Cancelled),
                });
            }
            return Ok(());
        }

        if let Some((batch, started)) = output.take() {
            let client = batch.client;
            let transaction = batch.transaction;
            match sender.try_send(batch) {
                Ok(()) => {
                    if waiting.remove(&transaction.raw()) {
                        telemetry(XAuthorityBackpressureTelemetry {
                            kind: XAuthorityBackpressureTelemetryKind::Resume,
                            client,
                            transaction,
                            waited: started.elapsed(),
                            failure: None,
                        });
                    }
                }
                Err(TrySendError::Full(batch)) => {
                    if waiting.insert(transaction.raw()) {
                        telemetry(XAuthorityBackpressureTelemetry {
                            kind: XAuthorityBackpressureTelemetryKind::Wait,
                            client,
                            transaction,
                            waited: Duration::ZERO,
                            failure: None,
                        });
                    }
                    output = Some((batch, started));
                    // Later consecutive batches are causally waiting behind
                    // this full boundary even though they cannot overtake it.
                    for batch in pending.values().flatten() {
                        if waiting.insert(batch.transaction.raw()) {
                            telemetry(XAuthorityBackpressureTelemetry {
                                kind: XAuthorityBackpressureTelemetryKind::Wait,
                                client: batch.client,
                                transaction: batch.transaction,
                                waited: Duration::ZERO,
                                failure: None,
                            });
                        }
                    }
                }
                Err(TrySendError::Disconnected(_)) => {
                    telemetry(XAuthorityBackpressureTelemetry {
                        kind: XAuthorityBackpressureTelemetryKind::TransportFailure,
                        client,
                        transaction,
                        waited: started.elapsed(),
                        failure: Some(XAuthorityBackpressureFailure::Disconnected),
                    });
                    for batch in pending.values().flatten() {
                        telemetry(XAuthorityBackpressureTelemetry {
                            kind: XAuthorityBackpressureTelemetryKind::TransportFailure,
                            client: batch.client,
                            transaction: batch.transaction,
                            waited: Duration::ZERO,
                            failure: Some(XAuthorityBackpressureFailure::Disconnected),
                        });
                    }
                    return Err(X11SetupSocketError::new(
                        "X authority observed transaction channel is disconnected",
                    ));
                }
            }
        }

        if input_disconnected && output.is_none() && pending.is_empty() {
            return Ok(());
        }
        if input_disconnected && output.is_none() && !pending.contains_key(&expected) {
            return Err(X11SetupSocketError::new(
                "authority egress closed with a missing transaction ticket",
            ));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(unix)]
pub fn run_x_server_frontend_routed_until_stopped_with_backpressure_observer(
    config: XServerFrontendConfig,
    transaction_sender: SyncSender<XAuthorityObservedTransactionBatch>,
    mut broker: XServerFrontendRouteBroker,
    service_commands: Receiver<XServerFrontendServiceCommand>,
    backpressure_observer: Arc<XAuthorityBackpressureObserver>,
) -> Result<(), X11SetupSocketError> {
    let mut frontend = XServerFrontend::bind(config)?;
    frontend
        .state
        .runtime
        .lock()
        .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?
        .set_input_authority(broker.registry.input_authority.clone());
    let observation_cancellation = Arc::new(AtomicBool::new(false));
    let worker_observation_cancellation = observation_cancellation.clone();
    let (egress_sender, egress_receiver) =
        sync_channel(X_AUTHORITY_OBSERVED_TRANSACTION_CHANNEL_CAPACITY);
    let egress_cancellation = observation_cancellation.clone();
    let egress_backpressure_observer = backpressure_observer.clone();
    let (egress_completion_sender, egress_completion_receiver) =
        std::sync::mpsc::sync_channel(1);
    let egress_thread = std::thread::spawn(move || {
        let result = run_x_authority_egress_sequencer(
            egress_receiver,
            transaction_sender,
            egress_cancellation,
            egress_backpressure_observer,
        );
        let _ = egress_completion_sender.send(
            result
                .as_ref()
                .map(|_| ())
                .map_err(std::string::ToString::to_string),
        );
        result
    });
    let dispatch_egress_sender = egress_sender.clone();
    let observer: Arc<X11CoreTraceObserver> = Arc::new(move |trace| {
        let envelope = XAuthorityEgressEnvelope {
            transaction: trace.transaction,
            batch: XAuthorityObservedTransactionBatch::from_dispatch_observation(&trace),
        };
        send_x_authority_egress_envelope(
            &dispatch_egress_sender,
            envelope,
            &worker_observation_cancellation,
        )
    });
    let mut accepting = true;
    let mut pending_raster_egress = None::<XAuthorityEgressEnvelope>;
    let mut raster_fallbacks = XRasterFallbackCoalescer::default();
    let mut egress_finished = false;
    let service_result: Result<(), X11SetupSocketError> = (|| {
        loop {
            let mut progressed = false;
            match (!egress_finished).then(|| egress_completion_receiver.try_recv()) {
                None => {}
                Some(Ok(Err(error))) => {
                    return Err(X11SetupSocketError::new(format!(
                        "authority egress failed: {error}"
                    )));
                }
                Some(Ok(Ok(()))) if observation_cancellation.load(Ordering::Acquire) => {
                    egress_finished = true;
                }
                Some(Ok(Ok(()))) => {
                    return Err(X11SetupSocketError::new(
                        "authority egress stopped before the frontend",
                    ));
                }
                Some(Err(TryRecvError::Empty)) => {}
                Some(Err(TryRecvError::Disconnected)) => {
                    return Err(X11SetupSocketError::new(
                        "authority egress completion channel disconnected",
                    ));
                }
            }
            match service_commands.try_recv() {
                Ok(XServerFrontendServiceCommand::StopAccepting)
                | Err(TryRecvError::Disconnected) => accepting = false,
                Ok(XServerFrontendServiceCommand::StopAndDisconnect) => {
                    accepting = false;
                    observation_cancellation.store(true, Ordering::Release);
                    frontend.shutdown_all_client_workers()?;
                    progressed = true;
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

            if observation_cancellation.load(Ordering::Acquire) {
                pending_raster_egress = None;
            }
            if pending_raster_egress.is_none() {
                match broker.try_recv_raster_requirements() {
                    Ok(requirements) => {
                        let transaction = frontend.state.allocate_transaction()?;
                        let response = frontend
                            .state
                            .runtime
                            .lock()
                            .map_err(|_| {
                                X11SetupSocketError::new("X11 authority runtime lock poisoned")
                            })?
                            .apply_surface_raster_requirements(transaction, &requirements);
                        match response {
                            Ok(crate::XSurfaceRasterOutcome::Satisfied(response)) => {
                                raster_fallbacks.report_satisfied(
                                    &requirements,
                                    response.identity.source_content_generation,
                                );
                                let batch = XAuthorityObservedTransactionBatch::from_raster_response(
                                    *response,
                                );
                                pending_raster_egress = Some(XAuthorityEgressEnvelope {
                                    transaction,
                                    batch: Some(batch),
                                });
                            }
                            Ok(crate::XSurfaceRasterOutcome::SampledFallback {
                                cause,
                                observed_content_generation,
                            }) => {
                                pending_raster_egress = Some(XAuthorityEgressEnvelope {
                                    transaction,
                                    batch: None,
                                });
                                raster_fallbacks.report(
                                    &requirements,
                                    cause,
                                    observed_content_generation,
                                );
                            }
                            Err(error) => {
                                // One surface's demand must never end the
                                // server. Answer it with the canonical raster
                                // and keep serving every other client.
                                pending_raster_egress = Some(XAuthorityEgressEnvelope {
                                    transaction,
                                    batch: None,
                                });
                                tracing::warn!(
                                    "sophia_x11_raster_requirement schema=1 status=refused surface={:?} content_generation={} requirement_generation={} error={error:?}",
                                    requirements.surface,
                                    requirements.committed_content_generation,
                                    requirements.requirement_generation,
                                );
                            }
                        }
                        progressed = true;
                    }
                    Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
                }
            }
            if let Some(envelope) = pending_raster_egress.take() {
                match egress_sender.try_send(envelope) {
                    Ok(()) => progressed = true,
                    Err(TrySendError::Full(envelope)) => {
                        pending_raster_egress = Some(envelope);
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        return Err(X11SetupSocketError::new(
                            "authority egress sequencer disconnected",
                        ));
                    }
                }
            }

            if accepting {
                while frontend.active_client_worker_count()
                    < frontend.config().max_concurrent_clients().get()
                {
                    if !frontend
                        .try_serve_next_concurrently_routed_traced(&broker, observer.clone())?
                    {
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

            if !accepting
                && frontend.active_client_worker_count() == 0
                && pending_raster_egress.is_none()
            {
                return Ok(());
            }
            if !progressed {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    })();
    let mut cleanup_failures = Vec::new();
    if service_result.is_err() {
        observation_cancellation.store(true, Ordering::Release);
        if let Err(error) = frontend.shutdown_all_client_workers() {
            cleanup_failures.push(format!("worker shutdown failed: {error}"));
        }
        if let Err(error) = frontend.wait_for_clients() {
            cleanup_failures.push(format!("worker reap failed: {error}"));
        }
    }
    drop(observer);
    drop(egress_sender);
    let egress_result = egress_thread.join().unwrap_or_else(|_| {
        Err(X11SetupSocketError::new(
            "authority egress sequencer panicked",
        ))
    });
    match service_result {
        Ok(()) => egress_result,
        Err(original) => {
            if let Err(error) = egress_result {
                cleanup_failures.push(format!("authority egress failed: {error}"));
            }
            Err(original.with_cleanup_failures(cleanup_failures))
        }
    }
}

#[cfg(unix)]
fn x_authority_observation_client_error(
    error: crate::XAuthorityTransportError,
) -> X11SetupSocketError {
    match error {
        crate::XAuthorityTransportError::Cancelled { .. } => {
            X11SetupSocketError::service_shutdown(error.to_string())
        }
        crate::XAuthorityTransportError::Disconnected { .. } => {
            X11SetupSocketError::new(error.to_string())
        }
        crate::XAuthorityTransportError::Backpressure { .. } => {
            X11SetupSocketError::client_failure(error.to_string())
        }
    }
}

#[cfg(unix)]
fn trace_x_authority_backpressure(event: XAuthorityBackpressureTelemetry) {
    let client = event.client.map(XServerFrontendClientId::raw);
    let client_id = client.unwrap_or(0);
    let client_known = client.is_some();
    let waited_msec = u64::try_from(event.waited.as_millis()).unwrap_or(u64::MAX);
    let failure = match event.failure {
        None => "none",
        Some(crate::XAuthorityBackpressureFailure::Cancelled) => "cancelled",
        Some(crate::XAuthorityBackpressureFailure::Disconnected) => "disconnected",
    };
    match event.kind {
        XAuthorityBackpressureTelemetryKind::Wait => tracing::warn!(
            "sophia_x11_authority_backpressure schema=1 status=waiting client={} client_known={} transaction={} waited_msec={} failure={}",
            client_id,
            client_known,
            event.transaction.raw(),
            waited_msec,
            failure,
        ),
        XAuthorityBackpressureTelemetryKind::Resume => tracing::info!(
            "sophia_x11_authority_backpressure schema=1 status=resumed client={} client_known={} transaction={} waited_msec={} failure={}",
            client_id,
            client_known,
            event.transaction.raw(),
            waited_msec,
            failure,
        ),
        XAuthorityBackpressureTelemetryKind::Shutdown => tracing::info!(
            "sophia_x11_authority_backpressure schema=1 status=shutdown client={} client_known={} transaction={} waited_msec={} failure={}",
            client_id,
            client_known,
            event.transaction.raw(),
            waited_msec,
            failure,
        ),
        XAuthorityBackpressureTelemetryKind::TransportFailure => tracing::warn!(
            "sophia_x11_authority_backpressure schema=1 status=transport_failure client={} client_known={} transaction={} waited_msec={} failure={}",
            client_id,
            client_known,
            event.transaction.raw(),
            waited_msec,
            failure,
        ),
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
    let state = X11CoreSocketServerState::new();
    let authorization = XServerFrontendSetupAuthorization::default();
    serve_x11_core_socket_listener_once_with_setup_authorization(
        &listener,
        namespace,
        &state,
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
    let state = X11CoreSocketServerState::new();
    serve_x11_core_socket_client_with_state(stream, namespace, &state)
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
    let state = X11CoreSocketServerState::new();
    serve_x11_core_socket_client_with_state_observed(stream, namespace, &state, move |result| {
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
