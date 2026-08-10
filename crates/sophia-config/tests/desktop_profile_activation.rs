use sophia_config::{
    ConfigDigest, ConfigGeneration, DesktopAuthority, DesktopProfileActivationEffectKind,
    DesktopProfileActivationError, DesktopProfileActivationKey, DesktopProfileActivationModel,
    DesktopProfileActivationMsg, DesktopProfileActivationPhase, reduce_desktop_profile_activation,
};

fn key(generation: u64, digest: u8) -> DesktopProfileActivationKey {
    DesktopProfileActivationKey::new(
        ConfigGeneration::from_raw(generation),
        ConfigDigest::new([digest; 32]),
    )
}

fn active(generation: u64, digest: u8) -> DesktopProfileActivationModel {
    DesktopProfileActivationModel::with_active(key(generation, digest)).unwrap()
}

fn begin(
    model: &DesktopProfileActivationModel,
    candidate: DesktopProfileActivationKey,
) -> DesktopProfileActivationModel {
    reduce_desktop_profile_activation(
        model,
        DesktopProfileActivationMsg::BeginCandidate { key: candidate },
    )
    .unwrap()
    .model
}

fn prepare_all(
    model: &DesktopProfileActivationModel,
    candidate: DesktopProfileActivationKey,
) -> DesktopProfileActivationModel {
    let mut model = model.clone();
    for authority in DesktopAuthority::ALL {
        model = reduce_desktop_profile_activation(
            &model,
            DesktopProfileActivationMsg::AuthorityPrepared {
                key: candidate,
                authority,
                success: true,
            },
        )
        .unwrap()
        .model;
    }
    model
}

fn complete_rollback(
    model: &DesktopProfileActivationModel,
    candidate: DesktopProfileActivationKey,
) -> DesktopProfileActivationModel {
    let mut model = model.clone();
    for authority in DesktopAuthority::ALL {
        model = reduce_desktop_profile_activation(
            &model,
            DesktopProfileActivationMsg::RollbackCompleted {
                key: candidate,
                authority,
                success: true,
            },
        )
        .unwrap()
        .model;
    }
    model
}

#[test]
fn all_authorities_prepare_and_activate_one_shared_generation() {
    let known_good = key(1, 1);
    let candidate = key(2, 2);
    let initial = DesktopProfileActivationModel::with_active(known_good).unwrap();
    let started = reduce_desktop_profile_activation(
        &initial,
        DesktopProfileActivationMsg::BeginCandidate { key: candidate },
    )
    .unwrap();
    assert_eq!(
        started.model.phase(),
        DesktopProfileActivationPhase::Preparing
    );
    assert_eq!(started.effects.len(), DesktopAuthority::ALL.len());
    assert_eq!(
        started
            .effects
            .iter()
            .map(|effect| effect.authority)
            .collect::<Vec<_>>(),
        DesktopAuthority::ALL
    );
    assert!(started.effects.iter().all(|effect| {
        effect.kind == DesktopProfileActivationEffectKind::PrepareAuthority
            && effect.key == candidate
    }));

    let prepared = prepare_all(&started.model, candidate);
    assert_eq!(prepared.phase(), DesktopProfileActivationPhase::Prepared);
    let activating = reduce_desktop_profile_activation(
        &prepared,
        DesktopProfileActivationMsg::ActivationRequested { key: candidate },
    )
    .unwrap();
    assert_eq!(
        activating.model.phase(),
        DesktopProfileActivationPhase::Activating
    );
    assert!(activating.effects.iter().all(|effect| {
        effect.kind == DesktopProfileActivationEffectKind::ActivateAuthority
            && effect.key == candidate
    }));
    assert_eq!(
        activating
            .effects
            .iter()
            .map(|effect| effect.authority)
            .collect::<Vec<_>>(),
        DesktopAuthority::STARTUP_ACTIVATION_ORDER
    );

    let mut model = activating.model;
    for authority in DesktopAuthority::STARTUP_ACTIVATION_ORDER {
        model = reduce_desktop_profile_activation(
            &model,
            DesktopProfileActivationMsg::AuthorityActivated {
                key: candidate,
                authority,
                success: true,
            },
        )
        .unwrap()
        .model;
    }
    assert_eq!(model.phase(), DesktopProfileActivationPhase::Idle);
    assert_eq!(model.active(), Some(candidate));
    assert_eq!(model.latest_generation(), 2);
    assert_eq!(model.candidate(), None);
}

#[test]
fn prepare_rejection_rolls_every_authority_back_to_last_known_good() {
    let known_good = key(4, 4);
    let candidate = key(5, 5);
    let model = begin(
        &DesktopProfileActivationModel::with_active(known_good).unwrap(),
        candidate,
    );
    let model = reduce_desktop_profile_activation(
        &model,
        DesktopProfileActivationMsg::AuthorityPrepared {
            key: candidate,
            authority: DesktopAuthority::Policy,
            success: true,
        },
    )
    .unwrap()
    .model;
    let rejected = reduce_desktop_profile_activation(
        &model,
        DesktopProfileActivationMsg::AuthorityPrepared {
            key: candidate,
            authority: DesktopAuthority::Shell,
            success: false,
        },
    )
    .unwrap();

    assert_eq!(
        rejected.model.phase(),
        DesktopProfileActivationPhase::RollingBack
    );
    assert_eq!(rejected.model.active(), Some(known_good));
    assert_eq!(rejected.effects.len(), DesktopAuthority::ALL.len());
    assert!(rejected.effects.iter().all(|effect| {
        effect.kind == DesktopProfileActivationEffectKind::RollbackAuthority
            && effect.key == candidate
    }));

    let rolled_back = complete_rollback(&rejected.model, candidate);
    assert_eq!(rolled_back.phase(), DesktopProfileActivationPhase::Idle);
    assert_eq!(rolled_back.active(), Some(known_good));
    assert_eq!(rolled_back.latest_generation(), 5);
    assert!(rolled_back.rollback_pending().is_empty());
}

#[test]
fn rejected_generations_never_reenter_admission() {
    let candidate = key(5, 5);
    let mut model = begin(&active(4, 4), candidate);
    model = reduce_desktop_profile_activation(
        &model,
        DesktopProfileActivationMsg::AuthorityPrepared {
            key: candidate,
            authority: DesktopAuthority::Policy,
            success: false,
        },
    )
    .unwrap()
    .model;
    model = complete_rollback(&model, candidate);

    assert_eq!(model.latest_generation(), 5);
    assert_eq!(
        reduce_desktop_profile_activation(
            &model,
            DesktopProfileActivationMsg::BeginCandidate { key: candidate },
        ),
        Err(DesktopProfileActivationError::InvalidCandidateIdentity)
    );

    let next = key(6, 5);
    let next_model = begin(&model, next);
    let stale = reduce_desktop_profile_activation(
        &next_model,
        DesktopProfileActivationMsg::AuthorityPrepared {
            key: candidate,
            authority: DesktopAuthority::Policy,
            success: true,
        },
    )
    .unwrap();
    assert_eq!(stale.model, next_model);
    assert!(stale.effects.is_empty());
}

#[test]
fn partial_activation_cannot_promote_and_stale_completions_are_inert() {
    let known_good = key(8, 8);
    let candidate = key(9, 9);
    let prepared = prepare_all(&begin(&active(8, 8), candidate), candidate);
    let mut model = reduce_desktop_profile_activation(
        &prepared,
        DesktopProfileActivationMsg::ActivationRequested { key: candidate },
    )
    .unwrap()
    .model;
    model = reduce_desktop_profile_activation(
        &model,
        DesktopProfileActivationMsg::AuthorityActivated {
            key: candidate,
            authority: DesktopAuthority::Policy,
            success: true,
        },
    )
    .unwrap()
    .model;
    model = reduce_desktop_profile_activation(
        &model,
        DesktopProfileActivationMsg::AuthorityActivated {
            key: candidate,
            authority: DesktopAuthority::Shell,
            success: false,
        },
    )
    .unwrap()
    .model;

    let before_stale = model.clone();
    let stale = reduce_desktop_profile_activation(
        &model,
        DesktopProfileActivationMsg::AuthorityActivated {
            key: key(7, 7),
            authority: DesktopAuthority::Shortcut,
            success: true,
        },
    )
    .unwrap();
    assert_eq!(stale.model, before_stale);
    assert_eq!(stale.model.active(), Some(known_good));
    assert_eq!(
        stale.model.phase(),
        DesktopProfileActivationPhase::RollingBack
    );
    assert!(stale.effects.is_empty());
}

#[test]
fn out_of_order_and_failed_rollback_are_explicit_errors() {
    let candidate = key(2, 2);
    let model = begin(&active(1, 1), candidate);
    assert_eq!(
        reduce_desktop_profile_activation(
            &model,
            DesktopProfileActivationMsg::ActivationRequested { key: candidate },
        ),
        Err(DesktopProfileActivationError::ActivationBarrierIncomplete)
    );
    let rolling_back = reduce_desktop_profile_activation(
        &model,
        DesktopProfileActivationMsg::AuthorityPrepared {
            key: candidate,
            authority: DesktopAuthority::Policy,
            success: false,
        },
    )
    .unwrap()
    .model;
    assert_eq!(
        reduce_desktop_profile_activation(
            &rolling_back,
            DesktopProfileActivationMsg::RollbackCompleted {
                key: candidate,
                authority: DesktopAuthority::Policy,
                success: false,
            },
        ),
        Err(DesktopProfileActivationError::RollbackIncomplete)
    );
    assert_eq!(rolling_back.active(), Some(key(1, 1)));
}

#[test]
fn generation_exhaustion_is_terminal_before_reuse() {
    let exhausted = active(u64::MAX, 1);
    assert_eq!(ConfigGeneration::from_raw(u64::MAX).next(), None);
    assert_eq!(
        reduce_desktop_profile_activation(
            &exhausted,
            DesktopProfileActivationMsg::BeginCandidate {
                key: key(u64::MAX, 2),
            },
        ),
        Err(DesktopProfileActivationError::InvalidCandidateIdentity)
    );
}
