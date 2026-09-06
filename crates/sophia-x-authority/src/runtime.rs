use std::collections::BTreeMap;
use std::os::fd::OwnedFd;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use sophia_portal::ClipboardPortal;
use sophia_protocol::{
    AuthoritySurface, NamespaceId, OutputTopologyError, OutputTopologySnapshot, Rect, Region, Size,
    TransactionId,
};

use crate::{
    ClipboardSelectionDispatch, ClipboardSelectionExecutionError,
    ClipboardSelectionExecutionOutcome, ClipboardSelectionFailureRequest,
    ClipboardSelectionHandoff, ClipboardSelectionNotify, ClipboardSelectionProxy,
    ClipboardSourcePayload, ClipboardTextProperty, PendingClipboardSelection, X_ATOM_ATOM,
    X_ATOM_NONE, XAtomTable, XAuthorityCpuBufferUpdate, XAuthorityPortalCommand,
    XAuthorityRasterCommand, XAuthorityRasterStore, XAuthorityRequestKind, XAuthorityRequestPacket,
    XAuthorityResponsePacket, XAuthorityRuntimeError, XAuthoritySelectionArtifact, XByteOrder,
    XDrawingUpdate, XFontFace, XGraphicsContextTable, XGraphicsContextValues, XOwnedTextDraw,
    XPoint, XPropertyChange, XPropertyMode, XPropertyTable, XPutImageSemantics, XRasterPoint,
    XRasterUnsupportedKind, XResourceKind, XResourceTable, XSelectionEvent, XSelectionMonitor,
    XShmSegmentTable, XSoftwareBufferStore, XTextDraw, XWindowLifecycleEvent, XWindowTable,
    clipboard_selection_failure_notify, dispatch_clipboard_selection_request,
    surface_transaction_from_drawing_update,
};

include!("runtime/clipboard.rs");
include!("runtime/color.rs");
include!("runtime/drawing.rs");
include!("runtime/drawing/image_ops.rs");
include!("runtime/render_resources.rs");
include!("runtime/render_pictures.rs");
include!("runtime/sync.rs");
include!("runtime/windows.rs");

/// Effects of releasing every currently supported resource allocated from one
/// X11 client connection's setup range.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XAuthorityClientResourceRelease {
    /// X11 windows whose properties must be removed from the frontend table.
    pub destroyed_windows: Vec<crate::XResourceId>,
    /// Sophia surfaces that must be removed from Engine's committed snapshot.
    pub removed_surfaces: Vec<sophia_protocol::SurfaceId>,
    pub released_pixmaps: usize,
    pub released_fonts: usize,
    pub released_cursors: usize,
    pub released_colormaps: usize,
    pub released_graphics_contexts: usize,
    pub released_shm_segments: usize,
    pub released_glx_contexts: usize,
    pub released_glx_windows: usize,
    /// Renderer-visible DRI3 sources released by disconnect cleanup.
    pub released_dma_bufs: Vec<sophia_protocol::BufferHandle>,
    /// Renderer-visible xshmfences released by disconnect cleanup.
    pub released_fences: Vec<sophia_protocol::FenceHandle>,
}

#[derive(Clone, Debug)]
struct XShmPixmapBinding {
    offset: u32,
    size: Size,
    mapping: Arc<sophia_sysv_shm::ClientMapping>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct XPixmapRecord {
    size: Size,
    depth: u8,
}

/// One DRI3-imported pixmap: the facts it was imported with, and the plane
/// descriptors it was imported from.
///
/// The descriptors are kept because DRI3 asks for them back. A client that
/// imported a pixmap may call `BuffersFromPixmap` to recover the same buffer,
/// and the authority cannot borrow the renderer's copy to answer: the renderer
/// import boundary owns keeping its handles out of protocol authorities. So the
/// authority keeps its own, for exactly as long as the pixmap lives.
#[derive(Clone, Debug)]
struct XDri3PixmapRecord {
    descriptor: sophia_protocol::DmaBufDescriptor,
    plane_fds: Vec<Arc<OwnedFd>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct XFontRecord {
    face: XFontFace,
}

/// What kind of thing a drawable id names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XDrawableKind {
    Root,
    Window,
    Pixmap,
    /// An offscreen GLX surface. It answers geometry, and nothing draws into it.
    GlxPbuffer,
}

/// The facts every drawable can answer, whatever kind it is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XDrawableFacts {
    pub kind: XDrawableKind,
    pub geometry: Rect,
    pub depth: u8,
}

/// One GLX drawable's bookkeeping.
///
/// GLX owns no pixels here. A window alias borrows its geometry from the X window
/// it names; anything else has to carry its own, because nothing else does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct XGlxDrawableRecord {
    owner: NamespaceId,
    fbconfig: u32,
    backing: XGlxDrawableBacking,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum XGlxDrawableBacking {
    Window(crate::XResourceId),
    /// An offscreen surface. Sophia stores no pixels for it, so it carries the
    /// extent it was created with; nothing else knows one.
    Pbuffer(Size),
}

#[derive(Debug)]
pub struct XAuthorityRuntime {
    resources: XResourceTable,
    windows: XWindowTable,
    shm_segments: XShmSegmentTable,
    selections: XSelectionMonitor,
    clipboard: ClipboardPortal,
    pending_clipboard: BTreeMap<sophia_protocol::PortalTransferId, PendingClipboardSelection>,
    clipboard_proxies: BTreeMap<crate::XResourceId, ClipboardSelectionProxy>,
    next_clipboard_proxy: u32,
    software_buffers: XSoftwareBufferStore,
    raster_store: XAuthorityRasterStore,
    pending_raster_command: Option<XAuthorityRasterCommand>,
    pixmaps: BTreeMap<crate::XResourceId, XPixmapRecord>,
    fonts: BTreeMap<crate::XResourceId, XFontRecord>,
    shm_pixmaps: BTreeMap<crate::XResourceId, XShmPixmapBinding>,
    shm_mappings: BTreeMap<u32, Weak<sophia_sysv_shm::ClientMapping>>,
    /// The live mapping for each descriptor-backed segment.
    ///
    /// Held here rather than on the segment record because a record is cloned
    /// and compared, and a mapping is neither. Dropped when the segment is
    /// detached or its client goes away, which is what unmaps it.
    shm_descriptor_mappings: BTreeMap<crate::XResourceId, Arc<sophia_sysv_shm::ClientMapping>>,
    /// Descriptors a `CreateSegment` reply still owes its client, held only
    /// until the socket layer puts them on the wire.
    shm_reply_descriptors: BTreeMap<crate::XResourceId, std::os::fd::OwnedFd>,
    dri3_pixmaps: BTreeMap<crate::XResourceId, XDri3PixmapRecord>,
    next_dma_buf_handle: u64,
    dri3_fences: BTreeMap<crate::XResourceId, sophia_protocol::FenceHandle>,
    sync_counters: BTreeMap<crate::XResourceId, i64>,
    xfixes_regions: BTreeMap<crate::XResourceId, Region>,
    render_pictures: BTreeMap<crate::XResourceId, XRenderPictureRecord>,
    /// Glyph-set resource ids, resolved through to shared stores in a later
    /// phase; the map exists now so resource sweeps stay exhaustive.
    render_glyphsets: BTreeMap<crate::XResourceId, u64>,
    next_fence_handle: u64,
    graphics_contexts: XGraphicsContextTable,
    window_background_pixels: BTreeMap<crate::XResourceId, u32>,
    window_visuals: BTreeMap<crate::XResourceId, (u8, u32, crate::XResourceId)>,
    colormaps: BTreeMap<crate::XResourceId, u32>,
    glx_contexts: BTreeMap<crate::XResourceId, (NamespaceId, u32, bool)>,
    glx_drawables: BTreeMap<crate::XResourceId, XGlxDrawableRecord>,
    last_cpu_buffer_updates: Vec<XAuthorityCpuBufferUpdate>,
    output_topology: OutputTopologySnapshot,
    input_focus: BTreeMap<NamespaceId, (crate::XResourceId, u8)>,
    defer_policy_maps: bool,
    xkb_keymap: crate::XkbKeymapSnapshot,
    input_authority: Arc<Mutex<crate::XInputAuthorityState>>,
}

impl Default for XAuthorityRuntime {
    fn default() -> Self {
        Self {
            resources: Default::default(),
            windows: Default::default(),
            shm_segments: Default::default(),
            selections: Default::default(),
            clipboard: Default::default(),
            pending_clipboard: Default::default(),
            clipboard_proxies: Default::default(),
            next_clipboard_proxy: 0,
            software_buffers: Default::default(),
            raster_store: Default::default(),
            pending_raster_command: None,
            pixmaps: Default::default(),
            fonts: Default::default(),
            shm_pixmaps: Default::default(),
            shm_mappings: Default::default(),
            shm_descriptor_mappings: Default::default(),
            shm_reply_descriptors: Default::default(),
            dri3_pixmaps: Default::default(),
            next_dma_buf_handle: 1,
            dri3_fences: Default::default(),
            sync_counters: Default::default(),
            xfixes_regions: Default::default(),
            render_pictures: Default::default(),
            render_glyphsets: Default::default(),
            next_fence_handle: 1,
            graphics_contexts: Default::default(),
            window_background_pixels: Default::default(),
            window_visuals: Default::default(),
            colormaps: Default::default(),
            glx_contexts: Default::default(),
            glx_drawables: Default::default(),
            last_cpu_buffer_updates: Vec::new(),
            output_topology: OutputTopologySnapshot::deterministic(),
            input_focus: Default::default(),
            defer_policy_maps: false,
            xkb_keymap: crate::XkbKeymapSnapshot::new(&crate::XkbRmlvoConfig::default())
                .expect("the deterministic default XKB keymap must compile"),
            input_authority: Arc::new(Mutex::new(crate::XInputAuthorityState::default())),
        }
    }
}

impl XAuthorityRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_xkb_config(
        config: &crate::XkbRmlvoConfig,
    ) -> Result<Self, crate::XkbKeyboardError> {
        Ok(Self {
            xkb_keymap: crate::XkbKeymapSnapshot::new(config)?,
            ..Self::default()
        })
    }

    pub const fn xkb_keymap(&self) -> &crate::XkbKeymapSnapshot {
        &self.xkb_keymap
    }

    pub fn set_policy_map_deferred(&mut self, deferred: bool) {
        self.defer_policy_maps = deferred;
    }

    pub fn input_authority_mut(&self) -> MutexGuard<'_, crate::XInputAuthorityState> {
        self.input_authority
            .lock()
            .expect("X11 input authority lock poisoned")
    }

    pub fn set_input_authority(
        &mut self,
        input_authority: Arc<Mutex<crate::XInputAuthorityState>>,
    ) {
        self.input_authority = input_authority;
    }

    pub fn with_output_topology(
        output_topology: OutputTopologySnapshot,
    ) -> Result<Self, OutputTopologyError> {
        output_topology.validate()?;
        Ok(Self {
            output_topology,
            ..Self::default()
        })
    }

    pub fn with_output_topology_and_xkb_config(
        output_topology: OutputTopologySnapshot,
        xkb_config: &crate::XkbRmlvoConfig,
    ) -> Result<Self, String> {
        output_topology
            .validate()
            .map_err(|error| format!("invalid Engine output topology: {error:?}"))?;
        Ok(Self {
            output_topology,
            xkb_keymap: crate::XkbKeymapSnapshot::new(xkb_config)
                .map_err(|error| format!("invalid XKB configuration: {error}"))?,
            ..Self::default()
        })
    }

    pub fn output_topology(&self) -> &OutputTopologySnapshot {
        &self.output_topology
    }

    pub fn update_output_topology(
        &mut self,
        output_topology: OutputTopologySnapshot,
    ) -> Result<bool, OutputTopologyError> {
        output_topology.validate()?;
        if output_topology.generation <= self.output_topology.generation {
            return Ok(false);
        }
        self.output_topology = output_topology;
        Ok(true)
    }

    pub fn input_focus(&self, namespace: NamespaceId) -> (crate::XResourceId, u8) {
        self.input_focus.get(&namespace).copied().unwrap_or((
            crate::XResourceId::new(u64::from(crate::X_SETUP_DEFAULT_ROOT), 1),
            1,
        ))
    }

    pub fn set_input_focus(
        &mut self,
        namespace: NamespaceId,
        focus: crate::XResourceId,
        revert_to: u8,
    ) -> Result<(), XAuthorityRuntimeError> {
        if revert_to > 2 {
            return Err(XAuthorityRuntimeError::InvalidResource);
        }
        if focus.local.raw() != 0 && focus.local.raw() != u64::from(crate::X_SETUP_DEFAULT_ROOT) {
            self.validate_window_access(namespace, focus)?;
        }
        self.input_focus.insert(namespace, (focus, revert_to));
        Ok(())
    }

    pub fn begin_dispatch(&mut self) {
        self.last_cpu_buffer_updates.clear();
        self.pending_raster_command = None;
    }

    /// Takes every immutable CPU-buffer mutation produced by one authority
    /// dispatch. A surface transaction may publish multiple density variants,
    /// so dispatch ownership is a bounded ordered collection rather than a
    /// singleton side channel.
    pub fn take_cpu_buffer_updates(&mut self) -> Vec<XAuthorityCpuBufferUpdate> {
        core::mem::take(&mut self.last_cpu_buffer_updates)
    }

    /// Compatibility accessor for direct runtime tests and single-buffer
    /// callers. Production dispatch uses [`Self::take_cpu_buffer_updates`].
    pub fn take_cpu_buffer_update(&mut self) -> Option<XAuthorityCpuBufferUpdate> {
        if self.last_cpu_buffer_updates.is_empty() {
            None
        } else {
            Some(self.last_cpu_buffer_updates.remove(0))
        }
    }

    pub fn apply(&mut self, request: XAuthorityRequestPacket) -> XAuthorityResponsePacket {
        match self.apply_checked(&request) {
            Ok(response) => response,
            Err(error) => {
                let mut response = XAuthorityResponsePacket::rejected(request.transaction, error);
                if let XAuthorityRequestKind::RequestSelection {
                    requestor,
                    selection,
                    target,
                    time,
                    transfer,
                    ..
                } = request.kind
                {
                    response
                        .selection_artifacts
                        .push(XAuthoritySelectionArtifact::Failure(
                            clipboard_selection_failure_notify(ClipboardSelectionFailureRequest {
                                transfer,
                                requestor,
                                selection,
                                target,
                                time,
                            }),
                        ));
                }
                response
            }
        }
    }

    fn apply_checked(
        &mut self,
        request: &XAuthorityRequestPacket,
    ) -> Result<XAuthorityResponsePacket, XAuthorityRuntimeError> {
        let mut response = XAuthorityResponsePacket::accepted(request.transaction);

        match &request.kind {
            XAuthorityRequestKind::CreateWindow {
                window,
                surface,
                geometry,
                constraints,
                generation,
            } => {
                self.resources.insert(
                    *window,
                    XResourceKind::Window,
                    request.namespace,
                    *generation,
                )?;
                if let Some(surface) = self.windows.apply(XWindowLifecycleEvent::Created {
                    id: *window,
                    surface: *surface,
                    namespace: request.namespace,
                    geometry: *geometry,
                    constraints: *constraints,
                    generation: *generation,
                })? {
                    response.surfaces.push(surface);
                }
            }
            XAuthorityRequestKind::MapWindow { window, generation } => {
                self.resources
                    .lookup(request.namespace, *window, XResourceKind::Window)?;
                let role = self
                    .windows
                    .get(*window)
                    .ok_or(XAuthorityRuntimeError::UnknownResource)?
                    .presentation_role();
                let event = if role == sophia_protocol::SurfacePresentationRole::ClientPositioned
                    || !self.defer_policy_maps
                {
                    XWindowLifecycleEvent::Mapped {
                        id: *window,
                        generation: *generation,
                    }
                } else {
                    XWindowLifecycleEvent::PolicyPending {
                        id: *window,
                        generation: *generation,
                    }
                };
                if let Some(surface) = self.windows.apply(event)? {
                    response.surfaces.push(surface);
                }
            }
            XAuthorityRequestKind::PresentPixmap {
                window,
                pixmap,
                damage,
                previous_committed_generation,
                timeout_msec,
            } => {
                let transaction = surface_transaction_from_drawing_update(
                    &self.windows,
                    XDrawingUpdate::present_pixmap(
                        request.transaction,
                        request.namespace,
                        *window,
                        *pixmap,
                        damage.clone(),
                        *previous_committed_generation,
                        *timeout_msec,
                    ),
                )?;
                self.windows
                    .advance_generation(*window, *previous_committed_generation)?;
                response.transactions.push(transaction);
            }
            XAuthorityRequestKind::SetSelectionOwner {
                selection,
                owner,
                timestamp,
                selection_timestamp,
                kind,
            } => {
                if let Some(owner) = owner {
                    self.resources
                        .lookup(request.namespace, *owner, XResourceKind::Window)?;
                }
                let update = self.selections.apply_event_in_namespace(
                    XSelectionEvent {
                        selection: *selection,
                        owner: *owner,
                        timestamp: *timestamp,
                        selection_timestamp: *selection_timestamp,
                        kind: *kind,
                    },
                    &self.windows,
                    Some(request.namespace),
                );
                if let Some(previous_owner) = update.previous.and_then(|record| record.owner)
                    && Some(previous_owner) != *owner
                {
                    response
                        .selection_artifacts
                        .push(XAuthoritySelectionArtifact::Clear {
                            owner: previous_owner,
                            selection: *selection,
                            time: *timestamp,
                        });
                }
            }
            XAuthorityRequestKind::RequestSelection {
                requestor,
                selection,
                target,
                target_name,
                property,
                time,
                transfer,
            } => {
                self.resources
                    .lookup(request.namespace, *requestor, XResourceKind::Window)?;
                let dispatch = dispatch_clipboard_selection_request(
                    crate::XSelectionRequest {
                        requestor: *requestor,
                        selection: *selection,
                        target: *target,
                        target_name: target_name.clone(),
                        property: *property,
                        time: *time,
                    },
                    &self.selections,
                    &self.windows,
                    *transfer,
                    &mut self.clipboard,
                )?;
                match dispatch {
                    ClipboardSelectionDispatch::SameNamespace(request) => response
                        .selection_artifacts
                        .push(XAuthoritySelectionArtifact::Request(request)),
                    ClipboardSelectionDispatch::CrossNamespace {
                        portal_request,
                        command,
                    } => {
                        self.pending_clipboard.insert(
                            *transfer,
                            PendingClipboardSelection {
                                namespace: request.namespace,
                                portal_request,
                                byte_order: XByteOrder::LittleEndian,
                            },
                        );
                        if let Some(command) = XAuthorityPortalCommand::from_portal_command(command)
                        {
                            response.portal_commands.push(command);
                        }
                    }
                }
            }
        }

        Ok(response)
    }
}
