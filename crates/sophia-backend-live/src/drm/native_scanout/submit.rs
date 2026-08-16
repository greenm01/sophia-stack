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
        descriptor,
        LibdrmNativePrimaryPlaneScanoutSubmitPolicy::modeset(),
    )
}

pub fn submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy<D>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelectionResult,
    descriptor: LiveRendererScanoutBufferDescriptor,
    policy: LibdrmNativePrimaryPlaneScanoutSubmitPolicy,
) -> LibdrmNativePrimaryPlaneScanoutSubmitResult
where
    D: LibdrmNativePropertyLookupDevice
        + LibdrmNativePrimaryPlaneResourceDevice
        + LibdrmNativeAtomicCommitDevice,
{
    submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_optional_dma_bufs(
        device, selection, descriptor, None, policy,
    )
}

pub fn submit_native_primary_plane_scanout_from_selection_and_renderer_dma_bufs_with_policy<D>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelectionResult,
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
    descriptor: LiveRendererScanoutBufferDescriptor,
    plane_fds: Option<[Option<OwnedFd>; 4]>,
    policy: LibdrmNativePrimaryPlaneScanoutSubmitPolicy,
) -> LibdrmNativePrimaryPlaneScanoutSubmitResult
where
    D: LibdrmNativePropertyLookupDevice
        + LibdrmNativePrimaryPlaneResourceDevice
        + LibdrmNativeAtomicCommitDevice,
{
    let mut prepare = match plane_fds {
        Some(plane_fds) => {
            prepare_native_primary_plane_scanout_from_selection_and_renderer_dma_bufs_with_policy(
                device, selection, descriptor, plane_fds, policy,
            )
        }
        None => {
            prepare_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
                device, selection, descriptor, policy,
            )
        }
    };
    if let Some(prepared) = prepare.prepared.take() {
        return submit_prepared_native_primary_plane_scanout(device, prepared);
    }

    let mut result = LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
        match prepare.status {
            LibdrmNativePrimaryPlaneScanoutPrepareStatus::Prepared => {
                LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicRequestBuildFailed
            }
            LibdrmNativePrimaryPlaneScanoutPrepareStatus::KmsTargetUnavailable => {
                LibdrmNativePrimaryPlaneScanoutSubmitStatus::KmsTargetUnavailable
            }
            LibdrmNativePrimaryPlaneScanoutPrepareStatus::ScanoutBufferUnavailable => {
                LibdrmNativePrimaryPlaneScanoutSubmitStatus::ScanoutBufferUnavailable
            }
            LibdrmNativePrimaryPlaneScanoutPrepareStatus::PropertyDiscoveryUnavailable => {
                LibdrmNativePrimaryPlaneScanoutSubmitStatus::PropertyDiscoveryUnavailable
            }
            LibdrmNativePrimaryPlaneScanoutPrepareStatus::ResourceCreationUnavailable => {
                LibdrmNativePrimaryPlaneScanoutSubmitStatus::ResourceCreationUnavailable
            }
            LibdrmNativePrimaryPlaneScanoutPrepareStatus::AtomicRequestBuildFailed => {
                LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicRequestBuildFailed
            }
        },
        prepare.selection,
        prepare.scanout_buffer,
        descriptor,
    );
    result.buffer_format = prepare.buffer_format;
    result.buffer_modifier = prepare.buffer_modifier;
    result.buffer_planes = prepare.buffer_planes;
    result.properties = prepare.properties;
    result.format_table = prepare.format_table;
    result.resources = prepare.resources;
    result.framebuffer = prepare.framebuffer;
    result.request = prepare.request;
    result.request_scope = prepare.request_scope;
    result.commit_flags = prepare.commit_flags;
    result.cleanup = prepare.cleanup;
    result
}
