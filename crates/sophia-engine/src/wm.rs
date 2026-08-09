use crate::WmTransactionUpdate;
use crate::prelude::*;
use sophia_protocol::{
    IpcMessageKind, PolicyConfiguration, WM_API_VERSION, WM_MAX_BINDINGS, WmActionId,
    WmBindingRegistration, WmCapabilities, WmChromePolicy, WmHello, WmModifierMask, WmPolicyAck,
    WmPolicyAckOutcome, WmPolicyUpdate, WmSessionDescriptor, decode_frame, decode_wm_hello_frame,
    decode_wm_policy_update_frame, decode_wm_response_frame, encode_wm_policy_ack_frame,
    encode_wm_request_frame, encode_wm_session_descriptor_frame,
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmShortcutRegistry {
    bindings: BTreeMap<(u32, u32), WmActionId>,
    held: BTreeMap<u32, WmActionId>,
    capabilities: WmCapabilities,
    policy_generation: u64,
    chrome: WmChromePolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WmShortcutDecision {
    pub action: Option<WmActionId>,
    pub consumed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WmPolicyApplyOutcome {
    DeferredUntilShortcutIdle,
    Acknowledged(WmPolicyAck),
}

impl WmShortcutRegistry {
    pub fn from_policy_configuration(
        configuration: &PolicyConfiguration,
    ) -> Result<Self, WmIpcError> {
        let bindings = configuration
            .bindings
            .iter()
            .map(|binding| WmBindingRegistration {
                action: binding.action,
                keycode: binding.keycode,
                modifiers: binding.modifiers,
            })
            .collect();
        Self::from_hello(&WmHello {
            api_version: WM_API_VERSION,
            capabilities: WmCapabilities::all_supported(),
            policy_generation: configuration.generation,
            bindings,
            chrome: configuration.chrome,
        })
    }

    pub fn from_hello(hello: &WmHello) -> Result<Self, WmIpcError> {
        if hello.api_version != WM_API_VERSION {
            return Err(WmIpcError::Negotiation("unsupported WM API version"));
        }
        if hello.capabilities.bits & !WmCapabilities::SUPPORTED != 0 {
            return Err(WmIpcError::Negotiation("unsupported WM capability"));
        }
        if hello.policy_generation == 0 {
            return Err(WmIpcError::Negotiation("invalid WM policy generation"));
        }
        if !valid_chrome_policy(hello.chrome) {
            return Err(WmIpcError::Negotiation("invalid WM chrome policy"));
        }
        if hello.bindings.len() > WM_MAX_BINDINGS {
            return Err(WmIpcError::Negotiation("too many WM bindings"));
        }

        let mut bindings = BTreeMap::new();
        let mut actions = BTreeSet::new();
        for binding in &hello.bindings {
            if !binding.action.is_valid() || binding.keycode == 0 || binding.keycode > 0x2ff {
                return Err(WmIpcError::Negotiation("invalid WM binding"));
            }
            if binding.modifiers.bits & !WmModifierMask::SUPPORTED != 0 {
                return Err(WmIpcError::Negotiation("unsupported WM modifier"));
            }
            if binding.keycode == 14
                && binding.modifiers.bits & (WmModifierMask::CONTROL | WmModifierMask::ALT)
                    == WmModifierMask::CONTROL | WmModifierMask::ALT
            {
                return Err(WmIpcError::Negotiation("reserved emergency chord"));
            }
            if !actions.insert(binding.action) {
                return Err(WmIpcError::Negotiation("duplicate WM action"));
            }
            if bindings
                .insert((binding.keycode, binding.modifiers.bits), binding.action)
                .is_some()
            {
                return Err(WmIpcError::Negotiation("duplicate WM chord"));
            }
        }

        Ok(Self {
            bindings,
            held: BTreeMap::new(),
            capabilities: hello.capabilities,
            policy_generation: hello.policy_generation,
            chrome: hello.chrome,
        })
    }

    pub fn handle_key(
        &mut self,
        keycode: u32,
        modifiers: WmModifierMask,
        pressed: bool,
    ) -> WmShortcutDecision {
        if !pressed {
            return WmShortcutDecision {
                action: None,
                consumed: self.held.remove(&keycode).is_some(),
            };
        }
        let Some(action) = self.bindings.get(&(keycode, modifiers.bits)).copied() else {
            return WmShortcutDecision {
                action: None,
                consumed: false,
            };
        };
        let first_press = self.held.insert(keycode, action).is_none();
        WmShortcutDecision {
            action: first_press.then_some(action),
            consumed: true,
        }
    }

    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }

    pub const fn policy_generation(&self) -> u64 {
        self.policy_generation
    }

    pub const fn chrome(&self) -> WmChromePolicy {
        self.chrome
    }

    pub const fn supports_chrome_policy(&self) -> bool {
        self.capabilities.bits & WmCapabilities::POLICY_CHROME_V2 != 0
    }

    pub fn is_idle(&self) -> bool {
        self.held.is_empty()
    }
}

pub const WM_MAX_SHORTCUT_SEATS: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmShortcutRouter {
    registry: WmShortcutRegistry,
    seats: BTreeMap<SeatId, WmSeatShortcutState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WmSeatShortcutState {
    shortcuts: WmShortcutRegistry,
    modifiers: WmPhysicalModifierState,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct WmPhysicalModifierState {
    left_shift: bool,
    right_shift: bool,
    left_control: bool,
    right_control: bool,
    left_alt: bool,
    right_alt: bool,
    left_super: bool,
    right_super: bool,
}

impl WmShortcutRouter {
    pub fn new(registry: WmShortcutRegistry) -> Self {
        Self {
            registry,
            seats: BTreeMap::new(),
        }
    }

    pub fn replace_registry(&mut self, registry: WmShortcutRegistry) {
        self.registry = registry;
        self.seats.clear();
    }

    pub fn route_key(&mut self, seat: SeatId, keycode: u32, pressed: bool) -> WmShortcutDecision {
        if !seat.is_valid() {
            return WmShortcutDecision {
                action: None,
                consumed: false,
            };
        }
        if !self.seats.contains_key(&seat) {
            if self.seats.len() >= WM_MAX_SHORTCUT_SEATS {
                return WmShortcutDecision {
                    action: None,
                    consumed: false,
                };
            }
            self.seats.insert(
                seat,
                WmSeatShortcutState {
                    shortcuts: self.registry.clone(),
                    modifiers: WmPhysicalModifierState::default(),
                },
            );
        }
        let state = self.seats.get_mut(&seat).expect("seat was inserted");
        let decision = state
            .shortcuts
            .handle_key(keycode, state.modifiers.mask(), pressed);
        state.modifiers.update(keycode, pressed);
        decision
    }

    pub fn clear_seat(&mut self, seat: SeatId) -> bool {
        self.seats.remove(&seat).is_some()
    }

    pub fn modifier_mask(&self, seat: SeatId) -> WmModifierMask {
        self.seats
            .get(&seat)
            .map(|state| state.modifiers.mask())
            .unwrap_or(WmModifierMask { bits: 0 })
    }

    pub const fn policy_generation(&self) -> u64 {
        self.registry.policy_generation()
    }

    pub fn binding_count(&self) -> usize {
        self.registry.binding_count()
    }

    pub const fn chrome(&self) -> WmChromePolicy {
        self.registry.chrome()
    }

    pub const fn supports_chrome_policy(&self) -> bool {
        self.registry.supports_chrome_policy()
    }

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

    pub fn shortcut_idle(&self) -> bool {
        self.seats
            .values()
            .all(|state| state.shortcuts.is_idle() && state.modifiers.is_idle())
    }
}

fn valid_chrome_policy(chrome: WmChromePolicy) -> bool {
    let valid_style = |enabled: bool, width: u32| {
        width <= 64 && ((enabled && width > 0) || (!enabled && width == 0))
    };
    valid_style(chrome.focus_ring.enabled, chrome.focus_ring.width)
        && valid_style(chrome.frame.enabled, chrome.frame.width)
}

impl WmPhysicalModifierState {
    fn is_idle(self) -> bool {
        !self.left_shift
            && !self.right_shift
            && !self.left_control
            && !self.right_control
            && !self.left_alt
            && !self.right_alt
            && !self.left_super
            && !self.right_super
    }

    fn mask(self) -> WmModifierMask {
        let mut bits = 0;
        if self.left_shift || self.right_shift {
            bits |= WmModifierMask::SHIFT;
        }
        if self.left_control || self.right_control {
            bits |= WmModifierMask::CONTROL;
        }
        if self.left_alt || self.right_alt {
            bits |= WmModifierMask::ALT;
        }
        if self.left_super || self.right_super {
            bits |= WmModifierMask::SUPER;
        }
        WmModifierMask { bits }
    }

    fn update(&mut self, keycode: u32, pressed: bool) {
        match keycode {
            42 => self.left_shift = pressed,
            54 => self.right_shift = pressed,
            29 => self.left_control = pressed,
            97 => self.right_control = pressed,
            56 => self.left_alt = pressed,
            100 => self.right_alt = pressed,
            125 => self.left_super = pressed,
            126 => self.right_super = pressed,
            _ => {}
        }
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
