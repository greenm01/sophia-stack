use crate::TransactionId;

pub const SOPHIA_IPC_MAGIC: u32 = 0x4850_4f53;
pub const SOPHIA_IPC_VERSION: u16 = 1;
pub const SOPHIA_IPC_HEADER_LEN: usize = 24;
pub const SOPHIA_IPC_MAX_PAYLOAD_LEN: usize = 64 * 1024;
pub const SOPHIA_IPC_MAX_ITEMS: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IpcMessageKind {
    WmRequest = 1,
    WmResponse = 2,
    BrokerHealth = 3,
    XAuthorityRequest = 4,
    XAuthorityResponse = 5,
    PortalBrokerRequest = 6,
    PortalBrokerResponse = 7,
    WmHello = 9,
    WmSessionDescriptor = 10,
    WmPolicyUpdate = 11,
    WmPolicyAck = 12,
    PortalClipboardPayload = 8,
    WmV1ClientHello = 32,
    WmV1ServerWelcome = 33,
    WmV1SnapshotBegin = 34,
    WmV1SnapshotChunk = 35,
    WmV1SnapshotEnd = 36,
    WmV1ProjectionRequest = 37,
    WmV1ProjectionBegin = 38,
    WmV1ProjectionChunk = 39,
    WmV1ProjectionEnd = 40,
    WmV1ProjectionOutcome = 41,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IpcFrameHeader {
    pub message_kind: IpcMessageKind,
    pub transaction: TransactionId,
    pub payload_len: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IpcCodecError {
    Truncated,
    BadMagic,
    UnsupportedVersion(u16),
    UnknownMessageKind(u16),
    PayloadTooLarge(usize),
    ReservedNonZero(u32),
    TrailingBytes(usize),
    CountTooLarge {
        count: usize,
        max: usize,
    },
    TextTooLarge {
        field: &'static str,
        len: usize,
        max: usize,
    },
    FieldTooLarge {
        field: &'static str,
        len: usize,
        max: usize,
    },
    InvalidTransaction(u64),
    InvalidUtf8 {
        field: &'static str,
    },
    InvalidEnum {
        field: &'static str,
        value: u32,
    },
    InvalidBool {
        field: &'static str,
        value: u8,
    },
}
