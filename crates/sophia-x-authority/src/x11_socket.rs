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
include!("x11_socket/connection/server.rs");
include!("x11_socket/connection/dispatch.rs");
include!("x11_socket/connection/event_state.rs");
include!("x11_socket/connection/writers.rs");

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
