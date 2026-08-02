use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XWindowMirror {
    pub window: XWindowId,
    pub parent: Option<XWindowId>,
    pub children: Vec<XWindowId>,
    pub toplevel: Option<XWindowId>,
    pub client: Option<XWindowId>,
    pub mapped: bool,
    pub stack_rank: u32,
    pub geometry: Rect,
    pub namespace: Option<NamespaceId>,
    pub stale_metadata: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoritySurface {
    pub authority: AuthorityKind,
    pub local_id: AuthorityLocalId,
    pub surface: SurfaceId,
    pub namespace: Option<NamespaceId>,
    pub presentation: SurfacePresentationRole,
    /// Protocol-neutral classification supplied by the frontend authority.
    pub kind: LayoutNodeKind,
    /// Initial placement preference. Policy remains authoritative over the
    /// resulting managed state.
    pub placement_preference: SurfacePlacementPreference,
    /// Opaque owner for an attached client-positioned surface.
    pub presentation_owner: Option<SurfaceId>,
    /// Authority-local bottom-to-top stacking order.
    pub stack_rank: u32,
    pub mapped: bool,
    pub geometry: Rect,
    pub constraints: SurfaceConstraints,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SurfacePresentationRole {
    #[default]
    PolicyManaged,
    ClientPositioned,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SurfacePlacementPreference {
    #[default]
    Default,
    Floating,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceConstraints {
    pub min_size: Option<Size>,
    pub max_size: Option<Size>,
}
