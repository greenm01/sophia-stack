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

/// Performs the typed effects the activation reducer emits.
///
/// The reducer stays pure and the executor owns every side effect, so a caller
/// can drive the same phase machine against real KMS or against a deterministic
/// stand-in. An implementation must never interpret the plan: it either carries
/// out the requested phase or reports why it could not.
pub trait NativeOutputActivationEffectExecutor {
    fn test(
        &mut self,
        key: NativeOutputActivationKey,
        plan: &NativeOutputActivationPlan,
    ) -> NativeOutputEffectCompletion;

    fn apply(
        &mut self,
        key: NativeOutputActivationKey,
        plan: &NativeOutputActivationPlan,
    ) -> NativeOutputEffectCompletion;

    fn rollback(
        &mut self,
        key: NativeOutputActivationKey,
        plan: &NativeOutputActivationPlan,
    ) -> NativeOutputEffectCompletion;
}

pub fn execute_native_output_activation_effect<E>(
    executor: &mut E,
    effect: &NativeOutputActivationEffect,
) -> NativeOutputActivationMsg
where
    E: NativeOutputActivationEffectExecutor,
{
    match effect {
        NativeOutputActivationEffect::Test { key, plan } => {
            NativeOutputActivationMsg::TestCompleted {
                key: *key,
                completion: executor.test(*key, plan),
            }
        }
        NativeOutputActivationEffect::Apply { key, plan } => {
            NativeOutputActivationMsg::ApplyCompleted {
                key: *key,
                completion: executor.apply(*key, plan),
            }
        }
        NativeOutputActivationEffect::Rollback { key, plan } => {
            NativeOutputActivationMsg::RollbackCompleted {
                key: *key,
                completion: executor.rollback(*key, plan),
            }
        }
    }
}

/// The reducer emits at most one test, one apply, and one rollback per
/// candidate, so a correct drive never exceeds three effects. Exceeding it means
/// the phase machine cycled, which fails closed rather than spinning.
const NATIVE_OUTPUT_ACTIVATION_EFFECT_BUDGET: usize = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeOutputActivationDriveError {
    /// `Begin` did not start a candidate, so the model was busy or terminal.
    NotStarted(NativeOutputActivationDisposition),
    /// A completion did not match the pending phase. The reducer reports this as
    /// a stale or unmatched disposition, which a single-threaded drive cannot
    /// legitimately produce.
    UnexpectedDisposition(NativeOutputActivationDisposition),
    /// The drive ran out of effects without reaching a settlement.
    DidNotSettle,
    /// The phase machine emitted more effects than one candidate can require.
    EffectBudgetExhausted,
}

impl std::fmt::Display for NativeOutputActivationDriveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotStarted(disposition) => write!(
                formatter,
                "native output activation did not start: {disposition:?}"
            ),
            Self::UnexpectedDisposition(disposition) => write!(
                formatter,
                "native output activation reported an unexpected disposition: {disposition:?}"
            ),
            Self::DidNotSettle => {
                formatter.write_str("native output activation ended without a settlement")
            }
            Self::EffectBudgetExhausted => {
                formatter.write_str("native output activation exceeded its effect budget")
            }
        }
    }
}

impl std::error::Error for NativeOutputActivationDriveError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeOutputActivationReport {
    pub model: NativeOutputActivationModel,
    pub settlement: NativeOutputActivationSettlement,
}

/// Drives one candidate from `Begin` to a terminal settlement.
///
/// Every effect is executed in order and its completion fed straight back, so
/// the reducer observes exactly the sequence it asked for. A rejected candidate
/// is an ordinary settlement, not an error; the error cases are reserved for a
/// phase machine that did not behave.
pub fn run_native_output_activation<E>(
    plan: NativeOutputActivationPlan,
    executor: &mut E,
) -> Result<NativeOutputActivationReport, NativeOutputActivationDriveError>
where
    E: NativeOutputActivationEffectExecutor,
{
    let mut update = reduce_native_output_activation(
        &NativeOutputActivationModel::default(),
        NativeOutputActivationMsg::Begin(plan),
    );
    if update.disposition != NativeOutputActivationDisposition::Started {
        return Err(NativeOutputActivationDriveError::NotStarted(
            update.disposition,
        ));
    }

    for _ in 0..NATIVE_OUTPUT_ACTIVATION_EFFECT_BUDGET {
        let Some(effect) = update.effect.take() else {
            break;
        };
        let message = execute_native_output_activation_effect(executor, &effect);
        update = reduce_native_output_activation(&update.model, message);
        match update.disposition {
            NativeOutputActivationDisposition::IgnoredStale
            | NativeOutputActivationDisposition::IgnoredBusy
            | NativeOutputActivationDisposition::IgnoredTerminal => {
                return Err(NativeOutputActivationDriveError::UnexpectedDisposition(
                    update.disposition,
                ));
            }
            _ => {}
        }
        if let Some(settlement) = update.model.settlement() {
            return Ok(NativeOutputActivationReport {
                model: update.model,
                settlement,
            });
        }
    }

    if update.effect.is_some() {
        return Err(NativeOutputActivationDriveError::EffectBudgetExhausted);
    }
    Err(NativeOutputActivationDriveError::DidNotSettle)
}

/// The executor used until a multi-connector atomic request builder exists.
///
/// It declines the test phase, which the reducer treats as a rejection needing
/// no rollback, so startup drives the real phase machine while performing no KMS
/// mutation at all. Apply and rollback are unreachable behind a declined test;
/// they fail closed rather than panic so a future caller cannot turn a wiring
/// mistake into an abort.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UnavailableNativeOutputExecutor;

impl NativeOutputActivationEffectExecutor for UnavailableNativeOutputExecutor {
    fn test(
        &mut self,
        _key: NativeOutputActivationKey,
        _plan: &NativeOutputActivationPlan,
    ) -> NativeOutputEffectCompletion {
        NativeOutputEffectCompletion::Failed(NativeOutputActivationFailure::WouldBlock)
    }

    fn apply(
        &mut self,
        _key: NativeOutputActivationKey,
        _plan: &NativeOutputActivationPlan,
    ) -> NativeOutputEffectCompletion {
        NativeOutputEffectCompletion::Failed(NativeOutputActivationFailure::WouldBlock)
    }

    fn rollback(
        &mut self,
        _key: NativeOutputActivationKey,
        _plan: &NativeOutputActivationPlan,
    ) -> NativeOutputEffectCompletion {
        NativeOutputEffectCompletion::Failed(NativeOutputActivationFailure::WouldBlock)
    }
}
