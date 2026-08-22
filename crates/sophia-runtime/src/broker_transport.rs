use std::io::{Read as _, Write as _};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use sophia_protocol::{
    BrokerV1ClientHello, BrokerV1Request, BrokerV1Response, BrokerV1ServerWelcome, IpcCodecError,
    MAX_CHROME_LABEL_LEN, SOPHIA_BROKER_INTERFACE_REVISION, SOPHIA_BROKER_MAX_SURFACES,
    SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAX_PAYLOAD_LEN, TransactionId,
    decode_broker_v1_client_hello_frame, decode_broker_v1_request_frame,
    decode_broker_v1_response_frame, decode_broker_v1_server_welcome_frame,
    encode_broker_v1_client_hello_frame, encode_broker_v1_request_frame,
    encode_broker_v1_response_frame, encode_broker_v1_server_welcome_frame,
};

use crate::{PolicyRole, PolicyRoleEndpoint, PolicyRoleEndpointError, ProtectionDomainEvidence};

const BROKER_IO_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrokerTransportError {
    Endpoint(PolicyRoleEndpointError),
    Io(String),
    Codec(IpcCodecError),
    UnsupportedRevision,
    InvalidConnectionEpoch,
    WrongTransaction,
    NotConnected,
}

impl core::fmt::Display for BrokerTransportError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for BrokerTransportError {}

impl From<PolicyRoleEndpointError> for BrokerTransportError {
    fn from(error: PolicyRoleEndpointError) -> Self {
        Self::Endpoint(error)
    }
}

impl From<IpcCodecError> for BrokerTransportError {
    fn from(error: IpcCodecError) -> Self {
        Self::Codec(error)
    }
}

pub struct MetadataBrokerSessionTransport {
    endpoint: PolicyRoleEndpoint,
    stream: Option<UnixStream>,
    connection_epoch: u64,
}

impl MetadataBrokerSessionTransport {
    pub fn bind_for_supervised_uid(
        directory: impl AsRef<Path>,
        expected_uid: u32,
    ) -> Result<Self, BrokerTransportError> {
        Ok(Self {
            endpoint: PolicyRoleEndpoint::bind_role_for_supervised_uid(
                directory,
                PolicyRole::Broker,
                expected_uid,
            )?,
            stream: None,
            connection_epoch: 0,
        })
    }

    /// Admits the broker process its supervisor launched into a protection
    /// domain.
    ///
    /// There is no PID-only counterpart on this transport. The metadata broker
    /// is a metadata-bearing role, so a caller that spawned it unprotected has
    /// nothing to pass here and fails to compile rather than admitting quietly.
    pub fn authorize_protected_peer(
        &mut self,
        evidence: &ProtectionDomainEvidence,
    ) -> Result<(), BrokerTransportError> {
        self.endpoint.authorize_protected_peer(evidence)?;
        Ok(())
    }

    pub fn socket_path(&self) -> &Path {
        self.endpoint.socket_path()
    }

    pub fn accept_and_negotiate(
        &mut self,
        connection_epoch: u64,
        timeout: Duration,
    ) -> Result<BrokerV1ServerWelcome, BrokerTransportError> {
        if connection_epoch == 0 || connection_epoch <= self.connection_epoch {
            return Err(BrokerTransportError::InvalidConnectionEpoch);
        }
        let mut stream = self.endpoint.accept_expected_timeout(timeout)?;
        configure_stream(&stream)?;
        let hello = decode_broker_v1_client_hello_frame(&read_frame(&mut stream)?)?;
        if hello.minimum_revision == 0
            || hello.minimum_revision > hello.maximum_revision
            || !(hello.minimum_revision..=hello.maximum_revision)
                .contains(&SOPHIA_BROKER_INTERFACE_REVISION)
        {
            return Err(BrokerTransportError::UnsupportedRevision);
        }
        let welcome = BrokerV1ServerWelcome {
            selected_revision: SOPHIA_BROKER_INTERFACE_REVISION,
            connection_epoch,
            max_surfaces: SOPHIA_BROKER_MAX_SURFACES,
            max_label_bytes: MAX_CHROME_LABEL_LEN as u16,
        };
        write_frame(
            &mut stream,
            &encode_broker_v1_server_welcome_frame(welcome)?,
        )?;
        self.stream = Some(stream);
        self.connection_epoch = connection_epoch;
        Ok(welcome)
    }

    pub fn request(
        &mut self,
        transaction: TransactionId,
        request: &BrokerV1Request,
    ) -> Result<BrokerV1Response, BrokerTransportError> {
        if request.connection_epoch() != self.connection_epoch {
            return Err(BrokerTransportError::InvalidConnectionEpoch);
        }
        let stream = self
            .stream
            .as_mut()
            .ok_or(BrokerTransportError::NotConnected)?;
        write_frame(
            stream,
            &encode_broker_v1_request_frame(transaction, request)?,
        )?;
        let (response_transaction, response) =
            decode_broker_v1_response_frame(&read_frame(stream)?)?;
        if response_transaction != transaction {
            return Err(BrokerTransportError::WrongTransaction);
        }
        if response.connection_epoch() != self.connection_epoch {
            return Err(BrokerTransportError::InvalidConnectionEpoch);
        }
        Ok(response)
    }

    pub fn disconnect(&mut self) -> Result<(), BrokerTransportError> {
        self.stream = None;
        if let Some(peer) = self.endpoint.active_peer() {
            self.endpoint.release_peer(peer)?;
        }
        Ok(())
    }
}

pub struct MetadataBrokerClientTransport {
    stream: UnixStream,
    connection_epoch: u64,
}

impl MetadataBrokerClientTransport {
    pub fn connect(path: impl AsRef<Path>) -> Result<Self, BrokerTransportError> {
        let mut stream = UnixStream::connect(path)
            .map_err(|error| BrokerTransportError::Io(error.to_string()))?;
        configure_stream(&stream)?;
        let hello = BrokerV1ClientHello {
            minimum_revision: SOPHIA_BROKER_INTERFACE_REVISION,
            maximum_revision: SOPHIA_BROKER_INTERFACE_REVISION,
        };
        write_frame(&mut stream, &encode_broker_v1_client_hello_frame(hello)?)?;
        let welcome = decode_broker_v1_server_welcome_frame(&read_frame(&mut stream)?)?;
        if welcome.selected_revision != SOPHIA_BROKER_INTERFACE_REVISION
            || welcome.connection_epoch == 0
            || welcome.max_surfaces > SOPHIA_BROKER_MAX_SURFACES
            || usize::from(welcome.max_label_bytes) > MAX_CHROME_LABEL_LEN
        {
            return Err(BrokerTransportError::UnsupportedRevision);
        }
        // The role server is allowed to be idle for the life of the desktop.
        // Only the session's request/response side has a bounded reply wait;
        // retaining the handshake read timeout here would kill a healthy broker
        // after five seconds without a metadata change.
        stream
            .set_read_timeout(None)
            .map_err(|error| BrokerTransportError::Io(error.to_string()))?;
        Ok(Self {
            stream,
            connection_epoch: welcome.connection_epoch,
        })
    }

    pub const fn connection_epoch(&self) -> u64 {
        self.connection_epoch
    }

    pub fn receive(&mut self) -> Result<(TransactionId, BrokerV1Request), BrokerTransportError> {
        let (transaction, request) =
            decode_broker_v1_request_frame(&read_frame(&mut self.stream)?)?;
        if request.connection_epoch() != self.connection_epoch {
            return Err(BrokerTransportError::InvalidConnectionEpoch);
        }
        Ok((transaction, request))
    }

    pub fn respond(
        &mut self,
        transaction: TransactionId,
        response: &BrokerV1Response,
    ) -> Result<(), BrokerTransportError> {
        if response.connection_epoch() != self.connection_epoch {
            return Err(BrokerTransportError::InvalidConnectionEpoch);
        }
        write_frame(
            &mut self.stream,
            &encode_broker_v1_response_frame(transaction, response)?,
        )
    }
}

fn configure_stream(stream: &UnixStream) -> Result<(), BrokerTransportError> {
    stream
        .set_read_timeout(Some(BROKER_IO_TIMEOUT))
        .and_then(|()| stream.set_write_timeout(Some(BROKER_IO_TIMEOUT)))
        .map_err(|error| BrokerTransportError::Io(error.to_string()))
}

fn write_frame(stream: &mut UnixStream, frame: &[u8]) -> Result<(), BrokerTransportError> {
    stream
        .write_all(frame)
        .map_err(|error| BrokerTransportError::Io(error.to_string()))
}

fn read_frame(stream: &mut UnixStream) -> Result<Vec<u8>, BrokerTransportError> {
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    stream
        .read_exact(&mut header)
        .map_err(|error| BrokerTransportError::Io(error.to_string()))?;
    let payload_len = u32::from_le_bytes(
        header[16..20]
            .try_into()
            .expect("fixed frame payload range is present"),
    ) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(BrokerTransportError::Codec(IpcCodecError::PayloadTooLarge(
            payload_len,
        )));
    }
    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    stream
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .map_err(|error| BrokerTransportError::Io(error.to_string()))?;
    Ok(frame)
}
