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

    // Public-policy admission activates this slot before runtime setup. A
    // reload stages a prepared candidate again; read the payload for that phase.
    pub(super) fn current(&self) -> &sophia_config::DesktopInputCandidate {
        use sophia_config::DesktopProfileParticipantPhase;
        match self.slot().participant().phase() {
            DesktopProfileParticipantPhase::Prepared => self.slot().candidate(),
            DesktopProfileParticipantPhase::Activated => self.slot().active(),
            _ => None,
        }
        .expect("runtime setup requires a prepared or activated input profile")
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
