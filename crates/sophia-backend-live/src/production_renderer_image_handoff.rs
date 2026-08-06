use sophia_renderer_live::LiveRendererImageId;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererImageHandoffAdmission {
    Ready,
    Missing,
    InvalidIdentity,
    DuplicateIdentity,
    CoverageMismatch,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LiveRendererImageResumePhase {
    #[default]
    AwaitingOutputOwner,
    AwaitingImageRestore,
    Ready,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererImageResumeObservation {
    OutputOwnerInitialized,
    ImagesRestored,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererImageResumeTransition {
    Advanced(LiveRendererImageResumePhase),
    Rejected,
}

pub const fn reduce_live_renderer_image_resume_observation(
    phase: LiveRendererImageResumePhase,
    observation: LiveRendererImageResumeObservation,
) -> LiveRendererImageResumeTransition {
    use LiveRendererImageResumeObservation::{ImagesRestored, OutputOwnerInitialized};
    use LiveRendererImageResumePhase::{AwaitingImageRestore, AwaitingOutputOwner, Ready};

    match (phase, observation) {
        (AwaitingOutputOwner, OutputOwnerInitialized) => {
            LiveRendererImageResumeTransition::Advanced(AwaitingImageRestore)
        }
        (AwaitingImageRestore, ImagesRestored) => {
            LiveRendererImageResumeTransition::Advanced(Ready)
        }
        _ => LiveRendererImageResumeTransition::Rejected,
    }
}

pub fn reduce_live_renderer_image_handoff_admission(
    retained: &[LiveRendererImageId],
    handoff: Option<&[LiveRendererImageId]>,
) -> LiveRendererImageHandoffAdmission {
    if retained.iter().any(|image| !image.is_valid())
        || handoff.into_iter().flatten().any(|image| !image.is_valid())
    {
        return LiveRendererImageHandoffAdmission::InvalidIdentity;
    }

    let retained_set = retained.iter().copied().collect::<BTreeSet<_>>();
    if retained_set.len() != retained.len() {
        return LiveRendererImageHandoffAdmission::DuplicateIdentity;
    }

    let Some(handoff) = handoff else {
        return if retained.is_empty() {
            LiveRendererImageHandoffAdmission::Ready
        } else {
            LiveRendererImageHandoffAdmission::Missing
        };
    };
    let handoff_set = handoff.iter().copied().collect::<BTreeSet<_>>();
    if handoff_set.len() != handoff.len() {
        return LiveRendererImageHandoffAdmission::DuplicateIdentity;
    }
    if retained_set == handoff_set {
        LiveRendererImageHandoffAdmission::Ready
    } else {
        LiveRendererImageHandoffAdmission::CoverageMismatch
    }
}
