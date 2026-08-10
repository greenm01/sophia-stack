use sophia_config::{
    ConfigDigest, ConfigGeneration, DesktopAuthority, DesktopProfileActivationKey,
    DesktopProfileParticipantError, DesktopProfileParticipantModel, DesktopProfileParticipantPhase,
    activate_desktop_profile_participant, prepare_desktop_profile_participant,
    rollback_desktop_profile_participant,
};

fn key(generation: u64, digest: u8) -> DesktopProfileActivationKey {
    DesktopProfileActivationKey::new(
        ConfigGeneration::from_raw(generation),
        ConfigDigest::new([digest; 32]),
    )
}

#[test]
fn every_authority_uses_the_same_initial_activation_and_rollback_invariants() {
    for authority in DesktopAuthority::ALL {
        let empty = DesktopProfileParticipantModel::new(authority);
        let prepared = prepare_desktop_profile_participant(&empty, key(1, 1)).unwrap();
        assert_eq!(prepared.authority(), authority);
        assert_eq!(prepared.phase(), DesktopProfileParticipantPhase::Prepared);
        assert_eq!(prepared.active(), None);

        let activated = activate_desktop_profile_participant(&prepared, key(1, 1)).unwrap();
        assert_eq!(activated.phase(), DesktopProfileParticipantPhase::Activated);
        assert_eq!(activated.active(), Some(key(1, 1)));

        let rolled_back = rollback_desktop_profile_participant(&activated, key(1, 1)).unwrap();
        assert_eq!(rolled_back.phase(), DesktopProfileParticipantPhase::Idle);
        assert_eq!(rolled_back.active(), None);
        assert_eq!(rolled_back.latest_generation(), 1);
    }
}

#[test]
fn activation_rollback_restores_the_exact_previous_identity() {
    let active =
        DesktopProfileParticipantModel::with_active(DesktopAuthority::Output, key(4, 4)).unwrap();
    let prepared = prepare_desktop_profile_participant(&active, key(5, 5)).unwrap();
    let activated = activate_desktop_profile_participant(&prepared, key(5, 5)).unwrap();
    let rolled_back = rollback_desktop_profile_participant(&activated, key(5, 5)).unwrap();

    assert_eq!(rolled_back.active(), Some(key(4, 4)));
    assert_eq!(rolled_back.candidate(), None);
    assert_eq!(rolled_back.phase(), DesktopProfileParticipantPhase::Idle);
}

#[test]
fn retries_are_idempotent_but_rejected_generations_do_not_reenter() {
    let empty = DesktopProfileParticipantModel::new(DesktopAuthority::Policy);
    let prepared = prepare_desktop_profile_participant(&empty, key(1, 1)).unwrap();
    assert_eq!(
        prepare_desktop_profile_participant(&prepared, key(1, 1)).unwrap(),
        prepared
    );
    let activated = activate_desktop_profile_participant(&prepared, key(1, 1)).unwrap();
    assert_eq!(
        activate_desktop_profile_participant(&activated, key(1, 1)).unwrap(),
        activated
    );
    let rolled_back = rollback_desktop_profile_participant(&activated, key(1, 1)).unwrap();
    assert_eq!(
        rollback_desktop_profile_participant(&rolled_back, key(1, 1)).unwrap(),
        rolled_back
    );
    assert_eq!(
        prepare_desktop_profile_participant(&rolled_back, key(1, 1)),
        Err(DesktopProfileParticipantError::InvalidCandidateIdentity)
    );
}

#[test]
fn a_new_prepare_finalizes_the_prior_activation_as_rollback_baseline() {
    let initial = DesktopProfileParticipantModel::new(DesktopAuthority::Session);
    let first = prepare_desktop_profile_participant(&initial, key(1, 1)).unwrap();
    let first = activate_desktop_profile_participant(&first, key(1, 1)).unwrap();
    let second = prepare_desktop_profile_participant(&first, key(2, 2)).unwrap();
    let rejected = rollback_desktop_profile_participant(&second, key(2, 2)).unwrap();
    assert_eq!(rejected.active(), Some(key(1, 1)));

    let second = prepare_desktop_profile_participant(&rejected, key(3, 3)).unwrap();
    let second = activate_desktop_profile_participant(&second, key(3, 3)).unwrap();
    let rolled_back = rollback_desktop_profile_participant(&second, key(3, 3)).unwrap();
    assert_eq!(rolled_back.active(), Some(key(1, 1)));
}

#[test]
fn mismatched_or_out_of_order_transitions_fail_closed() {
    let empty = DesktopProfileParticipantModel::new(DesktopAuthority::Input);
    assert_eq!(
        activate_desktop_profile_participant(&empty, key(1, 1)),
        Err(DesktopProfileParticipantError::NotPrepared)
    );
    assert_eq!(
        rollback_desktop_profile_participant(&empty, key(1, 1)),
        Err(DesktopProfileParticipantError::IdentityMismatch)
    );
    let prepared = prepare_desktop_profile_participant(&empty, key(1, 1)).unwrap();
    assert_eq!(
        prepare_desktop_profile_participant(&prepared, key(2, 2)),
        Err(DesktopProfileParticipantError::Busy)
    );
    assert_eq!(
        activate_desktop_profile_participant(&prepared, key(2, 2)),
        Err(DesktopProfileParticipantError::IdentityMismatch)
    );
    assert_eq!(
        rollback_desktop_profile_participant(&prepared, key(2, 2)),
        Err(DesktopProfileParticipantError::IdentityMismatch)
    );
    assert_eq!(
        rollback_desktop_profile_participant(&prepared, key(1, 2)),
        Err(DesktopProfileParticipantError::IdentityMismatch)
    );
    assert_eq!(
        rollback_desktop_profile_participant(&prepared, key(0, 0)),
        Err(DesktopProfileParticipantError::InvalidCandidateIdentity)
    );
}

#[test]
fn only_strictly_older_rollback_completions_are_inert() {
    let empty = DesktopProfileParticipantModel::new(DesktopAuthority::Shortcut);
    let prepared = prepare_desktop_profile_participant(&empty, key(2, 2)).unwrap();

    assert_eq!(
        rollback_desktop_profile_participant(&prepared, key(1, 1)).unwrap(),
        prepared
    );
    assert_eq!(
        rollback_desktop_profile_participant(&prepared, key(2, 3)),
        Err(DesktopProfileParticipantError::IdentityMismatch)
    );
}

#[test]
fn participant_generation_exhaustion_cannot_wrap() {
    let exhausted =
        DesktopProfileParticipantModel::with_active(DesktopAuthority::Broker, key(u64::MAX, 1))
            .unwrap();
    assert_eq!(ConfigGeneration::from_raw(u64::MAX).next(), None);
    assert_eq!(
        prepare_desktop_profile_participant(&exhausted, key(u64::MAX, 2)),
        Err(DesktopProfileParticipantError::InvalidCandidateIdentity)
    );
}
