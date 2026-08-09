use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use sophia_protocol::{
    IpcCodecError, IpcMessageKind, PolicyConfiguration, PolicyDirtyRequest,
    PolicyProjectionOutcome, PolicySessionOperationOutcome, PolicySessionOperationRequest,
    SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAX_PAYLOAD_LEN, SOPHIA_WM_CAPABILITY_CONFIGURATION,
    SOPHIA_WM_CAPABILITY_POLICY_DIRTY, SOPHIA_WM_CAPABILITY_SESSION_OPERATIONS, TransactionId,
    WmV1PolicyConfigurationOutcome, WmV1SnapshotBegin, WmV1SnapshotChunk, WmV1SnapshotEnd,
    decode_frame, decode_wm_v1_client_hello_frame, decode_wm_v1_policy_configuration,
    decode_wm_v1_policy_configuration_frame, decode_wm_v1_policy_dirty,
    decode_wm_v1_policy_dirty_frame, decode_wm_v1_policy_session_operation_request,
    decode_wm_v1_projection_begin_frame, decode_wm_v1_projection_chunk_frame,
    decode_wm_v1_projection_end_frame, decode_wm_v1_session_operation_request_frame,
    encode_wm_v1_policy_configuration_outcome_frame, encode_wm_v1_policy_projection_outcome,
    encode_wm_v1_policy_projection_request, encode_wm_v1_policy_session_operation_outcome,
    encode_wm_v1_projection_outcome_frame, encode_wm_v1_projection_request_frame,
    encode_wm_v1_server_welcome_frame, encode_wm_v1_session_operation_outcome_frame,
    encode_wm_v1_snapshot_begin_frame, encode_wm_v1_snapshot_chunk_frame,
    encode_wm_v1_snapshot_end_frame,
};

use crate::{
    PolicyConnectionState, PolicyPeerIdentity, PolicyRoleEndpoint, PolicyRoleEndpointError,
    PolicySnapshotAssembler, PolicyTransferError, QueuedPolicyProjection,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyTransportError {
    Endpoint(PolicyRoleEndpointError),
    Transfer(PolicyTransferError),
    Codec(IpcCodecError),
    Io(String),
    UnexpectedMessage(IpcMessageKind),
    NotConnected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyClientEvent {
    ProjectionPending,
    Projection(QueuedPolicyProjection),
    Configuration {
        transaction: TransactionId,
        configuration: PolicyConfiguration,
    },
    Dirty {
        transaction: TransactionId,
        request: PolicyDirtyRequest,
    },
    SessionOperation {
        transaction: TransactionId,
        request: PolicySessionOperationRequest,
    },
}

impl core::fmt::Display for PolicyTransportError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for PolicyTransportError {}

impl From<PolicyRoleEndpointError> for PolicyTransportError {
    fn from(error: PolicyRoleEndpointError) -> Self {
        Self::Endpoint(error)
    }
}

impl From<PolicyTransferError> for PolicyTransportError {
    fn from(error: PolicyTransferError) -> Self {
        Self::Transfer(error)
    }
}

impl From<IpcCodecError> for PolicyTransportError {
    fn from(error: IpcCodecError) -> Self {
        Self::Codec(error)
    }
}

/// Draft session-owned WM transport. It is not connected to the installed v7
/// path until the public-protocol milestone reaches its migration gate.
pub struct PolicyWmSessionTransport {
    endpoint: PolicyRoleEndpoint,
    connection: PolicyConnectionState,
    stream: Option<UnixStream>,
    read_buffer: Vec<u8>,
    peer: Option<PolicyPeerIdentity>,
}

impl PolicyWmSessionTransport {
    pub fn bind(
        directory: impl AsRef<Path>,
        expected_peer: PolicyPeerIdentity,
    ) -> Result<Self, PolicyTransportError> {
        Ok(Self {
            endpoint: PolicyRoleEndpoint::bind(directory, expected_peer)?,
            connection: PolicyConnectionState::default(),
            stream: None,
            read_buffer: Vec::new(),
            peer: None,
        })
    }

    pub fn bind_for_supervised_uid(
        directory: impl AsRef<Path>,
        expected_uid: u32,
    ) -> Result<Self, PolicyTransportError> {
        Ok(Self {
            endpoint: PolicyRoleEndpoint::bind_for_supervised_uid(directory, expected_uid)?,
            connection: PolicyConnectionState::default(),
            stream: None,
            read_buffer: Vec::new(),
            peer: None,
        })
    }

    pub fn authorize_supervised_pid(&mut self, pid: u32) -> Result<(), PolicyTransportError> {
        self.endpoint.authorize_supervised_pid(pid)?;
        Ok(())
    }

    pub fn socket_path(&self) -> &Path {
        self.endpoint.socket_path()
    }

    pub fn accept_and_negotiate(
        &mut self,
        connection_epoch: u64,
        timeout: Duration,
    ) -> Result<(), PolicyTransportError> {
        if self.stream.is_some() {
            return Err(PolicyTransportError::Transfer(
                PolicyTransferError::AlreadyConnected,
            ));
        }
        let mut stream = self.endpoint.accept_expected()?;
        let peer = self
            .endpoint
            .active_peer()
            .expect("accepted endpoint records its peer");
        let result = (|| {
            stream
                .set_read_timeout(Some(timeout))
                .map_err(|error| PolicyTransportError::Io(error.to_string()))?;
            stream
                .set_write_timeout(Some(timeout))
                .map_err(|error| PolicyTransportError::Io(error.to_string()))?;
            let frame = read_policy_frame(&mut stream)?;
            let hello = decode_wm_v1_client_hello_frame(&frame)?;
            let mut connection = self.connection.clone();
            connection.connect(connection_epoch)?;
            let welcome = connection.negotiate(&hello)?;
            let frame = encode_wm_v1_server_welcome_frame(&welcome)?;
            stream
                .write_all(&frame)
                .and_then(|()| stream.flush())
                .map_err(|error| PolicyTransportError::Io(error.to_string()))?;
            self.connection = connection;
            Ok(())
        })();
        if let Err(error) = result {
            let _ = self.endpoint.release_peer(peer);
            return Err(error);
        }
        self.stream = Some(stream);
        self.peer = Some(peer);
        Ok(())
    }

    pub fn receive_projection_part(
        &mut self,
    ) -> Result<Option<QueuedPolicyProjection>, PolicyTransportError> {
        match self.receive_client_event()? {
            PolicyClientEvent::ProjectionPending => Ok(None),
            PolicyClientEvent::Projection(projection) => Ok(Some(projection)),
            PolicyClientEvent::Configuration { .. } => Err(
                PolicyTransportError::UnexpectedMessage(IpcMessageKind::WmV1PolicyConfiguration),
            ),
            PolicyClientEvent::Dirty { .. } => Err(PolicyTransportError::UnexpectedMessage(
                IpcMessageKind::WmV1PolicyDirty,
            )),
            PolicyClientEvent::SessionOperation { .. } => {
                Err(PolicyTransportError::UnexpectedMessage(
                    IpcMessageKind::WmV1SessionOperationRequest,
                ))
            }
        }
    }

    pub fn receive_client_event(&mut self) -> Result<PolicyClientEvent, PolicyTransportError> {
        let frame = self
            .receive_frame(true)?
            .expect("blocking receive returns one frame");
        self.decode_client_event(&frame)
    }

    /// Polls one complete control frame without consuming a partial frame.
    pub fn try_receive_client_event(
        &mut self,
    ) -> Result<Option<PolicyClientEvent>, PolicyTransportError> {
        self.receive_frame(false)?
            .map(|frame| self.decode_client_event(&frame))
            .transpose()
    }

    fn decode_client_event(
        &mut self,
        frame: &[u8],
    ) -> Result<PolicyClientEvent, PolicyTransportError> {
        let (header, _) = decode_frame(&frame)?;
        match header.message_kind {
            IpcMessageKind::WmV1ProjectionBegin => {
                let (transaction, begin) = decode_wm_v1_projection_begin_frame(&frame)?;
                self.connection.begin_projection(transaction, begin)?;
                Ok(PolicyClientEvent::ProjectionPending)
            }
            IpcMessageKind::WmV1ProjectionChunk => {
                let (transaction, chunk) = decode_wm_v1_projection_chunk_frame(&frame)?;
                self.connection
                    .append_projection_chunk(transaction, chunk)?;
                Ok(PolicyClientEvent::ProjectionPending)
            }
            IpcMessageKind::WmV1ProjectionEnd => {
                let (transaction, end) = decode_wm_v1_projection_end_frame(&frame)?;
                self.connection.finish_projection(transaction, end)?;
                Ok(PolicyClientEvent::Projection(
                    self.connection
                        .settle_queued()
                        .expect("a finished projection queues one transfer"),
                ))
            }
            IpcMessageKind::WmV1PolicyConfiguration => {
                let (transaction, wire) = decode_wm_v1_policy_configuration_frame(&frame)?;
                let configuration = decode_wm_v1_policy_configuration(&wire)?;
                self.connection.admit_control_message(
                    transaction,
                    configuration.connection_epoch,
                    SOPHIA_WM_CAPABILITY_CONFIGURATION,
                )?;
                Ok(PolicyClientEvent::Configuration {
                    transaction,
                    configuration,
                })
            }
            IpcMessageKind::WmV1PolicyDirty => {
                let (transaction, wire) = decode_wm_v1_policy_dirty_frame(&frame)?;
                let request = decode_wm_v1_policy_dirty(&wire)?;
                self.connection.admit_control_message(
                    transaction,
                    request.connection_epoch,
                    SOPHIA_WM_CAPABILITY_POLICY_DIRTY,
                )?;
                Ok(PolicyClientEvent::Dirty {
                    transaction,
                    request,
                })
            }
            IpcMessageKind::WmV1SessionOperationRequest => {
                let (transaction, wire) = decode_wm_v1_session_operation_request_frame(&frame)?;
                let request = decode_wm_v1_policy_session_operation_request(&wire)?;
                self.connection.admit_control_message(
                    transaction,
                    request.connection_epoch,
                    SOPHIA_WM_CAPABILITY_SESSION_OPERATIONS,
                )?;
                Ok(PolicyClientEvent::SessionOperation {
                    transaction,
                    request,
                })
            }
            other => Err(PolicyTransportError::UnexpectedMessage(other)),
        }
    }

    fn receive_frame(&mut self, blocking: bool) -> Result<Option<Vec<u8>>, PolicyTransportError> {
        if let Some(frame) = take_buffered_frame(&mut self.read_buffer)? {
            return Ok(Some(frame));
        }
        let stream = self
            .stream
            .as_mut()
            .ok_or(PolicyTransportError::NotConnected)?;
        stream
            .set_nonblocking(!blocking)
            .map_err(|error| PolicyTransportError::Io(error.to_string()))?;
        let mut bytes = [0_u8; 8192];
        loop {
            match stream.read(&mut bytes) {
                Ok(0) => return Err(PolicyTransportError::Io("policy peer closed".into())),
                Ok(count) => {
                    self.read_buffer.extend_from_slice(&bytes[..count]);
                    if let Some(frame) = take_buffered_frame(&mut self.read_buffer)? {
                        if !blocking {
                            stream
                                .set_nonblocking(false)
                                .map_err(|error| PolicyTransportError::Io(error.to_string()))?;
                        }
                        return Ok(Some(frame));
                    }
                }
                Err(error) if !blocking && error.kind() == std::io::ErrorKind::WouldBlock => {
                    stream
                        .set_nonblocking(false)
                        .map_err(|error| PolicyTransportError::Io(error.to_string()))?;
                    return Ok(None);
                }
                Err(error) => return Err(PolicyTransportError::Io(error.to_string())),
            }
        }
    }

    pub fn send_snapshot(
        &mut self,
        transaction: TransactionId,
        begin: &WmV1SnapshotBegin,
        chunks: &[WmV1SnapshotChunk],
        end: &WmV1SnapshotEnd,
    ) -> Result<(), PolicyTransportError> {
        let stream = self
            .stream
            .as_mut()
            .ok_or(PolicyTransportError::NotConnected)?;
        let mut assembler = PolicySnapshotAssembler::new(begin.connection_epoch)?;
        assembler.begin(transaction, begin.clone())?;
        for chunk in chunks {
            assembler.append(transaction, chunk.clone())?;
        }
        assembler.finish(transaction, end.clone())?;

        let mut frames = Vec::with_capacity(chunks.len() + 2);
        frames.push(encode_wm_v1_snapshot_begin_frame(transaction, begin)?);
        for chunk in chunks {
            frames.push(encode_wm_v1_snapshot_chunk_frame(transaction, chunk)?);
        }
        frames.push(encode_wm_v1_snapshot_end_frame(transaction, end)?);
        for frame in frames {
            stream
                .write_all(&frame)
                .map_err(|error| PolicyTransportError::Io(error.to_string()))?;
        }
        stream
            .flush()
            .map_err(|error| PolicyTransportError::Io(error.to_string()))
    }

    pub fn send_projection_request(
        &mut self,
        transaction: TransactionId,
        request: &sophia_protocol::PolicyProjectionRequest,
    ) -> Result<(), PolicyTransportError> {
        let stream = self
            .stream
            .as_mut()
            .ok_or(PolicyTransportError::NotConnected)?;
        let request = encode_wm_v1_policy_projection_request(request)?;
        let frame = encode_wm_v1_projection_request_frame(transaction, &request)?;
        stream
            .write_all(&frame)
            .and_then(|()| stream.flush())
            .map_err(|error| PolicyTransportError::Io(error.to_string()))
    }

    pub fn send_projection_outcome(
        &mut self,
        transaction: TransactionId,
        request_id: u64,
        scene_generation: u64,
        outcome: PolicyProjectionOutcome,
    ) -> Result<(), PolicyTransportError> {
        let stream = self
            .stream
            .as_mut()
            .ok_or(PolicyTransportError::NotConnected)?;
        let outcome = encode_wm_v1_policy_projection_outcome(
            self.connection.connection_epoch(),
            request_id,
            scene_generation,
            outcome,
        )?;
        let frame = encode_wm_v1_projection_outcome_frame(transaction, &outcome)?;
        stream
            .write_all(&frame)
            .and_then(|()| stream.flush())
            .map_err(|error| PolicyTransportError::Io(error.to_string()))
    }

    pub fn send_configuration_outcome(
        &mut self,
        transaction: TransactionId,
        generation: u64,
        outcome: PolicyProjectionOutcome,
    ) -> Result<(), PolicyTransportError> {
        let stream = self
            .stream
            .as_mut()
            .ok_or(PolicyTransportError::NotConnected)?;
        let outcome = match outcome {
            PolicyProjectionOutcome::Committed => sophia_protocol::SOPHIA_WM_OUTCOME_COMMITTED,
            PolicyProjectionOutcome::RejectedStale => {
                sophia_protocol::SOPHIA_WM_OUTCOME_REJECTED_STALE
            }
            PolicyProjectionOutcome::RejectedInvalid => {
                sophia_protocol::SOPHIA_WM_OUTCOME_REJECTED_INVALID
            }
            PolicyProjectionOutcome::TimedOut => sophia_protocol::SOPHIA_WM_OUTCOME_TIMED_OUT,
            PolicyProjectionOutcome::Disconnected => {
                sophia_protocol::SOPHIA_WM_OUTCOME_DISCONNECTED
            }
        };
        let frame = encode_wm_v1_policy_configuration_outcome_frame(
            transaction,
            &WmV1PolicyConfigurationOutcome {
                connection_epoch: self.connection.connection_epoch(),
                configuration_generation: generation,
                outcome,
            },
        )?;
        stream
            .write_all(&frame)
            .and_then(|()| stream.flush())
            .map_err(|error| PolicyTransportError::Io(error.to_string()))
    }

    pub fn send_session_operation_outcome(
        &mut self,
        transaction: TransactionId,
        request_id: u64,
        outcome: PolicyProjectionOutcome,
    ) -> Result<(), PolicyTransportError> {
        let stream = self
            .stream
            .as_mut()
            .ok_or(PolicyTransportError::NotConnected)?;
        let outcome =
            encode_wm_v1_policy_session_operation_outcome(PolicySessionOperationOutcome {
                connection_epoch: self.connection.connection_epoch(),
                request_id,
                outcome,
            })?;
        let frame = encode_wm_v1_session_operation_outcome_frame(transaction, &outcome)?;
        stream
            .write_all(&frame)
            .and_then(|()| stream.flush())
            .map_err(|error| PolicyTransportError::Io(error.to_string()))
    }

    pub fn disconnect(&mut self) -> Result<(), PolicyTransportError> {
        if self.stream.take().is_none() {
            return Err(PolicyTransportError::NotConnected);
        }
        self.connection.disconnect()?;
        self.read_buffer.clear();
        let peer = self
            .peer
            .take()
            .expect("connected transport records its peer");
        self.endpoint.release_peer(peer)?;
        Ok(())
    }

    pub const fn connection(&self) -> &PolicyConnectionState {
        &self.connection
    }
}

fn take_buffered_frame(buffer: &mut Vec<u8>) -> Result<Option<Vec<u8>>, PolicyTransportError> {
    if buffer.len() < SOPHIA_IPC_HEADER_LEN {
        return Ok(None);
    }
    let payload_len = u32::from_le_bytes(
        buffer[16..20]
            .try_into()
            .expect("fixed frame payload range is present"),
    ) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(PolicyTransportError::Codec(IpcCodecError::PayloadTooLarge(
            payload_len,
        )));
    }
    let frame_len = SOPHIA_IPC_HEADER_LEN + payload_len;
    if buffer.len() < frame_len {
        return Ok(None);
    }
    Ok(Some(buffer.drain(..frame_len).collect()))
}

fn read_policy_frame(stream: &mut UnixStream) -> Result<Vec<u8>, PolicyTransportError> {
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    stream
        .read_exact(&mut header)
        .map_err(|error| PolicyTransportError::Io(error.to_string()))?;
    let payload_len = u32::from_le_bytes(
        header[16..20]
            .try_into()
            .expect("fixed frame payload range is present"),
    ) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(PolicyTransportError::Codec(IpcCodecError::PayloadTooLarge(
            payload_len,
        )));
    }
    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    stream
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .map_err(|error| PolicyTransportError::Io(error.to_string()))?;
    Ok(frame)
}
