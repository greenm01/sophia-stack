#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StartupOutputEvidence {
    pub required_submission: usize,
    pub presented_submissions: usize,
    pub callbacks: usize,
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
            })
            .collect(),
    )
}

pub(super) fn all_startup_outputs_presented(outputs: &[StartupOutputEvidence]) -> bool {
    !outputs.is_empty()
        && outputs.iter().all(|output| {
            output.callbacks > 0 && output.presented_submissions >= output.required_submission
        })
}
