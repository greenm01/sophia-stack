use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
use super::request::add_primary_plane_properties;

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
        }
    }

    pub const fn with_vrr(mut self, enabled: bool) -> Self {
        self.vrr_enabled = Some(enabled);
        self
    }
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
/// `None` means this head cannot be built that way: either the CRTC displays
/// nothing, or what it displays is the wrong size. No primary plane scaling exists
/// on this path, so a buffer that disagrees with the mode cannot drive it, and
/// failing here is better than a kernel refusal that says only `EINVAL`.
#[cfg(feature = "libdrm-events")]
pub fn compose_native_head_from_current_framebuffer<D>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelection,
    properties: LibdrmNativePrimaryPlanePropertyHandles,
    mode_blob: u64,
    scanout: Size,
) -> Option<LibdrmNativeAtomicHead>
where
    D: drm::control::Device,
{
    let framebuffer = device
        .get_crtc(selection.crtc_handle())
        .ok()?
        .framebuffer()?;
    let (width, height) = device.get_framebuffer(framebuffer).ok()?.size();
    if i32::try_from(width).ok()? != scanout.width || i32::try_from(height).ok()? != scanout.height
    {
        return None;
    }
    Some(LibdrmNativeAtomicHead::new(
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
