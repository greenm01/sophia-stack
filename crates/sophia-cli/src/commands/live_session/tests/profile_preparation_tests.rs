use super::*;

fn public_profile_test_config(prefix: &str) -> PersistentXtermSessionConfig {
    use std::time::{SystemTime, UNIX_EPOCH};

    let root = std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut config = PersistentXtermSessionConfig::from_args(&[
        "--wm-process=/usr/bin/true".to_owned(),
        "--wm-interface=sophia_wm_v1".to_owned(),
    ])
    .unwrap();
    config.wm_socket_path = root.with_extension("sock");
    config
}

#[test]
fn public_policy_launch_preparation_validates_fragments_and_cleans_up_before_launch() {
    use std::os::unix::fs::PermissionsExt as _;

    let config = public_profile_test_config("sophia-policy-prepare-test");
    let directory_path = config.wm_socket_path.with_extension("policy");
    let activation_key = sophia_config::DesktopProfileActivationKey::from(&config.desktop_profile);

    let prepared = PreparedPublicPolicyLaunch::new(&config).unwrap();

    assert_eq!(
        std::fs::metadata(&directory_path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    sophia_config::validate_desktop_profile_fragments(&prepared.profile_fragments, activation_key)
        .unwrap();
    assert_eq!(
        prepared.shortcut_profile_slot.participant().phase(),
        sophia_config::DesktopProfileParticipantPhase::Prepared
    );
    for authority in sophia_config::DesktopAuthority::ALL {
        assert!(prepared.profile_fragments.path(authority).is_file());
    }

    drop(prepared);
    assert!(!directory_path.exists());
}

#[test]
fn public_profile_startup_reaches_the_complete_prepare_barrier_before_launch() {
    let mut config = public_profile_test_config("sophia-profile-barrier-test");
    let key = sophia_config::DesktopProfileActivationKey::from(&config.desktop_profile);

    let prepared = LiveWmSession::prepare_public_launch(&mut config)
        .unwrap()
        .unwrap();

    assert_eq!(
        config.desktop_profile_activation.phase(),
        sophia_config::DesktopProfileActivationPhase::Prepared
    );
    assert_eq!(config.desktop_profile_activation.candidate(), Some(key));
    assert_eq!(config.desktop_profile_activation.active(), None);
    for participant in [
        prepared.policy_profile.slot.participant(),
        prepared.shell_profile.slot.participant(),
        prepared.shortcut_profile_slot.participant(),
        config.session_profile.slot().participant(),
        config.input_profile.slot().participant(),
        config.output_profile.slot().participant(),
        prepared.broker_profile.slot.participant(),
    ] {
        assert_eq!(
            participant.phase(),
            sophia_config::DesktopProfileParticipantPhase::Prepared
        );
        assert_eq!(participant.candidate(), Some(key));
        assert_eq!(participant.active(), None);
    }

    let directory_path = config.wm_socket_path.with_extension("policy");
    drop(prepared);
    assert!(!directory_path.exists());
}

#[test]
fn public_profile_activation_promotes_local_slots_and_pauses_at_policy() {
    let mut config = public_profile_test_config("sophia-profile-local-activation-test");
    let key = sophia_config::DesktopProfileActivationKey::from(&config.desktop_profile);
    let mut prepared = LiveWmSession::prepare_public_launch(&mut config)
        .unwrap()
        .unwrap();
    let model = config.desktop_profile_activation.clone();
    let mut executor = PublicProfilePreparationExecutor {
        policy: prepared.policy_profile.slot_mut(),
        shell: prepared.shell_profile.slot_mut(),
        shortcut: &mut prepared.shortcut_profile_slot,
        session: config.session_profile.slot_mut(),
        input: config.input_profile.slot_mut(),
        output: config.output_profile.slot_mut(),
        broker: prepared.broker_profile.slot_mut(),
    };

    let report = sophia_cli::desktop_profile_activation::run_desktop_profile_prepared_activation_until_policy(
        &model,
        key,
        &mut executor,
    )
    .unwrap();
    drop(executor);
    let policy_effect = report.effect.unwrap();

    assert_eq!(
        report.disposition,
        sophia_cli::desktop_profile_activation::DesktopProfileExternalActivationDisposition::AwaitingPolicy
    );
    assert_eq!(
        policy_effect.authority,
        sophia_config::DesktopAuthority::Policy
    );
    assert_eq!(
        prepared.policy_profile.slot.participant().phase(),
        sophia_config::DesktopProfileParticipantPhase::Prepared
    );
    for participant in [
        prepared.shell_profile.slot.participant(),
        prepared.shortcut_profile_slot.participant(),
        config.session_profile.slot().participant(),
        config.input_profile.slot().participant(),
        config.output_profile.slot().participant(),
        prepared.broker_profile.slot.participant(),
    ] {
        assert_eq!(
            participant.phase(),
            sophia_config::DesktopProfileParticipantPhase::Activated
        );
        assert_eq!(participant.active(), Some(key));
    }

    let rejected =
        sophia_cli::desktop_profile_activation::settle_desktop_profile_policy_activation(
            &report.model,
            policy_effect,
            false,
        )
        .unwrap();
    let mut rollback_executor = PublicProfilePreparationExecutor {
        policy: prepared.policy_profile.slot_mut(),
        shell: prepared.shell_profile.slot_mut(),
        shortcut: &mut prepared.shortcut_profile_slot,
        session: config.session_profile.slot_mut(),
        input: config.input_profile.slot_mut(),
        output: config.output_profile.slot_mut(),
        broker: prepared.broker_profile.slot_mut(),
    };
    let rolled_back = sophia_cli::desktop_profile_activation::run_desktop_profile_rollback(
        rejected.model,
        rejected.effects,
        &mut rollback_executor,
    )
    .unwrap();
    drop(rollback_executor);
    assert_eq!(
        rolled_back.phase(),
        sophia_config::DesktopProfileActivationPhase::Idle
    );
    assert_eq!(rolled_back.active(), None);
    for participant in [
        prepared.policy_profile.slot.participant(),
        prepared.shell_profile.slot.participant(),
        prepared.shortcut_profile_slot.participant(),
        config.session_profile.slot().participant(),
        config.input_profile.slot().participant(),
        config.output_profile.slot().participant(),
        prepared.broker_profile.slot.participant(),
    ] {
        assert_eq!(
            participant.phase(),
            sophia_config::DesktopProfileParticipantPhase::Idle
        );
        assert_eq!(participant.active(), None);
        assert_eq!(participant.candidate(), None);
    }
}

#[test]
fn public_profile_prepare_failure_rolls_every_owner_back_without_activation() {
    for (failure_index, authority) in sophia_config::DesktopAuthority::ALL.into_iter().enumerate() {
        let mut config =
            public_profile_test_config(&format!("sophia-profile-rollback-test-{failure_index}"));
        let key = sophia_config::DesktopProfileActivationKey::from(&config.desktop_profile);
        let mut prepared = PreparedPublicPolicyLaunch::new(&config).unwrap();
        match authority {
            sophia_config::DesktopAuthority::Policy => {
                *prepared.policy_profile.slot_mut() =
                    sophia_config::DesktopProfileCandidateSlot::new(authority);
            }
            sophia_config::DesktopAuthority::Shell => {
                *prepared.shell_profile.slot_mut() =
                    sophia_config::DesktopProfileCandidateSlot::new(authority);
            }
            sophia_config::DesktopAuthority::Shortcut => {
                prepared.shortcut_profile_slot =
                    sophia_config::DesktopProfileCandidateSlot::new(authority);
            }
            sophia_config::DesktopAuthority::Session => {
                *config.session_profile.slot_mut() =
                    sophia_config::DesktopProfileCandidateSlot::new(authority);
            }
            sophia_config::DesktopAuthority::Input => {
                *config.input_profile.slot_mut() =
                    sophia_config::DesktopProfileCandidateSlot::new(authority);
            }
            sophia_config::DesktopAuthority::Output => {
                *config.output_profile.slot_mut() =
                    sophia_config::DesktopProfileCandidateSlot::new(authority);
            }
            sophia_config::DesktopAuthority::Broker => {
                *prepared.broker_profile.slot_mut() =
                    sophia_config::DesktopProfileCandidateSlot::new(authority);
            }
        }

        let report = prepared
            .prepare_startup(
                &mut config.session_profile,
                &mut config.input_profile,
                &mut config.output_profile,
                &config.desktop_profile_activation,
                key,
            )
            .unwrap();

        assert_eq!(
            report.disposition,
            sophia_cli::desktop_profile_activation::DesktopProfileStartupPreparationDisposition::Rejected
        );
        assert_eq!(
            report.model.phase(),
            sophia_config::DesktopProfileActivationPhase::Idle
        );
        assert_eq!(report.model.active(), None);
        for participant in [
            prepared.policy_profile.slot.participant(),
            prepared.shell_profile.slot.participant(),
            prepared.shortcut_profile_slot.participant(),
            config.session_profile.slot().participant(),
            config.input_profile.slot().participant(),
            config.output_profile.slot().participant(),
            prepared.broker_profile.slot.participant(),
        ] {
            assert_eq!(
                participant.phase(),
                sophia_config::DesktopProfileParticipantPhase::Idle
            );
            assert_eq!(participant.active(), None);
            assert_eq!(participant.candidate(), None);
        }
    }
}
