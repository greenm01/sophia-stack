use crate::prelude::*;
use std::os::fd::OwnedFd;

/// Complete native resources and atomic request for one head, before the
/// kernel has accepted a page flip.
///
/// This is an affine owner. Callers must either submit it or cancel it against
/// the same DRM device; dropping it would leak the framebuffer/import/blob
/// resources it owns.
#[derive(Debug)]
pub struct LibdrmNativePrimaryPlanePreparedScanout {
    descriptor: LiveRendererScanoutBufferDescriptor,
    selection: LibdrmNativePrimaryPlaneSelectionStatus,
    properties: LibdrmNativePrimaryPlanePropertyDiscoveryStatus,
    format_table: LibdrmNativePrimaryPlaneFormatTableStatus,
    resources_status: LibdrmNativePrimaryPlaneResourceCreateStatus,
    framebuffer: Option<LibdrmNativePrimaryPlaneFramebufferCreateDetail>,
    request_scope: LibdrmNativeAtomicCommitRequestScope,
    commit_flags: LibdrmNativeAtomicCommitFlagsReport,
    selected: LibdrmNativePrimaryPlaneSelection,
    property_handles: LibdrmNativePrimaryPlanePropertyHandles,
    resources: LibdrmNativePrimaryPlaneResourceBundle,
    request: LibdrmNativeAtomicCommitRequest,
}

/// One enabled head's complete resources for a card-scoped topology commit.
///
/// The framebuffer/imports/mode blob remain affine. After the combined request
/// succeeds, adopt this owner into a scanout submission; otherwise cancel it.
#[derive(Debug)]
pub struct LibdrmNativePrimaryPlanePreparedTopologyHead {
    atomic_head: LibdrmNativeAtomicHead,
    resources: LibdrmNativePrimaryPlaneResourceBundle,
}

#[derive(Clone, Copy, Debug)]
pub struct LibdrmNativePreparedDisabledTopologyHead {
    atomic_head: LibdrmNativeAtomicDisabledHead,
}

impl LibdrmNativePreparedDisabledTopologyHead {
    pub const fn atomic_head(self) -> LibdrmNativeAtomicDisabledHead {
        self.atomic_head
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeDisabledTopologyHeadPrepareStatus {
    Prepared,
    PropertyDiscoveryUnavailable,
}

#[derive(Clone, Copy, Debug)]
pub struct LibdrmNativeDisabledTopologyHeadPrepareResult {
    pub status: LibdrmNativeDisabledTopologyHeadPrepareStatus,
    pub properties: LibdrmNativePrimaryPlanePropertyDiscoveryStatus,
    pub prepared: Option<LibdrmNativePreparedDisabledTopologyHead>,
}

impl LibdrmNativePrimaryPlanePreparedTopologyHead {
    pub const fn atomic_head(&self) -> LibdrmNativeAtomicHead {
        self.atomic_head
    }
}

/// Resolves the property handles needed to detach one disabled head.
///
/// Unlike an enabled head this owns no framebuffer or mode blob, but it is still
/// a required prepared member: property discovery must succeed before any card
/// applies, otherwise rollback could be required after a failure that was known
/// in advance.
pub fn prepare_native_disabled_topology_head<D>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelection,
) -> LibdrmNativeDisabledTopologyHeadPrepareResult
where
    D: LibdrmNativePropertyLookupDevice,
{
    let properties = discover_native_primary_plane_property_handles(
        device,
        selection.connector_handle(),
        selection.crtc_handle(),
        selection.plane_handle(),
    );
    let Some(handles) = properties.properties else {
        return LibdrmNativeDisabledTopologyHeadPrepareResult {
            status: LibdrmNativeDisabledTopologyHeadPrepareStatus::PropertyDiscoveryUnavailable,
            properties: properties.status,
            prepared: None,
        };
    };
    LibdrmNativeDisabledTopologyHeadPrepareResult {
        status: LibdrmNativeDisabledTopologyHeadPrepareStatus::Prepared,
        properties: properties.status,
        prepared: Some(LibdrmNativePreparedDisabledTopologyHead {
            atomic_head: LibdrmNativeAtomicDisabledHead::new(selection, handles),
        }),
    }
}

#[derive(Debug)]
pub struct LibdrmNativePrimaryPlaneScanoutPrepareResult {
    pub status: LibdrmNativePrimaryPlaneScanoutPrepareStatus,
    pub selection: LibdrmNativePrimaryPlaneSelectionStatus,
    pub scanout_buffer: LiveRendererScanoutBufferStatus,
    pub buffer_format: Option<LibdrmNativeScanoutBufferFormatDetail>,
    pub buffer_modifier: Option<LibdrmNativeScanoutBufferModifierDetail>,
    pub buffer_planes: Option<LibdrmNativeScanoutBufferPlaneDetail>,
    pub properties: Option<LibdrmNativePrimaryPlanePropertyDiscoveryStatus>,
    pub format_table: Option<LibdrmNativePrimaryPlaneFormatTableStatus>,
    pub resources: Option<LibdrmNativePrimaryPlaneResourceCreateStatus>,
    pub framebuffer: Option<LibdrmNativePrimaryPlaneFramebufferCreateDetail>,
    pub request: Option<LibdrmNativeAtomicRequestBuildStatus>,
    pub request_scope: Option<LibdrmNativeAtomicCommitRequestScope>,
    pub commit_flags: Option<LibdrmNativeAtomicCommitFlagsReport>,
    pub prepared: Option<LibdrmNativePrimaryPlanePreparedScanout>,
    pub cleanup: Option<LibdrmNativePrimaryPlaneResourceCleanup>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneScanoutPrepareStatus {
    Prepared,
    KmsTargetUnavailable,
    ScanoutBufferUnavailable,
    PropertyDiscoveryUnavailable,
    ResourceCreationUnavailable,
    AtomicRequestBuildFailed,
}

impl LibdrmNativePrimaryPlaneScanoutPrepareResult {
    fn from_descriptor(
        status: LibdrmNativePrimaryPlaneScanoutPrepareStatus,
        selection: LibdrmNativePrimaryPlaneSelectionStatus,
        scanout_buffer: LiveRendererScanoutBufferStatus,
        descriptor: LiveRendererScanoutBufferDescriptor,
    ) -> Self {
        Self {
            status,
            selection,
            scanout_buffer,
            buffer_format: Some(LibdrmNativeScanoutBufferFormatDetail::from_descriptor(
                descriptor,
            )),
            buffer_modifier: Some(LibdrmNativeScanoutBufferModifierDetail::from_descriptor(
                descriptor,
            )),
            buffer_planes: Some(LibdrmNativeScanoutBufferPlaneDetail::from_descriptor(
                descriptor,
            )),
            properties: None,
            format_table: None,
            resources: None,
            framebuffer: None,
            request: None,
            request_scope: None,
            commit_flags: None,
            prepared: None,
            cleanup: None,
        }
    }
}

pub fn prepare_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy<D>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelectionResult,
    descriptor: LiveRendererScanoutBufferDescriptor,
    policy: LibdrmNativePrimaryPlaneScanoutSubmitPolicy,
) -> LibdrmNativePrimaryPlaneScanoutPrepareResult
where
    D: LibdrmNativePropertyLookupDevice + LibdrmNativePrimaryPlaneResourceDevice,
{
    prepare_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_optional_dma_bufs(
        device, selection, descriptor, None, policy,
    )
}

pub fn prepare_native_primary_plane_scanout_from_selection_and_renderer_dma_bufs_with_policy<D>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelectionResult,
    descriptor: LiveRendererScanoutBufferDescriptor,
    plane_fds: [Option<OwnedFd>; 4],
    policy: LibdrmNativePrimaryPlaneScanoutSubmitPolicy,
) -> LibdrmNativePrimaryPlaneScanoutPrepareResult
where
    D: LibdrmNativePropertyLookupDevice + LibdrmNativePrimaryPlaneResourceDevice,
{
    prepare_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_optional_dma_bufs(
        device,
        selection,
        descriptor,
        Some(plane_fds),
        policy,
    )
}

fn prepare_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_optional_dma_bufs<
    D,
>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelectionResult,
    descriptor: LiveRendererScanoutBufferDescriptor,
    plane_fds: Option<[Option<OwnedFd>; 4]>,
    policy: LibdrmNativePrimaryPlaneScanoutSubmitPolicy,
) -> LibdrmNativePrimaryPlaneScanoutPrepareResult
where
    D: LibdrmNativePropertyLookupDevice + LibdrmNativePrimaryPlaneResourceDevice,
{
    let scanout_buffer = if descriptor.is_valid_scanout_buffer() {
        LiveRendererScanoutBufferStatus::Ready
    } else {
        LiveRendererScanoutBufferStatus::Invalid
    };
    if selection.status != LibdrmNativePrimaryPlaneSelectionStatus::Selected {
        return LibdrmNativePrimaryPlaneScanoutPrepareResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutPrepareStatus::KmsTargetUnavailable,
            selection.status,
            scanout_buffer,
            descriptor,
        );
    }
    let Some(selected) = selection.selection else {
        return LibdrmNativePrimaryPlaneScanoutPrepareResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutPrepareStatus::KmsTargetUnavailable,
            selection.status,
            scanout_buffer,
            descriptor,
        );
    };
    let buffer = LibdrmRendererScanoutBuffer::from_descriptor(descriptor);
    if buffer.is_none() {
        return LibdrmNativePrimaryPlaneScanoutPrepareResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutPrepareStatus::ScanoutBufferUnavailable,
            selection.status,
            scanout_buffer,
            descriptor,
        );
    }

    let properties = discover_native_primary_plane_property_handles(
        device,
        selected.connector,
        selected.crtc,
        selected.plane,
    );
    let Some(property_handles) = properties.properties else {
        let mut result = LibdrmNativePrimaryPlaneScanoutPrepareResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutPrepareStatus::PropertyDiscoveryUnavailable,
            selection.status,
            scanout_buffer,
            descriptor,
        );
        result.properties = Some(properties.status);
        return result;
    };
    let format_table =
        LibdrmNativePrimaryPlaneFormatTableStatus::from_property_handles(property_handles);
    let resources = match (policy.allow_modeset, plane_fds) {
        (true, Some(plane_fds)) => create_native_primary_plane_resources_from_dma_bufs(
            device, selected, descriptor, plane_fds,
        ),
        (false, Some(plane_fds)) => create_native_primary_plane_page_flip_resources_from_dma_bufs(
            device, selected, descriptor, plane_fds,
        ),
        (true, None) => create_native_primary_plane_resources(
            device,
            selected,
            buffer
                .as_ref()
                .expect("validated descriptor should produce a buffer"),
        ),
        (false, None) => create_native_primary_plane_page_flip_resources(
            device,
            selected,
            buffer
                .as_ref()
                .expect("validated descriptor should produce a buffer"),
        ),
    };
    let Some(resource_bundle) = resources.resources else {
        let mut result = LibdrmNativePrimaryPlaneScanoutPrepareResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutPrepareStatus::ResourceCreationUnavailable,
            selection.status,
            scanout_buffer,
            descriptor,
        );
        result.properties = Some(properties.status);
        result.format_table = Some(format_table);
        result.resources = Some(resources.status);
        result.framebuffer = resources.framebuffer;
        result.cleanup = resources.cleanup;
        return result;
    };

    let objects = resource_bundle.into_objects(selected);
    let request =
        build_native_primary_plane_atomic_request_for_policy(objects, property_handles, policy);
    let Some(request_owner) = request.request else {
        let destroy = destroy_native_primary_plane_resources(device, resource_bundle);
        let mut result = LibdrmNativePrimaryPlaneScanoutPrepareResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutPrepareStatus::AtomicRequestBuildFailed,
            selection.status,
            scanout_buffer,
            descriptor,
        );
        result.properties = Some(properties.status);
        result.format_table = Some(format_table);
        result.resources = Some(resources.status);
        result.framebuffer = resources.framebuffer;
        result.request = Some(request.status);
        result.cleanup = destroy.cleanup;
        return result;
    };
    let request_owner = if policy.allow_modeset {
        request_owner.allow_modeset()
    } else {
        request_owner
    };
    let request_owner = if policy.page_flip_event {
        request_owner
    } else {
        request_owner.without_page_flip_event()
    };
    let request_owner = if policy.nonblocking {
        request_owner
    } else {
        request_owner.blocking()
    };
    // A validating commit asks the driver about this exact framebuffer and
    // changes nothing. The flag owner clears the page-flip event for it,
    // since there is no flip to report and the kernel refuses the pair.
    let request_owner = if policy.test_only {
        request_owner.test_only()
    } else {
        request_owner
    };
    let request_scope = request_owner.reduced_scope();
    if request_scope != policy.expected_request_scope() {
        let commit_flags = request_owner.reduced_flags();
        let destroy = destroy_native_primary_plane_resources(device, resource_bundle);
        let mut result = LibdrmNativePrimaryPlaneScanoutPrepareResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutPrepareStatus::AtomicRequestBuildFailed,
            selection.status,
            scanout_buffer,
            descriptor,
        );
        result.properties = Some(properties.status);
        result.format_table = Some(format_table);
        result.resources = Some(resources.status);
        result.framebuffer = resources.framebuffer;
        result.request = Some(LibdrmNativeAtomicRequestBuildStatus::Built);
        result.request_scope = Some(request_scope);
        result.commit_flags = Some(commit_flags);
        result.cleanup = destroy.cleanup;
        return result;
    }
    let commit_flags = request_owner.reduced_flags();
    LibdrmNativePrimaryPlaneScanoutPrepareResult {
        status: LibdrmNativePrimaryPlaneScanoutPrepareStatus::Prepared,
        selection: selection.status,
        scanout_buffer,
        buffer_format: Some(LibdrmNativeScanoutBufferFormatDetail::from_descriptor(
            descriptor,
        )),
        buffer_modifier: Some(LibdrmNativeScanoutBufferModifierDetail::from_descriptor(
            descriptor,
        )),
        buffer_planes: Some(LibdrmNativeScanoutBufferPlaneDetail::from_descriptor(
            descriptor,
        )),
        properties: Some(properties.status),
        format_table: Some(format_table),
        resources: Some(resources.status),
        framebuffer: resources.framebuffer,
        request: Some(LibdrmNativeAtomicRequestBuildStatus::Built),
        request_scope: Some(request_scope),
        commit_flags: Some(commit_flags),
        prepared: Some(LibdrmNativePrimaryPlanePreparedScanout {
            descriptor,
            selection: selection.status,
            properties: properties.status,
            format_table,
            resources_status: resources.status,
            framebuffer: resources.framebuffer,
            request_scope,
            commit_flags,
            selected,
            property_handles,
            resources: resource_bundle,
            request: request_owner,
        }),
        cleanup: None,
    }
}

pub fn prepare_native_topology_head_from_prepared_scanout(
    prepared: LibdrmNativePrimaryPlanePreparedScanout,
    vrr_enabled: Option<bool>,
) -> Result<LibdrmNativePrimaryPlanePreparedTopologyHead, LibdrmNativePrimaryPlanePreparedScanout> {
    if prepared.request_scope != LibdrmNativeAtomicCommitRequestScope::Modeset
        || !prepared.resources.mode_blob.is_some_and(|blob| blob != 0)
    {
        return Err(prepared);
    }
    let mut atomic_head = LibdrmNativeAtomicHead::new(
        prepared.resources.into_objects(prepared.selected),
        prepared.property_handles,
    );
    if let Some(enabled) = vrr_enabled {
        atomic_head = atomic_head.with_vrr(enabled);
    }
    Ok(LibdrmNativePrimaryPlanePreparedTopologyHead {
        atomic_head,
        resources: prepared.resources,
    })
}

/// Transfers prepared resources into the ordinary page-flip retirement owner
/// after the containing card-scoped topology request was accepted.
pub fn adopt_prepared_native_topology_head_after_commit(
    prepared: LibdrmNativePrimaryPlanePreparedTopologyHead,
) -> LibdrmNativePrimaryPlaneScanoutSubmission {
    LibdrmNativePrimaryPlaneScanoutSubmission {
        resources: prepared.resources,
    }
}

pub fn cancel_prepared_native_topology_head<D>(
    device: &D,
    prepared: LibdrmNativePrimaryPlanePreparedTopologyHead,
) -> LibdrmNativePrimaryPlaneResourceDestroyReport
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    destroy_native_primary_plane_resources(device, prepared.resources)
}

/// Ask the driver whether a prepared scanout would be accepted.
///
/// The commit carries `TEST_ONLY`, so the screen does not change and no
/// page-flip event arrives. A refusal is an answer, not a fault: it means
/// this buffer cannot be scanned out on this plane, and the caller composes
/// instead. The prepared scanout is returned either way, because the
/// resources it holds are still owed a submit or a cancel.
///
/// The request is cloned rather than rebuilt, so the commit that flips is the
/// one the driver was asked about, down to the framebuffer id.
///
/// Errno is not inspected. The commit layer classifies every failure the same
/// way, and a driver refusing a modifier is indistinguishable here from one
/// refusing anything else -- which is why a refusal falls back rather than
/// retrying with a different guess.
#[cfg(feature = "libdrm-events")]
pub fn validate_prepared_native_primary_plane_scanout<D>(
    device: &D,
    prepared: LibdrmNativePrimaryPlanePreparedScanout,
) -> (
    LibdrmNativeAtomicCommitSubmitStatus,
    LibdrmNativePrimaryPlanePreparedScanout,
)
where
    D: LibdrmNativeAtomicCommitDevice,
{
    let (flags, native) = prepared.request.clone().test_only().into_native();
    let status = match device.submit_atomic_commit(flags, native) {
        Ok(()) => LibdrmNativeAtomicCommitSubmitStatus::Submitted,
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            LibdrmNativeAtomicCommitSubmitStatus::WouldBlock
        }
        Err(_) => LibdrmNativeAtomicCommitSubmitStatus::Rejected,
    };
    (status, prepared)
}

pub fn submit_prepared_native_primary_plane_scanout<D>(
    device: &D,
    prepared: LibdrmNativePrimaryPlanePreparedScanout,
) -> LibdrmNativePrimaryPlaneScanoutSubmitResult
where
    D: LibdrmNativeAtomicCommitDevice + LibdrmNativePrimaryPlaneResourceDevice,
{
    let (flags, request) = prepared.request.into_native();
    let submit = match device.submit_atomic_commit(flags, request) {
        Ok(()) => LibdrmNativeAtomicCommitSubmitStatus::Submitted,
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            LibdrmNativeAtomicCommitSubmitStatus::WouldBlock
        }
        Err(_) => LibdrmNativeAtomicCommitSubmitStatus::Rejected,
    };
    let mut result = LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
        if submit == LibdrmNativeAtomicCommitSubmitStatus::Submitted {
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
        } else {
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicSubmitFailed
        },
        prepared.selection,
        LiveRendererScanoutBufferStatus::Ready,
        prepared.descriptor,
    );
    result.properties = Some(prepared.properties);
    result.format_table = Some(prepared.format_table);
    result.resources = Some(prepared.resources_status);
    result.framebuffer = prepared.framebuffer;
    result.request = Some(LibdrmNativeAtomicRequestBuildStatus::Built);
    result.request_scope = Some(prepared.request_scope);
    result.commit_flags = Some(prepared.commit_flags);
    result.submit = Some(submit);
    if submit == LibdrmNativeAtomicCommitSubmitStatus::Submitted {
        result.submission = Some(LibdrmNativePrimaryPlaneScanoutSubmission {
            resources: prepared.resources,
        });
    } else {
        result.cleanup = destroy_native_primary_plane_resources(device, prepared.resources).cleanup;
    }
    result
}

pub fn cancel_prepared_native_primary_plane_scanout<D>(
    device: &D,
    prepared: LibdrmNativePrimaryPlanePreparedScanout,
) -> LibdrmNativePrimaryPlaneResourceDestroyReport
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    destroy_native_primary_plane_resources(device, prepared.resources)
}

/// Put a cursor on its plane, and nothing else.
///
/// Returns when the commit has been applied, because it blocks -- so the CRTC
/// is free by the time the caller reads the answer, and the owner never has
/// to guess at a completion it did not observe.
///
/// A refusal is reported rather than raised. The cursor's position stays
/// pending and the next commit carries it; a pointer that stutters is not a
/// reason to fail a session, and a cursor must never cost a frame.
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
pub fn submit_native_cursor_only_commit<D>(
    device: &D,
    request: LibdrmNativeAtomicCommitRequest,
) -> LibdrmNativeAtomicCommitSubmitStatus
where
    D: LibdrmNativeAtomicCommitDevice,
{
    let (flags, request) = request.into_native();
    match device.submit_atomic_commit(flags, request) {
        Ok(()) => LibdrmNativeAtomicCommitSubmitStatus::Submitted,
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            LibdrmNativeAtomicCommitSubmitStatus::WouldBlock
        }
        Err(_) => LibdrmNativeAtomicCommitSubmitStatus::Rejected,
    }
}
