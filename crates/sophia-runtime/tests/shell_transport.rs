use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sophia_protocol::{
    AttentionState, DisplayLabel, OutputId, ShellV1Activation, ShellV1ActivationAck,
    ShellV1ActivationDisposition, ShellV1Candidate, ShellV1CandidateEntry, ShellV1CandidateOutcome,
    ShellV1CandidateOutcomeKind, ShellV1Descriptor, ShellV1DescriptorSnapshot,
    ToplevelActionCapabilityRef, TransactionId, TrustLevel,
};
use sophia_runtime::{
    ProtectionBackendKind, ProtectionDomainEvidence, ProtectionDomainRole, ShellClientTransport,
    ShellSessionTransport, ShellTransportError,
};

fn evidence() -> ProtectionDomainEvidence {
    ProtectionDomainEvidence {
        backend: ProtectionBackendKind::Bubblewrap,
        supervisor_pid: std::process::id(),
        peer_pid: std::process::id(),
        roles: [ProtectionDomainRole::MetadataShell].into_iter().collect(),
    }
}

fn directory(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "sophia-shell-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn action() -> ToplevelActionCapabilityRef {
    ToplevelActionCapabilityRef {
        token: 3,
        issuer_epoch: 4,
        issuer_revocation_epoch: 5,
        recipient_epoch: 1,
        target_slot: 1,
        target_generation: 6,
    }
}

fn snapshot() -> ShellV1DescriptorSnapshot {
    ShellV1DescriptorSnapshot {
        connection_epoch: 1,
        snapshot_generation: 7,
        output: OutputId::from_raw(8),
        output_generation: 9,
        broker_epoch: 4,
        broker_revocation_epoch: 5,
        descriptors: vec![ShellV1Descriptor {
            slot: 1,
            generation: 6,
            label: Some(DisplayLabel {
                text: "Terminal".into(),
                redacted: false,
            }),
            trust_level: TrustLevel::Trusted,
            attention: AttentionState::None,
            action: action(),
        }],
    }
}

#[test]
fn protected_shell_completes_candidate_presentation_and_activation() {
    let mut session = ShellSessionTransport::bind_for_supervised_uid(
        directory("lifecycle"),
        rustix::process::geteuid().as_raw(),
    )
    .unwrap();
    session.authorize_protected_peer(&evidence()).unwrap();
    let socket = session.socket_path().to_path_buf();
    let client = std::thread::spawn(move || {
        let mut client = ShellClientTransport::connect(socket).unwrap();
        let (transaction, snapshot) = client.receive_snapshot().unwrap();
        let candidate = ShellV1Candidate {
            connection_epoch: client.connection_epoch(),
            snapshot_generation: snapshot.snapshot_generation,
            candidate_generation: 1,
            output: snapshot.output,
            visible: true,
            selected_slot: Some(1),
            entries: vec![ShellV1CandidateEntry {
                slot: 1,
                generation: 6,
            }],
        };
        client.send_candidate(transaction, &candidate).unwrap();
        let (_, outcome) = client.receive_candidate_outcome().unwrap();
        assert_eq!(outcome.kind, ShellV1CandidateOutcomeKind::Prepared);
        let (_, outcome) = client.receive_candidate_outcome().unwrap();
        assert_eq!(outcome.kind, ShellV1CandidateOutcomeKind::Presented);
        let (transaction, activation) = client.receive_activation().unwrap();
        assert_eq!(activation.action, action());
        client
            .acknowledge_activation(
                transaction,
                ShellV1ActivationAck {
                    connection_epoch: client.connection_epoch(),
                    activation: activation.activation,
                    disposition: ShellV1ActivationDisposition::Consumed,
                },
            )
            .unwrap();
    });

    session
        .accept_and_negotiate(1, Duration::from_secs(2))
        .unwrap();
    let transaction = TransactionId::from_raw(1);
    let candidate = session.request_candidate(transaction, &snapshot()).unwrap();
    assert_eq!(candidate.selected_slot, Some(1));
    session
        .send_candidate_outcome(
            transaction,
            ShellV1CandidateOutcome {
                connection_epoch: 1,
                candidate_generation: 1,
                presentation_epoch: 0,
                kind: ShellV1CandidateOutcomeKind::Prepared,
            },
        )
        .unwrap();
    session
        .send_candidate_outcome(
            transaction,
            ShellV1CandidateOutcome {
                connection_epoch: 1,
                candidate_generation: 1,
                presentation_epoch: 10,
                kind: ShellV1CandidateOutcomeKind::Presented,
            },
        )
        .unwrap();
    let activation_transaction = TransactionId::from_raw(2);
    session
        .queue_activation(
            activation_transaction,
            ShellV1Activation {
                connection_epoch: 1,
                candidate_generation: 1,
                presentation_epoch: 10,
                activation: 11,
                action: action(),
            },
        )
        .unwrap();
    assert_eq!(
        session.receive_activation_ack().unwrap().disposition,
        ShellV1ActivationDisposition::Consumed
    );
    session.disconnect().unwrap();
    client.join().unwrap();
}

#[test]
fn shell_activation_queue_saturation_revokes_the_connection() {
    let mut session = ShellSessionTransport::bind_for_supervised_uid(
        directory("saturation"),
        rustix::process::geteuid().as_raw(),
    )
    .unwrap();
    session.authorize_protected_peer(&evidence()).unwrap();
    let socket = session.socket_path().to_path_buf();
    let client = std::thread::spawn(move || {
        let mut client = ShellClientTransport::connect(socket).unwrap();
        let (transaction, snapshot) = client.receive_snapshot().unwrap();
        client
            .send_candidate(
                transaction,
                &ShellV1Candidate {
                    connection_epoch: 1,
                    snapshot_generation: snapshot.snapshot_generation,
                    candidate_generation: 1,
                    output: snapshot.output,
                    visible: true,
                    selected_slot: Some(1),
                    entries: vec![ShellV1CandidateEntry {
                        slot: 1,
                        generation: 6,
                    }],
                },
            )
            .unwrap();
        client.receive_candidate_outcome().unwrap();
        client.receive_candidate_outcome().unwrap();
        std::thread::sleep(Duration::from_millis(250));
    });
    session
        .accept_and_negotiate(1, Duration::from_secs(2))
        .unwrap();
    let candidate_transaction = TransactionId::from_raw(100);
    session
        .request_candidate(candidate_transaction, &snapshot())
        .unwrap();
    session
        .send_candidate_outcome(
            candidate_transaction,
            ShellV1CandidateOutcome {
                connection_epoch: 1,
                candidate_generation: 1,
                presentation_epoch: 0,
                kind: ShellV1CandidateOutcomeKind::Prepared,
            },
        )
        .unwrap();
    session
        .send_candidate_outcome(
            candidate_transaction,
            ShellV1CandidateOutcome {
                connection_epoch: 1,
                candidate_generation: 1,
                presentation_epoch: 1,
                kind: ShellV1CandidateOutcomeKind::Presented,
            },
        )
        .unwrap();
    for activation in 1..=sophia_protocol::SOPHIA_SHELL_MAX_PENDING_ACTIVATIONS {
        session
            .queue_activation(
                TransactionId::from_raw(activation as u64),
                ShellV1Activation {
                    connection_epoch: 1,
                    candidate_generation: 1,
                    presentation_epoch: 1,
                    activation: activation as u64,
                    action: action(),
                },
            )
            .unwrap();
    }
    assert_eq!(
        session.queue_activation(
            TransactionId::from_raw(99),
            ShellV1Activation {
                connection_epoch: 1,
                candidate_generation: 1,
                presentation_epoch: 1,
                activation: 99,
                action: action(),
            },
        ),
        Err(ShellTransportError::ActivationQueueSaturated)
    );
    client.join().unwrap();
}

#[test]
fn shell_transport_requires_an_exact_snapshot_and_prepare_order() {
    let mut session = ShellSessionTransport::bind_for_supervised_uid(
        directory("candidate-order"),
        rustix::process::geteuid().as_raw(),
    )
    .unwrap();
    session.authorize_protected_peer(&evidence()).unwrap();
    let socket = session.socket_path().to_path_buf();
    let client = std::thread::spawn(move || {
        let mut client = ShellClientTransport::connect(socket).unwrap();
        let (transaction, first) = client.receive_snapshot().unwrap();
        client
            .send_candidate(
                transaction,
                &ShellV1Candidate {
                    connection_epoch: 1,
                    snapshot_generation: first.snapshot_generation + 1,
                    candidate_generation: 1,
                    output: first.output,
                    visible: false,
                    selected_slot: None,
                    entries: Vec::new(),
                },
            )
            .unwrap();
        let (transaction, second) = client.receive_snapshot().unwrap();
        client
            .send_candidate(
                transaction,
                &ShellV1Candidate {
                    connection_epoch: 1,
                    snapshot_generation: second.snapshot_generation,
                    candidate_generation: 2,
                    output: second.output,
                    visible: false,
                    selected_slot: None,
                    entries: Vec::new(),
                },
            )
            .unwrap();
        assert_eq!(
            client.receive_candidate_outcome().unwrap().1.kind,
            ShellV1CandidateOutcomeKind::Prepared
        );
        assert_eq!(
            client.receive_candidate_outcome().unwrap().1.kind,
            ShellV1CandidateOutcomeKind::Presented
        );
    });
    session
        .accept_and_negotiate(1, Duration::from_secs(2))
        .unwrap();
    assert_eq!(
        session.request_candidate(TransactionId::from_raw(1), &snapshot()),
        Err(ShellTransportError::WrongCandidate)
    );
    session
        .request_candidate(TransactionId::from_raw(2), &snapshot())
        .unwrap();
    let presented = ShellV1CandidateOutcome {
        connection_epoch: 1,
        candidate_generation: 2,
        presentation_epoch: 3,
        kind: ShellV1CandidateOutcomeKind::Presented,
    };
    assert_eq!(
        session.send_candidate_outcome(TransactionId::from_raw(2), presented),
        Err(ShellTransportError::WrongCandidate)
    );
    session
        .send_candidate_outcome(
            TransactionId::from_raw(2),
            ShellV1CandidateOutcome {
                connection_epoch: 1,
                candidate_generation: 2,
                presentation_epoch: 0,
                kind: ShellV1CandidateOutcomeKind::Prepared,
            },
        )
        .unwrap();
    session
        .send_candidate_outcome(TransactionId::from_raw(2), presented)
        .unwrap();
    session.disconnect().unwrap();
    client.join().unwrap();
}
