use crate::prelude::*;
use std::os::fd::OwnedFd;

use super::commit::LibdrmNativeAtomicCommitDevice;

pub fn submit_native_primary_plane_scanout_from_renderer_descriptor<D>(
    device: &D,
    descriptor: LiveRendererScanoutBufferDescriptor,
) -> LibdrmNativePrimaryPlaneScanoutSubmitResult
where
    D: LibdrmNativeKmsSelectionDevice
        + LibdrmNativePropertyLookupDevice
        + LibdrmNativePrimaryPlaneResourceDevice
        + LibdrmNativeAtomicCommitDevice,
{
    let selection = select_native_primary_plane_target(device);
    submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor(
        device, selection, descriptor,
    )
}

pub fn submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor<D>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelectionResult,
    descriptor: LiveRendererScanoutBufferDescriptor,
) -> LibdrmNativePrimaryPlaneScanoutSubmitResult
where
    D: LibdrmNativePropertyLookupDevice
        + LibdrmNativePrimaryPlaneResourceDevice
        + LibdrmNativeAtomicCommitDevice,
{
    submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
        device,
        selection,
        &[],
        descriptor,
        LibdrmNativePrimaryPlaneScanoutSubmitPolicy::modeset(),
    )
}

pub fn submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy<D>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelectionResult,
    peers: &[LibdrmNativePrimaryPlaneSelection],
    descriptor: LiveRendererScanoutBufferDescriptor,
    policy: LibdrmNativePrimaryPlaneScanoutSubmitPolicy,
) -> LibdrmNativePrimaryPlaneScanoutSubmitResult
where
    D: LibdrmNativePropertyLookupDevice
        + LibdrmNativePrimaryPlaneResourceDevice
        + LibdrmNativeAtomicCommitDevice,
{
    submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_optional_dma_bufs(
        device, selection, peers, descriptor, None, policy,
    )
}

pub fn submit_native_primary_plane_scanout_from_selection_and_renderer_dma_bufs_with_policy<D>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelectionResult,
    peers: &[LibdrmNativePrimaryPlaneSelection],
    descriptor: LiveRendererScanoutBufferDescriptor,
    plane_fds: [Option<OwnedFd>; 4],
    policy: LibdrmNativePrimaryPlaneScanoutSubmitPolicy,
) -> LibdrmNativePrimaryPlaneScanoutSubmitResult
where
    D: LibdrmNativePropertyLookupDevice
        + LibdrmNativePrimaryPlaneResourceDevice
        + LibdrmNativeAtomicCommitDevice,
{
    submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_optional_dma_bufs(
        device,
        selection,
        peers,
        descriptor,
        Some(plane_fds),
        policy,
    )
}

fn submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_optional_dma_bufs<
    D,
>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelectionResult,
    peers: &[LibdrmNativePrimaryPlaneSelection],
    descriptor: LiveRendererScanoutBufferDescriptor,
    plane_fds: Option<[Option<OwnedFd>; 4]>,
    policy: LibdrmNativePrimaryPlaneScanoutSubmitPolicy,
) -> LibdrmNativePrimaryPlaneScanoutSubmitResult
where
    D: LibdrmNativePropertyLookupDevice
        + LibdrmNativePrimaryPlaneResourceDevice
        + LibdrmNativeAtomicCommitDevice,
{
    let scanout_buffer = if descriptor.is_valid_scanout_buffer() {
        LiveRendererScanoutBufferStatus::Ready
    } else {
        LiveRendererScanoutBufferStatus::Invalid
    };

    if selection.status != LibdrmNativePrimaryPlaneSelectionStatus::Selected {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::KmsTargetUnavailable,
            selection.status,
            scanout_buffer,
            descriptor,
        );
    }

    let Some(selected) = selection.selection else {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::KmsTargetUnavailable,
            selection.status,
            scanout_buffer,
            descriptor,
        );
    };

    let buffer = LibdrmRendererScanoutBuffer::from_descriptor(descriptor);
    if buffer.is_none() {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::ScanoutBufferUnavailable,
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
        let mut result = LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::PropertyDiscoveryUnavailable,
            selection.status,
            scanout_buffer,
            descriptor,
        );
        result.properties = Some(properties.status);
        return result;
    };
    let format_table =
        Some(LibdrmNativePrimaryPlaneFormatTableStatus::from_property_handles(property_handles));

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
        let mut result = LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::ResourceCreationUnavailable,
            selection.status,
            scanout_buffer,
            descriptor,
        );
        result.properties = Some(properties.status);
        result.format_table = format_table;
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
    let Some(request) = request.request else {
        let destroy = destroy_native_primary_plane_resources(device, resource_bundle);
        let mut result = LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicRequestBuildFailed,
            selection.status,
            scanout_buffer,
            descriptor,
        );
        result.properties = Some(properties.status);
        result.format_table = format_table;
        result.resources = Some(resources.status);
        result.framebuffer = resources.framebuffer;
        result.request = Some(request.status);
        result.cleanup = destroy.cleanup;
        return result;
    };

    let request = if policy.allow_modeset {
        request.allow_modeset()
    } else {
        request
    };
    let request = if policy.page_flip_event {
        request
    } else {
        request.without_page_flip_event()
    };
    let request = if policy.nonblocking {
        request
    } else {
        request.blocking()
    };
    let request_scope = request.reduced_scope();
    if request_scope != policy.expected_request_scope() {
        let destroy = destroy_native_primary_plane_resources(device, resource_bundle);
        let mut result = LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicRequestBuildFailed,
            selection.status,
            scanout_buffer,
            descriptor,
        );
        result.properties = Some(properties.status);
        result.format_table = format_table;
        result.resources = Some(resources.status);
        result.framebuffer = resources.framebuffer;
        result.request = Some(LibdrmNativeAtomicRequestBuildStatus::Built);
        result.request_scope = Some(request_scope);
        result.commit_flags = Some(request.reduced_flags());
        result.cleanup = destroy.cleanup;
        return result;
    }
    let commit_flags = request.reduced_flags();
    let (flags, request) = request.into_native();
    let submit = match device.submit_atomic_commit(flags, request) {
        Ok(()) => LibdrmNativeAtomicCommitSubmitStatus::Submitted,
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
            LibdrmNativeAtomicCommitSubmitStatus::WouldBlock
        }
        Err(_) => LibdrmNativeAtomicCommitSubmitStatus::Rejected,
    };

    if submit != LibdrmNativeAtomicCommitSubmitStatus::Submitted {
        let destroy = destroy_native_primary_plane_resources(device, resource_bundle);
        let mut result = LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicSubmitFailed,
            selection.status,
            scanout_buffer,
            descriptor,
        );
        result.properties = Some(properties.status);
        result.format_table = format_table;
        result.resources = Some(resources.status);
        result.framebuffer = resources.framebuffer;
        result.request = Some(LibdrmNativeAtomicRequestBuildStatus::Built);
        result.request_scope = Some(request_scope);
        result.commit_flags = Some(commit_flags);
        result.submit = Some(submit);
        result.cleanup = destroy.cleanup;
        return result;
    }

    // The first head has taken the frame, so the framebuffer is being scanned out
    // and the submission owns it from here. Every later failure keeps that
    // ownership: destroying the buffer now would pull it out from under a live
    // connector. This is X's local reference across the submit loop, expressed as
    // ownership rather than a refcount.
    let mut heads_committed = 1usize;
    let mut group_status = LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip;
    let mut group_submit = submit;

    for peer in peers {
        // Each head has its own connector, CRTC, and plane, so its own properties.
        let peer_properties = discover_native_primary_plane_property_handles(
            device,
            peer.connector_handle(),
            peer.crtc_handle(),
            peer.plane_handle(),
        );
        let Some(peer_handles) = peer_properties.properties else {
            group_status = LibdrmNativePrimaryPlaneScanoutSubmitStatus::PartiallySubmitted;
            break;
        };

        // The same framebuffer, named again for this head. The bundle is `Copy`,
        // so one buffer becomes one set of objects per connector.
        let peer_objects = resource_bundle.into_objects(*peer);
        let peer_request = if policy.allow_modeset {
            if let Some(vrr_enabled) = policy.vrr_enabled {
                build_native_primary_plane_atomic_request_with_vrr(
                    peer_objects,
                    peer_handles,
                    vrr_enabled,
                )
            } else {
                build_native_primary_plane_atomic_request(peer_objects, peer_handles)
            }
        } else if let Some(vrr_enabled) = policy.vrr_enabled {
            build_native_primary_plane_page_flip_atomic_request_with_vrr(
                peer_objects,
                peer_handles,
                vrr_enabled,
            )
        } else {
            build_native_primary_plane_page_flip_atomic_request(peer_objects, peer_handles)
        };
        let Some(peer_request) = peer_request.request else {
            group_status = LibdrmNativePrimaryPlaneScanoutSubmitStatus::PartiallySubmitted;
            break;
        };

        let peer_request = if policy.allow_modeset {
            peer_request.allow_modeset()
        } else {
            peer_request
        };
        let peer_request = if policy.page_flip_event {
            peer_request
        } else {
            peer_request.without_page_flip_event()
        };
        let peer_request = if policy.nonblocking {
            peer_request
        } else {
            peer_request.blocking()
        };
        let (peer_flags, peer_native) = peer_request.into_native();
        let peer_submit = match device.submit_atomic_commit(peer_flags, peer_native) {
            Ok(()) => LibdrmNativeAtomicCommitSubmitStatus::Submitted,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                LibdrmNativeAtomicCommitSubmitStatus::WouldBlock
            }
            Err(_) => LibdrmNativeAtomicCommitSubmitStatus::Rejected,
        };
        if peer_submit != LibdrmNativeAtomicCommitSubmitStatus::Submitted {
            // Commits already queued are left alone rather than cancelled; the
            // candidate fails closed and the heads that took it will complete.
            group_status = LibdrmNativePrimaryPlaneScanoutSubmitStatus::PartiallySubmitted;
            group_submit = peer_submit;
            break;
        }
        heads_committed = heads_committed.saturating_add(1);
    }

    let mut result = LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
        group_status,
        selection.status,
        scanout_buffer,
        descriptor,
    );
    result.properties = Some(properties.status);
    result.format_table = format_table;
    result.resources = Some(resources.status);
    result.framebuffer = resources.framebuffer;
    result.request = Some(LibdrmNativeAtomicRequestBuildStatus::Built);
    result.request_scope = Some(request_scope);
    result.commit_flags = Some(commit_flags);
    result.submit = Some(group_submit);
    result.heads_committed = heads_committed;
    result.submission = Some(LibdrmNativePrimaryPlaneScanoutSubmission {
        resources: resource_bundle,
    });
    result
}
