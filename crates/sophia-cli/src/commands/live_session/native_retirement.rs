use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NativePresentRetirementObservation {
    pub surface: SurfaceId,
    pub stable: bool,
    pub ust_usec: u64,
    pub msc: u64,
}

pub(super) fn record_native_present_retirement(
    layout: &mut PersistentLiveLayout,
    runtime: &LiveProductionVisualRuntime,
    native_scanout: &LiveProductionNativeScanout,
    retired: LiveProductionRetiredPresent,
    retired_present_surfaces: &mut BTreeMap<SurfaceId, TransactionId>,
    startup_surface_presentations: &mut StartupSurfacePresentationEvidence,
    startup_readiness: &mut SessionStartupReadiness,
) -> NativePresentRetirementObservation {
    layout.complete_admission_retirement(retired.surface, retired.transaction);
    let stable = runtime.stable_present(native_scanout, retired.transaction);
    retired_present_surfaces.insert(retired.surface, retired.transaction);
    if stable {
        startup_surface_presentations.observe_stable(
            retired.surface,
            native_scanout.presented_mixed_nonzero_rgb_pixels(retired.transaction),
        );
        let _ = reduce_session_startup(
            startup_readiness,
            SessionStartupEvent::StablePresented(retired.surface),
        );
    }

    let clip = retired.clip.map_or_else(
        || "none".to_owned(),
        |clip| format!("{}x{}_{}_{}", clip.width, clip.height, clip.x, clip.y),
    );
    println!(
        "sophia_live_session_present schema=2 status=retired transaction={} surface={} source={}x{} target={}x{}_{}_{} clip={} unit_scale={}",
        retired.transaction.raw(),
        retired.surface.index(),
        retired.source_size.width,
        retired.source_size.height,
        retired.target.width,
        retired.target.height,
        retired.target.x,
        retired.target.y,
        clip,
        retired.source_size.width == retired.clip.unwrap_or(retired.target).width
            && retired.source_size.height == retired.clip.unwrap_or(retired.target).height,
    );
    println!(
        "sophia_live_session_scanout schema=1 status={} kind=mixed transaction={} pending_primary={}",
        if stable { "stable" } else { "superseded" },
        retired.transaction.raw(),
        !stable,
    );

    NativePresentRetirementObservation {
        surface: retired.surface,
        stable,
        ust_usec: retired.ust_usec,
        msc: retired.msc,
    }
}

pub(super) fn correlate_physical_input_page_flip(
    input_delivery_complete: bool,
    input_pixel_change: bool,
    input_raw_ingress_msec: Option<u64>,
    input_change_submission_baseline: Option<usize>,
    native_scanout: &LiveProductionNativeScanout,
    input_presented_ust_usec: &mut Option<u64>,
    input_submit_to_page_flip: &mut Option<Duration>,
) {
    if input_presented_ust_usec.is_some() {
        return;
    }
    let (Some(ingress_ust_usec), Some(baseline_submission), Some(head)) = (
        input_raw_ingress_msec.and_then(|msec| msec.checked_mul(1_000)),
        input_change_submission_baseline,
        native_scanout.heads.first(),
    ) else {
        return;
    };
    if !physical_input_page_flip_correlates(
        input_delivery_complete,
        input_pixel_change,
        ingress_ust_usec,
        baseline_submission,
        head.presented_submissions,
        head.presented_submission_ust_usec,
        head.presented_page_flip_ust_usec,
    ) {
        return;
    }
    *input_presented_ust_usec = Some(head.presented_page_flip_ust_usec);
    *input_submit_to_page_flip = Some(head.presented_submit_to_page_flip);
}
