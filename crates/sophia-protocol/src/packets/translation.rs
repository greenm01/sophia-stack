use crate::{OutputId, SurfaceId};

/// A bounded, policy-declared group of final placements sharing a translation.
/// Group identity is private to one WM connection and has no layout semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyTranslationGroup {
    pub output: OutputId,
    pub group: u64,
    pub x: i32,
    pub y: i32,
    pub members: Vec<SurfaceId>,
}

/// Validated group membership carried with a committed layer into rendering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayerTranslation {
    pub connection_epoch: u64,
    pub group: u64,
    pub x: i32,
    pub y: i32,
}
