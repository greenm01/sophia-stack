use std::os::fd::OwnedFd;

use sophia_protocol::{SurfaceId, TransactionId};

use crate::{
    XAuthorityCpuBufferUpdate, XDispatchResult, XResourceId, XServerFrontendClientId,
    XWireClientResourceRange,
};

/// Stable, value-free request stages that may cross the X authority boundary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum X11ObservedRequestStage {
    GlxQueryServerString,
    GlxGetFbConfigs,
    GlxCreateContext,
    GlxCreateWindow,
    Dri3PixmapFromBuffers,
    PresentPixmap,
    KeyboardMapping,
    SelectionRequest,
    DisconnectCleanup,
    Other,
}

impl X11ObservedRequestStage {
    pub const fn evidence_name(self) -> &'static str {
        match self {
            Self::GlxQueryServerString => "GLX:QueryServerString",
            Self::GlxGetFbConfigs => "GLX:GetFBConfigs",
            Self::GlxCreateContext => "GLX:CreateContext",
            Self::GlxCreateWindow => "GLX:CreateWindow",
            Self::Dri3PixmapFromBuffers => "DRI3:PixmapFromBuffers",
            Self::PresentPixmap => "PRESENT:Pixmap",
            Self::KeyboardMapping => "GetKeyboardMapping",
            Self::SelectionRequest => "RequestSelection",
            Self::DisconnectCleanup => "DisconnectCleanup",
            Self::Other => "Other",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum X11ObservedDispatchFailure {
    ParseRejected,
}

/// Owned, bounded facts emitted after one X request dispatch.
///
/// Raw request strings and protocol object values remain inside the authority.
#[derive(Debug)]
pub struct X11DispatchObservation {
    pub client: XServerFrontendClientId,
    pub resource_id_range: XWireClientResourceRange,
    pub sequence: u16,
    pub major_opcode: u8,
    pub request_stage: X11ObservedRequestStage,
    pub failure: Option<X11ObservedDispatchFailure>,
    pub result: XDispatchResult,
    pub cpu_buffer_update: Option<XAuthorityCpuBufferUpdate>,
    pub received_fd_count: usize,
    pub received_fds: Vec<OwnedFd>,
    pub dri3_pixmap_import: Option<XAuthorityDri3PixmapImport>,
    pub dri3_fence_import: Option<XAuthorityDri3FenceImport>,
    pub present_submission: Option<XAuthorityPresentSubmission>,
    pub released_dma_bufs: Vec<sophia_protocol::BufferHandle>,
    pub released_fences: Vec<sophia_protocol::FenceHandle>,
    pub server_reply_fd_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityDri3PixmapImport {
    pub pixmap: XResourceId,
    pub descriptor: sophia_protocol::DmaBufDescriptor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityDri3FenceImport {
    pub fence: XResourceId,
    pub handle: sophia_protocol::FenceHandle,
    pub initially_triggered: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityPresentSubmission {
    pub transaction: TransactionId,
    pub surface: SurfaceId,
    pub buffer: sophia_protocol::BufferHandle,
    pub x_offset: i16,
    pub y_offset: i16,
    pub acquire_fence: Option<sophia_protocol::FenceHandle>,
    pub idle_fence: Option<sophia_protocol::FenceHandle>,
}
