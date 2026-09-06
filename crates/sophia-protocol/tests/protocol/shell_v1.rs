fn action(slot: u16, generation: u64) -> ToplevelActionCapabilityRef {
    ToplevelActionCapabilityRef {
        token: u64::from(slot) + 40,
        issuer_epoch: 3,
        issuer_revocation_epoch: 4,
        recipient_epoch: 5,
        target_slot: slot,
        target_generation: generation,
    }
}

fn snapshot() -> ShellV1DescriptorSnapshot {
    ShellV1DescriptorSnapshot {
        connection_epoch: 5,
        snapshot_generation: 6,
        output: OutputId::from_raw(7),
        output_generation: 8,
        broker_epoch: 3,
        broker_revocation_epoch: 4,
        descriptors: vec![
            ShellV1Descriptor {
                slot: 1,
                generation: 9,
                label: None,
                trust_level: TrustLevel::Trusted,
                attention: AttentionState::None,
                action: action(1, 9),
            },
            ShellV1Descriptor {
                slot: 2,
                generation: 10,
                label: Some(DisplayLabel {
                    text: "Browser".to_owned(),
                    redacted: true,
                }),
                trust_level: TrustLevel::Isolated,
                attention: AttentionState::Notice,
                action: action(2, 10),
            },
        ],
    }
}

#[test]
fn shell_v1_round_trips_complete_lifecycle_records() {
    let transaction = TransactionId::from_raw(11);
    let hello = ShellV1ClientHello {
        minimum_revision: 1,
        maximum_revision: 1,
        required_capabilities: SOPHIA_SHELL_CAPABILITY_DESCRIPTOR_SWITCHER,
    };
    assert_eq!(
        decode_shell_v1_client_hello_frame(&encode_shell_v1_client_hello_frame(hello).unwrap())
            .unwrap(),
        hello
    );

    let welcome = ShellV1ServerWelcome {
        selected_revision: 1,
        connection_epoch: 5,
        capabilities: SOPHIA_SHELL_CAPABILITY_DESCRIPTOR_SWITCHER,
        max_descriptors: SOPHIA_SHELL_MAX_DESCRIPTORS as u16,
        max_label_bytes: MAX_CHROME_LABEL_LEN as u16,
        max_pending_activations: SOPHIA_SHELL_MAX_PENDING_ACTIVATIONS as u16,
    };
    assert_eq!(
        decode_shell_v1_server_welcome_frame(
            &encode_shell_v1_server_welcome_frame(welcome).unwrap()
        )
        .unwrap(),
        welcome
    );

    let snapshot = snapshot();
    assert_eq!(
        decode_shell_v1_descriptor_snapshot_frame(
            &encode_shell_v1_descriptor_snapshot_frame(transaction, &snapshot).unwrap()
        )
        .unwrap(),
        (transaction, snapshot.clone())
    );

    let candidate = ShellV1Candidate {
        connection_epoch: 5,
        snapshot_generation: 6,
        candidate_generation: 1,
        output: OutputId::from_raw(7),
        visible: true,
        selected_slot: Some(2),
        reservation: None,
        entries: vec![
            ShellV1CandidateEntry {
                slot: 2,
                generation: 10,
            },
            ShellV1CandidateEntry {
                slot: 1,
                generation: 9,
            },
        ],
    };
    assert_eq!(
        decode_shell_v1_candidate_frame(
            &encode_shell_v1_candidate_frame(transaction, &candidate).unwrap()
        )
        .unwrap(),
        (transaction, candidate)
    );

    let outcome = ShellV1CandidateOutcome {
        connection_epoch: 5,
        candidate_generation: 1,
        presentation_epoch: 12,
        kind: ShellV1CandidateOutcomeKind::Presented,
    };
    assert_eq!(
        decode_shell_v1_candidate_outcome_frame(
            &encode_shell_v1_candidate_outcome_frame(transaction, outcome).unwrap()
        )
        .unwrap(),
        (transaction, outcome)
    );

    let activation = ShellV1Activation {
        connection_epoch: 5,
        candidate_generation: 1,
        presentation_epoch: 12,
        activation: 13,
        action: action(2, 10),
    };
    assert_eq!(
        decode_shell_v1_activation_frame(
            &encode_shell_v1_activation_frame(transaction, activation).unwrap()
        )
        .unwrap(),
        (transaction, activation)
    );

    let ack = ShellV1ActivationAck {
        connection_epoch: 5,
        activation: 13,
        disposition: ShellV1ActivationDisposition::Consumed,
    };
    assert_eq!(
        decode_shell_v1_activation_ack_frame(
            &encode_shell_v1_activation_ack_frame(transaction, ack).unwrap()
        )
        .unwrap(),
        (transaction, ack)
    );
}

#[test]
fn shell_v1_rejects_identity_leaks_and_torn_candidates() {
    let transaction = TransactionId::from_raw(1);
    let mut candidate = ShellV1Candidate {
        connection_epoch: 5,
        snapshot_generation: 6,
        candidate_generation: 1,
        output: OutputId::from_raw(7),
        visible: true,
        selected_slot: Some(2),
        reservation: None,
        entries: vec![ShellV1CandidateEntry {
            slot: 1,
            generation: 9,
        }],
    };
    assert_eq!(
        encode_shell_v1_candidate_frame(transaction, &candidate),
        Err(IpcCodecError::InvalidRecord("shell_candidate_selection"))
    );

    candidate.visible = false;
    candidate.selected_slot = None;
    assert_eq!(
        encode_shell_v1_candidate_frame(transaction, &candidate),
        Err(IpcCodecError::InvalidRecord("shell_candidate_visibility"))
    );

    let mut stale = snapshot();
    stale.descriptors[0].action.recipient_epoch = 4;
    assert_eq!(
        encode_shell_v1_descriptor_snapshot_frame(transaction, &stale),
        Err(IpcCodecError::InvalidRecord("shell_descriptor"))
    );

    let mut duplicate = snapshot();
    duplicate.descriptors[1].slot = 1;
    duplicate.descriptors[1].action.target_slot = 1;
    assert_eq!(
        encode_shell_v1_descriptor_snapshot_frame(transaction, &duplicate),
        Err(IpcCodecError::InvalidRecord("shell_descriptor"))
    );
}

#[test]
fn shell_v1_rejects_reserved_and_unknown_envelope_fields() {
    let transaction = TransactionId::from_raw(1);
    let candidate = ShellV1Candidate {
        connection_epoch: 5,
        snapshot_generation: 6,
        candidate_generation: 1,
        output: OutputId::from_raw(7),
        visible: false,
        selected_slot: None,
        reservation: None,
        entries: Vec::new(),
    };
    // Byte 33 carried a reserved zero until reservations claimed it for the
    // edge. It is still refused here, and now for the sharper reason: an edge
    // with no thickness, on a candidate that is not even visible.
    let mut frame = encode_shell_v1_candidate_frame(transaction, &candidate).unwrap();
    frame[SOPHIA_IPC_HEADER_LEN + 33] = 1;
    assert!(matches!(
        decode_shell_v1_candidate_frame(&frame),
        Err(IpcCodecError::InvalidRecord(_))
    ));

    // The per-entry reserved field is still reserved, so the envelope check
    // this test is named for keeps a live subject.
    let visible = ShellV1Candidate {
        visible: true,
        selected_slot: Some(2),
        entries: vec![ShellV1CandidateEntry {
            slot: 2,
            generation: 10,
        }],
        ..candidate.clone()
    };
    let mut frame = encode_shell_v1_candidate_frame(transaction, &visible).unwrap();
    frame[SOPHIA_IPC_HEADER_LEN + 42] = 1;
    assert!(matches!(
        decode_shell_v1_candidate_frame(&frame),
        Err(IpcCodecError::ReservedNonZero(1))
    ));

    let mut frame = encode_shell_v1_client_hello_frame(ShellV1ClientHello {
        minimum_revision: 1,
        maximum_revision: 1,
        required_capabilities: SOPHIA_SHELL_CAPABILITY_DESCRIPTOR_SWITCHER,
    })
    .unwrap();
    frame[6..8].copy_from_slice(&123_u16.to_le_bytes());
    assert_eq!(
        decode_shell_v1_client_hello_frame(&frame),
        Err(IpcCodecError::UnknownMessageKind(123))
    );
}
