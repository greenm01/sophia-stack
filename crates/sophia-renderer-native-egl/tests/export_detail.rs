#![cfg(feature = "gbm-platform")]

use sophia_renderer_native_egl::{
    NativeGbmScanoutBufferExportDetail, NativeGbmScanoutBufferExportStatus,
};

#[test]
fn mixed_composition_failure_stages_remain_reduced_and_degraded() {
    for detail in [
        NativeGbmScanoutBufferExportDetail::CpuLayerUploadFailed,
        NativeGbmScanoutBufferExportDetail::DmaBufImageCreateFailed,
        NativeGbmScanoutBufferExportDetail::DmaBufImageBindFailed,
        NativeGbmScanoutBufferExportDetail::CompositionDrawFailed,
        NativeGbmScanoutBufferExportDetail::CompositionFinishFailed,
        NativeGbmScanoutBufferExportDetail::EglImageDestroyFailed,
    ] {
        assert_eq!(
            detail.status(),
            NativeGbmScanoutBufferExportStatus::Degraded
        );
    }
}

#[test]
fn context_and_pipeline_failures_receive_a_bounded_retry() {
    for detail in [
        NativeGbmScanoutBufferExportDetail::EglMakeCurrentFailed,
        NativeGbmScanoutBufferExportDetail::EglSwapBuffersFailed,
        NativeGbmScanoutBufferExportDetail::GlSmokeFailed,
        NativeGbmScanoutBufferExportDetail::CpuLayerUploadFailed,
        NativeGbmScanoutBufferExportDetail::CompositionDrawFailed,
        NativeGbmScanoutBufferExportDetail::CompositionFinishFailed,
        NativeGbmScanoutBufferExportDetail::EglImageDestroyFailed,
    ] {
        assert!(detail.render_target_retryable());
    }
}

#[test]
fn export_surface_failures_do_not_retry_the_one_shot_target() {
    for detail in [
        NativeGbmScanoutBufferExportDetail::GbmSurfaceUnavailable,
        NativeGbmScanoutBufferExportDetail::EglSurfaceUnavailable,
        NativeGbmScanoutBufferExportDetail::FrontBufferLockFailed,
    ] {
        assert!(!detail.render_target_retryable());
    }
}
