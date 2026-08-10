#[derive(Clone, Debug, PartialEq)]
pub(super) struct PreparedSessionProfile {
    slot: sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopSessionCandidate>,
}

impl PreparedSessionProfile {
    pub(super) fn new(
        candidate: sophia_config::DesktopSessionCandidate,
    ) -> Result<Self, sophia_config::DesktopProfileCandidateSlotError> {
        Ok(Self {
            slot: sophia_config::DesktopProfileCandidateSlot::with_candidate(candidate)?,
        })
    }

    pub(super) fn candidate(&self) -> &sophia_config::DesktopSessionCandidate {
        debug_assert_eq!(
            self.slot().participant().phase(),
            sophia_config::DesktopProfileParticipantPhase::Prepared
        );
        self.slot()
            .candidate()
            .expect("trusted startup retains its prepared session candidate")
    }

    pub(super) const fn slot(
        &self,
    ) -> &sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopSessionCandidate> {
        &self.slot
    }

    pub(super) const fn slot_mut(
        &mut self,
    ) -> &mut sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopSessionCandidate>
    {
        &mut self.slot
    }
}
