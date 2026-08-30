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
    cursor_ride: Option<crate::LibdrmNativeAtomicCursor>,
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
    let mut prepare = prepare_rendered_primary_plane_scanout_from_target_and_selection_with_cursor(
        scanout_target,
        target,
        selection,
        vrr_enabled,
        cursor_ride,
        device,
        exporter,
    );
    if let Some(mut prepared) = prepare.prepared.take() {
        // A client's buffer on its way to the plane is the one commit whose
        // refusal must not be terminal. Ask the driver first, on the edge into
        // direct scanout, and treat a refusal as an answer: destroy what this
        // attempt imported and re-offer the same content for composition.
        //
        // The validating commit carries `TEST_ONLY`, so nothing reaches a
        // screen and no page-flip event arrives; the request that flips
        // afterwards is the one the driver was asked about, cloned rather than
        // rebuilt. See `PresentFlipOwnership.tla`, `TestRefuse`/`CommitRefused`.
        if prepared.scanout_buffer.is_direct_client_buffer()
            && exporter.direct_scanout_test_required()
        {
            let (test, primary_plane) =
                validate_prepared_native_primary_plane_scanout(device, prepared.primary_plane);
            prepared.primary_plane = primary_plane;
            let accepted = test == LibdrmNativeAtomicCommitSubmitStatus::Submitted;
            exporter.record_direct_scanout_test(accepted);
            if !accepted {
                return refuse_direct_rendered_primary_plane_scanout(
                    device, prepared, exporter, prepare, test,
                );
            }
        }
        let direct = prepared.scanout_buffer.is_direct_client_buffer();
        let mut result = submit_prepared_rendered_primary_plane_scanout(device, prepared);
        if direct {
            if result.status
                == LiveRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
            {
                // The driver took it. The composed form kept against a refusal
                // is dropped; the client's buffer is not, because it is what
                // the screen will scan until a successor flip retires it.
                exporter.commit_direct_scanout();
            } else {
                // A real commit rejected after a passing test, which the
                // driver is entitled to do. The frame is not lost and the
                // session is not over: the composed form goes back into the
                // pending cell and the status says deferred, not failed.
                //
                // Rewriting the status is the whole point. Left as
                // `PrimaryPlaneSubmitFailed` it reaches the terminal
                // submit-failure path, which discards the frame and, on a
                // mirror group, ends the session -- for a commit the fallback
                // has already recovered from. The cleanup the submit produced
                // is carried through unchanged, so the framebuffer and its
                // imported handles are still destroyed.
                exporter.fall_back_from_direct();
                result.status = LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportPending;
            }
        }
        return result;
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
        cursor_dropped: false,
    }
}

/// Turn a refused direct attempt into a composed one.
///
/// The prepared scanout still owns imported handles and a framebuffer, so it
/// is cancelled through the ordinary destroy path rather than dropped -- the
/// same cleanup a failed submit would have performed. The status reported is
/// `ScanoutExportPending`, not a submit failure: no frame was lost and none
/// was displayed, and the composed form is already back in the exporter's
/// pending cell waiting for the next pass. A direct refusal must never reach
/// the terminal submit-failure path, which on a mirror group ends the session.
#[cfg(feature = "libdrm-events")]
fn refuse_direct_rendered_primary_plane_scanout<D, E>(
    device: &D,
    prepared: LivePreparedRenderedPrimaryPlaneScanout<E::Owner>,
    exporter: &mut E,
    prepare: LiveRenderedPrimaryPlaneScanoutPrepareResult<E::Owner>,
    test: LibdrmNativeAtomicCommitSubmitStatus,
) -> LiveRenderedPrimaryPlaneScanoutSubmitResult<E::Owner>
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
    E: LiveRenderedScanoutBufferExporter,
{
    let cancelled = cancel_prepared_native_primary_plane_scanout(device, prepared.primary_plane);
    exporter.fall_back_from_direct();
    LiveRenderedPrimaryPlaneScanoutSubmitResult {
        status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportPending,
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
        submit: Some(LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicSubmitFailed),
        request_scope: prepare.request_scope,
        commit_flags: prepare.commit_flags,
        commit_submit: Some(test),
        submission: None,
        cursor_dropped: false,
        cleanup: cancelled
            .cleanup
            .map(|primary_plane| LiveRenderedPrimaryPlaneScanoutCleanup {
                scanout_buffer: prepared.scanout_buffer,
                primary_plane,
            }),
    }
}
