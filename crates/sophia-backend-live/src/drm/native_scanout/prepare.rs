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
    resources: LibdrmNativePrimaryPlaneResourceBundle,
    request: LibdrmNativeAtomicCommitRequest,
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
    let request = if policy.allow_modeset {
        if let Some(vrr_enabled) = policy.vrr_enabled {
            build_native_primary_plane_atomic_request_with_vrr(
                objects,
                property_handles,
                vrr_enabled,
            )
        } else {
            build_native_primary_plane_atomic_request(objects, property_handles)
        }
    } else if let Some(vrr_enabled) = policy.vrr_enabled {
        build_native_primary_plane_page_flip_atomic_request_with_vrr(
            objects,
            property_handles,
            vrr_enabled,
        )
    } else {
        build_native_primary_plane_page_flip_atomic_request(objects, property_handles)
    };
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
            resources: resource_bundle,
            request: request_owner,
        }),
        cleanup: None,
    }
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
