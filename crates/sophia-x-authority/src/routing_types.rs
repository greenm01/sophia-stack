use std::sync::mpsc::SyncSender;

use sophia_protocol::{
    ApplicationRouteLeaseIdentity, ClientAdmissionContext, ClientAdmissionId, Rect,
    RoutedInputRequest, SurfaceId, TransactionId,
};

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
    pub modifiers_after: u8,
    pub time_msec: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XAuthorityPointerEventKind {
    Motion,
    Button {
        button: u8,
        pressed: bool,
    },
    Axis {
        button: u8,
        pressed: bool,
        horizontal_position_v120: Option<i32>,
        vertical_position_v120: Option<i32>,
    },
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
    pub xi_event_window: Option<XResourceId>,
    pub xi_emulated_button_type: Option<u16>,
    pub xi_emulated_button_window: Option<XResourceId>,
    /// Selected XI2 pointer Enter/Leave events for this route.
    ///
    /// Keyboard FocusIn/FocusOut belongs to the authority focus transition,
    /// not to a later physical key delivery.
    pub xi_pointer_crossing_mask: u16,
    pub delivery: Option<XAuthorityInputDeliveryId>,
}

/// Protocol-neutral physical input after Engine hit-testing and focus policy.
#[derive(Clone, Debug, PartialEq)]
pub struct XAuthorityRoutedInput {
    pub request: RoutedInputRequest,
    pub route_lease: Option<ApplicationRouteLeaseIdentity>,
    pub delivery: Option<XAuthorityInputDeliveryId>,
    pub mode: XAuthorityRoutedInputMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XAuthorityRouteLeaseUpdateKind {
    Confirmed,
    Rejected,
    Released,
}

/// Sanitized frontend observation for one Engine-issued application lease.
/// X resource IDs and frontend connection IDs never cross this boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityRouteLeaseUpdate {
    pub identity: ApplicationRouteLeaseIdentity,
    pub target_surface: SurfaceId,
    pub admission: ClientAdmissionContext,
    pub kind: XAuthorityRouteLeaseUpdateKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityRouteLeaseRelease {
    pub identity: ApplicationRouteLeaseIdentity,
    pub admission: ClientAdmissionContext,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XAuthorityRoutedInputMode {
    Deliver,
    Repeat,
    StateOnly,
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
    TargetGone,
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
    AdmitSurface {
        transaction: TransactionId,
        surface: SurfaceId,
        geometry: Rect,
    },
    ConfigureSurface {
        transaction: TransactionId,
        surface: SurfaceId,
        geometry: Rect,
    },
    SetPresentationState {
        transaction: TransactionId,
        surface: SurfaceId,
        state: sophia_protocol::PolicyPresentationState,
    },
    RestorePresentationState {
        transaction: TransactionId,
        surface: SurfaceId,
        state: sophia_protocol::PolicyPresentationState,
    },
    FocusSurface {
        transaction: TransactionId,
        surface: SurfaceId,
    },
    ClearFocus {
        transaction: TransactionId,
        surface: SurfaceId,
    },
    CloseSurface {
        transaction: TransactionId,
        surface: SurfaceId,
    },
    WithdrawSurface {
        transaction: TransactionId,
        surface: SurfaceId,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum XAuthorityControlKind {
    AdmitSurface,
    ConfigureSurface,
    SetPresentationState,
    RestorePresentationState,
    FocusSurface,
    ClearFocus,
    CloseSurface,
    WithdrawSurface,
}

impl XAuthorityControlCommand {
    pub const fn kind(self) -> XAuthorityControlKind {
        match self {
            Self::AdmitSurface { .. } => XAuthorityControlKind::AdmitSurface,
            Self::ConfigureSurface { .. } => XAuthorityControlKind::ConfigureSurface,
            Self::SetPresentationState { .. } => XAuthorityControlKind::SetPresentationState,
            Self::RestorePresentationState { .. } => {
                XAuthorityControlKind::RestorePresentationState
            }
            Self::FocusSurface { .. } => XAuthorityControlKind::FocusSurface,
            Self::ClearFocus { .. } => XAuthorityControlKind::ClearFocus,
            Self::CloseSurface { .. } => XAuthorityControlKind::CloseSurface,
            Self::WithdrawSurface { .. } => XAuthorityControlKind::WithdrawSurface,
        }
    }

    pub const fn transaction(self) -> TransactionId {
        match self {
            Self::AdmitSurface { transaction, .. }
            | Self::ConfigureSurface { transaction, .. }
            | Self::SetPresentationState { transaction, .. }
            | Self::RestorePresentationState { transaction, .. }
            | Self::FocusSurface { transaction, .. }
            | Self::ClearFocus { transaction, .. }
            | Self::CloseSurface { transaction, .. }
            | Self::WithdrawSurface { transaction, .. } => transaction,
        }
    }

    pub const fn surface(self) -> SurfaceId {
        match self {
            Self::AdmitSurface { surface, .. }
            | Self::ConfigureSurface { surface, .. }
            | Self::SetPresentationState { surface, .. }
            | Self::RestorePresentationState { surface, .. }
            | Self::FocusSurface { surface, .. }
            | Self::ClearFocus { surface, .. }
            | Self::CloseSurface { surface, .. }
            | Self::WithdrawSurface { surface, .. } => surface,
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
    ClientGone,
    UnknownSurface,
    InvalidSize,
    AuthorityRejected,
    UnsupportedProtocol,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityControlAck {
    pub kind: XAuthorityControlKind,
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
    RegistryPoisoned,
}

/// Tracks the two independently ordered lifecycle phases of one X Present.
///
/// Copy normally idles its source before display completion; Flip normally
/// completes before its retained source becomes idle. The route remains live
/// until both phases have arrived exactly once.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XPresentFeedbackPhases {
    complete: bool,
    idle: bool,
}

impl XPresentFeedbackPhases {
    pub fn observe_complete(&mut self) -> bool {
        if self.complete {
            return false;
        }
        self.complete = true;
        true
    }

    pub fn observe_idle(&mut self) -> bool {
        if self.idle {
            return false;
        }
        self.idle = true;
        true
    }

    pub const fn finished(self) -> bool {
        self.complete && self.idle
    }
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
            Self::RegistryPoisoned => formatter.write_str("X11 route registry lock poisoned"),
        }
    }
}

impl std::error::Error for XServerFrontendRouteError {}
