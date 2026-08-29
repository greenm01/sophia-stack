#[derive(Clone, Debug, PartialEq)]
pub(super) struct PreparedInputProfile {
    slot: sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopInputCandidate>,
}

impl PreparedInputProfile {
    pub(super) fn new(
        candidate: sophia_config::DesktopInputCandidate,
    ) -> Result<Self, sophia_config::DesktopProfileCandidateSlotError> {
        Ok(Self {
            slot: sophia_config::DesktopProfileCandidateSlot::with_candidate(candidate)?,
        })
    }

    pub(super) fn candidate(&self) -> &sophia_config::DesktopInputCandidate {
        debug_assert_eq!(
            self.slot().participant().phase(),
            sophia_config::DesktopProfileParticipantPhase::Prepared
        );
        self.slot()
            .candidate()
            .expect("trusted startup retains its prepared input candidate")
    }

    pub(super) const fn slot(
        &self,
    ) -> &sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopInputCandidate> {
        &self.slot
    }

    pub(super) const fn slot_mut(
        &mut self,
    ) -> &mut sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopInputCandidate> {
        &mut self.slot
    }
}
