use std::collections::VecDeque;
use std::io::{Read as _, Write as _};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use sophia_protocol::{
    IpcCodecError, SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAX_PAYLOAD_LEN,
    SOPHIA_SHELL_CAPABILITY_DESCRIPTOR_SWITCHER, SOPHIA_SHELL_INTERFACE_REVISION,
    SOPHIA_SHELL_MAX_DESCRIPTORS, SOPHIA_SHELL_MAX_PENDING_ACTIVATIONS, ShellV1Activation,
    ShellV1ActivationAck, ShellV1Candidate, ShellV1CandidateOutcome, ShellV1ClientHello,
    ShellV1DescriptorSnapshot, ShellV1ServerWelcome, TransactionId,
    decode_shell_v1_activation_ack_frame, decode_shell_v1_activation_frame,
    decode_shell_v1_candidate_frame, decode_shell_v1_candidate_outcome_frame,
    decode_shell_v1_client_hello_frame, decode_shell_v1_descriptor_snapshot_frame,
    decode_shell_v1_server_welcome_frame, encode_shell_v1_activation_ack_frame,
    encode_shell_v1_activation_frame, encode_shell_v1_candidate_frame,
    encode_shell_v1_candidate_outcome_frame, encode_shell_v1_client_hello_frame,
    encode_shell_v1_descriptor_snapshot_frame, encode_shell_v1_server_welcome_frame,
};

use crate::{PolicyRole, PolicyRoleEndpoint, PolicyRoleEndpointError, ProtectionDomainEvidence};

const SHELL_IO_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShellTransportError {
    Endpoint(PolicyRoleEndpointError),
    Io(String),
    Codec(IpcCodecError),
    UnsupportedRevision,
    MissingCapability,
    InvalidConnectionEpoch,
    WrongTransaction,
    WrongCandidate,
    WrongActivation,
    ActivationQueueSaturated,
    NotConnected,
}

impl core::fmt::Display for ShellTransportError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ShellTransportError {}

impl From<PolicyRoleEndpointError> for ShellTransportError {
    fn from(error: PolicyRoleEndpointError) -> Self {
        Self::Endpoint(error)
    }
}

impl From<IpcCodecError> for ShellTransportError {
    fn from(error: IpcCodecError) -> Self {
        Self::Codec(error)
    }
}

pub struct ShellSessionTransport {
    endpoint: PolicyRoleEndpoint,
    stream: Option<UnixStream>,
    connection_epoch: u64,
    last_candidate_generation: u64,
    pending_candidate: Option<PendingShellCandidate>,
    presented_candidate: Option<(u64, u64)>,
    pending_activations: VecDeque<(TransactionId, u64)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingShellCandidate {
    transaction: TransactionId,
    generation: u64,
    visible: bool,
    prepared: bool,
}

impl ShellSessionTransport {
    pub fn bind_for_supervised_uid(
        directory: impl AsRef<Path>,
        expected_uid: u32,
    ) -> Result<Self, ShellTransportError> {
        Ok(Self {
            endpoint: PolicyRoleEndpoint::bind_role_for_supervised_uid(
                directory,
                PolicyRole::Shell,
                expected_uid,
            )?,
            stream: None,
            connection_epoch: 0,
            last_candidate_generation: 0,
            pending_candidate: None,
            presented_candidate: None,
            pending_activations: VecDeque::with_capacity(SOPHIA_SHELL_MAX_PENDING_ACTIVATIONS),
        })
    }

    pub fn authorize_protected_peer(
        &mut self,
        evidence: &ProtectionDomainEvidence,
    ) -> Result<(), ShellTransportError> {
        self.endpoint.authorize_protected_peer(evidence)?;
        Ok(())
    }

    pub fn socket_path(&self) -> &Path {
        self.endpoint.socket_path()
    }

    pub const fn connection_epoch(&self) -> u64 {
        self.connection_epoch
    }

    pub fn accept_and_negotiate(
        &mut self,
        connection_epoch: u64,
        timeout: Duration,
    ) -> Result<ShellV1ServerWelcome, ShellTransportError> {
        if connection_epoch == 0 || connection_epoch <= self.connection_epoch {
            return Err(ShellTransportError::InvalidConnectionEpoch);
        }
        let mut stream = self.endpoint.accept_expected_timeout(timeout)?;
        configure_stream(&stream)?;
        let hello = decode_shell_v1_client_hello_frame(&read_frame(&mut stream)?)?;
        if hello.minimum_revision == 0
            || hello.minimum_revision > hello.maximum_revision
            || !(hello.minimum_revision..=hello.maximum_revision)
                .contains(&SOPHIA_SHELL_INTERFACE_REVISION)
        {
            return Err(ShellTransportError::UnsupportedRevision);
        }
        if hello.required_capabilities & SOPHIA_SHELL_CAPABILITY_DESCRIPTOR_SWITCHER == 0 {
            return Err(ShellTransportError::MissingCapability);
        }
        let welcome = ShellV1ServerWelcome {
            selected_revision: SOPHIA_SHELL_INTERFACE_REVISION,
            connection_epoch,
            capabilities: SOPHIA_SHELL_CAPABILITY_DESCRIPTOR_SWITCHER,
            max_descriptors: SOPHIA_SHELL_MAX_DESCRIPTORS as u16,
            max_label_bytes: sophia_protocol::MAX_CHROME_LABEL_LEN as u16,
            max_pending_activations: SOPHIA_SHELL_MAX_PENDING_ACTIVATIONS as u16,
        };
        write_frame(&mut stream, &encode_shell_v1_server_welcome_frame(welcome)?)?;
        self.pending_activations.clear();
        self.last_candidate_generation = 0;
        self.pending_candidate = None;
        self.presented_candidate = None;
        self.connection_epoch = connection_epoch;
        self.stream = Some(stream);
        Ok(welcome)
    }

    pub fn request_candidate(
        &mut self,
        transaction: TransactionId,
        snapshot: &ShellV1DescriptorSnapshot,
    ) -> Result<ShellV1Candidate, ShellTransportError> {
        self.require_epoch(snapshot.connection_epoch)?;
        if self.pending_candidate.is_some() {
            return Err(ShellTransportError::WrongCandidate);
        }
        let stream = self.stream()?;
        write_frame(
            stream,
            &encode_shell_v1_descriptor_snapshot_frame(transaction, snapshot)?,
        )?;
        let (response_transaction, candidate) =
            decode_shell_v1_candidate_frame(&read_frame(stream)?)?;
        if response_transaction != transaction {
            return Err(ShellTransportError::WrongTransaction);
        }
        self.require_epoch(candidate.connection_epoch)?;
        if candidate.snapshot_generation != snapshot.snapshot_generation
            || candidate.output != snapshot.output
            || candidate.candidate_generation <= self.last_candidate_generation
            || candidate.entries.iter().any(|entry| {
                !snapshot.descriptors.iter().any(|descriptor| {
                    descriptor.slot == entry.slot && descriptor.generation == entry.generation
                })
            })
        {
            return Err(ShellTransportError::WrongCandidate);
        }
        self.last_candidate_generation = candidate.candidate_generation;
        self.pending_candidate = Some(PendingShellCandidate {
            transaction,
            generation: candidate.candidate_generation,
            visible: candidate.visible,
            prepared: false,
        });
        Ok(candidate)
    }

    pub fn send_candidate_outcome(
        &mut self,
        transaction: TransactionId,
        outcome: ShellV1CandidateOutcome,
    ) -> Result<(), ShellTransportError> {
        self.require_epoch(outcome.connection_epoch)?;
        let mut pending = self
            .pending_candidate
            .ok_or(ShellTransportError::WrongCandidate)?;
        if pending.transaction != transaction || pending.generation != outcome.candidate_generation
        {
            return Err(ShellTransportError::WrongCandidate);
        }
        match outcome.kind {
            sophia_protocol::ShellV1CandidateOutcomeKind::Prepared if !pending.prepared => {
                pending.prepared = true;
            }
            sophia_protocol::ShellV1CandidateOutcomeKind::Presented if pending.prepared => {}
            sophia_protocol::ShellV1CandidateOutcomeKind::Rejected
            | sophia_protocol::ShellV1CandidateOutcomeKind::Superseded => {}
            _ => return Err(ShellTransportError::WrongCandidate),
        }
        let frame = encode_shell_v1_candidate_outcome_frame(transaction, outcome)?;
        write_frame(self.stream()?, &frame)?;
        match outcome.kind {
            sophia_protocol::ShellV1CandidateOutcomeKind::Prepared => {
                self.pending_candidate = Some(pending);
            }
            sophia_protocol::ShellV1CandidateOutcomeKind::Presented => {
                self.presented_candidate = Some((
                    pending.generation,
                    if pending.visible {
                        outcome.presentation_epoch
                    } else {
                        0
                    },
                ));
                self.pending_candidate = None;
            }
            sophia_protocol::ShellV1CandidateOutcomeKind::Rejected
            | sophia_protocol::ShellV1CandidateOutcomeKind::Superseded => {
                self.pending_candidate = None;
            }
        }
        Ok(())
    }

    pub fn queue_activation(
        &mut self,
        transaction: TransactionId,
        activation: ShellV1Activation,
    ) -> Result<(), ShellTransportError> {
        self.require_epoch(activation.connection_epoch)?;
        if self.presented_candidate
            != Some((
                activation.candidate_generation,
                activation.presentation_epoch,
            ))
            || activation.action.recipient_epoch != self.connection_epoch
        {
            return Err(ShellTransportError::WrongActivation);
        }
        if self.pending_activations.len() >= SOPHIA_SHELL_MAX_PENDING_ACTIVATIONS {
            self.disconnect()?;
            return Err(ShellTransportError::ActivationQueueSaturated);
        }
        let frame = encode_shell_v1_activation_frame(transaction, activation)?;
        write_frame(self.stream()?, &frame)?;
        self.pending_activations
            .push_back((transaction, activation.activation));
        Ok(())
    }

    pub fn receive_activation_ack(&mut self) -> Result<ShellV1ActivationAck, ShellTransportError> {
        let (expected_transaction, expected_activation) = self
            .pending_activations
            .front()
            .copied()
            .ok_or(ShellTransportError::WrongActivation)?;
        let frame = read_frame(self.stream()?)?;
        let (transaction, ack) = decode_shell_v1_activation_ack_frame(&frame)?;
        if transaction != expected_transaction {
            return Err(ShellTransportError::WrongTransaction);
        }
        self.require_epoch(ack.connection_epoch)?;
        if ack.activation != expected_activation {
            return Err(ShellTransportError::WrongActivation);
        }
        self.pending_activations.pop_front();
        Ok(ack)
    }

    pub fn disconnect(&mut self) -> Result<(), ShellTransportError> {
        self.stream = None;
        self.pending_candidate = None;
        self.presented_candidate = None;
        self.pending_activations.clear();
        if let Some(peer) = self.endpoint.active_peer() {
            self.endpoint.release_peer(peer)?;
        }
        Ok(())
    }

    fn require_epoch(&self, epoch: u64) -> Result<(), ShellTransportError> {
        if epoch == self.connection_epoch && epoch != 0 {
            Ok(())
        } else {
            Err(ShellTransportError::InvalidConnectionEpoch)
        }
    }

    fn stream(&mut self) -> Result<&mut UnixStream, ShellTransportError> {
        self.stream
            .as_mut()
            .ok_or(ShellTransportError::NotConnected)
    }
}

pub struct ShellClientTransport {
    stream: UnixStream,
    connection_epoch: u64,
}

impl ShellClientTransport {
    pub fn connect(path: impl AsRef<Path>) -> Result<Self, ShellTransportError> {
        let mut stream = UnixStream::connect(path)
            .map_err(|error| ShellTransportError::Io(error.to_string()))?;
        configure_stream(&stream)?;
        let hello = ShellV1ClientHello {
            minimum_revision: SOPHIA_SHELL_INTERFACE_REVISION,
            maximum_revision: SOPHIA_SHELL_INTERFACE_REVISION,
            required_capabilities: SOPHIA_SHELL_CAPABILITY_DESCRIPTOR_SWITCHER,
        };
        write_frame(&mut stream, &encode_shell_v1_client_hello_frame(hello)?)?;
        let welcome = decode_shell_v1_server_welcome_frame(&read_frame(&mut stream)?)?;
        if welcome.selected_revision != SOPHIA_SHELL_INTERFACE_REVISION
            || welcome.capabilities & SOPHIA_SHELL_CAPABILITY_DESCRIPTOR_SWITCHER == 0
        {
            return Err(ShellTransportError::UnsupportedRevision);
        }
        stream
            .set_read_timeout(None)
            .map_err(|error| ShellTransportError::Io(error.to_string()))?;
        Ok(Self {
            stream,
            connection_epoch: welcome.connection_epoch,
        })
    }

    pub const fn connection_epoch(&self) -> u64 {
        self.connection_epoch
    }

    pub fn receive_snapshot(
        &mut self,
    ) -> Result<(TransactionId, ShellV1DescriptorSnapshot), ShellTransportError> {
        let (transaction, snapshot) =
            decode_shell_v1_descriptor_snapshot_frame(&read_frame(&mut self.stream)?)?;
        self.require_epoch(snapshot.connection_epoch)?;
        Ok((transaction, snapshot))
    }

    pub fn send_candidate(
        &mut self,
        transaction: TransactionId,
        candidate: &ShellV1Candidate,
    ) -> Result<(), ShellTransportError> {
        self.require_epoch(candidate.connection_epoch)?;
        write_frame(
            &mut self.stream,
            &encode_shell_v1_candidate_frame(transaction, candidate)?,
        )
    }

    pub fn receive_candidate_outcome(
        &mut self,
    ) -> Result<(TransactionId, ShellV1CandidateOutcome), ShellTransportError> {
        let (transaction, outcome) =
            decode_shell_v1_candidate_outcome_frame(&read_frame(&mut self.stream)?)?;
        self.require_epoch(outcome.connection_epoch)?;
        Ok((transaction, outcome))
    }

    pub fn receive_activation(
        &mut self,
    ) -> Result<(TransactionId, ShellV1Activation), ShellTransportError> {
        let (transaction, activation) =
            decode_shell_v1_activation_frame(&read_frame(&mut self.stream)?)?;
        self.require_epoch(activation.connection_epoch)?;
        Ok((transaction, activation))
    }

    pub fn acknowledge_activation(
        &mut self,
        transaction: TransactionId,
        ack: ShellV1ActivationAck,
    ) -> Result<(), ShellTransportError> {
        self.require_epoch(ack.connection_epoch)?;
        write_frame(
            &mut self.stream,
            &encode_shell_v1_activation_ack_frame(transaction, ack)?,
        )
    }

    fn require_epoch(&self, epoch: u64) -> Result<(), ShellTransportError> {
        if epoch == self.connection_epoch && epoch != 0 {
            Ok(())
        } else {
            Err(ShellTransportError::InvalidConnectionEpoch)
        }
    }
}

fn configure_stream(stream: &UnixStream) -> Result<(), ShellTransportError> {
    stream
        .set_read_timeout(Some(SHELL_IO_TIMEOUT))
        .and_then(|()| stream.set_write_timeout(Some(SHELL_IO_TIMEOUT)))
        .map_err(|error| ShellTransportError::Io(error.to_string()))
}

fn write_frame(stream: &mut UnixStream, frame: &[u8]) -> Result<(), ShellTransportError> {
    stream
        .write_all(frame)
        .map_err(|error| ShellTransportError::Io(error.to_string()))
}

fn read_frame(stream: &mut UnixStream) -> Result<Vec<u8>, ShellTransportError> {
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    stream
        .read_exact(&mut header)
        .map_err(|error| ShellTransportError::Io(error.to_string()))?;
    let payload_len = u32::from_le_bytes(
        header[16..20]
            .try_into()
            .expect("fixed frame payload range is present"),
    ) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(ShellTransportError::Codec(IpcCodecError::PayloadTooLarge(
            payload_len,
        )));
    }
    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    stream
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .map_err(|error| ShellTransportError::Io(error.to_string()))?;
    Ok(frame)
}
