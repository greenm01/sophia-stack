use super::*;

/// Protocol-neutral edge emitted when an authority client requests or
/// withdraws presentation of a surface.
///
/// Current presentation state remains an authority snapshot. This record is
/// deliberately an edge so a policy coordinator can distinguish a new request
/// from an unchanged unmapped surface without learning frontend identifiers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfacePresentationIntent {
    pub surface: SurfaceId,
    pub kind: SurfacePresentationIntentKind,
    pub role: SurfacePresentationRole,
    pub geometry: Rect,
    pub constraints: SurfaceConstraints,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfacePresentationIntentKind {
    Request,
    Withdraw,
}
