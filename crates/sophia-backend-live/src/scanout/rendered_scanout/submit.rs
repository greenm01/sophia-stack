#[cfg(feature = "libdrm-events")]
use super::*;
#[cfg(feature = "libdrm-events")]
use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
pub(crate) fn submit_rendered_primary_plane_scanout_from_scanout_target_and_selection_with<D, E>(
    scanout_target: LiveKmsScanoutTargetStatus,
    target: Option<LiveGbmEglFrameTargetRecord>,
    selection: LibdrmNativePrimaryPlaneSelectionResult,
    peers: &[LibdrmNativePrimaryPlaneSelection],
    vrr_enabled: Option<bool>,
    device: &D,
    exporter: &mut E,
) -> LiveRenderedPrimaryPlaneScanoutSubmitResult<E::Owner>
where
    D: LibdrmNativePropertyLookupDevice
        + LibdrmNativePrimaryPlaneResourceDevice
        + LibdrmNativeAtomicCommitDevice,
    E: LiveRenderedScanoutBufferExporter,
    E::Owner: LiveRenderedScanoutBufferPrimeSource,
{
    if scanout_target != LiveKmsScanoutTargetStatus::Ready {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult::stopped_before_native_submit(
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady,
            scanout_target,
            target.map(|target| target.status),
            None,
        );
    }

    let Some(target) = target else {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult::stopped_before_native_submit(
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::FrameTargetUnavailable,
            scanout_target,
            None,
            None,
        );
    };

    let scanout_target =
        reduced_scanout_target_status_from_native_selection(scanout_target, target, &selection);
    if scanout_target != LiveKmsScanoutTargetStatus::Ready {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult::stopped_before_native_submit(
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady,
            scanout_target,
            Some(target.status),
            None,
        );
    }

    let export = exporter.export_rendered_scanout_buffer(target).normalized();
    if export.status == LiveRendererScanoutBufferExportStatus::Pending {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult::stopped_before_native_submit(
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportPending,
            scanout_target,
            Some(target.status),
            Some(export.status),
        );
    }
    if export.status != LiveRendererScanoutBufferExportStatus::Exported {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult::stopped_before_native_submit(
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed,
            scanout_target,
            Some(target.status),
            Some(export.status),
        );
    }

    let (Some(descriptor), Some(owner)) = (export.descriptor, export.owner) else {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult::stopped_before_native_submit(
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed,
            scanout_target,
            Some(target.status),
            Some(export.status),
        );
    };

    let shares_kms_drm_file = owner.shares_kms_drm_file();
    let prime_fds = (!shares_kms_drm_file)
        .then(|| owner.export_scanout_dma_buf_fds().ok().flatten())
        .flatten();
    if !shares_kms_drm_file && prime_fds.is_none() {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult::stopped_before_native_submit(
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed,
            scanout_target,
            Some(target.status),
            Some(export.status),
        );
    }
    let mut submit = if shares_kms_drm_file {
        submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
            device,
            selection,
            peers,
            descriptor,
            rendered_page_flip_policy(vrr_enabled),
        )
    } else if let Some(prime_fds) = prime_fds {
        submit_native_primary_plane_scanout_from_selection_and_renderer_dma_bufs_with_policy(
            device,
            selection,
            peers,
            descriptor,
            prime_fds.into_plane_fds(),
            rendered_page_flip_policy(vrr_enabled),
        )
    } else {
        unreachable!("independent DRM files require PRIME transport")
    };
    // A partial group commit is a failed candidate that still owns its buffer, so
    // it falls through to the success path below rather than being treated as a
    // submit that produced nothing. Returning here would drop the submission and
    // leak a framebuffer a connector is scanning out.
    let partially_submitted =
        submit.status == LibdrmNativePrimaryPlaneScanoutSubmitStatus::PartiallySubmitted;
    if !partially_submitted
        && submit.status != LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::PrimaryPlaneSubmitFailed,
            scanout_target,
            target: Some(target.status),
            export: Some(export.status),
            scanout_buffer: Some(submit.scanout_buffer),
            buffer_format: submit.buffer_format,
            buffer_modifier: submit.buffer_modifier,
            buffer_planes: submit.buffer_planes,
            properties: submit.properties,
            format_table: submit.format_table,
            resources: submit.resources,
            framebuffer: submit.framebuffer,
            request: submit.request,
            submit: Some(submit.status),
            request_scope: submit.request_scope,
            commit_flags: submit.commit_flags,
            commit_submit: submit.submit,
            submission: None,
            cleanup: submit
                .cleanup
                .map(|primary_plane| LiveRenderedPrimaryPlaneScanoutCleanup {
                    scanout_buffer: owner,
                    primary_plane,
                }),
        };
    }

    let Some(primary_plane) = submit.submission.take() else {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::PrimaryPlaneSubmitFailed,
            scanout_target,
            target: Some(target.status),
            export: Some(export.status),
            scanout_buffer: Some(submit.scanout_buffer),
            buffer_format: submit.buffer_format,
            buffer_modifier: submit.buffer_modifier,
            buffer_planes: submit.buffer_planes,
            properties: submit.properties,
            format_table: submit.format_table,
            resources: submit.resources,
            framebuffer: submit.framebuffer,
            request: submit.request,
            submit: Some(submit.status),
            request_scope: submit.request_scope,
            commit_flags: submit.commit_flags,
            commit_submit: submit.submit,
            submission: None,
            cleanup: None,
        };
    };

    LiveRenderedPrimaryPlaneScanoutSubmitResult {
        status: if partially_submitted {
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::PartiallySubmitted
        } else {
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
        },
        scanout_target,
        target: Some(target.status),
        export: Some(export.status),
        scanout_buffer: Some(submit.scanout_buffer),
        buffer_format: submit.buffer_format,
        buffer_modifier: submit.buffer_modifier,
        buffer_planes: submit.buffer_planes,
        properties: submit.properties,
        format_table: submit.format_table,
        resources: submit.resources,
        framebuffer: submit.framebuffer,
        request: submit.request,
        submit: Some(submit.status),
        request_scope: submit.request_scope,
        commit_flags: submit.commit_flags,
        commit_submit: submit.submit,
        submission: Some(LiveRenderedPrimaryPlaneScanoutSubmission {
            scanout_buffer: owner,
            primary_plane,
            submitted_after_page_flip_serial: None,
        }),
        cleanup: None,
    }
}

#[cfg(feature = "libdrm-events")]
fn rendered_page_flip_policy(
    vrr_enabled: Option<bool>,
) -> LibdrmNativePrimaryPlaneScanoutSubmitPolicy {
    vrr_enabled.map_or_else(
        LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip,
        |enabled| {
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip().with_vrr_enabled(enabled)
        },
    )
}
