use sophia_broker::{
    MetadataBroker, MetadataBrokerCommand, MetadataBrokerEvent, MetadataBrokerRejection,
    trust_for_namespace_profile,
};
use sophia_protocol::{
    AttentionState, DisplayLabel, MetadataDisclosure, NamespaceProfile, ReducedMetadataCandidate,
    SanitizedChromeMetadata, SurfaceId, TrustLevel,
};

const SURFACE: SurfaceId = SurfaceId::new(4, 1);

fn admitted(profile: NamespaceProfile) -> MetadataBroker {
    let mut broker = MetadataBroker::new();
    broker
        .update(MetadataBrokerEvent::SurfaceAdmitted {
            surface: SURFACE,
            profile,
        })
        .expect("admission succeeds");
    broker
}

fn candidate(
    disclosure: MetadataDisclosure,
    text: Option<&str>,
    generation: u64,
) -> MetadataBrokerEvent {
    MetadataBrokerEvent::CandidateReduced(ReducedMetadataCandidate {
        surface: SURFACE,
        label: text.map(|text| DisplayLabel {
            text: text.to_owned(),
            redacted: false,
        }),
        disclosure,
        generation,
    })
}

fn descriptor(commands: Vec<MetadataBrokerCommand>) -> SanitizedChromeMetadata {
    commands
        .into_iter()
        .find_map(|command| match command {
            MetadataBrokerCommand::EmitDescriptor(descriptor) => Some(descriptor),
            _ => None,
        })
        .expect("a descriptor was emitted")
}

#[test]
fn admission_publishes_a_rule_that_discloses_nothing() {
    // An authority cannot reduce without a rule, and the window between a surface
    // appearing and someone deciding about it must not disclose a title.
    let broker = admitted(NamespaceProfile::Confined);
    let rule = broker.rule_for(SURFACE).expect("the surface is admitted");

    assert_eq!(rule.disclosure, MetadataDisclosure::None);
    assert_eq!(rule.surface, SURFACE);
    assert!(rule.icon.is_some());
}

#[test]
fn trust_follows_the_namespace_profile() {
    // Confinement describes what the namespace enforces, not suspicion about the
    // client, so it is Isolated rather than Untrusted. A sandboxed application from
    // a trusted vendor badged "untrusted" teaches users to ignore the badge.
    assert_eq!(
        trust_for_namespace_profile(NamespaceProfile::ClassicShared),
        TrustLevel::Trusted
    );
    assert_eq!(
        trust_for_namespace_profile(NamespaceProfile::Confined),
        TrustLevel::Isolated
    );

    let mut broker = admitted(NamespaceProfile::Confined);
    broker
        .set_disclosure(SURFACE, MetadataDisclosure::Full)
        .expect("the surface is admitted");
    let emitted = descriptor(
        broker
            .update(candidate(MetadataDisclosure::Full, Some("Report"), 1))
            .expect("a permitted candidate is accepted"),
    );

    assert_eq!(emitted.trust_level, TrustLevel::Isolated);
}

#[test]
fn an_authority_disclosing_more_than_its_rule_is_refused() {
    // The boundary rests on authorities applying the rule honestly, so a candidate
    // exceeding it is refused rather than trimmed. Trimming would hide a broken
    // authority behind a working desktop.
    let mut broker = admitted(NamespaceProfile::Confined);
    broker
        .set_disclosure(SURFACE, MetadataDisclosure::ClassOnly)
        .expect("the surface is admitted");

    assert_eq!(
        broker.update(candidate(MetadataDisclosure::Full, Some("Salary.ods"), 1)),
        Err(MetadataBrokerRejection::DisclosureExceeded)
    );
}

#[test]
fn one_icon_token_per_surface_survives_updates_and_readmission() {
    // A taskbar entry must not change icon because a client reconnected or renamed
    // its window.
    let mut broker = admitted(NamespaceProfile::ClassicShared);
    broker
        .set_disclosure(SURFACE, MetadataDisclosure::Full)
        .expect("the surface is admitted");

    let first = descriptor(
        broker
            .update(candidate(MetadataDisclosure::Full, Some("One"), 1))
            .expect("accepted"),
    );
    let second = descriptor(
        broker
            .update(candidate(MetadataDisclosure::Full, Some("Two"), 2))
            .expect("accepted"),
    );
    broker
        .update(MetadataBrokerEvent::SurfaceAdmitted {
            surface: SURFACE,
            profile: NamespaceProfile::ClassicShared,
        })
        .expect("readmission succeeds");
    let after_readmission = broker
        .rule_for(SURFACE)
        .expect("the surface is still admitted");

    assert_eq!(first.icon, second.icon);
    assert_eq!(after_readmission.icon, first.icon);
}

#[test]
fn a_retired_surface_never_gets_its_token_back() {
    // A recycled token would let a stale descriptor point at a different window.
    let mut broker = admitted(NamespaceProfile::ClassicShared);
    let original = broker.rule_for(SURFACE).expect("admitted").icon;

    broker
        .update(MetadataBrokerEvent::SurfaceRemoved { surface: SURFACE })
        .expect("retirement succeeds");
    assert!(broker.is_empty());

    broker
        .update(MetadataBrokerEvent::SurfaceAdmitted {
            surface: SURFACE,
            profile: NamespaceProfile::ClassicShared,
        })
        .expect("readmission succeeds");

    assert_ne!(broker.rule_for(SURFACE).expect("admitted").icon, original);
}

#[test]
fn a_stale_candidate_is_rejected_without_changing_anything() {
    let mut broker = admitted(NamespaceProfile::ClassicShared);
    broker
        .set_disclosure(SURFACE, MetadataDisclosure::Full)
        .expect("admitted");
    broker
        .update(candidate(MetadataDisclosure::Full, Some("Current"), 7))
        .expect("accepted");

    assert_eq!(
        broker.update(candidate(MetadataDisclosure::Full, Some("Older"), 6)),
        Err(MetadataBrokerRejection::StaleGeneration)
    );
}

#[test]
fn an_unknown_surface_is_refused_rather_than_invented() {
    let mut broker = MetadataBroker::new();

    assert_eq!(
        broker.update(candidate(MetadataDisclosure::Full, Some("Ghost"), 1)),
        Err(MetadataBrokerRejection::UnknownSurface)
    );
    assert_eq!(
        broker.update(MetadataBrokerEvent::SurfaceRemoved { surface: SURFACE }),
        Err(MetadataBrokerRejection::UnknownSurface)
    );
    assert!(broker.is_empty());
}

#[test]
fn attention_changes_without_a_candidate_and_without_a_new_rule() {
    // Attention is not identity, so it does not pass through disclosure and does not
    // advance the label's generation.
    let mut broker = admitted(NamespaceProfile::Confined);

    let emitted = descriptor(
        broker
            .update(MetadataBrokerEvent::AttentionChanged {
                surface: SURFACE,
                attention: AttentionState::Critical,
            })
            .expect("accepted"),
    );

    assert_eq!(emitted.attention, AttentionState::Critical);
    assert_eq!(emitted.label, None);
    assert_eq!(emitted.generation, 0);
}

#[test]
fn attention_changes_retain_the_last_reduced_label() {
    let mut broker = admitted(NamespaceProfile::Confined);
    broker
        .set_disclosure(SURFACE, MetadataDisclosure::ClassOnly)
        .expect("surface is admitted");
    let first = descriptor(
        broker
            .update(MetadataBrokerEvent::CandidateReduced(
                sophia_protocol::ReducedMetadataCandidate {
                    surface: SURFACE,
                    label: Some(sophia_protocol::DisplayLabel {
                        text: "Browser".into(),
                        redacted: true,
                    }),
                    disclosure: MetadataDisclosure::ClassOnly,
                    generation: 7,
                },
            ))
            .expect("candidate is accepted"),
    );
    assert_eq!(first.label.as_deref(), Some("Browser"));

    let attention = descriptor(
        broker
            .update(MetadataBrokerEvent::AttentionChanged {
                surface: SURFACE,
                attention: AttentionState::Critical,
            })
            .expect("attention is accepted"),
    );
    assert_eq!(attention.label.as_deref(), Some("Browser"));
    assert!(attention.label_redacted);
    assert_eq!(attention.generation, 7);
}

#[test]
fn lowering_disclosure_clears_the_retained_label_before_attention_changes() {
    let mut broker = admitted(NamespaceProfile::ClassicShared);
    broker
        .set_disclosure(SURFACE, MetadataDisclosure::Full)
        .expect("surface is admitted");
    broker
        .update(candidate(
            MetadataDisclosure::Full,
            Some("Private title"),
            7,
        ))
        .expect("candidate is accepted");

    let cleared = descriptor(
        broker
            .set_disclosure(SURFACE, MetadataDisclosure::None)
            .expect("disclosure is lowered"),
    );
    assert_eq!(cleared.label, None);
    assert!(!cleared.label_redacted);

    let attention = descriptor(
        broker
            .update(MetadataBrokerEvent::AttentionChanged {
                surface: SURFACE,
                attention: AttentionState::Notice,
            })
            .expect("attention is accepted"),
    );
    assert_eq!(attention.label, None);
    assert!(!attention.label_redacted);
    assert_eq!(attention.generation, 7);
}

#[test]
fn an_unchanged_attention_state_emits_nothing() {
    let mut broker = admitted(NamespaceProfile::Confined);

    assert!(
        broker
            .update(MetadataBrokerEvent::AttentionChanged {
                surface: SURFACE,
                attention: AttentionState::None,
            })
            .expect("accepted")
            .is_empty()
    );
}

#[test]
fn a_redacted_label_stays_marked_through_the_broker() {
    // The authority truncated it; the broker must not launder that away, or Engine
    // will present a shortened name as the client's chosen one.
    let mut broker = admitted(NamespaceProfile::ClassicShared);
    broker
        .set_disclosure(SURFACE, MetadataDisclosure::Full)
        .expect("admitted");

    let emitted = descriptor(
        broker
            .update(MetadataBrokerEvent::CandidateReduced(
                ReducedMetadataCandidate {
                    surface: SURFACE,
                    label: Some(DisplayLabel {
                        text: "Truncated".to_owned(),
                        redacted: true,
                    }),
                    disclosure: MetadataDisclosure::Full,
                    generation: 1,
                },
            ))
            .expect("accepted"),
    );

    assert!(emitted.label_redacted);
    assert_eq!(emitted.label.as_deref(), Some("Truncated"));
}
