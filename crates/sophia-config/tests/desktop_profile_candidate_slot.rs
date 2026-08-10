use sophia_config::{
    ConfigDigest, ConfigGeneration, DesktopAuthority, DesktopAuthorityCandidate,
    DesktopProfileActivationKey, DesktopProfileCandidatePayload, DesktopProfileCandidateSlot,
    DesktopProfileCandidateSlotError, DesktopProfileParticipantError,
    DesktopProfileParticipantPhase, activate_desktop_profile_candidate_slot,
    load_prepared_desktop_profile, prepare_desktop_profile_candidate_slot,
    prepare_desktop_profile_candidate_slot_from_fragment, rollback_desktop_profile_candidate_slot,
    stage_desktop_profile,
};

fn key(generation: u64, digest: u8) -> DesktopProfileActivationKey {
    DesktopProfileActivationKey::new(
        ConfigGeneration::from_raw(generation),
        ConfigDigest::new([digest; 32]),
    )
}

fn payload(authority: DesktopAuthority, generation: u64, digest: u8) -> DesktopAuthorityCandidate {
    DesktopAuthorityCandidate {
        authority,
        generation: ConfigGeneration::from_raw(generation),
        digest: ConfigDigest::new([digest; 32]),
        values: Vec::new(),
    }
}

#[test]
fn prepared_payload_types_report_one_exact_authority_and_key() {
    let prepared = load_prepared_desktop_profile(None, ConfigGeneration::INITIAL).unwrap();
    let key = prepared.activation_key();

    assert_eq!(
        prepared.candidates.shortcut.authority(),
        DesktopAuthority::Shortcut
    );
    assert_eq!(prepared.candidates.shortcut.activation_key(), key);
    assert_eq!(
        prepared.candidates.session.authority(),
        DesktopAuthority::Session
    );
    assert_eq!(prepared.candidates.session.activation_key(), key);
    assert_eq!(
        prepared.candidates.input.authority(),
        DesktopAuthority::Input
    );
    assert_eq!(prepared.candidates.input.activation_key(), key);
    assert_eq!(
        prepared.candidates.output.authority(),
        DesktopAuthority::Output
    );
    assert_eq!(prepared.candidates.output.activation_key(), key);
}

#[test]
fn slot_prepares_activates_and_restores_the_previous_payload() {
    let initial = payload(DesktopAuthority::Policy, 1, 1);
    let candidate = payload(DesktopAuthority::Policy, 2, 2);
    let slot = DesktopProfileCandidateSlot::with_active(initial.clone()).unwrap();
    let prepared = prepare_desktop_profile_candidate_slot(&slot, candidate.clone()).unwrap();
    assert_eq!(prepared.active(), Some(&initial));
    assert_eq!(prepared.candidate(), Some(&candidate));
    assert_eq!(
        prepared.participant().phase(),
        DesktopProfileParticipantPhase::Prepared
    );

    let activated = activate_desktop_profile_candidate_slot(&prepared, key(2, 2)).unwrap();
    assert_eq!(activated.active(), Some(&candidate));
    assert_eq!(
        activated.participant().phase(),
        DesktopProfileParticipantPhase::Activated
    );

    let rolled_back = rollback_desktop_profile_candidate_slot(&activated, key(2, 2)).unwrap();
    assert_eq!(rolled_back.active(), Some(&initial));
    assert_eq!(rolled_back.candidate(), None);
    assert_eq!(
        rolled_back.participant().phase(),
        DesktopProfileParticipantPhase::Idle
    );
}

#[test]
fn exact_retries_are_idempotent_and_same_key_payload_conflicts_fail_closed() {
    let slot = DesktopProfileCandidateSlot::new(DesktopAuthority::Policy);
    let mut candidate = payload(DesktopAuthority::Policy, 1, 1);
    candidate.values.push(sophia_config::DesktopProfileValue {
        key: "policy.layout".to_owned(),
        encoded: "layout \"scroller\"".to_owned(),
        provenance: sophia_config::DesktopValueProvenance {
            path: "/source".into(),
            ordinal: 1,
        },
    });
    let prepared = prepare_desktop_profile_candidate_slot(&slot, candidate.clone()).unwrap();
    let mut reconstructed = candidate.clone();
    reconstructed.values[0].provenance.path = "/staged/policy.profile.kdl".into();
    assert_eq!(
        prepare_desktop_profile_candidate_slot(&prepared, reconstructed).unwrap(),
        prepared
    );

    let mut conflicting = payload(DesktopAuthority::Policy, 1, 1);
    conflicting.values.push(sophia_config::DesktopProfileValue {
        key: "policy.layout".to_owned(),
        encoded: "layout \"grid\"".to_owned(),
        provenance: sophia_config::DesktopValueProvenance {
            path: "/different".into(),
            ordinal: 1,
        },
    });
    assert_eq!(
        prepare_desktop_profile_candidate_slot(&prepared, conflicting),
        Err(DesktopProfileCandidateSlotError::PayloadConflict)
    );

    let activated = activate_desktop_profile_candidate_slot(&prepared, key(1, 1)).unwrap();
    assert_eq!(
        activate_desktop_profile_candidate_slot(&activated, key(1, 1)).unwrap(),
        activated
    );
}

#[test]
fn authority_mismatch_and_invalid_identity_leave_the_slot_unchanged() {
    let slot = DesktopProfileCandidateSlot::new(DesktopAuthority::Session);
    assert_eq!(
        prepare_desktop_profile_candidate_slot(&slot, payload(DesktopAuthority::Policy, 1, 1),),
        Err(DesktopProfileCandidateSlotError::AuthorityMismatch)
    );
    assert_eq!(slot.active(), None);
    assert_eq!(slot.candidate(), None);

    assert_eq!(
        prepare_desktop_profile_candidate_slot(&slot, payload(DesktopAuthority::Session, 0, 1),),
        Err(DesktopProfileCandidateSlotError::Participant(
            DesktopProfileParticipantError::InvalidCandidateIdentity,
        ))
    );
    assert_eq!(slot.participant().latest_generation(), 0);
}

#[test]
fn unseen_rollback_tombstones_the_key_without_changing_active_payload() {
    let active = payload(DesktopAuthority::Output, 1, 1);
    let slot = DesktopProfileCandidateSlot::with_active(active.clone()).unwrap();
    let cancelled = rollback_desktop_profile_candidate_slot(&slot, key(2, 2)).unwrap();

    assert_eq!(cancelled.active(), Some(&active));
    assert_eq!(cancelled.candidate(), None);
    assert_eq!(cancelled.participant().latest_generation(), 2);
    assert_eq!(
        prepare_desktop_profile_candidate_slot(&cancelled, payload(DesktopAuthority::Output, 2, 2),),
        Err(DesktopProfileCandidateSlotError::Participant(
            DesktopProfileParticipantError::InvalidCandidateIdentity,
        ))
    );
}

#[test]
fn fragment_prepare_uses_the_slot_authority_and_leaves_rejection_unchanged() {
    use std::os::unix::fs::PermissionsExt as _;

    let directory = std::env::temp_dir().join(format!(
        "sophia-candidate-slot-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let prepared = load_prepared_desktop_profile(None, ConfigGeneration::INITIAL).unwrap();
    let fragments = stage_desktop_profile(&prepared.profile, &directory).unwrap();
    let slot = DesktopProfileCandidateSlot::new(DesktopAuthority::Policy);

    let candidate = prepare_desktop_profile_candidate_slot_from_fragment(
        &slot,
        fragments.path(DesktopAuthority::Policy),
        prepared.activation_key(),
    )
    .unwrap();
    assert_eq!(
        candidate.participant().phase(),
        DesktopProfileParticipantPhase::Prepared
    );
    assert_eq!(
        candidate.candidate().unwrap().authority,
        DesktopAuthority::Policy
    );

    assert!(matches!(
        prepare_desktop_profile_candidate_slot_from_fragment(
            &slot,
            fragments.path(DesktopAuthority::Session),
            prepared.activation_key(),
        ),
        Err(DesktopProfileCandidateSlotError::Profile(_))
    ));
    assert_eq!(slot.participant().latest_generation(), 0);
    assert_eq!(slot.candidate(), None);
    drop(fragments);
    std::fs::remove_dir(directory).unwrap();
}
