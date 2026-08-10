use std::fmt;

use crate::{DesktopAuthority, DesktopProfileActivationKey};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DesktopProfileParticipantPhase {
    #[default]
    Idle,
    Prepared,
    Activated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopProfileParticipantModel {
    authority: DesktopAuthority,
    phase: DesktopProfileParticipantPhase,
    active: Option<DesktopProfileActivationKey>,
    previous_active: Option<DesktopProfileActivationKey>,
    candidate: Option<DesktopProfileActivationKey>,
    latest_candidate: Option<DesktopProfileActivationKey>,
    latest_generation: u64,
}

impl DesktopProfileParticipantModel {
    pub const fn new(authority: DesktopAuthority) -> Self {
        Self {
            authority,
            phase: DesktopProfileParticipantPhase::Idle,
            active: None,
            previous_active: None,
            candidate: None,
            latest_candidate: None,
            latest_generation: 0,
        }
    }

    pub fn with_active(
        authority: DesktopAuthority,
        active: DesktopProfileActivationKey,
    ) -> Result<Self, DesktopProfileParticipantError> {
        if active.generation().raw() == 0 {
            return Err(DesktopProfileParticipantError::InvalidCandidateIdentity);
        }
        Ok(Self {
            authority,
            active: Some(active),
            latest_candidate: Some(active),
            latest_generation: active.generation().raw(),
            ..Self::new(authority)
        })
    }

    pub const fn authority(&self) -> DesktopAuthority {
        self.authority
    }

    pub const fn phase(&self) -> DesktopProfileParticipantPhase {
        self.phase
    }

    pub const fn active(&self) -> Option<DesktopProfileActivationKey> {
        self.active
    }

    pub const fn candidate(&self) -> Option<DesktopProfileActivationKey> {
        self.candidate
    }

    pub const fn latest_generation(&self) -> u64 {
        self.latest_generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopProfileParticipantError {
    InvalidCandidateIdentity,
    Busy,
    NotPrepared,
    IdentityMismatch,
}

impl fmt::Display for DesktopProfileParticipantError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidCandidateIdentity => {
                "desktop profile participant candidate identity is invalid"
            }
            Self::Busy => "desktop profile participant already has a prepared candidate",
            Self::NotPrepared => "desktop profile participant candidate is not prepared",
            Self::IdentityMismatch => "desktop profile participant identity does not match",
        })
    }
}

impl std::error::Error for DesktopProfileParticipantError {}

pub fn prepare_desktop_profile_participant(
    model: &DesktopProfileParticipantModel,
    key: DesktopProfileActivationKey,
) -> Result<DesktopProfileParticipantModel, DesktopProfileParticipantError> {
    if model.phase == DesktopProfileParticipantPhase::Prepared && model.candidate == Some(key) {
        return Ok(model.clone());
    }
    if model.phase == DesktopProfileParticipantPhase::Prepared {
        return Err(DesktopProfileParticipantError::Busy);
    }
    if key.generation().raw() == 0
        || key.generation().raw() <= model.latest_generation
        || model
            .active
            .is_some_and(|active| key.digest() == active.digest())
    {
        return Err(DesktopProfileParticipantError::InvalidCandidateIdentity);
    }
    let mut next = model.clone();
    next.phase = DesktopProfileParticipantPhase::Prepared;
    next.previous_active = None;
    next.candidate = Some(key);
    next.latest_candidate = Some(key);
    next.latest_generation = key.generation().raw();
    Ok(next)
}

pub fn activate_desktop_profile_participant(
    model: &DesktopProfileParticipantModel,
    key: DesktopProfileActivationKey,
) -> Result<DesktopProfileParticipantModel, DesktopProfileParticipantError> {
    if model.phase == DesktopProfileParticipantPhase::Activated
        && model.active == Some(key)
        && model.candidate == Some(key)
    {
        return Ok(model.clone());
    }
    if model.phase != DesktopProfileParticipantPhase::Prepared {
        return Err(DesktopProfileParticipantError::NotPrepared);
    }
    if model.candidate != Some(key) {
        return Err(DesktopProfileParticipantError::IdentityMismatch);
    }
    let mut next = model.clone();
    next.phase = DesktopProfileParticipantPhase::Activated;
    next.previous_active = next.active;
    next.active = Some(key);
    Ok(next)
}

pub fn rollback_desktop_profile_participant(
    model: &DesktopProfileParticipantModel,
    key: DesktopProfileActivationKey,
) -> Result<DesktopProfileParticipantModel, DesktopProfileParticipantError> {
    if key.generation().raw() == 0 {
        return Err(DesktopProfileParticipantError::InvalidCandidateIdentity);
    }
    if model.phase == DesktopProfileParticipantPhase::Prepared && model.candidate == Some(key) {
        let mut next = model.clone();
        clear_candidate(&mut next);
        return Ok(next);
    }
    if model.phase == DesktopProfileParticipantPhase::Activated
        && model.active == Some(key)
        && model.candidate == Some(key)
    {
        let mut next = model.clone();
        next.active = next.previous_active;
        clear_candidate(&mut next);
        return Ok(next);
    }
    if key.generation().raw() < model.latest_generation {
        return Ok(model.clone());
    }
    if model.latest_candidate == Some(key) {
        return Ok(model.clone());
    }
    Err(DesktopProfileParticipantError::IdentityMismatch)
}

fn clear_candidate(model: &mut DesktopProfileParticipantModel) {
    model.phase = DesktopProfileParticipantPhase::Idle;
    model.previous_active = None;
    model.candidate = None;
}
