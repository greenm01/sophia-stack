use std::fmt;

use sophia_config::{
    DesktopAuthority, DesktopProfileActivationEffect, DesktopProfileActivationEffectKind,
    DesktopProfileActivationError, DesktopProfileActivationKey, DesktopProfileActivationModel,
    DesktopProfileActivationMsg, DesktopProfileActivationPhase, reduce_desktop_profile_activation,
};

pub trait DesktopProfileAuthorityEffectExecutor {
    fn prepare_authority(
        &mut self,
        authority: DesktopAuthority,
        key: DesktopProfileActivationKey,
    ) -> bool;

    fn activate_authority(
        &mut self,
        authority: DesktopAuthority,
        key: DesktopProfileActivationKey,
    ) -> bool;

    fn rollback_authority(
        &mut self,
        authority: DesktopAuthority,
        key: DesktopProfileActivationKey,
    ) -> bool;
}

pub fn execute_desktop_profile_activation_effect<E>(
    executor: &mut E,
    effect: DesktopProfileActivationEffect,
) -> DesktopProfileActivationMsg
where
    E: DesktopProfileAuthorityEffectExecutor,
{
    let success = match effect.kind {
        DesktopProfileActivationEffectKind::PrepareAuthority => {
            executor.prepare_authority(effect.authority, effect.key)
        }
        DesktopProfileActivationEffectKind::ActivateAuthority => {
            executor.activate_authority(effect.authority, effect.key)
        }
        DesktopProfileActivationEffectKind::RollbackAuthority => {
            executor.rollback_authority(effect.authority, effect.key)
        }
    };
    match effect.kind {
        DesktopProfileActivationEffectKind::PrepareAuthority => {
            DesktopProfileActivationMsg::AuthorityPrepared {
                key: effect.key,
                authority: effect.authority,
                success,
            }
        }
        DesktopProfileActivationEffectKind::ActivateAuthority => {
            DesktopProfileActivationMsg::AuthorityActivated {
                key: effect.key,
                authority: effect.authority,
                success,
            }
        }
        DesktopProfileActivationEffectKind::RollbackAuthority => {
            DesktopProfileActivationMsg::RollbackCompleted {
                key: effect.key,
                authority: effect.authority,
                success,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopProfileStartupActivationDisposition {
    Activated,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopProfileStartupActivationReport {
    pub model: DesktopProfileActivationModel,
    pub disposition: DesktopProfileStartupActivationDisposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopProfileStartupActivationErrorKind {
    Reducer(DesktopProfileActivationError),
    UnexpectedEffect,
    IncompleteBarrier,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopProfileStartupActivationError {
    pub kind: DesktopProfileStartupActivationErrorKind,
    pub model: Box<DesktopProfileActivationModel>,
    pub effect: Option<DesktopProfileActivationEffect>,
}

impl fmt::Display for DesktopProfileStartupActivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            DesktopProfileStartupActivationErrorKind::Reducer(error) => error.fmt(formatter),
            DesktopProfileStartupActivationErrorKind::UnexpectedEffect => {
                formatter.write_str("desktop profile startup emitted an unexpected effect")
            }
            DesktopProfileStartupActivationErrorKind::IncompleteBarrier => {
                formatter.write_str("desktop profile startup barrier did not settle")
            }
        }
    }
}

impl std::error::Error for DesktopProfileStartupActivationError {}

pub fn run_desktop_profile_startup_activation<E>(
    model: &DesktopProfileActivationModel,
    key: DesktopProfileActivationKey,
    executor: &mut E,
) -> Result<DesktopProfileStartupActivationReport, DesktopProfileStartupActivationError>
where
    E: DesktopProfileAuthorityEffectExecutor,
{
    let started = reduce_desktop_profile_activation(
        model,
        DesktopProfileActivationMsg::BeginCandidate { key },
    )
    .map_err(|error| startup_error(error, model, None))?;
    let prepared = execute_candidate_batch(
        started.model,
        started.effects,
        DesktopProfileActivationPhase::Preparing,
        DesktopProfileActivationEffectKind::PrepareAuthority,
        executor,
    )?;
    let CandidateBatchResult::Completed(prepared) = prepared else {
        return Ok(rejected_report(prepared.model()));
    };
    if prepared.phase() != DesktopProfileActivationPhase::Prepared {
        return Err(incomplete_error(prepared));
    }

    let activation = reduce_desktop_profile_activation(
        &prepared,
        DesktopProfileActivationMsg::ActivationRequested { key },
    )
    .map_err(|error| startup_error(error, &prepared, None))?;
    let activated = execute_candidate_batch(
        activation.model,
        activation.effects,
        DesktopProfileActivationPhase::Activating,
        DesktopProfileActivationEffectKind::ActivateAuthority,
        executor,
    )?;
    let CandidateBatchResult::Completed(activated) = activated else {
        return Ok(rejected_report(activated.model()));
    };
    if activated.phase() != DesktopProfileActivationPhase::Idle || activated.active() != Some(key) {
        return Err(incomplete_error(activated));
    }
    Ok(DesktopProfileStartupActivationReport {
        model: activated,
        disposition: DesktopProfileStartupActivationDisposition::Activated,
    })
}

enum CandidateBatchResult {
    Completed(DesktopProfileActivationModel),
    Rejected(DesktopProfileActivationModel),
}

impl CandidateBatchResult {
    fn model(self) -> DesktopProfileActivationModel {
        match self {
            Self::Completed(model) | Self::Rejected(model) => model,
        }
    }
}

fn execute_candidate_batch<E>(
    mut model: DesktopProfileActivationModel,
    effects: Vec<DesktopProfileActivationEffect>,
    expected_phase: DesktopProfileActivationPhase,
    expected_effect: DesktopProfileActivationEffectKind,
    executor: &mut E,
) -> Result<CandidateBatchResult, DesktopProfileStartupActivationError>
where
    E: DesktopProfileAuthorityEffectExecutor,
{
    for effect in effects {
        if model.phase() != expected_phase || effect.kind != expected_effect {
            return Err(unexpected_effect(model, effect));
        }
        let message = execute_desktop_profile_activation_effect(executor, effect);
        let update = reduce_desktop_profile_activation(&model, message)
            .map_err(|error| startup_error(error, &model, Some(effect)))?;
        model = update.model;
        if !update.effects.is_empty() {
            return execute_rollback(model, update.effects, executor)
                .map(CandidateBatchResult::Rejected);
        }
    }
    Ok(CandidateBatchResult::Completed(model))
}

fn execute_rollback<E>(
    mut model: DesktopProfileActivationModel,
    effects: Vec<DesktopProfileActivationEffect>,
    executor: &mut E,
) -> Result<DesktopProfileActivationModel, DesktopProfileStartupActivationError>
where
    E: DesktopProfileAuthorityEffectExecutor,
{
    let mut first_failure = None;
    for effect in effects {
        if model.phase() != DesktopProfileActivationPhase::RollingBack
            || effect.kind != DesktopProfileActivationEffectKind::RollbackAuthority
        {
            return Err(unexpected_effect(model, effect));
        }
        let message = execute_desktop_profile_activation_effect(executor, effect);
        let update = match reduce_desktop_profile_activation(&model, message) {
            Ok(update) => update,
            Err(error) => {
                if first_failure.is_none() {
                    first_failure = Some((error, effect));
                }
                continue;
            }
        };
        if !update.effects.is_empty() {
            return Err(unexpected_effect(update.model, effect));
        }
        model = update.model;
    }
    if let Some((error, effect)) = first_failure {
        return Err(startup_error(error, &model, Some(effect)));
    }
    if model.phase() != DesktopProfileActivationPhase::Idle {
        return Err(incomplete_error(model));
    }
    Ok(model)
}

fn rejected_report(model: DesktopProfileActivationModel) -> DesktopProfileStartupActivationReport {
    DesktopProfileStartupActivationReport {
        model,
        disposition: DesktopProfileStartupActivationDisposition::Rejected,
    }
}

fn startup_error(
    error: DesktopProfileActivationError,
    model: &DesktopProfileActivationModel,
    effect: Option<DesktopProfileActivationEffect>,
) -> DesktopProfileStartupActivationError {
    DesktopProfileStartupActivationError {
        kind: DesktopProfileStartupActivationErrorKind::Reducer(error),
        model: Box::new(model.clone()),
        effect,
    }
}

fn unexpected_effect(
    model: DesktopProfileActivationModel,
    effect: DesktopProfileActivationEffect,
) -> DesktopProfileStartupActivationError {
    DesktopProfileStartupActivationError {
        kind: DesktopProfileStartupActivationErrorKind::UnexpectedEffect,
        model: Box::new(model),
        effect: Some(effect),
    }
}

fn incomplete_error(model: DesktopProfileActivationModel) -> DesktopProfileStartupActivationError {
    DesktopProfileStartupActivationError {
        kind: DesktopProfileStartupActivationErrorKind::IncompleteBarrier,
        model: Box::new(model),
        effect: None,
    }
}
