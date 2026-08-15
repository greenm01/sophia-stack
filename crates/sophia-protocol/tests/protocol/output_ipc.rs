#[test]
fn output_v1_handshake_round_trips_on_its_own_role_kinds() {
    let hello = OutputV1ClientHello {
        minimum_revision: 1,
        maximum_revision: 1,
        capabilities: SOPHIA_OUTPUT_CAPABILITY_OBSERVE | SOPHIA_OUTPUT_CAPABILITY_CONFIGURE,
    };
    assert_eq!(
        decode_output_v1_client_hello_frame(
            &encode_output_v1_client_hello_frame(hello).unwrap()
        )
        .unwrap(),
        hello
    );

    let welcome = OutputV1ServerWelcome {
        selected_revision: 1,
        capabilities: hello.capabilities,
        connection_epoch: 9,
        max_heads: MAX_OUTPUT_AUTHORITY_HEADS as u16,
        max_groups: MAX_OUTPUT_AUTHORITY_GROUPS as u16,
        max_modes_per_head: MAX_OUTPUT_AUTHORITY_MODES_PER_HEAD as u16,
        max_heads_per_group: MAX_OUTPUT_AUTHORITY_HEADS_PER_GROUP as u16,
    };
    assert_eq!(
        decode_output_v1_server_welcome_frame(
            &encode_output_v1_server_welcome_frame(welcome).unwrap()
        )
        .unwrap(),
        welcome
    );
}

#[test]
fn output_v1_snapshot_and_mixed_topology_proposal_round_trip() {
    let transaction = TransactionId::from_raw(77);
    let snapshot = OutputV1Snapshot {
        connection_epoch: 9,
        snapshot: output_authority_snapshot(),
    };
    assert_eq!(
        decode_output_v1_snapshot_frame(
            &encode_output_v1_snapshot_frame(transaction, &snapshot).unwrap()
        )
        .unwrap(),
        (transaction, snapshot)
    );

    let proposal = OutputV1Proposal {
        connection_epoch: 9,
        candidate: mixed_output_candidate(),
    };
    assert_eq!(
        decode_output_v1_proposal_frame(
            &encode_output_v1_proposal_frame(transaction, &proposal).unwrap()
        )
        .unwrap(),
        (transaction, proposal)
    );
}

#[test]
fn output_v1_terminal_outcomes_retain_transaction_and_topology_epoch() {
    let transaction = TransactionId::from_raw(81);
    for kind in [
        OutputV1OutcomeKind::Validated,
        OutputV1OutcomeKind::Committed,
        OutputV1OutcomeKind::Stale,
        OutputV1OutcomeKind::Rejected,
        OutputV1OutcomeKind::RolledBack,
        OutputV1OutcomeKind::Failed,
    ] {
        let outcome = OutputV1Outcome {
            connection_epoch: 9,
            topology_epoch: 8,
            kind,
            reason: 12,
        };
        assert_eq!(
            decode_output_v1_outcome_frame(
                &encode_output_v1_outcome_frame(transaction, outcome).unwrap()
            )
            .unwrap(),
            (transaction, outcome)
        );
    }
}

#[test]
fn output_v1_decoder_rejects_zero_transaction_and_excessive_counts() {
    let proposal = OutputV1Proposal {
        connection_epoch: 9,
        candidate: mixed_output_candidate(),
    };
    assert!(matches!(
        encode_output_v1_proposal_frame(TransactionId::INVALID, &proposal),
        Err(IpcCodecError::InvalidTransaction(0))
    ));

    let transaction = TransactionId::from_raw(90);
    let mut frame = encode_output_v1_proposal_frame(transaction, &proposal).unwrap();
    // Envelope (24), connection/base epochs (16), intent/primary (4), then heads.
    frame[44..46].copy_from_slice(&(MAX_OUTPUT_AUTHORITY_HEADS as u16 + 1).to_le_bytes());
    assert!(matches!(
        decode_output_v1_proposal_frame(&frame),
        Err(IpcCodecError::CountTooLarge { .. })
    ));
}

#[test]
fn output_v1_rejects_zero_connection_and_topology_epochs() {
    assert!(encode_output_v1_snapshot_frame(
        TransactionId::from_raw(1),
        &OutputV1Snapshot {
            connection_epoch: 0,
            snapshot: output_authority_snapshot(),
        },
    )
    .is_err());
    for (connection_epoch, topology_epoch) in [(0, 7), (9, 0)] {
        assert!(encode_output_v1_outcome_frame(
            TransactionId::from_raw(1),
            OutputV1Outcome {
                connection_epoch,
                topology_epoch,
                kind: OutputV1OutcomeKind::Rejected,
                reason: 1,
            },
        )
        .is_err());
    }
}
