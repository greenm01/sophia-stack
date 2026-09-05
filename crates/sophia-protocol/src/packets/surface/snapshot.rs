use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceSnapshot {
    pub surface: SurfaceId,
    pub window: XWindowId,
    pub toplevel: Option<XWindowId>,
    pub client: Option<XWindowId>,
    pub namespace: Option<NamespaceId>,
    pub mapped: bool,
    pub stack_rank: u32,
    pub geometry: Rect,
    pub source: BufferSource,
    pub damage: Region,
    pub generation: u64,
    pub resize_sync: ResizeSyncCapability,
}

impl SurfaceSnapshot {
    pub fn to_authority_surface(&self, authority: AuthorityKind) -> AuthoritySurface {
        AuthoritySurface {
            authority,
            local_id: AuthorityLocalId::from(self.window),
            surface: self.surface,
            namespace: self.namespace,
            presentation: SurfacePresentationRole::PolicyManaged,
            kind: LayoutNodeKind::Toplevel,
            placement_preference: SurfacePlacementPreference::Default,
            presentation_owner: None,
            stack_rank: self.stack_rank,
            mapped: self.mapped,
            geometry: self.geometry,
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: self.generation,
        }
    }

    pub fn to_surface_transaction(
        &self,
        transaction: TransactionId,
        authority: AuthorityKind,
        readiness: SurfaceTransactionReadiness,
        timeout_msec: u32,
        previous_committed_generation: u64,
    ) -> SurfaceTransaction {
        SurfaceTransaction {
            transaction,
            authority,
            surface: self.surface,
            namespace: self.namespace,
            target_geometry: self.geometry,
            // An X window's raster is its window-sized pixmap, so one size
            // serves both here. `LayerSnapshot` carries a measured raster
            // instead, because a layer outlives the configure that resized it.
            content: SurfaceContentSet::singleton(
                self.source,
                Size {
                    width: self.geometry.width,
                    height: self.geometry.height,
                },
            ),
            presentation_extent: Size {
                width: self.geometry.width,
                height: self.geometry.height,
            },
            damage: self.damage.clone(),
            readiness,
            timeout_msec,
            previous_committed_generation,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayerSnapshot {
    pub translation: Option<crate::LayerTranslation>,
    pub surface: SurfaceId,
    pub authority_local_id: Option<AuthorityLocalId>,
    pub namespace: Option<NamespaceId>,
    pub stack_rank: u32,
    pub geometry: Rect,
    pub source: BufferSource,
    /// The raster's own pixel size, which is not its placement.
    ///
    /// These are equal whenever a producer draws at the size it was placed at,
    /// which is the steady state and was long assumed to be the only one. It is
    /// false for the whole window between a configure and the client's redraw:
    /// a surface moved onto a smaller output still holds the buffer it drew for
    /// the larger one. Deriving it from `geometry` made every consumer --
    /// sampling classification, and the check that a lowered source is the
    /// buffer a plan measured -- believe a number no producer had reported, and
    /// ended a live session when the buffer disagreed.
    ///
    /// With no source there is no raster, and the field carries the geometry
    /// because nothing samples it.
    pub source_size: Size,
    pub damage: Region,
    pub opacity: f32,
    pub crop: Option<Rect>,
    pub transform: Transform,
    pub generation: u64,
    pub resize_sync: ResizeSyncCapability,
    /// The output whose projection placed this layer, if one did.
    ///
    /// Composition selects by this rather than by which output rectangle the
    /// geometry falls in. A scrolling layout puts columns past the edge of
    /// their own display on purpose, and with a second display to the right
    /// "past the edge" and "inside the neighbour" are the same region -- so
    /// geometry alone drew one display's window on another. Geometry still
    /// decides how much of a layer an output shows; it does not decide which
    /// output shows it.
    ///
    /// `None` means no policy has placed it, and such a layer is composited
    /// by no output. That is observable only if something could be presented
    /// before its first placement; the admission path places first.
    pub output: Option<OutputId>,
}

impl LayerSnapshot {
    pub fn to_surface_transaction(
        &self,
        transaction: TransactionId,
        authority: AuthorityKind,
        readiness: SurfaceTransactionReadiness,
        timeout_msec: u32,
        previous_committed_generation: u64,
    ) -> SurfaceTransaction {
        SurfaceTransaction {
            transaction,
            authority,
            surface: self.surface,
            namespace: self.namespace,
            target_geometry: self.geometry,
            content: SurfaceContentSet::singleton(self.source, self.source_size),
            // The layer's placement is what this raster was asked to fill.
            presentation_extent: Size {
                width: self.geometry.width,
                height: self.geometry.height,
            },
            damage: self.damage.clone(),
            readiness,
            timeout_msec,
            previous_committed_generation,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ResizeSyncCapability {
    #[default]
    ImplicitOnly,
    ExplicitSync,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BufferSource {
    None,
    XPixmap { pixmap: u32 },
    DmaBuf { handle: u64 },
    CpuBuffer { handle: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DamageFrame {
    pub output: OutputId,
    pub frame_serial: u64,
    pub buffer_age: u32,
    pub root_generation: u64,
    pub affected_surfaces: Vec<SurfaceId>,
    pub damage: Region,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrameSnapshot {
    pub output: OutputId,
    pub output_size: Size,
    pub output_scale: u32,
    pub frame_serial: u64,
    pub layers: Vec<LayerSnapshot>,
    pub commands: Vec<RenderCommand>,
    pub damage: Region,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderCommand {
    pub kind: RenderCommandKind,
    pub source: Option<SurfaceId>,
    pub output: OutputId,
    pub target: Region,
    pub clip: Option<Region>,
    pub transform: Transform,
    pub alpha: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderCommandKind {
    Blit,
    Clear,
    Composite,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompositorSurface {
    pub surface: SurfaceId,
    pub layer_generation: u64,
    pub geometry: Rect,
    pub active_buffer: BufferSource,
    pub output: Option<OutputId>,
    pub visible: bool,
    pub damage: Region,
}
