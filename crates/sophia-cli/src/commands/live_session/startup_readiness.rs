use sophia_protocol::Rect;

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

pub(super) fn synchronous_modeset_record(output: u64, submission: Option<usize>) -> Option<String> {
    submission.map(|submission| {
        format!(
            "sophia_live_native_startup_output schema=1 status=presented output={output} proof=synchronous_modeset submission={submission}"
        )
    })
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
