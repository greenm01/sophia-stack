use std::fmt;
use std::path::Path;

use crate::{
    DesktopAuthority, DesktopAuthorityCandidate, DesktopInputCandidate, DesktopOutputCandidate,
    DesktopProfileActivationKey, DesktopProfileError, DesktopProfileParticipantError,
    DesktopProfileParticipantModel, DesktopProfileParticipantPhase, DesktopSessionCandidate,
    DesktopShortcutCandidate, activate_desktop_profile_participant,
    load_desktop_authority_fragment, prepare_desktop_profile_participant,
    rollback_desktop_profile_participant,
};

pub trait DesktopProfileCandidatePayload {
    fn authority(&self) -> DesktopAuthority;
    fn activation_key(&self) -> DesktopProfileActivationKey;
    fn same_payload(&self, other: &Self) -> bool;
}

macro_rules! fixed_authority_payload {
    ($payload:ty, $authority:expr) => {
        impl DesktopProfileCandidatePayload for $payload {
            fn authority(&self) -> DesktopAuthority {
                $authority
            }

            fn activation_key(&self) -> DesktopProfileActivationKey {
                DesktopProfileActivationKey::new(self.generation, self.digest)
            }

            fn same_payload(&self, other: &Self) -> bool {
                self == other
            }
        }
    };
}

fixed_authority_payload!(DesktopShortcutCandidate, DesktopAuthority::Shortcut);
fixed_authority_payload!(DesktopSessionCandidate, DesktopAuthority::Session);
fixed_authority_payload!(DesktopInputCandidate, DesktopAuthority::Input);
fixed_authority_payload!(DesktopOutputCandidate, DesktopAuthority::Output);

impl DesktopProfileCandidatePayload for DesktopAuthorityCandidate {
    fn authority(&self) -> DesktopAuthority {
        self.authority
    }

    fn activation_key(&self) -> DesktopProfileActivationKey {
        DesktopProfileActivationKey::new(self.generation, self.digest)
    }

    fn same_payload(&self, other: &Self) -> bool {
        self.authority == other.authority
            && self.generation == other.generation
            && self.digest == other.digest
            && self.values.len() == other.values.len()
            && self
                .values
                .iter()
                .zip(&other.values)
                .all(|(left, right)| left.key == right.key && left.encoded == right.encoded)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DesktopProfileCandidateSlot<T> {
    participant: DesktopProfileParticipantModel,
    active: Option<T>,
    previous_active: Option<T>,
    candidate: Option<T>,
}

impl<T> DesktopProfileCandidateSlot<T>
where
    T: DesktopProfileCandidatePayload,
{
    pub const fn new(authority: DesktopAuthority) -> Self {
        Self {
            participant: DesktopProfileParticipantModel::new(authority),
            active: None,
            previous_active: None,
            candidate: None,
        }
    }

    pub fn with_active(payload: T) -> Result<Self, DesktopProfileCandidateSlotError> {
        let authority = payload.authority();
        let participant =
            DesktopProfileParticipantModel::with_active(authority, payload.activation_key())?;
        Ok(Self {
            participant,
            active: Some(payload),
            previous_active: None,
            candidate: None,
        })
    }

    pub fn with_candidate(payload: T) -> Result<Self, DesktopProfileCandidateSlotError>
    where
        T: Clone,
    {
        let slot = Self::new(payload.authority());
        prepare_desktop_profile_candidate_slot(&slot, payload)
    }

    pub const fn participant(&self) -> &DesktopProfileParticipantModel {
        &self.participant
    }

    pub const fn active(&self) -> Option<&T> {
        self.active.as_ref()
    }

    pub const fn candidate(&self) -> Option<&T> {
        self.candidate.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DesktopProfileCandidateSlotError {
    AuthorityMismatch,
    PayloadConflict,
    Participant(DesktopProfileParticipantError),
    Profile(DesktopProfileError),
}

impl fmt::Display for DesktopProfileCandidateSlotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthorityMismatch => {
                formatter.write_str("desktop profile payload crossed its authority boundary")
            }
            Self::PayloadConflict => formatter.write_str(
                "desktop profile payload conflicts with the candidate at the same identity",
            ),
            Self::Participant(error) => error.fmt(formatter),
            Self::Profile(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DesktopProfileCandidateSlotError {}

impl From<DesktopProfileParticipantError> for DesktopProfileCandidateSlotError {
    fn from(error: DesktopProfileParticipantError) -> Self {
        Self::Participant(error)
    }
}

impl From<DesktopProfileError> for DesktopProfileCandidateSlotError {
    fn from(error: DesktopProfileError) -> Self {
        Self::Profile(error)
    }
}

pub fn prepare_desktop_profile_candidate_slot<T>(
    slot: &DesktopProfileCandidateSlot<T>,
    payload: T,
) -> Result<DesktopProfileCandidateSlot<T>, DesktopProfileCandidateSlotError>
where
    T: Clone + DesktopProfileCandidatePayload,
{
    if payload.authority() != slot.participant.authority() {
        return Err(DesktopProfileCandidateSlotError::AuthorityMismatch);
    }
    let key = payload.activation_key();
    if slot.participant.phase() == DesktopProfileParticipantPhase::Prepared
        && slot.participant.candidate() == Some(key)
    {
        return if slot
            .candidate
            .as_ref()
            .is_some_and(|candidate| candidate.same_payload(&payload))
        {
            Ok(slot.clone())
        } else {
            Err(DesktopProfileCandidateSlotError::PayloadConflict)
        };
    }
    let participant = prepare_desktop_profile_participant(&slot.participant, key)?;
    let mut next = slot.clone();
    next.participant = participant;
    next.previous_active = None;
    next.candidate = Some(payload);
    Ok(next)
}

pub fn prepare_desktop_profile_candidate_slot_from_fragment(
    slot: &DesktopProfileCandidateSlot<DesktopAuthorityCandidate>,
    path: &Path,
    key: DesktopProfileActivationKey,
) -> Result<DesktopProfileCandidateSlot<DesktopAuthorityCandidate>, DesktopProfileCandidateSlotError>
{
    let payload = load_desktop_authority_fragment(path, slot.participant.authority(), key)?;
    prepare_desktop_profile_candidate_slot(slot, payload)
}

pub fn activate_desktop_profile_candidate_slot<T>(
    slot: &DesktopProfileCandidateSlot<T>,
    key: DesktopProfileActivationKey,
) -> Result<DesktopProfileCandidateSlot<T>, DesktopProfileCandidateSlotError>
where
    T: Clone + DesktopProfileCandidatePayload,
{
    let participant = activate_desktop_profile_participant(&slot.participant, key)?;
    if participant == slot.participant {
        return Ok(slot.clone());
    }
    let candidate = slot
        .candidate
        .as_ref()
        .filter(|payload| payload.activation_key() == key)
        .cloned()
        .ok_or(DesktopProfileCandidateSlotError::PayloadConflict)?;
    let mut next = slot.clone();
    next.participant = participant;
    next.previous_active = next.active.take();
    next.active = Some(candidate);
    Ok(next)
}

pub fn rollback_desktop_profile_candidate_slot<T>(
    slot: &DesktopProfileCandidateSlot<T>,
    key: DesktopProfileActivationKey,
) -> Result<DesktopProfileCandidateSlot<T>, DesktopProfileCandidateSlotError>
where
    T: Clone + DesktopProfileCandidatePayload,
{
    let participant = rollback_desktop_profile_participant(&slot.participant, key)?;
    if participant == slot.participant {
        return Ok(slot.clone());
    }
    let mut next = slot.clone();
    next.participant = participant;
    match slot.participant.phase() {
        DesktopProfileParticipantPhase::Prepared => {
            next.candidate = None;
            next.previous_active = None;
        }
        DesktopProfileParticipantPhase::Activated => {
            if slot.participant.active() == Some(key) {
                next.active = next.previous_active.take();
            } else {
                next.previous_active = None;
            }
            next.candidate = None;
        }
        DesktopProfileParticipantPhase::Idle => {}
    }
    Ok(next)
}
