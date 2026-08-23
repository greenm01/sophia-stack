use std::os::fd::OwnedFd;

use sophia_protocol::{SurfaceId, SurfaceOutputReservations, TransactionId};

use crate::{
    XAuthorityCpuBufferUpdate, XDispatchResult, XResourceId, XServerFrontendClientId,
    XWireClientResourceRange,
};

/// Stable, value-free request stages that may cross the X authority boundary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum X11ObservedRequestStage {
    GlxQueryServerString,
    GlxGetVisualConfigs,
    GlxGetFbConfigs,
    GlxCreateContext,
    GlxMakeCurrent,
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
            Self::GlxGetVisualConfigs => "GLX:GetVisualConfigs",
            Self::GlxGetFbConfigs => "GLX:GetFBConfigs",
            Self::GlxCreateContext => "GLX:CreateContext",
            Self::GlxMakeCurrent => "GLX:MakeCurrent",
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
    /// Global authority order, distinct from the client-local X11 sequence.
    pub transaction: TransactionId,
    pub client: XServerFrontendClientId,
    pub admission: Option<sophia_protocol::ClientAdmissionContext>,
    pub resource_id_range: XWireClientResourceRange,
    pub sequence: u16,
    pub major_opcode: u8,
    pub minor_opcode: u16,
    pub request_stage: X11ObservedRequestStage,
    pub failure: Option<X11ObservedDispatchFailure>,
    pub result: XDispatchResult,
    /// Authoritative owner routes for the live surfaces changed by this
    /// dispatch. The causing client above is not necessarily their owner.
    pub surface_routes: Vec<crate::XAuthoritySurfaceRouteObservation>,
    /// Complete, protocol-neutral reservation snapshots changed by this
    /// dispatch. Protocol property IDs and bytes remain authority-private.
    pub surface_output_reservations: Vec<SurfaceOutputReservations>,
    pub cpu_buffer_updates: Vec<XAuthorityCpuBufferUpdate>,
    pub received_fd_count: usize,
    pub received_fds: Vec<OwnedFd>,
    pub dri3_pixmap_import: Option<XAuthorityDri3PixmapImport>,
    pub dri3_fence_import: Option<XAuthorityDri3FenceImport>,
    pub present_submission: Option<XAuthorityPresentSubmission>,
    pub software_present_submission: Option<XAuthoritySoftwarePresentSubmission>,
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
    pub x_offset: i32,
    pub y_offset: i32,
    pub acquire_fence: Option<sophia_protocol::FenceHandle>,
    pub idle_fence: Option<sophia_protocol::FenceHandle>,
}

/// A complete software Present whose immutable pixels travel as a CPU buffer.
///
/// The renderer does not need an X pixmap or DMA-BUF identity, but the
/// presentation lifecycle still owns acquire/idle fences and page-flip-paced
/// Complete/Idle feedback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthoritySoftwarePresentSubmission {
    pub transaction: TransactionId,
    pub surface: SurfaceId,
    pub acquire_fence: Option<sophia_protocol::FenceHandle>,
    pub idle_fence: Option<sophia_protocol::FenceHandle>,
}

/// One authority-generated content-set replacement answering Engine raster
/// demand. The response retains exact causal identity separately from its
/// ordinary surface transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XAuthorityRasterRequirementResponse {
    pub identity: sophia_protocol::SurfaceRasterResponseIdentity,
    pub transaction: sophia_protocol::SurfaceTransaction,
    pub cpu_buffer_updates: Vec<XAuthorityCpuBufferUpdate>,
}

/// Outcome of answering one Engine raster requirement.
///
/// Fallback is never silent: the surface keeps publishing its canonical
/// raster as sampled compatibility content, and the cause names the operation
/// that prevented an authority-owned native-density variant.
#[derive(Clone, Debug)]
pub enum XSurfaceRasterOutcome {
    Satisfied(Box<XAuthorityRasterRequirementResponse>),
    SampledFallback {
        cause: crate::XRasterFallbackCause,
        /// The authority's own content generation when the requirement was
        /// evaluated. Logged beside the requested generation so a run shows
        /// requested-versus-observed directly instead of implying it.
        observed_content_generation: u64,
    },
}
