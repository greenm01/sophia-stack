use std::sync::mpsc::SyncSender;

use sophia_protocol::{ClientAdmissionId, RoutedInputRequest, Size, SurfaceId, TransactionId};

use crate::{XAuthorityOutputUpdateOutcome, XResourceId, XServerFrontendClientId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum XPresentCompletionMode {
    Copy = 0,
    Flip = 1,
    Skip = 2,
    SuboptimalCopy = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityKeyEvent {
    pub keycode: u8,
    pub pressed: bool,
    pub state: u16,
    pub time_msec: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XAuthorityPointerEventKind {
    Motion,
    Button { button: u8, pressed: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityPointerEvent {
    pub kind: XAuthorityPointerEventKind,
    pub surface: SurfaceId,
    pub root_x: i16,
    pub root_y: i16,
    pub event_x: i16,
    pub event_y: i16,
    pub state: u16,
    pub time_msec: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XAuthorityInputEvent {
    Key(XAuthorityKeyEvent),
    Pointer(XAuthorityPointerEvent),
}

/// An Engine-selected input event addressed to one live X11 connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityClientInputEvent {
    pub client: XServerFrontendClientId,
    pub event: XAuthorityInputEvent,
    pub target_window: Option<XResourceId>,
    pub xi_event_type: Option<u16>,
    pub xi_transition_mask: u16,
    pub delivery: Option<XAuthorityInputDeliveryId>,
}

/// Protocol-neutral physical input after Engine hit-testing and focus policy.
#[derive(Clone, Debug, PartialEq)]
pub struct XAuthorityRoutedInput {
    pub request: RoutedInputRequest,
    pub delivery: Option<XAuthorityInputDeliveryId>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct XAuthorityInputDeliveryId(u64);

impl XAuthorityInputDeliveryId {
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XAuthorityInputDeliveryOutcome {
    Flushed,
    RouteRejected,
    WriteFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityClientInputDelivery {
    pub client: XServerFrontendClientId,
    pub delivery: XAuthorityInputDeliveryId,
    pub outcome: XAuthorityInputDeliveryOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XAuthorityControlCommand {
    ConfigureSurface {
        transaction: TransactionId,
        surface: SurfaceId,
        size: Size,
    },
    FocusSurface {
        transaction: TransactionId,
        surface: SurfaceId,
    },
    CloseSurface {
        transaction: TransactionId,
        surface: SurfaceId,
    },
}

impl XAuthorityControlCommand {
    pub const fn transaction(self) -> TransactionId {
        match self {
            Self::ConfigureSurface { transaction, .. }
            | Self::FocusSurface { transaction, .. }
            | Self::CloseSurface { transaction, .. } => transaction,
        }
    }

    pub const fn surface(self) -> SurfaceId {
        match self {
            Self::ConfigureSurface { surface, .. }
            | Self::FocusSurface { surface, .. }
            | Self::CloseSurface { surface, .. } => surface,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityClientControlCommand {
    pub client: XServerFrontendClientId,
    pub command: XAuthorityControlCommand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XAuthorityControlOutcome {
    Delivered,
    UnknownSurface,
    InvalidSize,
    AuthorityRejected,
    UnsupportedProtocol,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityControlAck {
    pub transaction: TransactionId,
    pub surface: SurfaceId,
    pub outcome: XAuthorityControlOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityClientControlAck {
    pub client: XServerFrontendClientId,
    pub acknowledgement: XAuthorityControlAck,
}

#[derive(Clone, Debug)]
pub enum XServerFrontendServiceCommand {
    StopAccepting,
    StopAndDisconnect,
    RevokeAdmission {
        admission: ClientAdmissionId,
    },
    UpdateOutputTopology {
        snapshot: sophia_protocol::OutputTopologySnapshot,
        acknowledgement: SyncSender<XAuthorityOutputUpdateOutcome>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XServerFrontendRouteError {
    UnknownClient { client: XServerFrontendClientId },
    UnknownSurface { surface: SurfaceId },
    ClientQueueFull { client: XServerFrontendClientId },
    DuplicatePresentation { transaction: TransactionId },
    ClientQueueDisconnected { client: XServerFrontendClientId },
    DuplicateClient { client: XServerFrontendClientId },
    InputDeliveryQueueFull,
    RegistryPoisoned,
}

impl core::fmt::Display for XServerFrontendRouteError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownClient { client } => {
                write!(
                    formatter,
                    "X11 route targets unknown client {}",
                    client.raw()
                )
            }
            Self::UnknownSurface { surface } => write!(
                formatter,
                "X11 route targets unknown Sophia surface {}:{}",
                surface.index(),
                surface.generation()
            ),
            Self::ClientQueueFull { client } => {
                write!(
                    formatter,
                    "X11 route queue is full for client {}",
                    client.raw()
                )
            }
            Self::DuplicatePresentation { transaction } => write!(
                formatter,
                "X11 Present transaction {} is already pending",
                transaction.raw()
            ),
            Self::ClientQueueDisconnected { client } => write!(
                formatter,
                "X11 route queue disconnected for client {}",
                client.raw()
            ),
            Self::DuplicateClient { client } => {
                write!(
                    formatter,
                    "X11 route client {} is already registered",
                    client.raw()
                )
            }
            Self::InputDeliveryQueueFull => {
                formatter.write_str("X11 input delivery acknowledgement queue is full")
            }
            Self::RegistryPoisoned => formatter.write_str("X11 route registry lock poisoned"),
        }
    }
}

impl std::error::Error for XServerFrontendRouteError {}
