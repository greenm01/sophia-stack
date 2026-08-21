use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use sophia_protocol::{
    IpcCodecError, OutputAuthoritySnapshot, OutputV1Outcome, OutputV1Snapshot,
    SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAX_PAYLOAD_LEN, TransactionId,
    decode_output_v1_client_hello_frame, decode_output_v1_proposal_frame,
    encode_output_v1_outcome_frame, encode_output_v1_server_welcome_frame,
    encode_output_v1_snapshot_frame,
};

use crate::{
    AdmittedOutputProposal, OutputConnectionState, OutputProposalAdmission, OutputTransferError,
    PolicyPeerIdentity, PolicyRole, PolicyRoleEndpoint, PolicyRoleEndpointError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutputTransportError {
    Endpoint(PolicyRoleEndpointError),
    Transfer(OutputTransferError),
    Codec(IpcCodecError),
    Io(String),
    NotConnected,
    PeerDisconnected,
    ProposalRejected {
        transaction: TransactionId,
        error: OutputTransferError,
    },
}

/// A write that failed because the reader left is that reader leaving.
///
/// The read path already draws this line -- an exhausted stream is
/// `PeerDisconnected`, not an error -- and the write path did not, so any frame
/// sent to a client that had finished ended the whole service and took its
/// listening socket with it. A client is entitled to close as soon as it has
/// what it asked for; publishing a snapshot to one that just did is ordinary.
fn write_failure(error: std::io::Error) -> OutputTransportError {
    if peer_departed(&error) {
        return OutputTransportError::PeerDisconnected;
    }
    OutputTransportError::Io(error.to_string())
}

fn read_failure(error: std::io::Error) -> OutputTransportError {
    if peer_departed(&error) {
        return OutputTransportError::PeerDisconnected;
    }
    OutputTransportError::Io(error.to_string())
}

/// Whether this error is the peer having gone rather than the link having
/// broken.
///
/// A unix stream says so differently depending on which way the socket was
/// being used: a write to a departed reader breaks the pipe, a read with
/// nothing left ends unexpectedly, and a close that discarded frames still
/// queued for that peer resets the connection. One predicate, because they are
/// one situation.
fn peer_departed(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::UnexpectedEof
    )
}

impl core::fmt::Display for OutputTransportError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for OutputTransportError {}

impl From<PolicyRoleEndpointError> for OutputTransportError {
    fn from(error: PolicyRoleEndpointError) -> Self {
        Self::Endpoint(error)
    }
}

impl From<OutputTransferError> for OutputTransportError {
    fn from(error: OutputTransferError) -> Self {
        Self::Transfer(error)
    }
}

impl From<IpcCodecError> for OutputTransportError {
    fn from(error: IpcCodecError) -> Self {
        Self::Codec(error)
    }
}

pub struct OutputSessionTransport {
    endpoint: PolicyRoleEndpoint,
    connection: OutputConnectionState,
    stream: Option<UnixStream>,
    peer: Option<PolicyPeerIdentity>,
    read_buffer: Vec<u8>,
    peer_closed: bool,
}

impl OutputSessionTransport {
    fn with_endpoint(endpoint: PolicyRoleEndpoint) -> Self {
        Self {
            endpoint,
            connection: OutputConnectionState::default(),
            stream: None,
            peer: None,
            read_buffer: Vec::new(),
            peer_closed: false,
        }
    }

    pub fn bind(
        directory: impl AsRef<Path>,
        expected_peer: PolicyPeerIdentity,
    ) -> Result<Self, OutputTransportError> {
        Ok(Self::with_endpoint(PolicyRoleEndpoint::bind_role(
            directory,
            PolicyRole::Output,
            expected_peer,
        )?))
    }

    pub fn bind_for_supervised_uid(
        directory: impl AsRef<Path>,
        expected_uid: u32,
    ) -> Result<Self, OutputTransportError> {
        Ok(Self::with_endpoint(
            PolicyRoleEndpoint::bind_role_for_supervised_uid(
                directory,
                PolicyRole::Output,
                expected_uid,
            )?,
        ))
    }

    pub fn authorize_supervised_pid(&mut self, pid: u32) -> Result<(), OutputTransportError> {
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
    ) -> Result<(), OutputTransportError> {
        if self.stream.is_some() {
            return Err(OutputTransportError::Transfer(
                OutputTransferError::AlreadyConnected,
            ));
        }
        let mut stream = self.endpoint.accept_expected_timeout(timeout)?;
        let peer = self
            .endpoint
            .active_peer()
            .expect("accepted endpoint records its peer");
        let result = (|| {
            stream
                .set_read_timeout(Some(timeout))
                .map_err(|error| OutputTransportError::Io(error.to_string()))?;
            stream
                .set_write_timeout(Some(timeout))
                .map_err(|error| OutputTransportError::Io(error.to_string()))?;
            let hello = decode_output_v1_client_hello_frame(&read_output_frame(&mut stream)?)?;
            let mut connection = self.connection.clone();
            connection.connect(connection_epoch)?;
            let welcome = connection.negotiate(hello)?;
            stream
                .write_all(&encode_output_v1_server_welcome_frame(welcome)?)
                .and_then(|()| stream.flush())
                .map_err(|error| OutputTransportError::Io(error.to_string()))?;
            self.connection = connection;
            Ok(())
        })();
        if let Err(error) = result {
            let _ = self.endpoint.release_peer(peer);
            return Err(error);
        }
        self.stream = Some(stream);
        self.peer = Some(peer);
        self.read_buffer.clear();
        self.peer_closed = false;
        Ok(())
    }

    pub fn send_snapshot(
        &mut self,
        transaction: TransactionId,
        snapshot: &OutputAuthoritySnapshot,
    ) -> Result<(), OutputTransportError> {
        self.connection.require_observe()?;
        let frame = encode_output_v1_snapshot_frame(
            transaction,
            &OutputV1Snapshot {
                connection_epoch: self.connection.connection_epoch(),
                snapshot: snapshot.clone(),
            },
        )?;
        self.write_frame(&frame)
    }

    pub fn receive_proposal(
        &mut self,
        snapshot: &OutputAuthoritySnapshot,
    ) -> Result<(TransactionId, OutputProposalAdmission), OutputTransportError> {
        let frame = read_output_frame(
            self.stream
                .as_mut()
                .ok_or(OutputTransportError::NotConnected)?,
        )?;
        let (transaction, proposal) = decode_output_v1_proposal_frame(&frame)?;
        match self
            .connection
            .admit_proposal(transaction, proposal, snapshot)
        {
            Ok(admission) => Ok((transaction, admission)),
            Err(error) => Err(OutputTransportError::ProposalRejected { transaction, error }),
        }
    }

    /// Polls one complete proposal without blocking on a partial local-stream
    /// frame.
    ///
    /// The retained byte buffer is part of the transport, not the session
    /// owner. This lets an optional supervised output client pause between
    /// header and payload while the owner continues rendering, hotplug work,
    /// and bounded shutdown.
    pub fn try_receive_admitted_proposal(
        &mut self,
        snapshot: &OutputAuthoritySnapshot,
    ) -> Result<Option<(AdmittedOutputProposal, OutputProposalAdmission)>, OutputTransportError>
    {
        self.read_available()?;
        let Some(frame_len) = buffered_output_frame_len(&self.read_buffer)? else {
            if self.peer_closed {
                return Err(OutputTransportError::PeerDisconnected);
            }
            return Ok(None);
        };
        let frame = self.read_buffer.drain(..frame_len).collect::<Vec<_>>();
        let (transaction, proposal) = decode_output_v1_proposal_frame(&frame)?;
        let admitted = AdmittedOutputProposal {
            transaction,
            message: proposal.clone(),
        };
        match self
            .connection
            .admit_proposal(transaction, proposal, snapshot)
        {
            Ok(admission) => Ok(Some((admitted, admission))),
            Err(error) => Err(OutputTransportError::ProposalRejected { transaction, error }),
        }
    }

    pub fn send_outcome(
        &mut self,
        transaction: TransactionId,
        outcome: OutputV1Outcome,
    ) -> Result<(), OutputTransportError> {
        if outcome.connection_epoch != self.connection.connection_epoch() {
            return Err(OutputTransportError::Transfer(
                OutputTransferError::InvalidConnectionEpoch,
            ));
        }
        let frame = encode_output_v1_outcome_frame(transaction, outcome)?;
        self.write_frame(&frame)
    }

    pub fn settle_active(
        &mut self,
        transaction: TransactionId,
    ) -> Result<Option<&AdmittedOutputProposal>, OutputTransportError> {
        self.connection
            .settle_active(transaction)
            .map_err(Into::into)
    }

    pub fn disconnect(&mut self) -> Result<Vec<AdmittedOutputProposal>, OutputTransportError> {
        if self.stream.take().is_none() {
            return Err(OutputTransportError::NotConnected);
        }
        let abandoned = self.connection.disconnect()?;
        self.read_buffer.clear();
        self.peer_closed = false;
        let peer = self
            .peer
            .take()
            .expect("connected output transport records its peer");
        self.endpoint.release_peer(peer)?;
        Ok(abandoned)
    }

    pub const fn connection(&self) -> &OutputConnectionState {
        &self.connection
    }

    fn write_frame(&mut self, frame: &[u8]) -> Result<(), OutputTransportError> {
        self.stream
            .as_mut()
            .ok_or(OutputTransportError::NotConnected)?
            .write_all(frame)
            .and_then(|()| {
                self.stream
                    .as_mut()
                    .expect("connected stream remains present")
                    .flush()
            })
            .map_err(write_failure)
    }

    fn read_available(&mut self) -> Result<(), OutputTransportError> {
        let stream = self
            .stream
            .as_mut()
            .ok_or(OutputTransportError::NotConnected)?;
        stream
            .set_nonblocking(true)
            .map_err(|error| OutputTransportError::Io(error.to_string()))?;
        let mut result = Ok(());
        let mut bytes = [0u8; 4096];
        loop {
            match stream.read(&mut bytes) {
                Ok(0) => {
                    self.peer_closed = true;
                    break;
                }
                Ok(count) => {
                    self.read_buffer.extend_from_slice(&bytes[..count]);
                    if self.read_buffer.len()
                        > SOPHIA_IPC_HEADER_LEN.saturating_add(SOPHIA_IPC_MAX_PAYLOAD_LEN)
                    {
                        result = Err(OutputTransportError::Codec(IpcCodecError::PayloadTooLarge(
                            self.read_buffer.len().saturating_sub(SOPHIA_IPC_HEADER_LEN),
                        )));
                        break;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                // The departure this reader meets: a client that closed while
                // frames were still queued for it discards them, and the poll
                // that follows draws a reset rather than the clean end above.
                Err(error) if peer_departed(&error) => {
                    self.peer_closed = true;
                    break;
                }
                Err(error) => {
                    result = Err(OutputTransportError::Io(error.to_string()));
                    break;
                }
            }
        }
        if let Err(error) = stream.set_nonblocking(false) {
            return Err(OutputTransportError::Io(error.to_string()));
        }
        result
    }
}

fn buffered_output_frame_len(buffer: &[u8]) -> Result<Option<usize>, OutputTransportError> {
    if buffer.len() < SOPHIA_IPC_HEADER_LEN {
        return Ok(None);
    }
    let payload_len = u32::from_le_bytes(
        buffer[16..20]
            .try_into()
            .expect("buffered frame payload range is present"),
    ) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(OutputTransportError::Codec(IpcCodecError::PayloadTooLarge(
            payload_len,
        )));
    }
    let frame_len = SOPHIA_IPC_HEADER_LEN + payload_len;
    Ok((buffer.len() >= frame_len).then_some(frame_len))
}

fn read_output_frame(stream: &mut UnixStream) -> Result<Vec<u8>, OutputTransportError> {
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    // A departed peer reaches a reader two ways, and `read_exact` turns both
    // into errors: no bytes at all is `UnexpectedEof`, and a close that
    // discarded frames still queued for it is a reset. Neither is a broken
    // link, and reading them as one ended the service that owns the socket.
    stream.read_exact(&mut header).map_err(read_failure)?;
    let payload_len = u32::from_le_bytes(
        header[16..20]
            .try_into()
            .expect("fixed frame payload range is present"),
    ) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(OutputTransportError::Codec(IpcCodecError::PayloadTooLarge(
            payload_len,
        )));
    }
    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    stream
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .map_err(read_failure)?;
    Ok(frame)
}
