#[cfg(unix)]
use std::net::Shutdown;
#[cfg(unix)]
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(unix)]
use std::sync::mpsc::{
    Receiver, RecvTimeoutError, Sender, SyncSender, TryRecvError, TrySendError, sync_channel,
};
#[cfg(unix)]
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    io::{ErrorKind, IoSlice, IoSliceMut, Read, Write},
    mem::MaybeUninit,
    num::NonZeroUsize,
    os::fd::{AsFd, OwnedFd},
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[cfg(unix)]
use crate::{
    X_ATOM_NAME_WM_DELETE_WINDOW, X_ATOM_NAME_WM_PROTOCOLS, X_SETUP_CLIENT_PREFIX_LEN,
    X_SETUP_DEFAULT_RESOURCE_ID_MASK, X_SETUP_DEFAULT_ROOT, X11DispatchObservation,
    X11ObservedDispatchFailure, X11ObservedRequestStage, XAtomTable, XAuthorityClientControlAck,
    XAuthorityClientControlCommand, XAuthorityClientInputDelivery, XAuthorityClientInputEvent,
    XAuthorityControlAck, XAuthorityControlCommand, XAuthorityControlOutcome,
    XAuthorityDri3FenceImport, XAuthorityDri3PixmapImport, XAuthorityInputDeliveryId,
    XAuthorityInputDeliveryOutcome, XAuthorityInputEvent, XAuthorityKeyEvent,
    XAuthorityObservedTransactionBatch, XAuthorityPointerEvent, XAuthorityPointerEventKind,
    XAuthorityPresentSubmission, XAuthorityResponsePacket, XAuthorityRoutedInput,
    XAuthorityRuntime, XByteOrder, XClientEvent, XDispatchContext, XDispatchResult,
    XPresentCompletionMode, XPropertyTable, XResourceId, XServerFrontendAdmissionError,
    XServerFrontendAdmissionPolicy, XServerFrontendAdmissionRequest, XServerFrontendClientId,
    XServerFrontendConfig, XServerFrontendPeerCredentials, XServerFrontendRenderDeviceError,
    XServerFrontendRenderDeviceProvider, XServerFrontendRouteError, XServerFrontendServiceCommand,
    XServerFrontendSetupAuthorization, XSetupFailure, XSetupRequest, XSetupSuccess,
    XWireClientContext, decode_x11_core_request, dispatch_x11_parse_error,
    dispatch_x11_wire_request, encode_x_client_event, encode_x11_setup_failure,
    encode_x11_setup_success, parse_x11_setup_request, try_emit_x_authority_observation,
    x11_setup_request_total_len,
};
#[cfg(all(unix, test))]
use sophia_protocol::RoutedInputRequest;
#[cfg(unix)]
use sophia_protocol::{
    ClientAdmissionContext, ClientAdmissionId, InputEventKind, NamespaceId, SeatId, Size,
    SurfaceId, TransactionId,
};

include!("x11_socket/routing/broker.rs");
include!("x11_socket/routing/registry.rs");
include!("x11_socket/routing/input.rs");
include!("x11_socket/frontend/service.rs");
include!("x11_socket/frontend/clipboard.rs");
include!("x11_socket/frontend/setup.rs");
include!("x11_socket/state.rs");

#[cfg(unix)]
const X11_CLIENT_RESOURCE_RANGE_SIZE: u32 = X_SETUP_DEFAULT_RESOURCE_ID_MASK + 1;
#[cfg(unix)]
const X11_MAX_CLIENT_RESOURCE_RANGES: u16 = (u32::MAX / X11_CLIENT_RESOURCE_RANGE_SIZE) as u16;
#[cfg(unix)]
/// One ordered X11 socket write and the descriptors attached to its first byte.
///
/// Protocol dispatch remains byte-only and data-oriented. Native descriptor
/// ownership starts at this Unix-socket boundary and ends after the record is
/// sent or rejected, so descriptors cannot leak into authority runtime state.
#[cfg(unix)]
#[derive(Debug)]
pub struct X11SocketOutputRecord {
    bytes: Vec<u8>,
    fds: Vec<OwnedFd>,
}

#[cfg(unix)]
impl X11SocketOutputRecord {
    pub fn new(bytes: Vec<u8>, fds: Vec<OwnedFd>) -> Result<Self, X11SetupSocketError> {
        if bytes.is_empty() {
            return Err(X11SetupSocketError::new(
                "X11 socket output record cannot be empty",
            ));
        }
        if fds.len() > sophia_protocol::DMA_BUF_MAX_PLANES {
            return Err(X11SetupSocketError::new(format!(
                "X11 socket output record carried {} file descriptors; maximum is {}",
                fds.len(),
                sophia_protocol::DMA_BUF_MAX_PLANES,
            )));
        }
        Ok(Self { bytes, fds })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn fd_count(&self) -> usize {
        self.fds.len()
    }
}

#[cfg(unix)]
impl TryFrom<Vec<u8>> for X11SocketOutputRecord {
    type Error = X11SetupSocketError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        Self::new(bytes, Vec::new())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct X11SetupSocketError {
    message: String,
    client_disconnect: bool,
    client_failure: bool,
}

impl X11SetupSocketError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            client_disconnect: false,
            client_failure: false,
        }
    }

    fn client_disconnect(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            client_disconnect: true,
            client_failure: false,
        }
    }

    fn client_failure(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            client_disconnect: false,
            client_failure: true,
        }
    }
}

impl core::fmt::Display for X11SetupSocketError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for X11SetupSocketError {}

/// The setup allocation retained for one connected X11 client.
///
/// The range is a connection lease, not a namespace boundary: in a classic
/// shared-X session other trusted clients may still reference a resource after
/// its creator made it. It is retained so disconnect cleanup can reclaim only
/// resources whose XIDs this client was allowed to create.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct XServerFrontendClientLease {
    client: XServerFrontendClientId,
    resource_id_range: crate::XWireClientResourceRange,
}

#[cfg(all(unix, target_os = "linux"))]
fn x11_peer_credentials(
    stream: &UnixStream,
) -> Result<Option<XServerFrontendPeerCredentials>, X11SetupSocketError> {
    let credentials = rustix::net::sockopt::socket_peercred(stream).map_err(|error| {
        X11SetupSocketError::new(format!("failed to read X11 peer credentials: {error}"))
    })?;
    let process_id = u32::try_from(credentials.pid.as_raw_pid()).map_err(|_| {
        X11SetupSocketError::new("X11 peer process ID is outside the supported range")
    })?;
    Ok(Some(XServerFrontendPeerCredentials {
        process_id,
        user_id: credentials.uid.as_raw(),
        group_id: credentials.gid.as_raw(),
    }))
}

#[cfg(all(unix, not(target_os = "linux")))]
fn x11_peer_credentials(
    _stream: &UnixStream,
) -> Result<Option<XServerFrontendPeerCredentials>, X11SetupSocketError> {
    Ok(None)
}

#[cfg(unix)]
struct XServerFrontendAdmissionLease {
    policy: Arc<dyn XServerFrontendAdmissionPolicy>,
    context: Option<ClientAdmissionContext>,
}

#[cfg(unix)]
impl XServerFrontendAdmissionLease {
    fn new(
        policy: Arc<dyn XServerFrontendAdmissionPolicy>,
        context: ClientAdmissionContext,
    ) -> Self {
        Self {
            policy,
            context: Some(context),
        }
    }

    fn context(&self) -> ClientAdmissionContext {
        self.context
            .expect("active X11 admission lease must retain its context")
    }

    fn revoke(&mut self) -> Result<(), XServerFrontendAdmissionError> {
        let Some(context) = self.context.take() else {
            return Ok(());
        };
        self.policy.revoke(context)
    }
}

#[cfg(unix)]
impl Drop for XServerFrontendAdmissionLease {
    fn drop(&mut self) {
        let _ = self.revoke();
    }
}

/// A trace callback used by a bounded concurrent frontend worker.
#[cfg(unix)]
pub type X11CoreTraceObserver =
    dyn Fn(X11DispatchObservation) -> Result<(), X11SetupSocketError> + Send + Sync + 'static;

#[cfg(unix)]
struct X11CoreClientWorkerCompletion {
    worker_id: u64,
    result: Result<(), X11SetupSocketError>,
}

#[cfg(unix)]
#[derive(Debug)]
struct X11CoreClientWorker {
    thread: std::thread::JoinHandle<()>,
    shutdown: UnixStream,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct X11CoreClientWorkerAdmission {
    worker_id: u64,
    admission: ClientAdmissionId,
}

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

#[cfg(unix)]
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

#[cfg(unix)]
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
    let mut state = X11CoreSocketServerState::new();
    serve_x11_core_socket_client_with_trace_observer_and_input(
        &mut stream,
        namespace,
        &mut state,
        Some(X11InputEventReceiver::Plain(input_receiver)),
        None,
        None,
        &XServerFrontendSetupAuthorization::default(),
        None,
        None,
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
    let mut state = X11CoreSocketServerState::new();
    serve_x11_core_socket_client_with_trace_observer_and_input(
        &mut stream,
        namespace,
        &mut state,
        Some(X11InputEventReceiver::Routed {
            receiver: input_receiver,
            deliveries: None,
        }),
        Some(X11ControlChannels::Routed {
            receiver: control_receiver,
            acknowledgements: control_ack_sender,
        }),
        None,
        &XServerFrontendSetupAuthorization::default(),
        None,
        None,
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
    serve_x11_core_socket_client_with_trace_observer_and_setup_authorization_and_routing(
        stream,
        namespace,
        state,
        authorization,
        admission_policy,
        None,
        None,
        observer,
    )
}

#[cfg(unix)]
fn serve_x11_core_socket_client_with_trace_observer_and_setup_authorization_and_routing(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
    authorization: &XServerFrontendSetupAuthorization,
    admission_policy: Option<Arc<dyn XServerFrontendAdmissionPolicy>>,
    client_routing: Option<XServerFrontendRouteRegistry>,
    worker_admission: Option<(u64, Sender<X11CoreClientWorkerAdmission>)>,
    observer: impl FnMut(X11DispatchObservation) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    serve_x11_core_socket_client_with_trace_observer_and_input(
        stream,
        namespace,
        state,
        None,
        None,
        client_routing,
        authorization,
        admission_policy,
        worker_admission,
        observer,
    )
}

#[cfg(unix)]
fn serve_x11_core_socket_client_with_trace_observer_and_input(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
    input_receiver: Option<X11InputEventReceiver>,
    control_channels: Option<X11ControlChannels>,
    client_routing: Option<XServerFrontendRouteRegistry>,
    authorization: &XServerFrontendSetupAuthorization,
    admission_policy: Option<Arc<dyn XServerFrontendAdmissionPolicy>>,
    worker_admission: Option<(u64, Sender<X11CoreClientWorkerAdmission>)>,
    mut observer: impl FnMut(X11DispatchObservation) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let peer_credentials = if admission_policy.is_some() {
        x11_peer_credentials(stream)?
    } else {
        None
    };
    let mut setup_lease = None;
    let mut admission_lease = None;
    let mut admission_failure = None;
    let Some((setup, _setup_success)) = serve_x11_setup_socket_client_with_setup_authorization(
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
                output_stream.clone(),
                setup.byte_order,
                event_sequence.clone(),
                focused_surface_window.clone(),
                core_event_selections.clone(),
                xkb_state_details.clone(),
                xkb_modifiers.clone(),
                surface_windows.clone(),
                client,
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
                released_dma_bufs,
                released_fences,
                mut server_reply_fds,
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
                            ..
                        } => Some((*wait_fence, *idle_fence)),
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
                        }
                        if let Some((window, parent)) = hierarchy_create {
                            selections.register(window, parent);
                        }
                        if let Some((window, sibling, mode)) = hierarchy_restack {
                            selections.restack(window, sibling, mode);
                        }
                        if let Some(window) = mapped_window {
                            selections.observe_mapped(window);
                        }
                        if let Some(window) = unmapped_window {
                            selections.observe_unmapped(window);
                        }
                        if let Some(window) = destroyed_window {
                            selections.remove(window);
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
                        .and_then(|(wait_fence, idle_fence)| {
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
                    (
                        output,
                        cpu_buffer_update,
                        dri3_pixmap_import,
                        dri3_fence_import,
                        present_submission,
                        released_dma_buf.into_iter().collect::<Vec<_>>(),
                        released_fence.into_iter().collect::<Vec<_>>(),
                        server_reply_fds,
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
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    )
                }
            };
            if let Some(routing) = protocol_routing.as_ref()
                && let Some((index, requestor, property)) = output
                    .outputs
                    .iter()
                    .enumerate()
                    .find_map(|(index, output)| match output {
                        crate::XClientOutput::Event(XClientEvent::SelectionNotify {
                            requestor,
                            property,
                            ..
                        }) => Some((index, *requestor, *property)),
                        _ => None,
                    })
            {
                let mut runtime = state
                    .runtime
                    .lock()
                    .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?;
                if runtime.is_clipboard_proxy(namespace, requestor) {
                    let mut properties = state.properties.lock().map_err(|_| {
                        X11SetupSocketError::new("X11 property table lock poisoned")
                    })?;
                    let payload = runtime
                        .capture_clipboard_source_payload(requestor, property, &mut properties)
                        .map_err(|error| {
                            X11SetupSocketError::new(format!(
                                "failed to capture clipboard source payload: {error:?}"
                            ))
                        })?;
                    routing.source_payload_sender.try_send(payload).map_err(
                        |error| match error {
                            TrySendError::Full(_) => {
                                X11SetupSocketError::new("clipboard source payload queue is full")
                            }
                            TrySendError::Disconnected(_) => X11SetupSocketError::new(
                                "clipboard source payload queue is disconnected",
                            ),
                        },
                    )?;
                    output.outputs.remove(index);
                }
            }
            if let Some(routing) = protocol_routing.as_ref()
                && let Some((index, destination, event)) = output
                    .outputs
                    .iter()
                    .enumerate()
                    .find_map(|(index, output)| match output {
                        crate::XClientOutput::Event(
                            event @ XClientEvent::SelectionNotify { requestor, .. },
                        ) => Some((index, *requestor, *event)),
                        crate::XClientOutput::Event(
                            event @ XClientEvent::SelectionRequest { owner, .. },
                        ) => Some((index, *owner, *event)),
                        crate::XClientOutput::Event(
                            event @ XClientEvent::SelectionClear { owner, .. },
                        ) => Some((index, *owner, *event)),
                        _ => None,
                    })
                && let Some(target) = state.client_for_resource(destination)?
                && target != client
            {
                routing.route_protocol(target, event).map_err(|error| {
                    X11SetupSocketError::new(format!("failed to route X11 protocol event: {error}"))
                })?;
                output.outputs.remove(index);
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
                request_stage,
                failure: parse_failed.then_some(X11ObservedDispatchFailure::ParseRejected),
                result: output.clone(),
                cpu_buffer_update: cpu_buffer_update.clone(),
                received_fd_count: received_fds.len(),
                received_fds: observed_received_fds,
                dri3_pixmap_import,
                dri3_fence_import,
                present_submission,
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
            request_stage: X11ObservedRequestStage::DisconnectCleanup,
            failure: None,
            result: cleanup,
            cpu_buffer_update: None,
            received_fd_count: 0,
            received_fds: Vec::new(),
            dri3_pixmap_import: None,
            dri3_fence_import: None,
            present_submission: None,
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

#[cfg(unix)]
fn x11_core_event_selection_update(
    request: &crate::XWireRequest,
) -> Option<(XResourceId, Option<u32>, Option<u32>)> {
    match request {
        crate::XWireRequest::CreateWindow {
            packet:
                crate::XAuthorityRequestPacket {
                    kind: crate::XAuthorityRequestKind::CreateWindow { window, .. },
                    ..
                },
            event_mask,
            do_not_propagate_mask,
            ..
        }
        | crate::XWireRequest::ChangeWindowAttributes {
            window,
            event_mask,
            do_not_propagate_mask,
        } => Some((*window, *event_mask, *do_not_propagate_mask)),
        _ => None,
    }
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Default)]
struct XCoreWindowEventSelection {
    mask: u32,
    do_not_propagate_mask: u32,
}

#[cfg(unix)]
#[derive(Debug)]
struct XCoreEventSelectionState {
    windows: BTreeMap<XResourceId, XCoreWindowEventSelection>,
    parents: BTreeMap<XResourceId, XResourceId>,
    stacking: Vec<XResourceId>,
    mapped: BTreeSet<XResourceId>,
    fallback_mapped_window: XResourceId,
}

#[cfg(unix)]
impl Default for XCoreEventSelectionState {
    fn default() -> Self {
        Self {
            windows: BTreeMap::new(),
            parents: BTreeMap::new(),
            stacking: Vec::new(),
            mapped: BTreeSet::new(),
            fallback_mapped_window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
        }
    }
}

#[cfg(unix)]
impl XCoreEventSelectionState {
    const KEY_MASKS: u32 = (1 << 0) | (1 << 1);

    fn update(
        &mut self,
        window: XResourceId,
        event_mask: Option<u32>,
        do_not_propagate_mask: Option<u32>,
    ) {
        if event_mask.is_none() && do_not_propagate_mask.is_none() {
            return;
        }
        let selection = self.windows.entry(window).or_default();
        if let Some(mask) = event_mask {
            selection.mask = mask;
        }
        if let Some(mask) = do_not_propagate_mask {
            selection.do_not_propagate_mask = mask;
        }
    }

    fn register(&mut self, window: XResourceId, parent: XResourceId) {
        self.parents.insert(window, parent);
        self.stacking.retain(|candidate| *candidate != window);
        self.stacking.push(window);
    }

    fn restack(&mut self, window: XResourceId, sibling: Option<XResourceId>, mode: Option<u8>) {
        self.stacking.retain(|candidate| *candidate != window);
        let sibling_index = sibling.and_then(|sibling| {
            self.stacking
                .iter()
                .position(|candidate| *candidate == sibling)
        });
        let index = match (mode, sibling_index) {
            (Some(1 | 3), Some(index)) => index,
            (Some(1 | 3), None) => 0,
            (Some(0 | 2 | 4), Some(index)) => index.saturating_add(1),
            _ => self.stacking.len(),
        };
        self.stacking.insert(index.min(self.stacking.len()), window);
    }

    fn observe_mapped(&mut self, window: XResourceId) {
        self.mapped.insert(window);
        self.fallback_mapped_window = window;
    }

    fn observe_unmapped(&mut self, window: XResourceId) {
        self.mapped.remove(&window);
        if self.fallback_mapped_window == window {
            self.fallback_mapped_window = self
                .stacking
                .iter()
                .rev()
                .copied()
                .find(|candidate| self.mapped.contains(candidate))
                .unwrap_or_else(|| XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1));
        }
    }

    fn remove(&mut self, window: XResourceId) {
        self.windows.remove(&window);
        self.parents.remove(&window);
        self.stacking.retain(|candidate| *candidate != window);
        self.mapped.remove(&window);
        if self.fallback_mapped_window == window {
            self.fallback_mapped_window = XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1);
        }
    }

    fn keyboard_target(&self, focused: XResourceId) -> XResourceId {
        self.selected_keyboard_target(focused)
            .unwrap_or_else(|| self.keyboard_fallback(focused))
    }

    fn selected_keyboard_target(&self, focused: XResourceId) -> Option<XResourceId> {
        let mut candidate = self.keyboard_fallback(focused);
        for _ in 0..64 {
            if self
                .windows
                .get(&candidate)
                .is_some_and(|selection| selection.mask & Self::KEY_MASKS != 0)
            {
                return Some(candidate);
            }
            candidate = self.parents.get(&candidate).copied()?;
        }
        None
    }

    fn keyboard_fallback(&self, focused: XResourceId) -> XResourceId {
        let root = XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1);
        if focused == root {
            self.stacking
                .iter()
                .rev()
                .copied()
                .find(|window| self.mapped.contains(window))
                .unwrap_or(self.fallback_mapped_window)
        } else {
            focused
        }
    }

    fn ancestors(&self, window: XResourceId) -> Vec<XResourceId> {
        let mut ancestors = Vec::new();
        let mut candidate = window;
        for _ in 0..64 {
            let Some(parent) = self.parents.get(&candidate).copied() else {
                break;
            };
            ancestors.push(parent);
            candidate = parent;
        }
        ancestors
    }
}

#[cfg(unix)]
struct X11InputEventWriter {
    stop: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<Result<(), X11SetupSocketError>>,
}

#[cfg(unix)]
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
        | XClientEvent::RandrScreenChange { sequence, .. }
        | XClientEvent::RandrCrtcChange { sequence, .. }
        | XClientEvent::RandrOutputChange { sequence, .. }
        | XClientEvent::RandrResourceChange { sequence, .. }
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
    channels: X11ControlChannels,
) -> Result<X11ControlWriter, X11SetupSocketError> {
    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = stop.clone();
    macro_rules! terminate_client {
        ($transaction:expr, $surface:expr) => {{
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
            let window = surface_windows
                .lock()
                .map_err(|_| X11SetupSocketError::new("X11 surface/window map lock poisoned"))?
                .get(&surface)
                .copied();
            let Some(window) = window else {
                channels.send_ack(
                    client,
                    XAuthorityControlAck {
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
                    vec![
                        encode_x_client_event(
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
                        ),
                        encode_x_client_event(
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
                        ),
                    ]
                }
                XAuthorityControlCommand::CloseSurface { .. } => {
                    let atoms = atoms
                        .lock()
                        .map_err(|_| X11SetupSocketError::new("X11 atom table lock poisoned"))?;
                    let Some(protocols) = atoms.atom(X_ATOM_NAME_WM_PROTOCOLS) else {
                        terminate_client!(transaction, surface);
                    };
                    let Some(delete) = atoms.atom(X_ATOM_NAME_WM_DELETE_WINDOW) else {
                        terminate_client!(transaction, surface);
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
                        terminate_client!(transaction, surface);
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
fn spawn_x11_input_event_writer(
    stream: Arc<Mutex<UnixStream>>,
    byte_order: XByteOrder,
    sequence: Arc<AtomicU16>,
    focused_surface_window: Arc<AtomicU64>,
    core_event_selections: Arc<Mutex<XCoreEventSelectionState>>,
    xkb_state_details: Arc<AtomicU16>,
    xkb_modifiers: Arc<AtomicU16>,
    surface_windows: Arc<Mutex<BTreeMap<SurfaceId, XResourceId>>>,
    client: XServerFrontendClientId,
    receiver: X11InputEventReceiver,
) -> Result<X11InputEventWriter, X11SetupSocketError> {
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
                    let previous = xkb_modifiers.swap(key.state, Ordering::AcqRel);
                    let changed = previous ^ key.state;
                    let selected = xkb_state_details.load(Ordering::Acquire);
                    if changed != 0 && selected & 1 != 0 {
                        let state_notify = encode_x_client_event(
                            byte_order,
                            XClientEvent::XkbStateNotify {
                                sequence,
                                time: key.time_msec,
                                modifiers: key.state as u8,
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

#[cfg(unix)]
fn is_x11_client_disconnect(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::BrokenPipe | ErrorKind::ConnectionReset | ErrorKind::UnexpectedEof
    )
}

#[cfg(all(test, unix))]
mod routing_tests {
    use super::*;
    use sophia_protocol::{DeviceId, Point};
    use std::sync::mpsc::sync_channel;

    #[test]
    fn listener_transaction_ids_are_global_across_client_workers() {
        let state = X11CoreSocketServerState::new();
        let first_worker = state.clone();
        let second_worker = state.clone();

        let first = first_worker.allocate_transaction().unwrap();
        let second = second_worker.allocate_transaction().unwrap();
        let third = first_worker.allocate_transaction().unwrap();

        assert_ne!(first, second);
        assert_ne!(second, third);
        assert_eq!(first.raw() + 1, second.raw());
        assert_eq!(second.raw() + 1, third.raw());
    }

    #[test]
    fn routed_input_discards_another_clients_event() {
        let first = XServerFrontendClientId(1);
        let second = XServerFrontendClientId(2);
        let (sender, receiver) = sync_channel(2);
        sender
            .send(XAuthorityClientInputEvent {
                client: second,
                event: XAuthorityKeyEvent {
                    keycode: 24,
                    pressed: true,
                    state: 0,
                    time_msec: 1,
                }
                .into(),
                target_window: None,
                xi_event_type: None,
                xi_transition_mask: 0,
                delivery: None,
            })
            .unwrap();
        sender
            .send(XAuthorityClientInputEvent {
                client: first,
                event: XAuthorityKeyEvent {
                    keycode: 25,
                    pressed: true,
                    state: 0,
                    time_msec: 2,
                }
                .into(),
                target_window: None,
                xi_event_type: None,
                xi_transition_mask: 0,
                delivery: None,
            })
            .unwrap();

        let receiver = X11InputEventReceiver::Routed {
            receiver,
            deliveries: None,
        };
        assert_eq!(receiver.recv_timeout(first), Err(RecvTimeoutError::Timeout));
        assert_eq!(
            receiver.recv_timeout(first).unwrap(),
            (
                XAuthorityInputEvent::Key(XAuthorityKeyEvent {
                    keycode: 25,
                    pressed: true,
                    state: 0,
                    time_msec: 2,
                }),
                None,
                None,
                0,
                None,
            )
        );
    }

    #[test]
    fn routed_control_discards_another_clients_command_and_labels_its_ack() {
        let first = XServerFrontendClientId(1);
        let second = XServerFrontendClientId(2);
        let surface = SurfaceId::new(44, 1);
        let (command_sender, command_receiver) = sync_channel(2);
        let (ack_sender, ack_receiver) = sync_channel(1);
        let command = XAuthorityControlCommand::FocusSurface {
            transaction: TransactionId::from_raw(7),
            surface,
        };
        command_sender
            .send(XAuthorityClientControlCommand {
                client: second,
                command,
            })
            .unwrap();
        command_sender
            .send(XAuthorityClientControlCommand {
                client: first,
                command,
            })
            .unwrap();

        let channels = X11ControlChannels::Routed {
            receiver: command_receiver,
            acknowledgements: ack_sender,
        };
        assert_eq!(channels.recv_timeout(first), Err(RecvTimeoutError::Timeout));
        assert_eq!(channels.recv_timeout(first).unwrap(), command);
        let acknowledgement = XAuthorityControlAck {
            transaction: command.transaction(),
            surface: command.surface(),
            outcome: XAuthorityControlOutcome::Delivered,
        };
        channels.send_ack(first, acknowledgement).unwrap();
        assert_eq!(
            ack_receiver.recv().unwrap(),
            XAuthorityClientControlAck {
                client: first,
                acknowledgement,
            }
        );
    }

    #[test]
    fn route_broker_delivers_to_the_registered_client_only() {
        let client = XServerFrontendClientId(9);
        let mut broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(2).unwrap());
        let (registration, channels) = broker.registry.register_client(client).unwrap();
        let input = XAuthorityInputEvent::Key(XAuthorityKeyEvent {
            keycode: 38,
            pressed: true,
            state: 0,
            time_msec: 3,
        });
        let command = XAuthorityControlCommand::FocusSurface {
            transaction: TransactionId::from_raw(8),
            surface: SurfaceId::new(45, 1),
        };

        broker
            .input_sender()
            .send(XAuthorityClientInputEvent {
                client,
                event: input,
                target_window: None,
                xi_event_type: None,
                xi_transition_mask: 0,
                delivery: None,
            })
            .unwrap();
        broker
            .control_sender()
            .send(XAuthorityClientControlCommand { client, command })
            .unwrap();

        assert_eq!(broker.route_pending(), Ok(2));
        assert_eq!(
            channels.input.recv().unwrap(),
            XAuthorityClientInputEvent {
                client,
                event: input,
                target_window: None,
                xi_event_type: None,
                xi_transition_mask: 0,
                delivery: None,
            }
        );
        assert_eq!(channels.control.recv().unwrap(), command);
        let acknowledgement = XAuthorityControlAck {
            transaction: command.transaction(),
            surface: command.surface(),
            outcome: XAuthorityControlOutcome::Delivered,
        };
        let channels = X11ControlChannels::ClientBound {
            receiver: channels.control,
            acknowledgements: broker.registry.acknowledgement_sender.clone(),
        };
        channels.send_ack(client, acknowledgement).unwrap();
        assert_eq!(
            broker
                .recv_control_ack_timeout(Duration::from_millis(1))
                .unwrap(),
            XAuthorityClientControlAck {
                client,
                acknowledgement,
            }
        );
        assert_eq!(broker.registered_client_count(), 1);

        drop(registration);
        assert_eq!(broker.registered_client_count(), 0);
        broker
            .input_sender()
            .send(XAuthorityClientInputEvent {
                client,
                event: input,
                target_window: None,
                xi_event_type: None,
                xi_transition_mask: 0,
                delivery: None,
            })
            .unwrap();
        assert_eq!(
            broker.route_pending(),
            Err(XServerFrontendRouteError::UnknownClient { client })
        );
    }

    #[test]
    fn clearing_old_present_selection_preserves_active_window_feedback() {
        let namespace = NamespaceId::from_raw(10);
        let client = XServerFrontendClientId(9);
        let surface = SurfaceId::new(11, 1);
        let bootstrap_window = XResourceId::new(0x200009, 1);
        let bootstrap_event = XResourceId::new(0x20000d, 1);
        let main_window = XResourceId::new(0x200010, 1);
        let main_event = XResourceId::new(0x200014, 1);
        let pixmap = XResourceId::new(0x200015, 1);
        let idle_fence = XResourceId::new(0x200016, 1);
        let transaction = TransactionId::from_raw(202);
        let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(8).unwrap());
        let (_registration, channels) = broker.registry.register_client(client).unwrap();
        broker
            .registry
            .register_surface(client, namespace, surface, main_window)
            .unwrap();

        broker
            .registry
            .select_present_input(client, bootstrap_event, bootstrap_window, 7)
            .unwrap();
        broker
            .registry
            .select_present_input(client, main_event, main_window, 7)
            .unwrap();
        broker
            .registry
            .select_present_input(client, bootstrap_event, bootstrap_window, 0)
            .unwrap();
        broker
            .registry
            .queue_present(
                transaction,
                client,
                main_window,
                pixmap,
                1,
                Some(idle_fence),
            )
            .unwrap();

        assert_eq!(
            broker.route_present_complete(
                transaction,
                1_188_203,
                7_668_086,
                XPresentCompletionMode::Flip,
            ),
            Ok(true)
        );
        assert_eq!(
            channels.protocol.recv().unwrap(),
            XClientEvent::PresentCompleteNotify {
                sequence: 0,
                event_id: main_event,
                window: main_window,
                serial: 1,
                ust: 1_188_203,
                msc: 7_668_086,
                mode: XPresentCompletionMode::Flip as u8,
            }
        );
        assert_eq!(broker.route_present_idle(transaction), Ok(true));
        assert_eq!(
            channels.protocol.recv().unwrap(),
            XClientEvent::PresentIdleNotify {
                sequence: 0,
                event_id: main_event,
                window: main_window,
                serial: 1,
                pixmap,
                idle_fence: Some(idle_fence),
            }
        );
    }

    #[test]
    fn present_feedback_reaches_every_matching_event_selection() {
        let namespace = NamespaceId::from_raw(10);
        let client = XServerFrontendClientId(10);
        let surface = SurfaceId::new(12, 1);
        let window = XResourceId::new(0x300010, 1);
        let first_event = XResourceId::new(0x300014, 1);
        let second_event = XResourceId::new(0x300015, 1);
        let pixmap = XResourceId::new(0x300016, 1);
        let transaction = TransactionId::from_raw(203);
        let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(8).unwrap());
        let (registration, channels) = broker.registry.register_client(client).unwrap();
        broker
            .registry
            .register_surface(client, namespace, surface, window)
            .unwrap();
        for event_id in [first_event, second_event] {
            broker
                .registry
                .select_present_input(client, event_id, window, 7)
                .unwrap();
        }
        broker
            .registry
            .queue_present(transaction, client, window, pixmap, 2, None)
            .unwrap();

        assert_eq!(
            broker.route_present_complete(transaction, 10, 20, XPresentCompletionMode::Flip),
            Ok(true)
        );
        for event_id in [first_event, second_event] {
            assert!(matches!(
                channels.protocol.recv().unwrap(),
                XClientEvent::PresentCompleteNotify {
                    event_id: routed_event,
                    ..
                } if routed_event == event_id
            ));
        }
        assert_eq!(broker.route_present_idle(transaction), Ok(true));
        for event_id in [first_event, second_event] {
            assert!(matches!(
                channels.protocol.recv().unwrap(),
                XClientEvent::PresentIdleNotify {
                    event_id: routed_event,
                    ..
                } if routed_event == event_id
            ));
        }

        let disconnected = TransactionId::from_raw(204);
        broker
            .registry
            .queue_present(disconnected, client, window, pixmap, 3, None)
            .unwrap();
        drop(registration);
        assert_eq!(
            broker.route_present_complete(disconnected, 30, 40, XPresentCompletionMode::Flip,),
            Ok(false)
        );
    }

    #[test]
    fn route_broker_fails_closed_when_a_client_queue_is_backpressured() {
        let client = XServerFrontendClientId(10);
        let mut broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(1).unwrap());
        let (_registration, _channels) = broker.registry.register_client(client).unwrap();
        for time_msec in [4, 5] {
            broker
                .input_sender()
                .send(XAuthorityClientInputEvent {
                    client,
                    event: XAuthorityKeyEvent {
                        keycode: 39,
                        pressed: true,
                        state: 0,
                        time_msec,
                    }
                    .into(),
                    target_window: None,
                    xi_event_type: None,
                    xi_transition_mask: 0,
                    delivery: None,
                })
                .unwrap();
            if time_msec == 4 {
                assert_eq!(broker.route_pending(), Ok(1));
            }
        }

        assert_eq!(
            broker.route_pending(),
            Err(XServerFrontendRouteError::ClientQueueFull { client })
        );
    }

    #[test]
    fn route_broker_reports_rejected_delivery_for_an_unknown_client() {
        let client = XServerFrontendClientId(12);
        let (control_ack_sender, _control_ack_receiver) = sync_channel(1);
        let (delivery_sender, delivery_receiver) = sync_channel(1);
        let mut broker = XServerFrontendRouteBroker::with_control_and_input_delivery_senders(
            NonZeroUsize::new(1).unwrap(),
            control_ack_sender,
            delivery_sender,
        );
        let delivery = XAuthorityInputDeliveryId::from_raw(7);
        broker
            .input_sender()
            .send(XAuthorityClientInputEvent {
                client,
                event: XAuthorityKeyEvent {
                    keycode: 38,
                    pressed: true,
                    state: 0,
                    time_msec: 1,
                }
                .into(),
                target_window: None,
                xi_event_type: None,
                xi_transition_mask: 0,
                delivery: Some(delivery),
            })
            .unwrap();

        assert_eq!(
            broker.route_pending(),
            Err(XServerFrontendRouteError::UnknownClient { client })
        );
        assert_eq!(
            delivery_receiver.recv().unwrap(),
            XAuthorityClientInputDelivery {
                client,
                delivery,
                outcome: XAuthorityInputDeliveryOutcome::RouteRejected,
            }
        );
    }

    #[test]
    fn active_keyboard_grab_redirects_engine_routed_input_and_window() {
        let namespace = NamespaceId::from_raw(9);
        let focused = XServerFrontendClientId(1);
        let grabber = XServerFrontendClientId(2);
        let surface = SurfaceId::new(10, 1);
        let grab_window = XResourceId::new(0x400001, 1);
        let mut broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(4).unwrap());
        let (_focused_registration, focused_channels) =
            broker.registry.register_client(focused).unwrap();
        let (_grab_registration, grab_channels) = broker.registry.register_client(grabber).unwrap();
        broker
            .registry
            .register_surface(focused, namespace, surface, XResourceId::new(0x200001, 1))
            .unwrap();
        broker
            .registry
            .input_authority
            .lock()
            .unwrap()
            .grab_keyboard(
                namespace,
                crate::XActiveInputGrab {
                    owner: grabber.raw(),
                    window: grab_window,
                    owner_events: false,
                    pointer_mode: 1,
                    keyboard_mode: 1,
                    event_mask: 0,
                },
            )
            .unwrap();
        broker
            .registry
            .input_authority
            .lock()
            .unwrap()
            .select_xi_events(namespace, grabber.raw(), grab_window, &[(1, vec![1 << 2])]);
        broker
            .routed_input_sender()
            .send(XAuthorityRoutedInput {
                request: RoutedInputRequest {
                    serial: 1,
                    seat: SeatId::from_raw(1),
                    device: DeviceId::from_raw(1),
                    time_msec: 1,
                    target_surface: surface,
                    global_position: Point::default(),
                    local_position: Point::default(),
                    kind: InputEventKind::Key {
                        keycode: 30,
                        pressed: true,
                    },
                },
                delivery: None,
            })
            .unwrap();
        assert_eq!(broker.route_pending(), Ok(1));
        assert!(matches!(
            focused_channels.input.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        let routed = grab_channels.input.recv().unwrap();
        assert_eq!(routed.client, grabber);
        assert_eq!(routed.target_window, Some(grab_window));
        assert_eq!(routed.xi_event_type, Some(2));
    }

    #[test]
    fn synchronous_keyboard_grab_queues_until_allow_events() {
        let namespace = NamespaceId::from_raw(10);
        let client = XServerFrontendClientId(3);
        let surface = SurfaceId::new(11, 1);
        let mut broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(2).unwrap());
        let (_registration, channels) = broker.registry.register_client(client).unwrap();
        broker
            .registry
            .register_surface(client, namespace, surface, XResourceId::new(0x200001, 1))
            .unwrap();
        broker
            .registry
            .input_authority
            .lock()
            .unwrap()
            .grab_keyboard(
                namespace,
                crate::XActiveInputGrab {
                    owner: client.raw(),
                    window: XResourceId::new(0x200001, 1),
                    owner_events: false,
                    pointer_mode: 1,
                    keyboard_mode: 0,
                    event_mask: 0,
                },
            )
            .unwrap();
        broker
            .routed_input_sender()
            .send(XAuthorityRoutedInput {
                request: RoutedInputRequest {
                    serial: 2,
                    seat: SeatId::from_raw(1),
                    device: DeviceId::from_raw(1),
                    time_msec: 2,
                    target_surface: surface,
                    global_position: Point::default(),
                    local_position: Point::default(),
                    kind: InputEventKind::Key {
                        keycode: 30,
                        pressed: true,
                    },
                },
                delivery: None,
            })
            .unwrap();
        assert_eq!(broker.route_pending(), Ok(1));
        assert!(matches!(
            channels.input.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        broker
            .registry
            .input_authority
            .lock()
            .unwrap()
            .allow_events(namespace, client.raw(), 3)
            .unwrap();
        assert_eq!(broker.route_pending(), Ok(1));
        assert_eq!(channels.input.recv().unwrap().client, client);
    }

    #[test]
    fn xi2_device_event_uses_xge_header_and_fp1616_local_coordinates() {
        let bytes = encode_xi_device_event(
            XByteOrder::LittleEndian,
            7,
            6,
            XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                kind: XAuthorityPointerEventKind::Motion,
                surface: SurfaceId::new(1, 1),
                root_x: 11,
                root_y: 12,
                event_x: 3,
                event_y: -4,
                state: 5,
                time_msec: 9,
            }),
            XResourceId::new(0x200001, 1),
        );
        assert_eq!(bytes.len(), 80);
        assert_eq!(bytes[0], 35);
        assert_eq!(bytes[1], crate::X_INPUT_MAJOR_OPCODE);
        assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 6);
        assert_eq!(u16::from_le_bytes([bytes[10], bytes[11]]), 2);
        assert_eq!(
            i32::from_le_bytes(bytes[40..44].try_into().unwrap()),
            3 << 16
        );
        assert_eq!(
            i32::from_le_bytes(bytes[44..48].try_into().unwrap()),
            -4 << 16
        );
        let crossing = encode_xi_crossing_event(
            XByteOrder::LittleEndian,
            8,
            7,
            XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                kind: XAuthorityPointerEventKind::Motion,
                surface: SurfaceId::new(1, 1),
                root_x: 11,
                root_y: 12,
                event_x: 3,
                event_y: -4,
                state: 5,
                time_msec: 9,
            }),
            XResourceId::new(0x200001, 1),
        );
        assert_eq!(crossing.len(), 72);
        assert_eq!(u16::from_le_bytes([crossing[8], crossing[9]]), 7);
        assert_eq!(crossing[48], 1);
    }

    #[test]
    fn keyboard_focus_propagates_only_through_its_ancestor_chain() {
        let mut selections = XCoreEventSelectionState::default();
        let parent = XResourceId::new(0x200007, 1);
        let child = XResourceId::new(0x200001, 1);
        selections.register(child, parent);
        assert_eq!(selections.selected_keyboard_target(child), None);
        selections.update(parent, Some(1), None);

        assert_eq!(selections.keyboard_target(child), parent);
        assert_eq!(selections.selected_keyboard_target(child), Some(parent));

        assert_eq!(
            selections.keyboard_target(XResourceId::new(0x200009, 1)),
            XResourceId::new(0x200009, 1)
        );
    }

    #[test]
    fn keyboard_delivery_falls_back_to_engine_focused_surface() {
        let selections = XCoreEventSelectionState::default();

        assert_eq!(
            selections.keyboard_target(XResourceId::new(0x200001, 1)),
            XResourceId::new(0x200001, 1)
        );
    }

    #[test]
    fn root_focus_uses_mapped_stacking_order_and_restacking() {
        let root = XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1);
        let lower = XResourceId::new(0x200001, 1);
        let upper = XResourceId::new(0x200002, 1);
        let mut selections = XCoreEventSelectionState::default();
        for window in [lower, upper] {
            selections.register(window, root);
            selections.update(window, Some(1), None);
            selections.observe_mapped(window);
        }
        assert_eq!(selections.keyboard_target(root), upper);

        selections.restack(lower, Some(upper), Some(0));
        assert_eq!(selections.keyboard_target(root), lower);

        selections.observe_unmapped(lower);
        assert_eq!(selections.keyboard_target(root), upper);
    }
}

fn x11_observed_request_stage(request: &crate::XWireRequest) -> X11ObservedRequestStage {
    match request {
        crate::XWireRequest::GlxQueryServerString { .. } => {
            X11ObservedRequestStage::GlxQueryServerString
        }
        crate::XWireRequest::GlxGetFbConfigs { .. } => X11ObservedRequestStage::GlxGetFbConfigs,
        crate::XWireRequest::GlxCreateContext { .. } => X11ObservedRequestStage::GlxCreateContext,
        crate::XWireRequest::GlxCreateWindow { .. } => X11ObservedRequestStage::GlxCreateWindow,
        crate::XWireRequest::Dri3PixmapFromBuffers { .. } => {
            X11ObservedRequestStage::Dri3PixmapFromBuffers
        }
        crate::XWireRequest::PresentPixmap { .. }
        | crate::XWireRequest::Authority(crate::XAuthorityRequestPacket {
            kind: crate::XAuthorityRequestKind::PresentPixmap { .. },
            ..
        }) => X11ObservedRequestStage::PresentPixmap,
        crate::XWireRequest::GetKeyboardMapping { .. } => X11ObservedRequestStage::KeyboardMapping,
        crate::XWireRequest::Authority(crate::XAuthorityRequestPacket {
            kind: crate::XAuthorityRequestKind::RequestSelection { .. },
            ..
        }) => X11ObservedRequestStage::SelectionRequest,
        _ => X11ObservedRequestStage::Other,
    }
}

#[cfg(unix)]
impl From<crate::XAuthorityTransportError> for X11SetupSocketError {
    fn from(error: crate::XAuthorityTransportError) -> Self {
        Self::new(error.to_string())
    }
}

#[cfg(unix)]
pub fn read_x11_setup_request(
    stream: &mut UnixStream,
) -> Result<XSetupRequest, X11SetupSocketError> {
    let mut bytes = vec![0; X_SETUP_CLIENT_PREFIX_LEN];
    stream.read_exact(&mut bytes).map_err(|error| {
        X11SetupSocketError::new(format!("failed to read X11 setup prefix: {error}"))
    })?;
    let total_len = x11_setup_request_total_len(&bytes)
        .map_err(|error| X11SetupSocketError::new(format!("invalid X11 setup prefix: {error}")))?;
    bytes.resize(total_len, 0);
    stream
        .read_exact(&mut bytes[X_SETUP_CLIENT_PREFIX_LEN..])
        .map_err(|error| {
            X11SetupSocketError::new(format!("failed to read X11 setup auth fields: {error}"))
        })?;
    parse_x11_setup_request(&bytes)
        .map_err(|error| X11SetupSocketError::new(format!("invalid X11 setup request: {error}")))
}

/// Send one X11 output record while attaching its descriptors exactly once.
///
/// `SCM_RIGHTS` accompanies the first successful byte range. If the stream
/// accepts only part of the byte payload, the remainder is written without
/// ancillary data so the receiver cannot observe duplicate descriptors.
#[cfg(unix)]
pub fn write_x11_socket_output_record(
    stream: &mut UnixStream,
    record: X11SocketOutputRecord,
) -> std::io::Result<()> {
    let X11SocketOutputRecord { bytes, fds } = record;
    if fds.is_empty() {
        return stream.write_all(&bytes);
    }

    let borrowed = fds.iter().map(AsFd::as_fd).collect::<Vec<_>>();
    let mut ancillary_space = [MaybeUninit::uninit();
        rustix::cmsg_space!(ScmRights(sophia_protocol::DMA_BUF_MAX_PLANES))];
    let mut ancillary = rustix::net::SendAncillaryBuffer::new(&mut ancillary_space);
    if !ancillary.push(rustix::net::SendAncillaryMessage::ScmRights(&borrowed)) {
        return Err(std::io::Error::other(
            "failed to encode X11 output file descriptors",
        ));
    }

    let sent = loop {
        match rustix::net::sendmsg(
            &*stream,
            &[IoSlice::new(&bytes)],
            &mut ancillary,
            rustix::net::SendFlags::empty(),
        ) {
            Ok(sent) => break sent,
            Err(error) => {
                let error = std::io::Error::from(error);
                if error.kind() == ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
        }
    };
    if sent == 0 {
        return Err(std::io::Error::new(
            ErrorKind::WriteZero,
            "failed to write X11 output record",
        ));
    }
    stream.write_all(&bytes[sent..])
}

#[cfg(unix)]
#[derive(Debug)]
pub struct X11ReceivedCoreRequest {
    pub major_opcode: u8,
    pub bytes: Vec<u8>,
    pub fds: Vec<OwnedFd>,
}

pub fn read_x11_core_request(
    stream: &mut UnixStream,
    byte_order: crate::XByteOrder,
) -> Result<Option<X11ReceivedCoreRequest>, X11SetupSocketError> {
    let mut header = [0; 4];
    let mut ancillary_space = [MaybeUninit::uninit();
        rustix::cmsg_space!(ScmRights(sophia_protocol::DMA_BUF_MAX_PLANES))];
    let mut ancillary = rustix::net::RecvAncillaryBuffer::new(&mut ancillary_space);
    let mut iov = [IoSliceMut::new(&mut header)];
    let received = match rustix::net::recvmsg(
        &*stream,
        &mut iov,
        &mut ancillary,
        rustix::net::RecvFlags::CMSG_CLOEXEC,
    ) {
        Ok(received) => received,
        Err(error) => {
            let error = std::io::Error::from(error);
            if matches!(
                error.kind(),
                ErrorKind::UnexpectedEof
                    | ErrorKind::ConnectionReset
                    | ErrorKind::TimedOut
                    | ErrorKind::WouldBlock
            ) {
                return Ok(None);
            }
            return Err(X11SetupSocketError::new(format!(
                "failed to read X11 request header: {error}"
            )));
        }
    };
    if received.bytes == 0 {
        return Ok(None);
    }
    if received.flags.contains(rustix::net::ReturnFlags::CTRUNC) {
        return Err(X11SetupSocketError::new(
            "X11 request carried too many ancillary file descriptors",
        ));
    }
    let mut fds = Vec::new();
    for message in ancillary.drain() {
        if let rustix::net::RecvAncillaryMessage::ScmRights(rights) = message {
            fds.extend(rights);
        }
    }
    if fds.len() > sophia_protocol::DMA_BUF_MAX_PLANES {
        return Err(X11SetupSocketError::new(
            "X11 request carried too many file descriptors",
        ));
    }
    if received.bytes < header.len() {
        match stream.read_exact(&mut header[received.bytes..]) {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::UnexpectedEof
                        | ErrorKind::ConnectionReset
                        | ErrorKind::TimedOut
                        | ErrorKind::WouldBlock
                ) =>
            {
                return Ok(None);
            }
            Err(error) => {
                return Err(X11SetupSocketError::new(format!(
                    "failed to read X11 request header: {error}"
                )));
            }
        }
    }

    let length = usize::from(byte_order.u16(&header[2..4])) * 4;
    if length < 4 {
        return Ok(Some(X11ReceivedCoreRequest {
            major_opcode: header[0],
            bytes: header.to_vec(),
            fds,
        }));
    }
    // The setup reply advertises the full core u16 request-length range. Keep
    // the socket reader consistent with that wire contract: Firefox emits
    // large, but still ordinary, requests just below the 65,535-unit limit.
    // BIG-REQUESTS extended (zero u16 plus u32 length) frames remain outside
    // this bounded reader until a captured client requires them.
    let max_len = usize::from(crate::X_SETUP_DEFAULT_MAX_REQUEST_UNITS) * 4;
    if length > max_len {
        return Err(X11SetupSocketError::new(format!(
            "X11 request payload too large: {length}"
        )));
    }

    let mut request = Vec::with_capacity(length);
    request.extend_from_slice(&header);
    request.resize(length, 0);
    stream.read_exact(&mut request[4..]).map_err(|error| {
        X11SetupSocketError::new(format!("failed to read X11 request payload: {error}"))
    })?;

    Ok(Some(X11ReceivedCoreRequest {
        major_opcode: header[0],
        bytes: request,
        fds,
    }))
}
