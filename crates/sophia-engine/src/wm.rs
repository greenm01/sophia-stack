use crate::WmTransactionUpdate;
use crate::prelude::*;
use crate::shortcut::{WmShortcutRegistry, WmShortcutRouter};
use sophia_protocol::{
    IpcMessageKind, WM_API_VERSION, WmHello, WmPolicyAck, WmPolicyAckOutcome, WmPolicyUpdate,
    WmSessionDescriptor, decode_frame, decode_wm_hello_frame, decode_wm_policy_update_frame,
    decode_wm_response_frame, encode_wm_policy_ack_frame, encode_wm_request_frame,
    encode_wm_session_descriptor_frame,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WmIpcError {
    Codec(IpcCodecError),
    Io(String),
    TransactionMismatch {
        expected: TransactionId,
        actual: TransactionId,
    },
    Negotiation(&'static str),
}

impl fmt::Display for WmIpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Negotiation(message) => write!(f, "WM negotiation failed: {message}"),
            Self::Codec(error) => write!(f, "codec error: {error:?}"),
            Self::Io(error) => f.write_str(error),
            Self::TransactionMismatch { expected, actual } => write!(
                f,
                "transaction mismatch, expected {}, got {}",
                expected.raw(),
                actual.raw()
            ),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WmPolicyApplyOutcome {
    DeferredUntilShortcutIdle,
    Acknowledged(WmPolicyAck),
}

/// Builds a shortcut registry from a v7 hello.
///
/// The only thing this adds over `WmShortcutRegistry::new` is the API-version
/// check, which is the one part of the hello that is genuinely about the protocol
/// revision rather than about the bindings. Everything else is forwarded.
impl WmShortcutRegistry {
    pub fn from_hello(hello: &WmHello) -> Result<Self, WmIpcError> {
        if hello.api_version != WM_API_VERSION {
            return Err(WmIpcError::Negotiation("unsupported WM API version"));
        }
        Self::new(
            &hello.bindings,
            hello.capabilities,
            hello.policy_generation,
            hello.chrome,
        )
        .map_err(WmIpcError::Negotiation)
    }
}

/// Applies a v7 policy update to the router.
///
/// Kept here rather than on the router itself: it speaks `WmPolicyUpdate` and
/// `WmPolicyAck`, so it belongs to the revision that defines them and should be
/// deleted with it.
impl WmShortcutRouter {
    pub fn apply_policy_update(&mut self, update: &WmPolicyUpdate) -> WmPolicyApplyOutcome {
        if update.generation <= self.registry.policy_generation() {
            return WmPolicyApplyOutcome::Acknowledged(WmPolicyAck {
                generation: update.generation,
                outcome: WmPolicyAckOutcome::RejectedStale,
            });
        }
        if !self.registry.supports_chrome_policy() && update.chrome != self.registry.chrome() {
            return WmPolicyApplyOutcome::Acknowledged(WmPolicyAck {
                generation: update.generation,
                outcome: WmPolicyAckOutcome::RejectedInvalid,
            });
        }
        if !self.shortcut_idle() {
            return WmPolicyApplyOutcome::DeferredUntilShortcutIdle;
        }
        let hello = WmHello {
            api_version: update.api_version,
            capabilities: self.registry.capabilities,
            policy_generation: update.generation,
            bindings: update.bindings.clone(),
            chrome: update.chrome,
        };
        let Ok(registry) = WmShortcutRegistry::from_hello(&hello) else {
            return WmPolicyApplyOutcome::Acknowledged(WmPolicyAck {
                generation: update.generation,
                outcome: WmPolicyAckOutcome::RejectedInvalid,
            });
        };
        self.replace_registry(registry);
        WmPolicyApplyOutcome::Acknowledged(WmPolicyAck {
            generation: update.generation,
            outcome: WmPolicyAckOutcome::Applied,
        })
    }
}

impl std::error::Error for WmIpcError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WmRuntimeAction {
    KeepRunning,
    RestartWm { reason: WmRestartReason },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WmRestartReason {
    IpcFailure(WmIpcError),
}

impl WmTransactionUpdate {
    pub fn runtime_action(&self) -> WmRuntimeAction {
        match &self.ipc_error {
            Some(error) => WmRuntimeAction::RestartWm {
                reason: WmRestartReason::IpcFailure(error.clone()),
            },
            None => WmRuntimeAction::KeepRunning,
        }
    }
}

pub fn update_wm_supervisor_from_runtime_action(
    state: SupervisorState,
    action: WmRuntimeAction,
    policy: RestartPolicy,
) -> (SupervisorState, SupervisorCommand) {
    debug_assert_eq!(state.process, SupervisedProcessKind::WindowManager);

    match action {
        WmRuntimeAction::KeepRunning => {
            debug!(
                process = ?state.process,
                running = state.running,
                restart_attempts = state.restart_attempts,
                "WM runtime action keeps supervisor state"
            );
            (state, SupervisorCommand::None)
        }
        WmRuntimeAction::RestartWm { .. } => {
            warn!(
                process = ?state.process,
                running = state.running,
                restart_attempts = state.restart_attempts,
                "WM runtime action requests supervisor restart"
            );
            update_supervisor(state, SupervisorEvent::RestartRequested, policy)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WmSocketTransportConfig {
    pub response_timeout: Duration,
}

impl Default for WmSocketTransportConfig {
    fn default() -> Self {
        Self {
            response_timeout: Duration::from_millis(250),
        }
    }
}

#[cfg(unix)]
pub struct WmSocketTransport {
    stream: UnixStream,
    config: WmSocketTransportConfig,
}

#[cfg(unix)]
#[derive(Clone, Debug, PartialEq)]
pub enum WmSocketIncoming {
    Response(WmResponsePacket),
    PolicyUpdate(WmPolicyUpdate),
}

#[cfg(unix)]
impl WmSocketTransport {
    pub fn new(stream: UnixStream, config: WmSocketTransportConfig) -> Self {
        Self { stream, config }
    }

    pub const fn response_timeout(&self) -> Duration {
        self.config.response_timeout
    }

    pub fn negotiate(
        &mut self,
        descriptor: &WmSessionDescriptor,
    ) -> Result<WmShortcutRegistry, WmIpcError> {
        self.stream
            .set_read_timeout(Some(self.config.response_timeout))
            .map_err(|error| WmIpcError::Io(error.to_string()))?;
        let frame = read_ipc_frame(&mut self.stream)?;
        let hello = decode_wm_hello_frame(&frame).map_err(WmIpcError::Codec)?;
        let registry = WmShortcutRegistry::from_hello(&hello)?;
        let frame = encode_wm_session_descriptor_frame(descriptor).map_err(WmIpcError::Codec)?;
        self.stream
            .write_all(&frame)
            .and_then(|()| self.stream.flush())
            .map_err(|error| WmIpcError::Io(error.to_string()))?;
        Ok(registry)
    }

    pub fn request(&mut self, request: &WmRequestPacket) -> Result<WmResponsePacket, WmIpcError> {
        self.send_request(request)?;
        match self.poll_incoming(self.config.response_timeout)? {
            Some(WmSocketIncoming::Response(response))
                if response.transaction == request.transaction =>
            {
                Ok(response)
            }
            Some(WmSocketIncoming::Response(response)) => Err(WmIpcError::TransactionMismatch {
                expected: request.transaction,
                actual: response.transaction,
            }),
            Some(WmSocketIncoming::PolicyUpdate(_)) => Err(WmIpcError::Negotiation(
                "WM policy update requires multiplexed transport",
            )),
            None => Err(WmIpcError::Io("WM response timed out".to_owned())),
        }
    }

    pub fn send_request(&mut self, request: &WmRequestPacket) -> Result<(), WmIpcError> {
        let frame = encode_wm_request_frame(request).map_err(WmIpcError::Codec)?;
        self.stream
            .write_all(&frame)
            .and_then(|()| self.stream.flush())
            .map_err(|error| WmIpcError::Io(error.to_string()))
    }

    pub fn poll_incoming(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<WmSocketIncoming>, WmIpcError> {
        self.stream
            .set_read_timeout(Some(timeout))
            .map_err(|error| WmIpcError::Io(error.to_string()))?;
        let mut ready = [0u8; 1];
        match rustix::net::recv(&self.stream, &mut ready, rustix::net::RecvFlags::PEEK) {
            Ok((0, _)) => return Err(WmIpcError::Io("WM socket disconnected".to_owned())),
            Ok(_) => {}
            Err(error) => {
                let error = std::io::Error::from(error);
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) {
                    return Ok(None);
                }
                return Err(WmIpcError::Io(error.to_string()));
            }
        }
        let frame = read_ipc_frame(&mut self.stream)?;
        let (header, _) = decode_frame(&frame).map_err(WmIpcError::Codec)?;
        match header.message_kind {
            IpcMessageKind::WmResponse => decode_wm_response_frame(&frame)
                .map(WmSocketIncoming::Response)
                .map(Some)
                .map_err(WmIpcError::Codec),
            IpcMessageKind::WmPolicyUpdate => decode_wm_policy_update_frame(&frame)
                .map(WmSocketIncoming::PolicyUpdate)
                .map(Some)
                .map_err(WmIpcError::Codec),
            _ => Err(WmIpcError::Negotiation("unexpected incoming WM message")),
        }
    }

    pub fn poll_policy_update(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<WmPolicyUpdate>, WmIpcError> {
        match self.poll_incoming(timeout)? {
            Some(WmSocketIncoming::PolicyUpdate(update)) => Ok(Some(update)),
            Some(WmSocketIncoming::Response(_)) => Err(WmIpcError::Negotiation(
                "unexpected WM response without an active request",
            )),
            None => Ok(None),
        }
    }

    pub fn acknowledge_policy_update(&mut self, ack: WmPolicyAck) -> Result<(), WmIpcError> {
        let frame = encode_wm_policy_ack_frame(ack).map_err(WmIpcError::Codec)?;
        self.stream
            .write_all(&frame)
            .and_then(|()| self.stream.flush())
            .map_err(|error| WmIpcError::Io(error.to_string()))
    }
}

pub fn request_wm_over_stream<S>(
    stream: &mut S,
    request: &WmRequestPacket,
) -> Result<WmResponsePacket, WmIpcError>
where
    S: Read + Write,
{
    let frame = encode_wm_request_frame(request).map_err(WmIpcError::Codec)?;
    debug!(
        transaction = request.transaction.raw(),
        request_bytes = frame.len(),
        "sending WM request frame"
    );
    stream
        .write_all(&frame)
        .map_err(|error| WmIpcError::Io(error.to_string()))?;
    stream
        .flush()
        .map_err(|error| WmIpcError::Io(error.to_string()))?;

    let response = read_wm_response_frame(stream)?;
    if response.transaction != request.transaction {
        warn!(
            expected_transaction = request.transaction.raw(),
            actual_transaction = response.transaction.raw(),
            "rejected WM response with mismatched transaction"
        );
        return Err(WmIpcError::TransactionMismatch {
            expected: request.transaction,
            actual: response.transaction,
        });
    }
    debug!(
        transaction = response.transaction.raw(),
        response_commands = response.commands.len(),
        "received WM response frame"
    );

    Ok(response)
}

pub fn read_wm_response_frame<R>(reader: &mut R) -> Result<WmResponsePacket, WmIpcError>
where
    R: Read,
{
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    reader
        .read_exact(&mut header)
        .map_err(|error| WmIpcError::Io(error.to_string()))?;
    let payload_len = u32::from_le_bytes(
        header[16..20]
            .try_into()
            .expect("fixed IPC header payload range should be present"),
    ) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        warn!(
            payload_len,
            max_payload_len = SOPHIA_IPC_MAX_PAYLOAD_LEN,
            "rejected oversized WM response frame"
        );
        return Err(WmIpcError::Codec(IpcCodecError::PayloadTooLarge(
            payload_len,
        )));
    }

    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    reader
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .map_err(|error| WmIpcError::Io(error.to_string()))?;

    decode_wm_response_frame(&frame).map_err(WmIpcError::Codec)
}

pub fn read_ipc_frame<R>(reader: &mut R) -> Result<Vec<u8>, WmIpcError>
where
    R: Read,
{
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    reader
        .read_exact(&mut header)
        .map_err(|error| WmIpcError::Io(error.to_string()))?;
    let payload_len = u32::from_le_bytes(
        header[16..20]
            .try_into()
            .expect("fixed IPC header payload range should be present"),
    ) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(WmIpcError::Codec(IpcCodecError::PayloadTooLarge(
            payload_len,
        )));
    }
    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    reader
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .map_err(|error| WmIpcError::Io(error.to_string()))?;
    Ok(frame)
}
