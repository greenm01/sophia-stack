#[cfg(feature = "libdrm-events")]
use super::*;
#[cfg(feature = "libdrm-events")]
use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
pub(crate) fn submit_rendered_primary_plane_scanout_from_scanout_target_and_selection_with<D, E>(
    scanout_target: LiveKmsScanoutTargetStatus,
    target: Option<LiveGbmEglFrameTargetRecord>,
    selection: LibdrmNativePrimaryPlaneSelectionResult,
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
    let mut prepare = prepare_rendered_primary_plane_scanout_from_target_and_selection_with(
        scanout_target,
        target,
        selection,
        vrr_enabled,
        device,
        exporter,
    );
    if let Some(prepared) = prepare.prepared.take() {
        return submit_prepared_rendered_primary_plane_scanout(device, prepared);
    }
    let status = match prepare.status {
        LiveRenderedPrimaryPlaneScanoutPrepareStatus::ScanoutExportPending => {
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportPending
        }
        LiveRenderedPrimaryPlaneScanoutPrepareStatus::ScanoutTargetNotReady => {
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady
        }
        LiveRenderedPrimaryPlaneScanoutPrepareStatus::FrameTargetUnavailable => {
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::FrameTargetUnavailable
        }
        LiveRenderedPrimaryPlaneScanoutPrepareStatus::ScanoutExportFailed => {
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed
        }
        LiveRenderedPrimaryPlaneScanoutPrepareStatus::Prepared
        | LiveRenderedPrimaryPlaneScanoutPrepareStatus::PrimaryPlanePrepareFailed => {
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::PrimaryPlaneSubmitFailed
        }
    };
    LiveRenderedPrimaryPlaneScanoutSubmitResult {
        status,
        scanout_target: prepare.scanout_target,
        target: prepare.target,
        export: prepare.export,
        scanout_buffer: prepare.scanout_buffer,
        buffer_format: prepare.buffer_format,
        buffer_modifier: prepare.buffer_modifier,
        buffer_planes: prepare.buffer_planes,
        properties: prepare.properties,
        format_table: prepare.format_table,
        resources: prepare.resources,
        framebuffer: prepare.framebuffer,
        request: prepare.request,
        submit: prepare.submit,
        request_scope: prepare.request_scope,
        commit_flags: prepare.commit_flags,
        commit_submit: None,
        submission: None,
        cleanup: prepare.cleanup,
    }
}
