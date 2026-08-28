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

/// What one head owes before the focused surface counts as presented on it.
///
/// The submission count alone was satisfiable by a flip carrying a
/// composition planned before the surface had content: the requirement is
/// pinned when visual detail first appears, an already-running render then
/// finishes into a later submission, and the barrier passed while the glass
/// still showed the pre-content frame. The content frame pins the newest
/// composition the head held anywhere in its pipeline at that same moment,
/// and presentation must exceed it -- a picture planned at or after the
/// content, not merely a flip that happened after it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StartupHeadRequirement {
    pub submission: usize,
    pub content_frame: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StartupOutputEvidence {
    pub required_submission: usize,
    pub presented_submissions: usize,
    pub required_content_frame: u64,
    pub presented_content_frame: u64,
    pub callbacks: usize,
    pub synchronous_modeset: bool,
}

pub(super) fn startup_output_evidence(
    native: &sophia_backend_live::LiveProductionNativeScanout,
    required_submissions: Option<&BTreeMap<sophia_engine::RenderHeadId, StartupHeadRequirement>>,
) -> Option<Vec<StartupOutputEvidence>> {
    if required_submissions.is_some_and(|required| {
        required.len() != native.heads.len()
            || native
                .heads
                .iter()
                .any(|head| !required.contains_key(&head.head))
    }) {
        return None;
    }
    Some(
        native
            .heads
            .iter()
            .map(|head| {
                let requirement =
                    startup_required_submission_for_head(required_submissions, head.head)
                        .unwrap_or(StartupHeadRequirement {
                            submission: 0,
                            content_frame: 0,
                        });
                StartupOutputEvidence {
                    required_submission: requirement.submission,
                    presented_submissions: head.presented_submissions,
                    required_content_frame: requirement.content_frame,
                    presented_content_frame: head
                        .presented_content
                        .map_or(0, |content| content.frame().raw()),
                    callbacks: head.callback_accepted,
                    synchronous_modeset: head.initial_modeset_submission.is_some(),
                }
            })
            .collect(),
    )
}

pub(super) fn startup_required_submission_for_head(
    required: Option<&BTreeMap<sophia_engine::RenderHeadId, StartupHeadRequirement>>,
    head: sophia_engine::RenderHeadId,
) -> Option<StartupHeadRequirement> {
    required
        .map(|required| required.get(&head).copied())
        .unwrap_or(Some(StartupHeadRequirement {
            submission: 0,
            content_frame: 0,
        }))
}

pub(super) fn all_startup_outputs_presented(outputs: &[StartupOutputEvidence]) -> bool {
    !outputs.is_empty()
        && outputs.iter().all(|output| {
            (output.callbacks > 0 || output.synchronous_modeset)
                && output.presented_submissions >= output.required_submission
                // Only where a submission is owed: a head the focused surface
                // does not intersect never advances its content for it, and
                // demanding newness there would wait on a blank output
                // forever.
                && (output.required_submission == 0
                    || output.presented_content_frame > output.required_content_frame)
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

/// Whether one head drove its own submit, callback, and retire lifecycle.
///
/// This asks about transport only. Pixel content is a separate question,
/// answered once for the session by [`native_session_exported_pixels`],
/// because an output holding no windows composes an all-black frame and that
/// is the correct picture for it: demanding nonzero pixels of every head
/// refuses a legitimately empty second monitor forever.
pub(super) const fn independent_native_output_presented(
    submissions: usize,
    retirements: usize,
    callbacks: usize,
    synchronous_modeset: bool,
) -> bool {
    let asynchronous_lifecycle =
        retirements > 0 && callbacks == retirements && submissions == retirements + 1;
    let synchronous_lifecycle =
        synchronous_modeset && submissions == 1 && retirements == 0 && callbacks == 0;
    asynchronous_lifecycle || synchronous_lifecycle
}

/// Whether the session put real pixels on any screen at all.
///
/// A desktop where every head exported nothing rendered nothing, however
/// healthy each head's transport looked.
pub(super) fn native_session_exported_pixels(
    head_nonzero_exports: impl IntoIterator<Item = usize>,
) -> bool {
    head_nonzero_exports
        .into_iter()
        .any(|nonzero_exports| nonzero_exports > 0)
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
