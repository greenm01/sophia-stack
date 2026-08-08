use crate::{
    LayoutNodeCapabilities, OutputId, Rect, Size, SurfaceConstraints, SurfaceId, TransactionId,
    WmActionId, WmBindingRegistration, WmChromePolicy,
};

pub const POLICY_MAX_OUTPUTS: usize = 16;
pub const POLICY_MAX_SURFACES: usize = 1024;
pub const POLICY_MAX_BINDINGS: usize = 256;
pub const POLICY_MAX_INDICATORS: usize = 256;
pub const POLICY_MAX_OUTPUT_STATUSES: usize = 16;
pub const POLICY_MAX_INDICATORS_PER_OUTPUT: usize = 32;
pub const POLICY_INDICATOR_STATE_ACTIVE: u16 = 1 << 0;
pub const POLICY_INDICATOR_STATE_URGENT: u16 = 1 << 1;
pub const POLICY_INDICATOR_STATE_OCCUPIED: u16 = 1 << 2;
pub const POLICY_INDICATOR_STATE_VISIBLE_ELSEWHERE: u16 = 1 << 3;
pub const POLICY_INDICATOR_STATE_MASK: u16 = (1 << 4) - 1;
pub const POLICY_OUTPUT_STATUS_HAS_FOCUSED_SURFACE: u16 = 1;
pub const POLICY_OUTPUT_STATUS_FOCUS_MASK: u16 = POLICY_OUTPUT_STATUS_HAS_FOCUSED_SURFACE;

/// A complete, metadata-free output record issued to spatial policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyOutputSnapshot {
    pub output: OutputId,
    pub generation: u64,
    pub focus: Option<SurfaceId>,
    pub bounds: Rect,
    pub work_area: Rect,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u16)]
pub enum PolicySurfaceKind {
    Toplevel = 1,
    Dialog = 2,
    Utility = 3,
    Popup = 4,
    #[default]
    Unknown = 5,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PolicyPresentationState {
    pub fullscreen: bool,
    pub maximized: bool,
    pub minimized: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicySessionOperation {
    pub token: u64,
    /// A profile-local identifier shared by the session and policy client.
    /// Engine never assigns semantics to this value.
    pub slot: u16,
    pub permits_surface_target: bool,
}

/// A complete, metadata-free manageable-surface record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicySurfaceSnapshot {
    pub surface: SurfaceId,
    pub generation: u64,
    /// The committed output, or `None` while the surface is hidden.
    pub current_output: Option<OutputId>,
    pub kind: PolicySurfaceKind,
    pub capabilities: LayoutNodeCapabilities,
    pub constraints: SurfaceConstraints,
    pub exact_size: Option<Size>,
    pub requested_state: PolicyPresentationState,
    pub current_state: PolicyPresentationState,
    pub transient_owner: Option<SurfaceId>,
    pub geometry: Rect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicySceneSnapshot {
    pub generation: u64,
    pub outputs: Vec<PolicyOutputSnapshot>,
    pub surfaces: Vec<PolicySurfaceSnapshot>,
    pub session_operations: Vec<PolicySessionOperation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyConfiguration {
    pub connection_epoch: u64,
    pub generation: u64,
    pub bindings: Vec<WmBindingRegistration>,
    pub chrome: WmChromePolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyDirtyRequest {
    pub connection_epoch: u64,
    pub policy_generation: u64,
    pub affected_outputs: Vec<OutputId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicySessionOperationRequest {
    pub connection_epoch: u64,
    pub request_id: u64,
    pub operation: u64,
    pub target: Option<SurfaceId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicySessionOperationOutcome {
    pub connection_epoch: u64,
    pub request_id: u64,
    pub outcome: PolicyProjectionOutcome,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u16)]
pub enum PolicyInteractionPhase {
    #[default]
    Begin = 1,
    Update = 2,
    End = 3,
    Cancel = 4,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u16)]
pub enum PolicyInteractionKind {
    #[default]
    Move = 1,
    Resize = 2,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PolicyRequestCause {
    #[default]
    SceneChanged,
    Action {
        activation_serial: u64,
        action: WmActionId,
    },
    Focus {
        target: SurfaceId,
    },
    Interaction {
        phase: PolicyInteractionPhase,
        kind: PolicyInteractionKind,
        target: SurfaceId,
        geometry: Rect,
    },
}

/// Identity of one server-issued projection request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyProjectionRequest {
    pub connection_epoch: u64,
    pub request_id: u64,
    pub scene_generation: u64,
    pub affected_outputs: Vec<OutputId>,
    pub cause: PolicyRequestCause,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PolicySurfacePlacement {
    pub surface: SurfaceId,
    pub surface_generation: u64,
    pub geometry: Rect,
    pub requested_size: Option<Size>,
    pub crop: Option<Rect>,
    pub transform: PolicyTransform,
    pub presentation: PolicyPresentationState,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u16)]
pub enum PolicyTransform {
    #[default]
    Identity = 1,
}

/// Complete replacement for one affected output. Vector order is back to front.
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyOutputProjection {
    pub output: OutputId,
    pub placements: Vec<PolicySurfacePlacement>,
    pub focus: Option<SurfaceId>,
}

/// Policy-authored presentation state committed with an output projection.
/// Engine validates the record's shape but never assigns workspace semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyProjectionIndicator {
    pub output: OutputId,
    pub slot: u32,
    pub indicator: u64,
    pub action: Option<WmActionId>,
    pub state_bits: u16,
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyProjectionOutputStatus {
    pub output: OutputId,
    pub focus_bits: u16,
    pub layout: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PolicyProjectionProposal {
    pub transaction: TransactionId,
    pub connection_epoch: u64,
    pub request_id: u64,
    pub base_generation: u64,
    pub outputs: Vec<PolicyOutputProjection>,
    pub indicators: Vec<PolicyProjectionIndicator>,
    pub output_statuses: Vec<PolicyProjectionOutputStatus>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyProjectionOutcome {
    Committed,
    RejectedStale,
    RejectedInvalid,
    TimedOut,
    Disconnected,
}
