use sophia_protocol::{OutputId, Rect, SurfaceId};
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StartupNativeRecoveryReason {
    MissingOutputCallback,
}

impl StartupNativeRecoveryReason {
    pub(super) const fn reduced_name(self) -> &'static str {
        match self {
            Self::MissingOutputCallback => "missing_output_callback",
        }
    }
}

pub(super) fn startup_native_recovery_reason(
    missing_output_callback: bool,
    elapsed: Duration,
) -> Option<StartupNativeRecoveryReason> {
    (missing_output_callback && elapsed >= Duration::from_millis(750))
        .then_some(StartupNativeRecoveryReason::MissingOutputCallback)
}

#[derive(Debug, Default)]
pub(super) struct StartupSurfacePresentationEvidence {
    stable_nonzero_rgb_pixels: BTreeMap<SurfaceId, usize>,
}

impl StartupSurfacePresentationEvidence {
    pub(super) fn observe_stable(&mut self, surface: SurfaceId, nonzero_rgb_pixels: usize) {
        self.stable_nonzero_rgb_pixels
            .entry(surface)
            .and_modify(|observed| *observed = (*observed).max(nonzero_rgb_pixels))
            .or_insert(nonzero_rgb_pixels);
    }

    pub(super) fn stable_presented(&self, surface: SurfaceId) -> bool {
        self.stable_nonzero_rgb_pixels.contains_key(&surface)
    }

    pub(super) fn nonzero_rgb_pixels(&self, surface: SurfaceId) -> usize {
        self.stable_nonzero_rgb_pixels
            .get(&surface)
            .copied()
            .unwrap_or(0)
    }

    pub(super) fn visual_detail(&self, surface: SurfaceId) -> bool {
        self.nonzero_rgb_pixels(surface) != 0
    }

    pub(super) fn clear(&mut self) {
        self.stable_nonzero_rgb_pixels.clear();
    }
}

pub(super) fn startup_surface_visual_detail(
    cpu_visual_detail: Option<bool>,
    stable_nonzero_rgb_pixels: usize,
) -> bool {
    cpu_visual_detail.unwrap_or(false) || stable_nonzero_rgb_pixels != 0
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StartupOutputEvidence {
    pub required_submission: usize,
    pub presented_submissions: usize,
    pub callbacks: usize,
    pub synchronous_modeset: bool,
}

pub(super) fn startup_output_evidence(
    native: &sophia_backend_live::LiveProductionNativeScanout,
    required_submissions: Option<&[usize]>,
) -> Option<Vec<StartupOutputEvidence>> {
    if required_submissions.is_some_and(|required| required.len() != native.heads.len()) {
        return None;
    }
    Some(
        native
            .heads
            .iter()
            .enumerate()
            .map(|(index, head)| StartupOutputEvidence {
                required_submission: required_submissions
                    .and_then(|required| required.get(index))
                    .copied()
                    .unwrap_or(0),
                presented_submissions: head.presented_submissions,
                callbacks: head.callback_accepted,
                synchronous_modeset: head.initial_modeset_submission.is_some(),
            })
            .collect(),
    )
}

pub(super) fn all_startup_outputs_presented(outputs: &[StartupOutputEvidence]) -> bool {
    !outputs.is_empty()
        && outputs.iter().all(|output| {
            (output.callbacks > 0 || output.synchronous_modeset)
                && output.presented_submissions >= output.required_submission
        })
}

/// Reduces physical-head readiness to the logical outputs exposed to Engine.
///
/// Every head of a mirror group must be ready before its shared output is ready,
/// but the group contributes only one output to startup progress.
pub(super) fn logical_startup_output_progress(
    heads: impl IntoIterator<Item = (OutputId, bool)>,
) -> (usize, usize) {
    let mut outputs = BTreeMap::<OutputId, bool>::new();
    for (output, ready) in heads {
        outputs
            .entry(output)
            .and_modify(|group_ready| *group_ready &= ready)
            .or_insert(ready);
    }
    let ready = outputs.values().filter(|ready| **ready).count();
    (ready, outputs.len())
}

pub(super) fn synchronous_modeset_record(output: u64, submission: Option<usize>) -> Option<String> {
    submission.map(|submission| {
        format!(
            "sophia_live_native_startup_output schema=1 status=presented output={output} proof=synchronous_modeset submission={submission}"
        )
    })
}

/// Emits one synchronous startup record per logical output.
///
/// A mirror group has synchronous proof only when every physical head has it.
pub(super) fn logical_synchronous_modeset_records(
    heads: impl IntoIterator<Item = (OutputId, Option<usize>)>,
) -> Vec<String> {
    let mut outputs = BTreeMap::<OutputId, (bool, usize)>::new();
    for (output, submission) in heads {
        let state = outputs.entry(output).or_insert((true, 0));
        state.0 &= submission.is_some();
        if let Some(submission) = submission {
            state.1 = state.1.max(submission);
        }
    }
    outputs
        .into_iter()
        .filter_map(|(output, (synchronous, submission))| {
            synchronous
                .then(|| synchronous_modeset_record(output.raw(), Some(submission)))
                .flatten()
        })
        .collect()
}

pub(super) const fn independent_native_output_presented(
    submissions: usize,
    retirements: usize,
    callbacks: usize,
    synchronous_modeset: bool,
    nonzero_exports: usize,
) -> bool {
    let asynchronous_lifecycle =
        retirements > 0 && callbacks == retirements && submissions == retirements + 1;
    let synchronous_lifecycle =
        synchronous_modeset && submissions == 1 && retirements == 0 && callbacks == 0;
    nonzero_exports > 0 && (asynchronous_lifecycle || synchronous_lifecycle)
}

pub(super) const fn startup_submission_requirement(
    submissions: usize,
    presented_submissions: usize,
    surface_intersects_output: bool,
) -> usize {
    if surface_intersects_output {
        if submissions > presented_submissions {
            submissions
        } else {
            presented_submissions.saturating_add(1)
        }
    } else {
        0
    }
}

pub(super) const fn rects_intersect(left: Rect, right: Rect) -> bool {
    !left.is_empty()
        && !right.is_empty()
        && left.x < right.x.saturating_add(right.width)
        && right.x < left.x.saturating_add(left.width)
        && left.y < right.y.saturating_add(right.height)
        && right.y < left.y.saturating_add(left.height)
}
