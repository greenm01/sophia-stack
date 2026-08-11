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
