use std::collections::BTreeSet;
use std::fmt;

use crate::{ConfigDigest, ConfigGeneration, DesktopAuthority, DesktopProfileGeneration};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopProfileActivationKey {
    generation: ConfigGeneration,
    digest: ConfigDigest,
}

impl DesktopProfileActivationKey {
    pub const fn new(generation: ConfigGeneration, digest: ConfigDigest) -> Self {
        Self { generation, digest }
    }

    pub const fn generation(self) -> ConfigGeneration {
        self.generation
    }

    pub const fn digest(self) -> ConfigDigest {
        self.digest
    }
}

impl From<&DesktopProfileGeneration> for DesktopProfileActivationKey {
    fn from(profile: &DesktopProfileGeneration) -> Self {
        Self::new(profile.generation, profile.digest)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DesktopProfileActivationPhase {
    #[default]
    Idle,
    Preparing,
    Prepared,
    Activating,
    RollingBack,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DesktopProfileActivationModel {
    phase: DesktopProfileActivationPhase,
    active: Option<DesktopProfileActivationKey>,
    latest_generation: u64,
    candidate: Option<DesktopProfileActivationKey>,
    prepared: BTreeSet<DesktopAuthority>,
    activated: BTreeSet<DesktopAuthority>,
    rollback_pending: BTreeSet<DesktopAuthority>,
}

impl DesktopProfileActivationModel {
    pub fn with_active(
        active: DesktopProfileActivationKey,
    ) -> Result<Self, DesktopProfileActivationError> {
        if active.generation().raw() == 0 {
            return Err(DesktopProfileActivationError::InvalidCandidateIdentity);
        }
        Ok(Self {
            active: Some(active),
            latest_generation: active.generation().raw(),
            ..Self::default()
        })
    }

    pub const fn phase(&self) -> DesktopProfileActivationPhase {
        self.phase
    }

    pub const fn active(&self) -> Option<DesktopProfileActivationKey> {
        self.active
    }

    pub const fn latest_generation(&self) -> u64 {
        self.latest_generation
    }

    pub const fn candidate(&self) -> Option<DesktopProfileActivationKey> {
        self.candidate
    }

    pub fn prepared_authorities(&self) -> &BTreeSet<DesktopAuthority> {
        &self.prepared
    }

    pub fn activated_authorities(&self) -> &BTreeSet<DesktopAuthority> {
        &self.activated
    }

    pub fn rollback_pending(&self) -> &BTreeSet<DesktopAuthority> {
        &self.rollback_pending
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopProfileActivationMsg {
    BeginCandidate {
        key: DesktopProfileActivationKey,
    },
    AuthorityPrepared {
        key: DesktopProfileActivationKey,
        authority: DesktopAuthority,
        success: bool,
    },
    ActivationRequested {
        key: DesktopProfileActivationKey,
    },
    AuthorityActivated {
        key: DesktopProfileActivationKey,
        authority: DesktopAuthority,
        success: bool,
    },
    RollbackCompleted {
        key: DesktopProfileActivationKey,
        authority: DesktopAuthority,
        success: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopProfileActivationEffectKind {
    PrepareAuthority,
    ActivateAuthority,
    RollbackAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopProfileActivationEffect {
    pub kind: DesktopProfileActivationEffectKind,
    pub authority: DesktopAuthority,
    pub key: DesktopProfileActivationKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopProfileActivationUpdate {
    pub model: DesktopProfileActivationModel,
    pub effects: Vec<DesktopProfileActivationEffect>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopProfileActivationError {
    InvalidCandidateIdentity,
    PreparationOutOfOrder,
    ActivationBarrierIncomplete,
    ActivationOutOfOrder,
    RollbackIncomplete,
}

impl fmt::Display for DesktopProfileActivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidCandidateIdentity => "desktop profile candidate identity is invalid",
            Self::PreparationOutOfOrder => "desktop authority preparation is out of order",
            Self::ActivationBarrierIncomplete => "desktop profile activation barrier is incomplete",
            Self::ActivationOutOfOrder => "desktop authority activation is out of order",
            Self::RollbackIncomplete => "desktop authority rollback is incomplete",
        })
    }
}

impl std::error::Error for DesktopProfileActivationError {}

pub fn reduce_desktop_profile_activation(
    model: &DesktopProfileActivationModel,
    message: DesktopProfileActivationMsg,
) -> Result<DesktopProfileActivationUpdate, DesktopProfileActivationError> {
    match message {
        DesktopProfileActivationMsg::BeginCandidate { key } => begin_candidate(model, key),
        DesktopProfileActivationMsg::AuthorityPrepared {
            key,
            authority,
            success,
        } => authority_prepared(model, key, authority, success),
        DesktopProfileActivationMsg::ActivationRequested { key } => {
            activation_requested(model, key)
        }
        DesktopProfileActivationMsg::AuthorityActivated {
            key,
            authority,
            success,
        } => authority_activated(model, key, authority, success),
        DesktopProfileActivationMsg::RollbackCompleted {
            key,
            authority,
            success,
        } => rollback_completed(model, key, authority, success),
    }
}

fn begin_candidate(
    model: &DesktopProfileActivationModel,
    key: DesktopProfileActivationKey,
) -> Result<DesktopProfileActivationUpdate, DesktopProfileActivationError> {
    if model.phase != DesktopProfileActivationPhase::Idle
        || key.generation().raw() == 0
        || key.generation().raw() <= model.latest_generation
        || model.active.is_some_and(|active| {
            key.generation().raw() <= active.generation().raw() || key.digest() == active.digest()
        })
    {
        return Err(DesktopProfileActivationError::InvalidCandidateIdentity);
    }
    let mut next = model.clone();
    next.phase = DesktopProfileActivationPhase::Preparing;
    next.latest_generation = key.generation().raw();
    next.candidate = Some(key);
    Ok(DesktopProfileActivationUpdate {
        model: next,
        effects: effects(DesktopProfileActivationEffectKind::PrepareAuthority, key),
    })
}

fn authority_prepared(
    model: &DesktopProfileActivationModel,
    key: DesktopProfileActivationKey,
    authority: DesktopAuthority,
    success: bool,
) -> Result<DesktopProfileActivationUpdate, DesktopProfileActivationError> {
    if model.candidate != Some(key) {
        return Ok(unchanged(model));
    }
    if model.phase != DesktopProfileActivationPhase::Preparing
        || model.prepared.contains(&authority)
    {
        return Err(DesktopProfileActivationError::PreparationOutOfOrder);
    }
    if !success {
        return Ok(begin_rollback(model, key));
    }
    let mut next = model.clone();
    next.prepared.insert(authority);
    if all_authorities_settled(&next.prepared) {
        next.phase = DesktopProfileActivationPhase::Prepared;
    }
    Ok(DesktopProfileActivationUpdate {
        model: next,
        effects: Vec::new(),
    })
}

fn activation_requested(
    model: &DesktopProfileActivationModel,
    key: DesktopProfileActivationKey,
) -> Result<DesktopProfileActivationUpdate, DesktopProfileActivationError> {
    if model.candidate != Some(key)
        || model.phase != DesktopProfileActivationPhase::Prepared
        || !all_authorities_settled(&model.prepared)
    {
        return Err(DesktopProfileActivationError::ActivationBarrierIncomplete);
    }
    let mut next = model.clone();
    next.phase = DesktopProfileActivationPhase::Activating;
    Ok(DesktopProfileActivationUpdate {
        model: next,
        effects: effects(DesktopProfileActivationEffectKind::ActivateAuthority, key),
    })
}

fn authority_activated(
    model: &DesktopProfileActivationModel,
    key: DesktopProfileActivationKey,
    authority: DesktopAuthority,
    success: bool,
) -> Result<DesktopProfileActivationUpdate, DesktopProfileActivationError> {
    if model.candidate != Some(key) || model.phase == DesktopProfileActivationPhase::RollingBack {
        return Ok(unchanged(model));
    }
    if model.phase != DesktopProfileActivationPhase::Activating
        || model.activated.contains(&authority)
    {
        return Err(DesktopProfileActivationError::ActivationOutOfOrder);
    }
    if !success {
        return Ok(begin_rollback(model, key));
    }
    let mut next = model.clone();
    next.activated.insert(authority);
    if all_authorities_settled(&next.activated) {
        next.active = next.candidate;
        clear_candidate(&mut next);
    }
    Ok(DesktopProfileActivationUpdate {
        model: next,
        effects: Vec::new(),
    })
}

fn rollback_completed(
    model: &DesktopProfileActivationModel,
    key: DesktopProfileActivationKey,
    authority: DesktopAuthority,
    success: bool,
) -> Result<DesktopProfileActivationUpdate, DesktopProfileActivationError> {
    if model.candidate != Some(key) {
        return Ok(unchanged(model));
    }
    if model.phase != DesktopProfileActivationPhase::RollingBack
        || !model.rollback_pending.contains(&authority)
        || !success
    {
        return Err(DesktopProfileActivationError::RollbackIncomplete);
    }
    let mut next = model.clone();
    next.rollback_pending.remove(&authority);
    if next.rollback_pending.is_empty() {
        clear_candidate(&mut next);
    }
    Ok(DesktopProfileActivationUpdate {
        model: next,
        effects: Vec::new(),
    })
}

fn begin_rollback(
    model: &DesktopProfileActivationModel,
    key: DesktopProfileActivationKey,
) -> DesktopProfileActivationUpdate {
    let mut next = model.clone();
    next.phase = DesktopProfileActivationPhase::RollingBack;
    next.rollback_pending = DesktopAuthority::ALL.into_iter().collect();
    DesktopProfileActivationUpdate {
        model: next,
        effects: effects(DesktopProfileActivationEffectKind::RollbackAuthority, key),
    }
}

fn clear_candidate(model: &mut DesktopProfileActivationModel) {
    model.phase = DesktopProfileActivationPhase::Idle;
    model.candidate = None;
    model.prepared.clear();
    model.activated.clear();
    model.rollback_pending.clear();
}

fn effects(
    kind: DesktopProfileActivationEffectKind,
    key: DesktopProfileActivationKey,
) -> Vec<DesktopProfileActivationEffect> {
    DesktopAuthority::ALL
        .into_iter()
        .map(|authority| DesktopProfileActivationEffect {
            kind,
            authority,
            key,
        })
        .collect()
}

fn all_authorities_settled(authorities: &BTreeSet<DesktopAuthority>) -> bool {
    authorities.len() == DesktopAuthority::ALL.len()
        && DesktopAuthority::ALL
            .into_iter()
            .all(|authority| authorities.contains(&authority))
}

fn unchanged(model: &DesktopProfileActivationModel) -> DesktopProfileActivationUpdate {
    DesktopProfileActivationUpdate {
        model: model.clone(),
        effects: Vec::new(),
    }
}
