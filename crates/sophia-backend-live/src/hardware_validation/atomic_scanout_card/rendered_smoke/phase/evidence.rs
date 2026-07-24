use crate::prelude::*;

pub(super) fn reduced_smoke_evidence_for_phase(
    phase: LibdrmNativeAtomicScanoutSmokePhase,
    reports: LibdrmNativeScanoutPipelineReports<'_>,
) -> LibdrmNativeAtomicScanoutSmokeEvidence {
    match phase {
        LibdrmNativeAtomicScanoutSmokePhase::InitialModeset => {
            LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports_with_gbm_export_detail(
                reports,
            )
        }
        LibdrmNativeAtomicScanoutSmokePhase::SteadyPageFlip => {
            LibdrmNativeAtomicScanoutSmokeEvidence::from_page_flip_pipeline_reports_with_gbm_export_detail(
                reports,
            )
        }
    }
}
