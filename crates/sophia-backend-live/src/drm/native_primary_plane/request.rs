use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LibdrmNativeAtomicRequestBuildResult {
    pub status: LibdrmNativeAtomicRequestBuildStatus,
    pub request: Option<LibdrmNativeAtomicCommitRequest>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeAtomicRequestBuildStatus {
    Built,
    InvalidSize,
    MissingModeBlob,
    MissingVrrProperty,
}

#[cfg(feature = "libdrm-events")]
const LIBDRM_NATIVE_PRIMARY_PLANE_SOURCE_FIXED_POINT_SHIFT: u32 = 16;

#[cfg(feature = "libdrm-events")]
const LIBDRM_NATIVE_PRIMARY_PLANE_MAX_SOURCE_DIMENSION: i32 =
    (u32::MAX >> LIBDRM_NATIVE_PRIMARY_PLANE_SOURCE_FIXED_POINT_SHIFT) as i32;

#[cfg(feature = "libdrm-events")]
pub fn build_native_primary_plane_atomic_request(
    objects: LibdrmNativePrimaryPlaneObjects,
    properties: LibdrmNativePrimaryPlanePropertyHandles,
) -> LibdrmNativeAtomicRequestBuildResult {
    build_native_primary_plane_atomic_request_with_scope(
        objects,
        properties,
        LibdrmNativeAtomicCommitRequestScope::Modeset,
        None,
        None,
    )
}

#[cfg(feature = "libdrm-events")]
pub fn build_native_primary_plane_atomic_request_with_vrr(
    objects: LibdrmNativePrimaryPlaneObjects,
    properties: LibdrmNativePrimaryPlanePropertyHandles,
    enabled: bool,
) -> LibdrmNativeAtomicRequestBuildResult {
    build_native_primary_plane_atomic_request_with_scope(
        objects,
        properties,
        LibdrmNativeAtomicCommitRequestScope::Modeset,
        Some(enabled),
        None,
    )
}

#[cfg(feature = "libdrm-events")]
pub fn build_native_primary_plane_page_flip_atomic_request(
    objects: LibdrmNativePrimaryPlaneObjects,
    properties: LibdrmNativePrimaryPlanePropertyHandles,
) -> LibdrmNativeAtomicRequestBuildResult {
    build_native_primary_plane_atomic_request_with_scope(
        objects,
        properties,
        LibdrmNativeAtomicCommitRequestScope::PageFlip,
        None,
        None,
    )
}

#[cfg(feature = "libdrm-events")]
pub fn build_native_primary_plane_page_flip_atomic_request_with_vrr(
    objects: LibdrmNativePrimaryPlaneObjects,
    properties: LibdrmNativePrimaryPlanePropertyHandles,
    enabled: bool,
) -> LibdrmNativeAtomicRequestBuildResult {
    build_native_primary_plane_atomic_request_with_scope(
        objects,
        properties,
        LibdrmNativeAtomicCommitRequestScope::PageFlip,
        Some(enabled),
        None,
    )
}

#[cfg(feature = "libdrm-events")]
/// The request a submit policy asks for.
///
/// One entry point rather than a wrapper per combination: scope, VRR, and now
/// the cursor are all things a policy carries, and a function per pairing
/// would have been eight of them the moment the cursor arrived.
#[cfg(feature = "libdrm-events")]
pub fn build_native_primary_plane_atomic_request_for_policy(
    objects: LibdrmNativePrimaryPlaneObjects,
    properties: LibdrmNativePrimaryPlanePropertyHandles,
    policy: LibdrmNativePrimaryPlaneScanoutSubmitPolicy,
) -> LibdrmNativeAtomicRequestBuildResult {
    build_native_primary_plane_atomic_request_with_scope(
        objects,
        properties,
        policy.expected_request_scope(),
        policy.vrr_enabled,
        policy.cursor,
    )
}

fn build_native_primary_plane_atomic_request_with_scope(
    objects: LibdrmNativePrimaryPlaneObjects,
    properties: LibdrmNativePrimaryPlanePropertyHandles,
    scope: LibdrmNativeAtomicCommitRequestScope,
    vrr_enabled: Option<bool>,
    cursor: Option<LibdrmNativeAtomicCursor>,
) -> LibdrmNativeAtomicRequestBuildResult {
    if !is_valid_native_primary_plane_scanout_size(objects.size) {
        return LibdrmNativeAtomicRequestBuildResult {
            status: LibdrmNativeAtomicRequestBuildStatus::InvalidSize,
            request: None,
        };
    }

    let width = objects.size.width as u64;
    let height = objects.size.height as u64;
    let mut request = drm::control::atomic::AtomicModeReq::new();
    if vrr_enabled.is_some() && properties.crtc_vrr_enabled().is_none() {
        return LibdrmNativeAtomicRequestBuildResult {
            status: LibdrmNativeAtomicRequestBuildStatus::MissingVrrProperty,
            request: None,
        };
    }
    if scope == LibdrmNativeAtomicCommitRequestScope::Modeset {
        let Some(mode_blob) = objects.mode_blob else {
            return LibdrmNativeAtomicRequestBuildResult {
                status: LibdrmNativeAtomicRequestBuildStatus::MissingModeBlob,
                request: None,
            };
        };
        if mode_blob == 0 {
            return LibdrmNativeAtomicRequestBuildResult {
                status: LibdrmNativeAtomicRequestBuildStatus::MissingModeBlob,
                request: None,
            };
        }
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
    // The cursor rides the frame's own commit when it has one to ride. A
    // cursor that moved while this frame was going out cannot have a commit
    // of its own -- the kernel serializes them per CRTC -- so this is the
    // cheap case the owner prefers.
    if let Some(cursor) = cursor {
        add_cursor_plane_properties(
            &mut request,
            cursor.plane,
            objects.crtc,
            cursor.properties,
            cursor.placement,
        );
    }
    if let (Some(enabled), Some(property)) = (vrr_enabled, properties.crtc_vrr_enabled()) {
        request.add_property(
            objects.crtc,
            property,
            drm::control::property::Value::Boolean(enabled),
        );
    }

    LibdrmNativeAtomicRequestBuildResult {
        status: LibdrmNativeAtomicRequestBuildStatus::Built,
        request: Some(match scope {
            LibdrmNativeAtomicCommitRequestScope::PageFlip => {
                LibdrmNativeAtomicCommitRequest::new(request)
            }
            LibdrmNativeAtomicCommitRequestScope::Modeset => {
                LibdrmNativeAtomicCommitRequest::modeset(request)
            }
        }),
    }
}

#[cfg(feature = "libdrm-events")]
pub(crate) const fn is_valid_native_primary_plane_scanout_size(size: Size) -> bool {
    size.width > 0
        && size.height > 0
        && size.width <= LIBDRM_NATIVE_PRIMARY_PLANE_MAX_SOURCE_DIMENSION
        && size.height <= LIBDRM_NATIVE_PRIMARY_PLANE_MAX_SOURCE_DIMENSION
}

#[cfg(feature = "libdrm-events")]
pub(super) fn add_primary_plane_properties(
    request: &mut drm::control::atomic::AtomicModeReq,
    objects: LibdrmNativePrimaryPlaneObjects,
    properties: LibdrmNativePrimaryPlanePropertyHandles,
    width: u64,
    height: u64,
) {
    request.add_property(
        objects.plane,
        properties.plane_fb_id,
        drm::control::property::Value::Framebuffer(Some(objects.framebuffer)),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_id,
        drm::control::property::Value::CRTC(Some(objects.crtc)),
    );
    request.add_property(
        objects.plane,
        properties.plane_src_x,
        drm::control::property::Value::UnsignedRange(0),
    );
    request.add_property(
        objects.plane,
        properties.plane_src_y,
        drm::control::property::Value::UnsignedRange(0),
    );
    request.add_property(
        objects.plane,
        properties.plane_src_w,
        drm::control::property::Value::UnsignedRange(
            width << LIBDRM_NATIVE_PRIMARY_PLANE_SOURCE_FIXED_POINT_SHIFT,
        ),
    );
    request.add_property(
        objects.plane,
        properties.plane_src_h,
        drm::control::property::Value::UnsignedRange(
            height << LIBDRM_NATIVE_PRIMARY_PLANE_SOURCE_FIXED_POINT_SHIFT,
        ),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_x,
        drm::control::property::Value::SignedRange(0),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_y,
        drm::control::property::Value::SignedRange(0),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_w,
        drm::control::property::Value::UnsignedRange(width),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_h,
        drm::control::property::Value::UnsignedRange(height),
    );
}

/// One cursor plane's contribution to a head's atomic request.
///
/// The same ten properties the primary contributes, with two differences that
/// are the whole reason a cursor plane exists. `CRTC_X`/`CRTC_Y` carry the
/// pointer's position rather than the origin, and they are the only values
/// that change as the pointer moves -- the framebuffer and the sizes stay put
/// once installed, which is what makes a cursor-only commit cheap.
///
/// Hiding is the same request with no framebuffer and no CRTC. A head the
/// pointer has left is hidden by the commit that moves the cursor elsewhere,
/// in that same request, which is what keeps two heads from showing two
/// cursors (`CursorPlaneTransactionOwner.tla`, `CursorLeavesNoGhost`).
#[cfg(feature = "libdrm-events")]
pub fn add_cursor_plane_properties(
    request: &mut drm::control::atomic::AtomicModeReq,
    plane: drm::control::plane::Handle,
    crtc: drm::control::crtc::Handle,
    properties: LibdrmNativeCursorPlanePropertyHandles,
    placement: Option<LibdrmNativeCursorPlacement>,
) {
    let Some(placement) = placement else {
        request.add_property(
            plane,
            properties.plane_fb_id,
            drm::control::property::Value::Framebuffer(None),
        );
        request.add_property(
            plane,
            properties.plane_crtc_id,
            drm::control::property::Value::CRTC(None),
        );
        return;
    };
    request.add_property(
        plane,
        properties.plane_fb_id,
        drm::control::property::Value::Framebuffer(Some(placement.framebuffer)),
    );
    request.add_property(
        plane,
        properties.plane_crtc_id,
        drm::control::property::Value::CRTC(Some(crtc)),
    );
    request.add_property(
        plane,
        properties.plane_src_x,
        drm::control::property::Value::UnsignedRange(0),
    );
    request.add_property(
        plane,
        properties.plane_src_y,
        drm::control::property::Value::UnsignedRange(0),
    );
    request.add_property(
        plane,
        properties.plane_src_w,
        drm::control::property::Value::UnsignedRange(
            u64::from(placement.width) << LIBDRM_NATIVE_PRIMARY_PLANE_SOURCE_FIXED_POINT_SHIFT,
        ),
    );
    request.add_property(
        plane,
        properties.plane_src_h,
        drm::control::property::Value::UnsignedRange(
            u64::from(placement.height) << LIBDRM_NATIVE_PRIMARY_PLANE_SOURCE_FIXED_POINT_SHIFT,
        ),
    );
    request.add_property(
        plane,
        properties.plane_crtc_x,
        drm::control::property::Value::SignedRange(i64::from(placement.x)),
    );
    request.add_property(
        plane,
        properties.plane_crtc_y,
        drm::control::property::Value::SignedRange(i64::from(placement.y)),
    );
    request.add_property(
        plane,
        properties.plane_crtc_w,
        drm::control::property::Value::UnsignedRange(u64::from(placement.width)),
    );
    request.add_property(
        plane,
        properties.plane_crtc_h,
        drm::control::property::Value::UnsignedRange(u64::from(placement.height)),
    );
}

/// Where a cursor sits on one head, and which buffer it shows.
///
/// Absent placement means hidden, which is a state a head genuinely has --
/// the pointer is on another monitor -- rather than an error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeCursorPlacement {
    pub framebuffer: drm::control::framebuffer::Handle,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// A commit that carries a cursor and nothing else.
///
/// The only request in the frame path that attaches no framebuffer to the
/// primary. Atomic requests are sparse, so naming only cursor properties
/// leaves the primary scanning whatever it was scanning -- which is what lets
/// a pointer move across a directly scanned client buffer without evicting
/// it.
///
/// Blocking, with no page-flip event. Blocking because the CRTC is then free
/// when the call returns, which is the only way the owner can enforce one
/// outstanding commit per CRTC without inventing a completion it did not
/// observe. No event because the event reader and the presentation feedback
/// beneath it account for *frames*, and a cursor arriving there would be a
/// pointer that looks like a retired Present.
#[cfg(feature = "libdrm-events")]
pub fn build_native_cursor_only_atomic_request(
    plane: drm::control::plane::Handle,
    crtc: drm::control::crtc::Handle,
    properties: LibdrmNativeCursorPlanePropertyHandles,
    placement: Option<LibdrmNativeCursorPlacement>,
) -> LibdrmNativeAtomicCommitRequest {
    let mut request = drm::control::atomic::AtomicModeReq::new();
    add_cursor_plane_properties(&mut request, plane, crtc, properties, placement);
    LibdrmNativeAtomicCommitRequest::new(request)
        .without_page_flip_event()
        .blocking()
}
