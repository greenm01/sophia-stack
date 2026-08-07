mod prelude {
    pub(crate) use core::fmt;
    pub(crate) use std::collections::{BTreeMap, BTreeSet};
    pub(crate) use std::fs;
    pub(crate) use std::io::{self, Read, Write};
    #[cfg(unix)]
    pub(crate) use std::os::unix::net::UnixStream;
    pub(crate) use std::path::{Path, PathBuf};
    pub(crate) use std::sync::mpsc::{Receiver, TryRecvError};
    pub(crate) use std::time::Duration;

    pub(crate) use sophia_portal::{
        MAX_NOTIFICATION_ACTION_LEN, MAX_NOTIFICATION_ACTIONS, MAX_NOTIFICATION_BODY_LEN,
        MAX_NOTIFICATION_SUMMARY_LEN, NotificationRequest, NotificationUrgency, PortalCommand,
    };
    pub(crate) use sophia_protocol::{
        AttentionState, AxisSpan, BrokerHealthPacket, BufferSource, ChromeActionKind,
        ChromeActionRequest, ChromeDescriptor, CommittedSurfaceState, DamageFrame, DeviceId,
        DisplayLabel, FrameSnapshot, IconTokenId, InputEventKind, InputEventPacket, InputRoute,
        InputRouteOutcome, IpcCodecError, LayerSnapshot, LayoutNodeSnapshot, LayoutTransaction,
        OutputEdge, OutputId, OutputReservation, Point, PortalTransferId, Rect, Region,
        RenderCommand, RenderCommandKind, ResizeSyncCapability, RoutedInputRequest,
        SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAX_PAYLOAD_LEN, SeatId, Size, SurfaceId,
        SurfaceOutputReservations, SurfacePresentationRole, SurfaceTransaction,
        SurfaceTransactionKey, SurfaceTransactionReadiness, TransactionCommit, TransactionId,
        TransactionOutcome, TrustLevel, WmRequestKind, WmRequestPacket, WmResponsePacket,
        WorkspaceId,
    };
    pub(crate) use sophia_runtime::{
        RestartPolicy, RuntimeScanoutState, SessionRuntimeCommand, SessionRuntimeLoop,
        SessionRuntimeObservation, SessionRuntimeObservationError, SessionRuntimeState,
        SophiaErrorExt, SophiaErrorKind, SupervisedProcessKind, SupervisorCommand, SupervisorEvent,
        SupervisorState, update_supervisor,
    };
    pub(crate) use tracing::{debug, instrument, trace, warn};
}

mod backend_assembly;
mod chrome;
mod compositor_graphics;
mod drm;
mod engine;
mod error;
mod frame;
mod input;
mod layout_epoch;
mod live_backend;
mod output;
mod policy_projection;
mod render;
mod runtime_driver;
mod session;
mod surface_admission;
mod surface_content_stream;
mod visual_state;
mod wm;

mod wm_policy;
pub use backend_assembly::*;
pub use chrome::*;
pub use compositor_graphics::*;
pub use drm::*;
pub use engine::*;
pub use error::*;
pub use frame::*;
pub use input::*;
pub use layout_epoch::*;
pub use live_backend::*;
pub use output::*;
pub use policy_projection::*;
pub use render::*;
pub use runtime_driver::*;
pub use session::*;
pub use surface_admission::*;
pub use surface_content_stream::*;
pub use visual_state::*;
pub use wm::*;

pub use sophia_runtime::{RuntimeScanoutState, SessionRuntimeObservation};
pub use wm_policy::*;
