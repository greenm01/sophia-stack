use sophia_cli::desktop_profile_activation::{
    DesktopProfileAuthorityEffectExecutor, DesktopProfileExternalActivationDisposition,
    DesktopProfileStartupActivationDisposition, DesktopProfileStartupActivationErrorKind,
    DesktopProfileStartupPreparationDisposition, execute_desktop_profile_activation_effect,
    run_desktop_profile_prepared_activation, run_desktop_profile_prepared_activation_until_policy,
    run_desktop_profile_rollback, run_desktop_profile_startup_activation,
    run_desktop_profile_startup_preparation, settle_desktop_profile_policy_activation,
};
use sophia_config::{
    ConfigDigest, ConfigGeneration, DesktopAuthority, DesktopAuthorityCandidate,
    DesktopProfileActivationEffect, DesktopProfileActivationEffectKind,
    DesktopProfileActivationError, DesktopProfileActivationKey, DesktopProfileActivationModel,
    DesktopProfileActivationMsg, DesktopProfileActivationPhase, DesktopProfileCandidatePayload,
    DesktopProfileCandidateSlot, DesktopProfileFragments, DesktopProfileParticipantPhase,
    activate_desktop_profile_candidate_slot, prepare_desktop_profile_candidate_slot,
    prepare_desktop_profile_candidate_slot_from_fragment, reduce_desktop_profile_activation,
    rollback_desktop_profile_candidate_slot,
};

#[derive(Default)]
struct FakeExecutor {
    calls: Vec<(
        DesktopProfileActivationEffectKind,
        DesktopAuthority,
        DesktopProfileActivationKey,
    )>,
    prepare_failure: Option<DesktopAuthority>,
    activate_failure: Option<DesktopAuthority>,
    rollback_failure: Option<DesktopAuthority>,
}

impl DesktopProfileAuthorityEffectExecutor for FakeExecutor {
    fn prepare_authority(
        &mut self,
        authority: DesktopAuthority,
        key: DesktopProfileActivationKey,
    ) -> bool {
        self.calls.push((
            DesktopProfileActivationEffectKind::PrepareAuthority,
            authority,
            key,
        ));
        self.prepare_failure != Some(authority)
    }

    fn activate_authority(
        &mut self,
        authority: DesktopAuthority,
        key: DesktopProfileActivationKey,
    ) -> bool {
        self.calls.push((
            DesktopProfileActivationEffectKind::ActivateAuthority,
            authority,
            key,
        ));
        self.activate_failure != Some(authority)
    }

    fn rollback_authority(
        &mut self,
        authority: DesktopAuthority,
        key: DesktopProfileActivationKey,
    ) -> bool {
        self.calls.push((
            DesktopProfileActivationEffectKind::RollbackAuthority,
            authority,
            key,
        ));
        self.rollback_failure != Some(authority)
    }
}

struct CandidateSlotExecutor {
    slots: Vec<DesktopProfileCandidateSlot<DesktopAuthorityCandidate>>,
    fragments: Option<DesktopProfileFragments>,
    prepare_failure: Option<DesktopAuthority>,
    activate_failure: Option<DesktopAuthority>,
    rollback_failure: Option<DesktopAuthority>,
}

impl CandidateSlotExecutor {
    fn with_active(active: DesktopProfileActivationKey) -> Self {
        Self {
            slots: DesktopAuthority::ALL
                .into_iter()
                .map(|authority| {
                    DesktopProfileCandidateSlot::with_active(payload(authority, active)).unwrap()
                })
                .collect(),
            fragments: None,
            prepare_failure: None,
            activate_failure: None,
            rollback_failure: None,
        }
    }

    fn with_staged_profile(
        active: DesktopProfileActivationKey,
        fragments: DesktopProfileFragments,
    ) -> Self {
        Self {
            fragments: Some(fragments),
            ..Self::with_active(active)
        }
    }

    fn slot(
        &self,
        authority: DesktopAuthority,
    ) -> &DesktopProfileCandidateSlot<DesktopAuthorityCandidate> {
        &self.slots[self.index(authority)]
    }

    fn index(&self, authority: DesktopAuthority) -> usize {
        self.slots
            .iter()
            .position(|slot| slot.participant().authority() == authority)
            .expect("all desktop authorities have one candidate slot")
    }
}

impl DesktopProfileAuthorityEffectExecutor for CandidateSlotExecutor {
    fn prepare_authority(
        &mut self,
        authority: DesktopAuthority,
        key: DesktopProfileActivationKey,
    ) -> bool {
        if self.prepare_failure == Some(authority) {
            return false;
        }
        let index = self.index(authority);
        let prepared = match self.fragments.as_ref() {
            Some(fragments) => prepare_desktop_profile_candidate_slot_from_fragment(
                &self.slots[index],
                fragments.path(authority),
                key,
            ),
            None => {
                prepare_desktop_profile_candidate_slot(&self.slots[index], payload(authority, key))
            }
        };
        let Ok(slot) = prepared else {
            return false;
        };
        self.slots[index] = slot;
        true
    }

    fn activate_authority(
        &mut self,
        authority: DesktopAuthority,
        key: DesktopProfileActivationKey,
    ) -> bool {
        if self.activate_failure == Some(authority) {
            return false;
        }
        let index = self.index(authority);
        let Ok(slot) = activate_desktop_profile_candidate_slot(&self.slots[index], key) else {
            return false;
        };
        self.slots[index] = slot;
        true
    }

    fn rollback_authority(
        &mut self,
        authority: DesktopAuthority,
        key: DesktopProfileActivationKey,
    ) -> bool {
        if self.rollback_failure == Some(authority) {
            return false;
        }
        let index = self.index(authority);
        let Ok(slot) = rollback_desktop_profile_candidate_slot(&self.slots[index], key) else {
            return false;
        };
        self.slots[index] = slot;
        true
    }
}

fn payload(
    authority: DesktopAuthority,
    key: DesktopProfileActivationKey,
) -> DesktopAuthorityCandidate {
    DesktopAuthorityCandidate {
        authority,
        generation: key.generation(),
        digest: key.digest(),
        values: Vec::new(),
    }
}

fn key() -> DesktopProfileActivationKey {
    DesktopProfileActivationKey::new(ConfigGeneration::from_raw(7), ConfigDigest::new([7; 32]))
}

fn effect(
    kind: DesktopProfileActivationEffectKind,
    authority: DesktopAuthority,
) -> DesktopProfileActivationEffect {
    DesktopProfileActivationEffect {
        kind,
        authority,
        key: key(),
    }
}

#[test]
fn executor_dispatches_each_effect_to_its_authority_handler() {
    let mut executor = FakeExecutor::default();

    let prepared = execute_desktop_profile_activation_effect(
        &mut executor,
        effect(
            DesktopProfileActivationEffectKind::PrepareAuthority,
            DesktopAuthority::Policy,
        ),
    );
    let activated = execute_desktop_profile_activation_effect(
        &mut executor,
        effect(
            DesktopProfileActivationEffectKind::ActivateAuthority,
            DesktopAuthority::Output,
        ),
    );
    let rolled_back = execute_desktop_profile_activation_effect(
        &mut executor,
        effect(
            DesktopProfileActivationEffectKind::RollbackAuthority,
            DesktopAuthority::Broker,
        ),
    );

    assert_eq!(
        executor.calls,
        vec![
            (
                DesktopProfileActivationEffectKind::PrepareAuthority,
                DesktopAuthority::Policy,
                key(),
            ),
            (
                DesktopProfileActivationEffectKind::ActivateAuthority,
                DesktopAuthority::Output,
                key(),
            ),
            (
                DesktopProfileActivationEffectKind::RollbackAuthority,
                DesktopAuthority::Broker,
                key(),
            ),
        ]
    );
    assert_eq!(
        prepared,
        DesktopProfileActivationMsg::AuthorityPrepared {
            key: key(),
            authority: DesktopAuthority::Policy,
            success: true,
        }
    );
    assert_eq!(
        activated,
        DesktopProfileActivationMsg::AuthorityActivated {
            key: key(),
            authority: DesktopAuthority::Output,
            success: true,
        }
    );
    assert_eq!(
        rolled_back,
        DesktopProfileActivationMsg::RollbackCompleted {
            key: key(),
            authority: DesktopAuthority::Broker,
            success: true,
        }
    );
}

#[test]
fn executor_preserves_failed_completion_identity() {
    let mut executor = FakeExecutor {
        prepare_failure: Some(DesktopAuthority::Shell),
        ..FakeExecutor::default()
    };
    let message = execute_desktop_profile_activation_effect(
        &mut executor,
        effect(
            DesktopProfileActivationEffectKind::PrepareAuthority,
            DesktopAuthority::Shell,
        ),
    );

    assert_eq!(
        message,
        DesktopProfileActivationMsg::AuthorityPrepared {
            key: key(),
            authority: DesktopAuthority::Shell,
            success: false,
        }
    );
}

fn active_model() -> DesktopProfileActivationModel {
    DesktopProfileActivationModel::with_active(DesktopProfileActivationKey::new(
        ConfigGeneration::from_raw(6),
        ConfigDigest::new([6; 32]),
    ))
    .unwrap()
}

#[test]
fn startup_driver_prepares_then_activates_every_authority() {
    let mut executor = FakeExecutor::default();
    let report =
        run_desktop_profile_startup_activation(&active_model(), key(), &mut executor).unwrap();

    assert_eq!(
        report.disposition,
        DesktopProfileStartupActivationDisposition::Activated
    );
    assert_eq!(report.model.active(), Some(key()));
    assert_eq!(report.model.phase(), DesktopProfileActivationPhase::Idle);
    assert_eq!(executor.calls.len(), DesktopAuthority::ALL.len() * 2);
    for (index, authority) in DesktopAuthority::ALL.into_iter().enumerate() {
        assert_eq!(
            executor.calls[index],
            (
                DesktopProfileActivationEffectKind::PrepareAuthority,
                authority,
                key(),
            )
        );
    }
    for (index, authority) in DesktopAuthority::STARTUP_ACTIVATION_ORDER
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            executor.calls[index + DesktopAuthority::ALL.len()],
            (
                DesktopProfileActivationEffectKind::ActivateAuthority,
                authority,
                key(),
            )
        );
    }
}

#[test]
fn preparation_driver_stops_at_the_complete_prepare_barrier() {
    let mut executor = FakeExecutor::default();
    let report =
        run_desktop_profile_startup_preparation(&active_model(), key(), &mut executor).unwrap();

    assert_eq!(
        report.disposition,
        DesktopProfileStartupPreparationDisposition::Prepared
    );
    assert_eq!(
        report.model.phase(),
        DesktopProfileActivationPhase::Prepared
    );
    assert_eq!(report.model.active(), active_model().active());
    assert_eq!(report.model.candidate(), Some(key()));
    assert_eq!(executor.calls.len(), DesktopAuthority::ALL.len());
    assert!(executor.calls.iter().all(|(kind, _, effect_key)| {
        *kind == DesktopProfileActivationEffectKind::PrepareAuthority && *effect_key == key()
    }));
}

#[test]
fn preparation_driver_rejection_rolls_back_without_activation() {
    for failure in DesktopAuthority::ALL {
        let mut executor = FakeExecutor {
            prepare_failure: Some(failure),
            ..FakeExecutor::default()
        };
        let report =
            run_desktop_profile_startup_preparation(&active_model(), key(), &mut executor).unwrap();

        assert_eq!(
            report.disposition,
            DesktopProfileStartupPreparationDisposition::Rejected
        );
        assert_eq!(report.model.phase(), DesktopProfileActivationPhase::Idle);
        assert_eq!(report.model.active(), active_model().active());
        assert!(executor.calls.iter().all(|(kind, _, _)| {
            *kind != DesktopProfileActivationEffectKind::ActivateAuthority
        }));
        assert_eq!(
            executor
                .calls
                .iter()
                .filter(|(kind, _, _)| {
                    *kind == DesktopProfileActivationEffectKind::RollbackAuthority
                })
                .count(),
            DesktopAuthority::ALL.len()
        );
    }
}

#[test]
fn prepared_activation_driver_does_not_repeat_preparation() {
    let mut preparation_executor = FakeExecutor::default();
    let prepared =
        run_desktop_profile_startup_preparation(&active_model(), key(), &mut preparation_executor)
            .unwrap();
    let mut activation_executor = FakeExecutor::default();
    let activated =
        run_desktop_profile_prepared_activation(&prepared.model, key(), &mut activation_executor)
            .unwrap();

    assert_eq!(
        activated.disposition,
        DesktopProfileStartupActivationDisposition::Activated
    );
    assert_eq!(activated.model.active(), Some(key()));
    assert_eq!(
        activation_executor
            .calls
            .iter()
            .map(|(kind, authority, _)| (*kind, *authority))
            .collect::<Vec<_>>(),
        DesktopAuthority::STARTUP_ACTIVATION_ORDER
            .into_iter()
            .map(|authority| {
                (
                    DesktopProfileActivationEffectKind::ActivateAuthority,
                    authority,
                )
            })
            .collect::<Vec<_>>()
    );
}

#[test]
fn prepared_activation_pauses_after_local_authorities_and_settles_policy_last() {
    let mut preparation_executor = FakeExecutor::default();
    let prepared =
        run_desktop_profile_startup_preparation(&active_model(), key(), &mut preparation_executor)
            .unwrap();
    let mut activation_executor = FakeExecutor::default();
    let awaiting = run_desktop_profile_prepared_activation_until_policy(
        &prepared.model,
        key(),
        &mut activation_executor,
    )
    .unwrap();

    assert_eq!(
        awaiting.disposition,
        DesktopProfileExternalActivationDisposition::AwaitingPolicy
    );
    assert_eq!(
        activation_executor
            .calls
            .iter()
            .map(|(_, authority, _)| *authority)
            .collect::<Vec<_>>(),
        DesktopAuthority::STARTUP_ACTIVATION_ORDER[..6]
    );
    assert_eq!(
        awaiting.effect,
        Some(DesktopProfileActivationEffect {
            kind: DesktopProfileActivationEffectKind::ActivateAuthority,
            authority: DesktopAuthority::Policy,
            key: key(),
        })
    );
    let settled =
        settle_desktop_profile_policy_activation(&awaiting.model, awaiting.effect.unwrap(), true)
            .unwrap();
    assert_eq!(settled.model.phase(), DesktopProfileActivationPhase::Idle);
    assert_eq!(settled.model.active(), Some(key()));
    assert!(settled.effects.is_empty());
}

#[test]
fn local_activation_rejection_rolls_back_before_policy_is_contacted() {
    for failure in DesktopAuthority::STARTUP_ACTIVATION_ORDER[..6]
        .iter()
        .copied()
    {
        let mut preparation_executor = FakeExecutor::default();
        let prepared = run_desktop_profile_startup_preparation(
            &active_model(),
            key(),
            &mut preparation_executor,
        )
        .unwrap();
        let mut activation_executor = FakeExecutor {
            activate_failure: Some(failure),
            ..FakeExecutor::default()
        };
        let report = run_desktop_profile_prepared_activation_until_policy(
            &prepared.model,
            key(),
            &mut activation_executor,
        )
        .unwrap();

        assert_eq!(
            report.disposition,
            DesktopProfileExternalActivationDisposition::Rejected
        );
        assert_eq!(report.model.phase(), DesktopProfileActivationPhase::Idle);
        assert_eq!(report.model.active(), active_model().active());
        assert_eq!(report.effect, None);
        assert!(
            !activation_executor
                .calls
                .iter()
                .any(|(kind, authority, _)| {
                    *kind == DesktopProfileActivationEffectKind::ActivateAuthority
                        && *authority == DesktopAuthority::Policy
                })
        );
    }
}

#[test]
fn policy_rejection_emits_complete_typed_rollback_batch() {
    let mut preparation_executor = FakeExecutor::default();
    let prepared =
        run_desktop_profile_startup_preparation(&active_model(), key(), &mut preparation_executor)
            .unwrap();
    let mut activation_executor = FakeExecutor::default();
    let awaiting = run_desktop_profile_prepared_activation_until_policy(
        &prepared.model,
        key(),
        &mut activation_executor,
    )
    .unwrap();
    let rejected =
        settle_desktop_profile_policy_activation(&awaiting.model, awaiting.effect.unwrap(), false)
            .unwrap();

    assert_eq!(
        rejected.model.phase(),
        DesktopProfileActivationPhase::RollingBack
    );
    assert_eq!(rejected.model.active(), active_model().active());
    assert_eq!(rejected.effects.len(), DesktopAuthority::ALL.len());
    assert!(
        rejected
            .effects
            .iter()
            .zip(DesktopAuthority::ALL)
            .all(|(effect, authority)| {
                effect.kind == DesktopProfileActivationEffectKind::RollbackAuthority
                    && effect.authority == authority
                    && effect.key == key()
            })
    );

    let rolled_back =
        run_desktop_profile_rollback(rejected.model, rejected.effects, &mut activation_executor)
            .unwrap();
    assert_eq!(rolled_back.phase(), DesktopProfileActivationPhase::Idle);
    assert_eq!(rolled_back.active(), active_model().active());
    assert_eq!(rolled_back.candidate(), None);
}

#[test]
fn startup_driver_activates_the_initial_profile_from_empty_state() {
    let initial =
        DesktopProfileActivationKey::new(ConfigGeneration::INITIAL, ConfigDigest::new([1; 32]));
    let mut executor = FakeExecutor::default();
    let report = run_desktop_profile_startup_activation(
        &DesktopProfileActivationModel::default(),
        initial,
        &mut executor,
    )
    .unwrap();

    assert_eq!(
        report.disposition,
        DesktopProfileStartupActivationDisposition::Activated
    );
    assert_eq!(report.model.active(), Some(initial));
}

#[test]
fn prepare_failure_cancels_batch_and_rolls_back_all_authorities() {
    for (failure_index, failure) in DesktopAuthority::ALL.into_iter().enumerate() {
        let mut executor = FakeExecutor {
            prepare_failure: Some(failure),
            ..FakeExecutor::default()
        };
        let report =
            run_desktop_profile_startup_activation(&active_model(), key(), &mut executor).unwrap();

        assert_eq!(
            report.disposition,
            DesktopProfileStartupActivationDisposition::Rejected
        );
        assert_eq!(report.model.active(), active_model().active());
        assert_eq!(report.model.latest_generation(), 7);
        assert_eq!(report.model.phase(), DesktopProfileActivationPhase::Idle);
        let prepare_count = failure_index + 1;
        assert_eq!(
            executor.calls[..prepare_count]
                .iter()
                .map(|(_, authority, _)| *authority)
                .collect::<Vec<_>>(),
            DesktopAuthority::ALL[..prepare_count]
        );
        assert!(
            executor.calls[..prepare_count]
                .iter()
                .all(|(kind, _, _)| *kind == DesktopProfileActivationEffectKind::PrepareAuthority)
        );
        assert!(
            executor.calls[prepare_count..]
                .iter()
                .all(|(kind, _, _)| *kind == DesktopProfileActivationEffectKind::RollbackAuthority)
        );
        assert_eq!(
            executor.calls.len(),
            prepare_count + DesktopAuthority::ALL.len()
        );

        let mut retry_executor = FakeExecutor::default();
        let retry =
            run_desktop_profile_startup_activation(&report.model, key(), &mut retry_executor)
                .unwrap_err();
        assert_eq!(
            retry.kind,
            DesktopProfileStartupActivationErrorKind::Reducer(
                DesktopProfileActivationError::InvalidCandidateIdentity
            )
        );
        assert!(retry_executor.calls.is_empty());
    }
}

#[test]
fn activation_failure_cancels_batch_and_restores_last_known_good() {
    for (failure_index, failure) in DesktopAuthority::STARTUP_ACTIVATION_ORDER
        .into_iter()
        .enumerate()
    {
        let mut executor = FakeExecutor {
            activate_failure: Some(failure),
            ..FakeExecutor::default()
        };
        let report =
            run_desktop_profile_startup_activation(&active_model(), key(), &mut executor).unwrap();

        assert_eq!(
            report.disposition,
            DesktopProfileStartupActivationDisposition::Rejected
        );
        assert_eq!(report.model.active(), active_model().active());
        assert_eq!(report.model.phase(), DesktopProfileActivationPhase::Idle);
        assert_eq!(
            executor
                .calls
                .iter()
                .filter(|(kind, _, _)| {
                    *kind == DesktopProfileActivationEffectKind::PrepareAuthority
                })
                .count(),
            DesktopAuthority::ALL.len()
        );
        assert_eq!(
            executor
                .calls
                .iter()
                .filter(|(kind, _, _)| {
                    *kind == DesktopProfileActivationEffectKind::ActivateAuthority
                })
                .count(),
            failure_index + 1
        );
        assert_eq!(
            executor
                .calls
                .iter()
                .filter(|(kind, _, _)| {
                    *kind == DesktopProfileActivationEffectKind::RollbackAuthority
                })
                .count(),
            DesktopAuthority::ALL.len()
        );
    }
}

#[test]
fn rollback_failure_returns_pending_model_and_exact_failed_effect() {
    for failure in DesktopAuthority::ALL {
        let mut executor = FakeExecutor {
            prepare_failure: Some(DesktopAuthority::Policy),
            rollback_failure: Some(failure),
            ..FakeExecutor::default()
        };
        let error = run_desktop_profile_startup_activation(&active_model(), key(), &mut executor)
            .unwrap_err();

        assert_eq!(
            error.kind,
            DesktopProfileStartupActivationErrorKind::Reducer(
                DesktopProfileActivationError::RollbackIncomplete
            )
        );
        assert_eq!(error.model.active(), active_model().active());
        assert_eq!(
            error.model.phase(),
            DesktopProfileActivationPhase::RollingBack
        );
        assert_eq!(
            error.effect,
            Some(effect(
                DesktopProfileActivationEffectKind::RollbackAuthority,
                failure,
            ))
        );
        assert!(error.model.rollback_pending().contains(&failure));
        assert_eq!(error.model.rollback_pending().len(), 1);
        assert_eq!(
            executor
                .calls
                .iter()
                .filter(|(kind, _, _)| {
                    *kind == DesktopProfileActivationEffectKind::RollbackAuthority
                })
                .count(),
            DesktopAuthority::ALL.len()
        );
    }
}

#[test]
fn stale_generation_is_rejected_before_any_authority_call() {
    let mut executor = FakeExecutor::default();
    let error = run_desktop_profile_startup_activation(
        &active_model(),
        DesktopProfileActivationKey::new(ConfigGeneration::from_raw(6), ConfigDigest::new([9; 32])),
        &mut executor,
    )
    .unwrap_err();

    assert_eq!(
        error.kind,
        DesktopProfileStartupActivationErrorKind::Reducer(
            DesktopProfileActivationError::InvalidCandidateIdentity
        )
    );
    assert!(error.effect.is_none());
    assert!(executor.calls.is_empty());
}

fn assert_slot_active(
    slot: &DesktopProfileCandidateSlot<DesktopAuthorityCandidate>,
    key: DesktopProfileActivationKey,
) {
    assert_eq!(
        slot.active().map(|payload| payload.activation_key()),
        Some(key)
    );
}

#[test]
fn coordinator_success_aligns_every_independent_candidate_slot() {
    let known_good = active_model().active().unwrap();
    let mut executor = CandidateSlotExecutor::with_active(known_good);
    let report =
        run_desktop_profile_startup_activation(&active_model(), key(), &mut executor).unwrap();

    assert_eq!(report.model.active(), Some(key()));
    for authority in DesktopAuthority::ALL {
        let slot = executor.slot(authority);
        let participant = slot.participant();
        assert_eq!(participant.active(), Some(key()));
        assert_eq!(participant.candidate(), Some(key()));
        assert_eq!(participant.latest_generation(), key().generation().raw());
        assert_eq!(
            participant.phase(),
            DesktopProfileParticipantPhase::Activated
        );
        assert_slot_active(slot, key());
        assert_eq!(
            slot.candidate().map(|payload| payload.authority),
            Some(authority)
        );
    }
}

#[test]
fn coordinator_promotes_every_owner_safe_staged_fragment_payload() {
    use std::os::unix::fs::PermissionsExt as _;

    let directory = std::env::temp_dir().join(format!(
        "sophia-profile-refinement-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let prepared =
        sophia_config::load_prepared_desktop_profile(None, ConfigGeneration::from_raw(7)).unwrap();
    let activation_key = prepared.activation_key();
    let fragments = sophia_config::stage_desktop_profile(&prepared.profile, &directory).unwrap();
    let known_good = active_model().active().unwrap();
    let mut executor = CandidateSlotExecutor::with_staged_profile(known_good, fragments);

    let report =
        run_desktop_profile_startup_activation(&active_model(), activation_key, &mut executor)
            .unwrap();

    assert_eq!(report.model.active(), Some(activation_key));
    for authority in DesktopAuthority::ALL {
        let active = executor
            .slot(authority)
            .active()
            .expect("every staged authority payload becomes active");
        let expected = prepared
            .profile
            .candidates
            .get(&authority)
            .expect("the prepared profile contains every authority");
        assert!(active.same_payload(expected));
    }
    drop(executor);
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn coordinator_prepare_failure_tombstones_every_candidate_slot() {
    let known_good = active_model().active().unwrap();
    for failure in DesktopAuthority::ALL {
        let mut executor = CandidateSlotExecutor::with_active(known_good);
        executor.prepare_failure = Some(failure);
        let report =
            run_desktop_profile_startup_activation(&active_model(), key(), &mut executor).unwrap();

        assert_eq!(
            report.disposition,
            DesktopProfileStartupActivationDisposition::Rejected
        );
        for authority in DesktopAuthority::ALL {
            let slot = executor.slot(authority);
            let participant = slot.participant();
            assert_eq!(participant.active(), Some(known_good));
            assert_eq!(participant.candidate(), None);
            assert_eq!(participant.latest_generation(), key().generation().raw());
            assert_eq!(participant.phase(), DesktopProfileParticipantPhase::Idle);
            assert_slot_active(slot, known_good);
            assert_eq!(slot.candidate(), None);
        }
    }
}

#[test]
fn coordinator_activation_failure_restores_every_candidate_slot() {
    let known_good = active_model().active().unwrap();
    for failure in DesktopAuthority::ALL {
        let mut executor = CandidateSlotExecutor::with_active(known_good);
        executor.activate_failure = Some(failure);
        let report =
            run_desktop_profile_startup_activation(&active_model(), key(), &mut executor).unwrap();

        assert_eq!(
            report.disposition,
            DesktopProfileStartupActivationDisposition::Rejected
        );
        for authority in DesktopAuthority::ALL {
            let slot = executor.slot(authority);
            let participant = slot.participant();
            assert_eq!(participant.active(), Some(known_good));
            assert_eq!(participant.candidate(), None);
            assert_eq!(participant.latest_generation(), key().generation().raw());
            assert_eq!(participant.phase(), DesktopProfileParticipantPhase::Idle);
            assert_slot_active(slot, known_good);
            assert_eq!(slot.candidate(), None);
        }
    }
}

#[test]
fn coordinator_rollback_failure_preserves_and_recovers_exact_divergence() {
    let known_good = active_model().active().unwrap();
    for failure in DesktopAuthority::ALL {
        let mut executor = CandidateSlotExecutor::with_active(known_good);
        executor.prepare_failure = Some(DesktopAuthority::Policy);
        executor.rollback_failure = Some(failure);
        let error = run_desktop_profile_startup_activation(&active_model(), key(), &mut executor)
            .unwrap_err();

        assert_eq!(
            error
                .model
                .rollback_pending()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![failure]
        );
        for authority in DesktopAuthority::ALL {
            let slot = executor.slot(authority);
            let participant = slot.participant();
            let expected_generation = if authority == failure { 6 } else { 7 };
            assert_eq!(participant.active(), Some(known_good));
            assert_eq!(participant.latest_generation(), expected_generation);
            assert_slot_active(slot, known_good);
        }

        executor.rollback_failure = None;
        assert!(executor.rollback_authority(failure, key()));
        let recovered = reduce_desktop_profile_activation(
            &error.model,
            DesktopProfileActivationMsg::RollbackCompleted {
                key: key(),
                authority: failure,
                success: true,
            },
        )
        .unwrap()
        .model;
        assert_eq!(recovered.phase(), DesktopProfileActivationPhase::Idle);
        assert_eq!(recovered.active(), Some(known_good));
        for authority in DesktopAuthority::ALL {
            let slot = executor.slot(authority);
            let participant = slot.participant();
            assert_eq!(participant.phase(), DesktopProfileParticipantPhase::Idle);
            assert_eq!(participant.active(), Some(known_good));
            assert_eq!(participant.latest_generation(), key().generation().raw());
            assert_slot_active(slot, known_good);
            assert_eq!(slot.candidate(), None);
        }
    }
}
