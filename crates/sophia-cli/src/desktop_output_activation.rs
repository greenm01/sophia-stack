use sophia_config::{ConfigDigest, ConfigGeneration};

use crate::desktop_output_topology::NativeOutputActivationPlan;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeOutputActivationKey {
    generation: ConfigGeneration,
    digest: ConfigDigest,
}

impl NativeOutputActivationKey {
    pub const fn generation(self) -> ConfigGeneration {
        self.generation
    }

    pub const fn digest(self) -> ConfigDigest {
        self.digest
    }
}

impl From<&NativeOutputActivationPlan> for NativeOutputActivationKey {
    fn from(plan: &NativeOutputActivationPlan) -> Self {
        Self {
            generation: plan.generation(),
            digest: plan.digest(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOutputActivationFailure {
    Invalidated,
    Rejected,
    WouldBlock,
    TimedOut,
    Disconnected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOutputEffectCompletion {
    Succeeded,
    Failed(NativeOutputActivationFailure),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeOutputActivationMsg {
    Begin(NativeOutputActivationPlan),
    TestCompleted {
        key: NativeOutputActivationKey,
        completion: NativeOutputEffectCompletion,
    },
    ApplyCompleted {
        key: NativeOutputActivationKey,
        completion: NativeOutputEffectCompletion,
    },
    RollbackCompleted {
        key: NativeOutputActivationKey,
        completion: NativeOutputEffectCompletion,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeOutputActivationEffect {
    Test {
        key: NativeOutputActivationKey,
        plan: NativeOutputActivationPlan,
    },
    Apply {
        key: NativeOutputActivationKey,
        plan: NativeOutputActivationPlan,
    },
    Rollback {
        key: NativeOutputActivationKey,
        plan: NativeOutputActivationPlan,
    },
}

impl NativeOutputActivationEffect {
    pub const fn key(&self) -> NativeOutputActivationKey {
        match self {
            Self::Test { key, .. } | Self::Apply { key, .. } | Self::Rollback { key, .. } => *key,
        }
    }

    pub const fn plan(&self) -> &NativeOutputActivationPlan {
        match self {
            Self::Test { plan, .. } | Self::Apply { plan, .. } | Self::Rollback { plan, .. } => {
                plan
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOutputActivationPhase {
    Idle,
    Testing,
    Applying,
    RollingBack,
    Activated,
    Rejected,
    RecoveryFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOutputRollbackSettlement {
    NotRequired,
    Succeeded,
    Failed(NativeOutputActivationFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOutputActivationSettlement {
    Activated {
        key: NativeOutputActivationKey,
    },
    Rejected {
        key: NativeOutputActivationKey,
        cause: NativeOutputActivationFailure,
        rollback: NativeOutputRollbackSettlement,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeOutputActivationModel {
    state: NativeOutputActivationState,
}

impl NativeOutputActivationModel {
    pub const fn phase(&self) -> NativeOutputActivationPhase {
        self.state.phase()
    }

    pub const fn settlement(&self) -> Option<NativeOutputActivationSettlement> {
        match self.state {
            NativeOutputActivationState::Activated { key } => {
                Some(NativeOutputActivationSettlement::Activated { key })
            }
            NativeOutputActivationState::Rejected {
                key,
                cause,
                rollback,
            } => Some(NativeOutputActivationSettlement::Rejected {
                key,
                cause,
                rollback,
            }),
            _ => None,
        }
    }

    pub const fn pending_plan(&self) -> Option<&NativeOutputActivationPlan> {
        match &self.state {
            NativeOutputActivationState::Testing { plan, .. }
            | NativeOutputActivationState::Applying { plan, .. }
            | NativeOutputActivationState::RollingBack { plan, .. } => Some(plan),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
enum NativeOutputActivationState {
    #[default]
    Idle,
    Testing {
        key: NativeOutputActivationKey,
        plan: NativeOutputActivationPlan,
    },
    Applying {
        key: NativeOutputActivationKey,
        plan: NativeOutputActivationPlan,
    },
    RollingBack {
        key: NativeOutputActivationKey,
        plan: NativeOutputActivationPlan,
        cause: NativeOutputActivationFailure,
    },
    Activated {
        key: NativeOutputActivationKey,
    },
    Rejected {
        key: NativeOutputActivationKey,
        cause: NativeOutputActivationFailure,
        rollback: NativeOutputRollbackSettlement,
    },
}

impl NativeOutputActivationState {
    const fn phase(&self) -> NativeOutputActivationPhase {
        match self {
            Self::Idle => NativeOutputActivationPhase::Idle,
            Self::Testing { .. } => NativeOutputActivationPhase::Testing,
            Self::Applying { .. } => NativeOutputActivationPhase::Applying,
            Self::RollingBack { .. } => NativeOutputActivationPhase::RollingBack,
            Self::Activated { .. } => NativeOutputActivationPhase::Activated,
            Self::Rejected {
                rollback: NativeOutputRollbackSettlement::Failed(_),
                ..
            } => NativeOutputActivationPhase::RecoveryFailed,
            Self::Rejected { .. } => NativeOutputActivationPhase::Rejected,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOutputActivationDisposition {
    Started,
    Advanced,
    Activated,
    Rejected,
    RecoveryFailed,
    IgnoredStale,
    IgnoredBusy,
    IgnoredTerminal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeOutputActivationUpdate {
    pub model: NativeOutputActivationModel,
    pub disposition: NativeOutputActivationDisposition,
    pub effect: Option<NativeOutputActivationEffect>,
}

pub fn reduce_native_output_activation(
    model: &NativeOutputActivationModel,
    message: NativeOutputActivationMsg,
) -> NativeOutputActivationUpdate {
    match message {
        NativeOutputActivationMsg::Begin(plan) => begin(model, plan),
        NativeOutputActivationMsg::TestCompleted { key, completion } => {
            complete_test(model, key, completion)
        }
        NativeOutputActivationMsg::ApplyCompleted { key, completion } => {
            complete_apply(model, key, completion)
        }
        NativeOutputActivationMsg::RollbackCompleted { key, completion } => {
            complete_rollback(model, key, completion)
        }
    }
}

fn begin(
    model: &NativeOutputActivationModel,
    plan: NativeOutputActivationPlan,
) -> NativeOutputActivationUpdate {
    if model.settlement().is_some() {
        return unchanged(model, NativeOutputActivationDisposition::IgnoredTerminal);
    }
    if model.phase() != NativeOutputActivationPhase::Idle {
        return unchanged(model, NativeOutputActivationDisposition::IgnoredBusy);
    }
    let key = NativeOutputActivationKey::from(&plan);
    NativeOutputActivationUpdate {
        model: NativeOutputActivationModel {
            state: NativeOutputActivationState::Testing {
                key,
                plan: plan.clone(),
            },
        },
        disposition: NativeOutputActivationDisposition::Started,
        effect: Some(NativeOutputActivationEffect::Test { key, plan }),
    }
}

fn complete_test(
    model: &NativeOutputActivationModel,
    key: NativeOutputActivationKey,
    completion: NativeOutputEffectCompletion,
) -> NativeOutputActivationUpdate {
    let NativeOutputActivationState::Testing {
        key: pending_key,
        plan,
    } = &model.state
    else {
        return unmatched(model);
    };
    if *pending_key != key {
        return unchanged(model, NativeOutputActivationDisposition::IgnoredStale);
    }
    match completion {
        NativeOutputEffectCompletion::Succeeded => NativeOutputActivationUpdate {
            model: NativeOutputActivationModel {
                state: NativeOutputActivationState::Applying {
                    key,
                    plan: plan.clone(),
                },
            },
            disposition: NativeOutputActivationDisposition::Advanced,
            effect: Some(NativeOutputActivationEffect::Apply {
                key,
                plan: plan.clone(),
            }),
        },
        NativeOutputEffectCompletion::Failed(cause) => rejected(
            key,
            cause,
            NativeOutputRollbackSettlement::NotRequired,
            NativeOutputActivationDisposition::Rejected,
        ),
    }
}

fn complete_apply(
    model: &NativeOutputActivationModel,
    key: NativeOutputActivationKey,
    completion: NativeOutputEffectCompletion,
) -> NativeOutputActivationUpdate {
    let NativeOutputActivationState::Applying {
        key: pending_key,
        plan,
    } = &model.state
    else {
        return unmatched(model);
    };
    if *pending_key != key {
        return unchanged(model, NativeOutputActivationDisposition::IgnoredStale);
    }
    match completion {
        NativeOutputEffectCompletion::Succeeded => NativeOutputActivationUpdate {
            model: NativeOutputActivationModel {
                state: NativeOutputActivationState::Activated { key },
            },
            disposition: NativeOutputActivationDisposition::Activated,
            effect: None,
        },
        NativeOutputEffectCompletion::Failed(cause) => NativeOutputActivationUpdate {
            model: NativeOutputActivationModel {
                state: NativeOutputActivationState::RollingBack {
                    key,
                    plan: plan.clone(),
                    cause,
                },
            },
            disposition: NativeOutputActivationDisposition::Advanced,
            effect: Some(NativeOutputActivationEffect::Rollback {
                key,
                plan: plan.clone(),
            }),
        },
    }
}

fn complete_rollback(
    model: &NativeOutputActivationModel,
    key: NativeOutputActivationKey,
    completion: NativeOutputEffectCompletion,
) -> NativeOutputActivationUpdate {
    let NativeOutputActivationState::RollingBack {
        key: pending_key,
        cause,
        ..
    } = &model.state
    else {
        return unmatched(model);
    };
    if *pending_key != key {
        return unchanged(model, NativeOutputActivationDisposition::IgnoredStale);
    }
    match completion {
        NativeOutputEffectCompletion::Succeeded => rejected(
            key,
            *cause,
            NativeOutputRollbackSettlement::Succeeded,
            NativeOutputActivationDisposition::Rejected,
        ),
        NativeOutputEffectCompletion::Failed(rollback_failure) => rejected(
            key,
            *cause,
            NativeOutputRollbackSettlement::Failed(rollback_failure),
            NativeOutputActivationDisposition::RecoveryFailed,
        ),
    }
}

fn unmatched(model: &NativeOutputActivationModel) -> NativeOutputActivationUpdate {
    let disposition = if model.settlement().is_some() {
        NativeOutputActivationDisposition::IgnoredTerminal
    } else {
        NativeOutputActivationDisposition::IgnoredStale
    };
    unchanged(model, disposition)
}

fn rejected(
    key: NativeOutputActivationKey,
    cause: NativeOutputActivationFailure,
    rollback: NativeOutputRollbackSettlement,
    disposition: NativeOutputActivationDisposition,
) -> NativeOutputActivationUpdate {
    NativeOutputActivationUpdate {
        model: NativeOutputActivationModel {
            state: NativeOutputActivationState::Rejected {
                key,
                cause,
                rollback,
            },
        },
        disposition,
        effect: None,
    }
}

fn unchanged(
    model: &NativeOutputActivationModel,
    disposition: NativeOutputActivationDisposition,
) -> NativeOutputActivationUpdate {
    NativeOutputActivationUpdate {
        model: model.clone(),
        disposition,
        effect: None,
    }
}
