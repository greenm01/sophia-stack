use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
use super::request::{add_cursor_plane_properties, add_primary_plane_properties};

/// One head's contribution to a combined atomic request.
///
/// A head is one connector driven by one CRTC through one primary plane. Several
/// heads may reference the same framebuffer, which is what output mirroring is:
/// one composed frame scanned out by every head in the group.
#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug)]
pub struct LibdrmNativeAtomicHead {
    pub objects: LibdrmNativePrimaryPlaneObjects,
    pub properties: LibdrmNativePrimaryPlanePropertyHandles,
    pub vrr_enabled: Option<bool>,
    /// This head's cursor plane and where its pointer sits, when the card
    /// offers a cursor plane and the session drives it atomically.
    ///
    /// The inner `Option` is the pointer being on another head: absent
    /// placement hides the cursor here, in the same request that shows it
    /// elsewhere, which is what keeps two heads from showing two cursors.
    /// The outer one is a head with no cursor plane at all, which keeps the
    /// legacy ioctl and contributes nothing.
    pub cursor: Option<LibdrmNativeAtomicCursor>,
}

/// A cursor plane's contribution to one head's part of a request.
#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug)]
pub struct LibdrmNativeAtomicCursor {
    pub plane: drm::control::plane::Handle,
    pub properties: LibdrmNativeCursorPlanePropertyHandles,
    pub placement: Option<LibdrmNativeCursorPlacement>,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeAtomicHead {
    pub const fn new(
        objects: LibdrmNativePrimaryPlaneObjects,
        properties: LibdrmNativePrimaryPlanePropertyHandles,
    ) -> Self {
        Self {
            objects,
            properties,
            vrr_enabled: None,
            cursor: None,
        }
    }

    pub const fn with_vrr(mut self, enabled: bool) -> Self {
        self.vrr_enabled = Some(enabled);
        self
    }

    /// Carry this head's cursor plane in the same request as its primary.
    pub const fn with_cursor(mut self, cursor: LibdrmNativeAtomicCursor) -> Self {
        self.cursor = Some(cursor);
        self
    }
}

/// One head removed from the active topology by a combined modeset.
#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug)]
pub struct LibdrmNativeAtomicDisabledHead {
    pub selection: LibdrmNativePrimaryPlaneSelection,
    pub properties: LibdrmNativePrimaryPlanePropertyHandles,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeAtomicDisabledHead {
    pub const fn new(
        selection: LibdrmNativePrimaryPlaneSelection,
        properties: LibdrmNativePrimaryPlanePropertyHandles,
    ) -> Self {
        Self {
            selection,
            properties,
        }
    }
}

/// Complete per-card topology change after every enabled target owns a buffer.
#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug)]
pub enum LibdrmNativeAtomicTopologyChange {
    Enabled(LibdrmNativeAtomicHead),
    Disabled(LibdrmNativeAtomicDisabledHead),
}

/// One head's contribution to a topology-only request: which connector a CRTC
/// drives, at which mode. No plane and no framebuffer.
///
/// This shape exists because hardware proved it validates. `native-topology-probe`
/// established that a `TEST_ONLY` modeset naming only the connector's `CRTC_ID` and
/// the CRTC's `MODE_ID` and `ACTIVE` is accepted, so a topology can be checked
/// before any framebuffer exists for it. An output that is not active yet has no
/// framebuffer to name, and inventing a handle to satisfy `LibdrmNativeAtomicHead`
/// would put a lie in the request. Validation therefore takes this type, and only
/// apply — which happens once buffers exist — takes a full head.
///
/// The property handles come from primary-plane discovery because that is where
/// connector and CRTC handles are already found together; only the connector and
/// CRTC entries are read here.
#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug)]
pub struct LibdrmNativeAtomicTopologyHead {
    pub connector: drm::control::connector::Handle,
    pub crtc: drm::control::crtc::Handle,
    pub mode_blob: u64,
    pub properties: LibdrmNativePrimaryPlanePropertyHandles,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeAtomicTopologyHead {
    pub const fn new(
        connector: drm::control::connector::Handle,
        crtc: drm::control::crtc::Handle,
        mode_blob: u64,
        properties: LibdrmNativePrimaryPlanePropertyHandles,
    ) -> Self {
        Self {
            connector,
            crtc,
            mode_blob,
            properties,
        }
    }

    /// Builds a head from the selection that already routes this output.
    ///
    /// A selection names the connector and CRTC together, which is exactly the
    /// pair a topology head needs, and it lets a caller outside this crate compose
    /// a head without naming DRM handle types.
    pub const fn from_selection(
        selection: LibdrmNativePrimaryPlaneSelection,
        mode_blob: u64,
        properties: LibdrmNativePrimaryPlanePropertyHandles,
    ) -> Self {
        Self::new(
            selection.connector_handle(),
            selection.crtc_handle(),
            mode_blob,
            properties,
        )
    }
}

/// Builds a head that scans out whatever the CRTC already displays.
///
/// Applying a topology needs plane state, and plane state needs a framebuffer. The
/// one framebuffer guaranteed to exist before anything has been rendered is the one
/// the CRTC is already scanning out, so reusing it is how a topology can be applied
/// without first allocating at the new mode's size.
///
/// A refusal names what was found, because "no usable framebuffer" alone cannot
/// separate an output displaying nothing from one displaying the wrong size, and
/// those want different fixes. No primary plane scaling exists on this path, so a
/// buffer that disagrees with the mode cannot drive it; failing here beats a kernel
/// refusal that says only `EINVAL`.
#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug)]
pub enum LibdrmNativeCurrentFramebufferHead {
    Composed(LibdrmNativeAtomicHead),
    /// The CRTC scans out nothing, so there is no buffer to reuse.
    NoFramebuffer,
    /// The CRTC scans out a buffer of this size, which is not the size asked for.
    SizeMismatch {
        width: u32,
        height: u32,
    },
    /// The framebuffer exists but could not be read.
    Unreadable,
}

/// What a CRTC is scanning out right now, if anything.
///
/// One reader for one fact. Head composition needs the handle and the size
/// together; a caller deciding whether a topology is ready to apply needs only the
/// size. Both must agree about what "currently displayed" means, so neither derives
/// it independently.
#[cfg(feature = "libdrm-events")]
pub fn read_native_current_framebuffer<D>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelection,
) -> Option<(drm::control::framebuffer::Handle, Size)>
where
    D: drm::control::Device,
{
    let framebuffer = device
        .get_crtc(selection.crtc_handle())
        .ok()?
        .framebuffer()?;
    let (width, height) = device.get_framebuffer(framebuffer).ok()?.size();
    Some((
        framebuffer,
        Size {
            width: i32::try_from(width).ok()?,
            height: i32::try_from(height).ok()?,
        },
    ))
}

#[cfg(feature = "libdrm-events")]
pub fn compose_native_head_from_current_framebuffer<D>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelection,
    properties: LibdrmNativePrimaryPlanePropertyHandles,
    mode_blob: u64,
    scanout: Size,
) -> LibdrmNativeCurrentFramebufferHead
where
    D: drm::control::Device,
{
    // Absent and unreadable are separated by asking the CRTC first: a CRTC with no
    // framebuffer scans out nothing, which is a different fact from one whose
    // framebuffer could not be read.
    let Ok(crtc) = device.get_crtc(selection.crtc_handle()) else {
        return LibdrmNativeCurrentFramebufferHead::Unreadable;
    };
    if crtc.framebuffer().is_none() {
        return LibdrmNativeCurrentFramebufferHead::NoFramebuffer;
    }
    let Some((framebuffer, current)) = read_native_current_framebuffer(device, selection) else {
        return LibdrmNativeCurrentFramebufferHead::Unreadable;
    };
    if current != scanout {
        return LibdrmNativeCurrentFramebufferHead::SizeMismatch {
            width: u32::try_from(current.width).unwrap_or(0),
            height: u32::try_from(current.height).unwrap_or(0),
        };
    }
    LibdrmNativeCurrentFramebufferHead::Composed(LibdrmNativeAtomicHead::new(
        LibdrmNativePrimaryPlaneObjects::new(
            selection.connector_handle(),
            selection.crtc_handle(),
            selection.plane_handle(),
            framebuffer,
            mode_blob,
            scanout,
        ),
        properties,
    ))
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeMultiHeadRequestBuildStatus {
    Built,
    /// A request must drive at least one head.
    NoHeads,
    /// Two heads named the same connector, CRTC, or plane. Each KMS object
    /// belongs to exactly one head within a single request.
    OverlappingObjects,
    /// Heads sharing one framebuffer disagreed on scanout size. No primary plane
    /// scaling exists on this path, so a mirror group must be same-mode.
    MismatchedMirrorSize,
    InvalidSize,
    MissingModeBlob,
    MissingVrrProperty,
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LibdrmNativeMultiHeadRequestBuildResult {
    pub status: LibdrmNativeMultiHeadRequestBuildStatus,
    pub request: Option<LibdrmNativeAtomicCommitRequest>,
    /// Number of heads folded into the request, retained for evidence.
    pub heads: usize,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeMultiHeadRequestBuildResult {
    const fn rejected(status: LibdrmNativeMultiHeadRequestBuildStatus, heads: usize) -> Self {
        Self {
            status,
            request: None,
            heads,
        }
    }
}

/// Folds every head into one atomic request.
///
/// The whole point of one request is that the kernel accepts or rejects the
/// complete topology, so a partially applied desktop is never observable. The
/// caller decides whether to commit or only test it; `test_only` on the returned
/// request performs the validation pass without touching hardware.
///
/// Validation is fail-closed and happens before any property is added, so a
/// rejected build never yields a half-populated request.
#[cfg(feature = "libdrm-events")]
pub fn build_native_multi_head_atomic_request(
    heads: &[LibdrmNativeAtomicHead],
    scope: LibdrmNativeAtomicCommitRequestScope,
) -> LibdrmNativeMultiHeadRequestBuildResult {
    let count = heads.len();
    if heads.is_empty() {
        return LibdrmNativeMultiHeadRequestBuildResult::rejected(
            LibdrmNativeMultiHeadRequestBuildStatus::NoHeads,
            count,
        );
    }
    if let Some(status) = reject_invalid_heads(heads, scope) {
        return LibdrmNativeMultiHeadRequestBuildResult::rejected(status, count);
    }

    let mut request = drm::control::atomic::AtomicModeReq::new();
    for head in heads {
        let objects = head.objects;
        let properties = head.properties;
        let width = objects.size.width as u64;
        let height = objects.size.height as u64;
        if scope == LibdrmNativeAtomicCommitRequestScope::Modeset {
            let mode_blob = objects
                .mode_blob
                .expect("validated heads carry a mode blob for a modeset");
            request.add_property(
                objects.connector,
                properties.connector_crtc_id,
                drm::control::property::Value::CRTC(Some(objects.crtc)),
            );
            request.add_property(
                objects.crtc,
                properties.crtc_mode_id,
                drm::control::property::Value::Blob(mode_blob),
            );
            request.add_property(
                objects.crtc,
                properties.crtc_active,
                drm::control::property::Value::Boolean(true),
            );
        }
        add_primary_plane_properties(&mut request, objects, properties, width, height);
        // The cursor rides the same request as the primary it sits over, which
        // is the point of the transaction owner: one commit per CRTC, not one
        // for the frame and another racing it for the pointer.
        if let Some(cursor) = head.cursor {
            add_cursor_plane_properties(
                &mut request,
                cursor.plane,
                objects.crtc,
                cursor.properties,
                cursor.placement,
            );
        }
        if let (Some(enabled), Some(property)) = (head.vrr_enabled, properties.crtc_vrr_enabled()) {
            request.add_property(
                objects.crtc,
                property,
                drm::control::property::Value::Boolean(enabled),
            );
        }
    }

    LibdrmNativeMultiHeadRequestBuildResult {
        status: LibdrmNativeMultiHeadRequestBuildStatus::Built,
        request: Some(match scope {
            LibdrmNativeAtomicCommitRequestScope::PageFlip => {
                LibdrmNativeAtomicCommitRequest::new(request)
            }
            LibdrmNativeAtomicCommitRequestScope::Modeset => {
                LibdrmNativeAtomicCommitRequest::modeset(request)
            }
        }),
        heads: count,
    }
}

/// Builds one card-scoped modeset containing both enabled and disabled heads.
///
/// Enabled heads carry their independently rendered framebuffer. Disabled heads
/// explicitly detach plane, connector, and CRTC state. The kernel therefore
/// accepts or rejects the entire card transition; callers only need userspace
/// rollback across cards, never between heads on one card.
#[cfg(feature = "libdrm-events")]
pub fn build_native_topology_change_atomic_request(
    changes: &[LibdrmNativeAtomicTopologyChange],
) -> LibdrmNativeMultiHeadRequestBuildResult {
    let count = changes.len();
    if changes.is_empty() {
        return LibdrmNativeMultiHeadRequestBuildResult::rejected(
            LibdrmNativeMultiHeadRequestBuildStatus::NoHeads,
            count,
        );
    }
    for (index, change) in changes.iter().enumerate() {
        let (connector, crtc, plane) = topology_change_objects(*change);
        if let LibdrmNativeAtomicTopologyChange::Enabled(head) = change
            && let Some(status) = reject_invalid_heads(
                core::slice::from_ref(head),
                LibdrmNativeAtomicCommitRequestScope::Modeset,
            )
        {
            return LibdrmNativeMultiHeadRequestBuildResult::rejected(status, count);
        }
        for peer in changes.iter().skip(index + 1).copied() {
            let (peer_connector, peer_crtc, peer_plane) = topology_change_objects(peer);
            if connector == peer_connector || crtc == peer_crtc || plane == peer_plane {
                return LibdrmNativeMultiHeadRequestBuildResult::rejected(
                    LibdrmNativeMultiHeadRequestBuildStatus::OverlappingObjects,
                    count,
                );
            }
            if let (
                LibdrmNativeAtomicTopologyChange::Enabled(head),
                LibdrmNativeAtomicTopologyChange::Enabled(peer),
            ) = (*change, peer)
                && head.objects.framebuffer == peer.objects.framebuffer
                && head.objects.size != peer.objects.size
            {
                return LibdrmNativeMultiHeadRequestBuildResult::rejected(
                    LibdrmNativeMultiHeadRequestBuildStatus::MismatchedMirrorSize,
                    count,
                );
            }
        }
    }

    let mut request = drm::control::atomic::AtomicModeReq::new();
    for change in changes.iter().copied() {
        match change {
            LibdrmNativeAtomicTopologyChange::Enabled(head) => {
                let objects = head.objects;
                let properties = head.properties;
                let width = objects.size.width as u64;
                let height = objects.size.height as u64;
                request.add_property(
                    objects.connector,
                    properties.connector_crtc_id,
                    drm::control::property::Value::CRTC(Some(objects.crtc)),
                );
                request.add_property(
                    objects.crtc,
                    properties.crtc_mode_id,
                    drm::control::property::Value::Blob(
                        objects
                            .mode_blob
                            .expect("validated topology head carries a mode blob"),
                    ),
                );
                request.add_property(
                    objects.crtc,
                    properties.crtc_active,
                    drm::control::property::Value::Boolean(true),
                );
                add_primary_plane_properties(&mut request, objects, properties, width, height);
                if let Some(cursor) = head.cursor {
                    add_cursor_plane_properties(
                        &mut request,
                        cursor.plane,
                        objects.crtc,
                        cursor.properties,
                        cursor.placement,
                    );
                }
                if let (Some(enabled), Some(property)) =
                    (head.vrr_enabled, properties.crtc_vrr_enabled())
                {
                    request.add_property(
                        objects.crtc,
                        property,
                        drm::control::property::Value::Boolean(enabled),
                    );
                }
            }
            LibdrmNativeAtomicTopologyChange::Disabled(head) => {
                let selection = head.selection;
                let properties = head.properties;
                request.add_property(
                    selection.plane_handle(),
                    properties.plane_fb_id,
                    drm::control::property::Value::Framebuffer(None),
                );
                request.add_property(
                    selection.plane_handle(),
                    properties.plane_crtc_id,
                    drm::control::property::Value::CRTC(None),
                );
                request.add_property(
                    selection.connector_handle(),
                    properties.connector_crtc_id,
                    drm::control::property::Value::CRTC(None),
                );
                request.add_property(
                    selection.crtc_handle(),
                    properties.crtc_mode_id,
                    drm::control::property::Value::Blob(0),
                );
                request.add_property(
                    selection.crtc_handle(),
                    properties.crtc_active,
                    drm::control::property::Value::Boolean(false),
                );
                if let Some(property) = properties.crtc_vrr_enabled() {
                    request.add_property(
                        selection.crtc_handle(),
                        property,
                        drm::control::property::Value::Boolean(false),
                    );
                }
            }
        }
    }
    LibdrmNativeMultiHeadRequestBuildResult {
        status: LibdrmNativeMultiHeadRequestBuildStatus::Built,
        request: Some(LibdrmNativeAtomicCommitRequest::modeset(request)),
        heads: count,
    }
}

#[cfg(feature = "libdrm-events")]
fn topology_change_objects(
    change: LibdrmNativeAtomicTopologyChange,
) -> (
    drm::control::connector::Handle,
    drm::control::crtc::Handle,
    drm::control::plane::Handle,
) {
    match change {
        LibdrmNativeAtomicTopologyChange::Enabled(head) => (
            head.objects.connector,
            head.objects.crtc,
            head.objects.plane,
        ),
        LibdrmNativeAtomicTopologyChange::Disabled(head) => (
            head.selection.connector_handle(),
            head.selection.crtc_handle(),
            head.selection.plane_handle(),
        ),
    }
}

/// Folds every topology head into one validation-shaped atomic request.
///
/// Same guarantee as the full builder: one request means the kernel judges the
/// complete topology, so a partially applied desktop is never observable. The
/// difference is what the request omits. With no plane state there is no scanout
/// size to check and no mirror group to detect, because sharing a framebuffer is
/// what makes heads a group and no framebuffer is named. Connector and CRTC
/// exclusivity still holds: each belongs to exactly one head in one request.
///
/// The result carries `ALLOW_MODESET` and is intended for `test_only` submission.
/// Applying it would activate CRTCs with no plane, which is not a desktop.
#[cfg(feature = "libdrm-events")]
pub fn build_native_multi_head_topology_request(
    heads: &[LibdrmNativeAtomicTopologyHead],
) -> LibdrmNativeMultiHeadRequestBuildResult {
    let count = heads.len();
    if heads.is_empty() {
        return LibdrmNativeMultiHeadRequestBuildResult::rejected(
            LibdrmNativeMultiHeadRequestBuildStatus::NoHeads,
            count,
        );
    }
    if let Some(status) = reject_invalid_topology_heads(heads) {
        return LibdrmNativeMultiHeadRequestBuildResult::rejected(status, count);
    }

    let mut request = drm::control::atomic::AtomicModeReq::new();
    for head in heads {
        request.add_property(
            head.connector,
            head.properties.connector_crtc_id,
            drm::control::property::Value::CRTC(Some(head.crtc)),
        );
        request.add_property(
            head.crtc,
            head.properties.crtc_mode_id,
            drm::control::property::Value::Blob(head.mode_blob),
        );
        request.add_property(
            head.crtc,
            head.properties.crtc_active,
            drm::control::property::Value::Boolean(true),
        );
    }

    LibdrmNativeMultiHeadRequestBuildResult {
        status: LibdrmNativeMultiHeadRequestBuildStatus::Built,
        request: Some(LibdrmNativeAtomicCommitRequest::modeset(request)),
        heads: count,
    }
}

#[cfg(feature = "libdrm-events")]
fn reject_invalid_topology_heads(
    heads: &[LibdrmNativeAtomicTopologyHead],
) -> Option<LibdrmNativeMultiHeadRequestBuildStatus> {
    for (index, head) in heads.iter().enumerate() {
        if head.mode_blob == 0 {
            return Some(LibdrmNativeMultiHeadRequestBuildStatus::MissingModeBlob);
        }
        for other in heads.iter().skip(index + 1) {
            if head.connector == other.connector || head.crtc == other.crtc {
                return Some(LibdrmNativeMultiHeadRequestBuildStatus::OverlappingObjects);
            }
        }
    }
    None
}

#[cfg(feature = "libdrm-events")]
fn reject_invalid_heads(
    heads: &[LibdrmNativeAtomicHead],
    scope: LibdrmNativeAtomicCommitRequestScope,
) -> Option<LibdrmNativeMultiHeadRequestBuildStatus> {
    for (index, head) in heads.iter().enumerate() {
        let objects = head.objects;
        if !is_valid_native_primary_plane_scanout_size(objects.size) {
            return Some(LibdrmNativeMultiHeadRequestBuildStatus::InvalidSize);
        }
        if scope == LibdrmNativeAtomicCommitRequestScope::Modeset
            && !objects.mode_blob.is_some_and(|blob| blob != 0)
        {
            return Some(LibdrmNativeMultiHeadRequestBuildStatus::MissingModeBlob);
        }
        if head.vrr_enabled.is_some() && head.properties.crtc_vrr_enabled().is_none() {
            return Some(LibdrmNativeMultiHeadRequestBuildStatus::MissingVrrProperty);
        }
        for other in heads.iter().skip(index + 1) {
            let peer = other.objects;
            if objects.connector == peer.connector
                || objects.crtc == peer.crtc
                || objects.plane == peer.plane
            {
                return Some(LibdrmNativeMultiHeadRequestBuildStatus::OverlappingObjects);
            }
            // Heads sharing a framebuffer are a mirror group. Without primary
            // plane scaling, one buffer cannot satisfy two scanout sizes, so the
            // mismatch fails here rather than reaching the kernel or silently
            // letterboxing.
            if objects.framebuffer == peer.framebuffer && objects.size != peer.size {
                return Some(LibdrmNativeMultiHeadRequestBuildStatus::MismatchedMirrorSize);
            }
        }
    }
    None
}
