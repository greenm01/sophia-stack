use sophia_protocol::*;

fn identity() -> WmV1ProfileIdentity {
    WmV1ProfileIdentity::new(9, 7, [0x5a; WM_V1_PROFILE_DIGEST_BYTES]).unwrap()
}

fn command() -> WmV1ProfileCommand {
    WmV1ProfileCommand {
        transaction: TransactionId::from_raw(11),
        identity: identity(),
    }
}

fn completion(outcome: WmV1ProfileOutcome) -> WmV1ProfileCompletion {
    WmV1ProfileCompletion {
        transaction: TransactionId::from_raw(11),
        identity: identity(),
        outcome,
    }
}

#[test]
fn typed_profile_commands_roundtrip_exact_identity() {
    let command = command();
    assert_eq!(
        decode_wm_v1_profile_prepare(&encode_wm_v1_profile_prepare(command).unwrap()).unwrap(),
        command
    );
    assert_eq!(
        decode_wm_v1_profile_activate(&encode_wm_v1_profile_activate(command).unwrap()).unwrap(),
        command
    );
    assert_eq!(
        decode_wm_v1_profile_rollback(&encode_wm_v1_profile_rollback(command).unwrap()).unwrap(),
        command
    );
}

#[test]
fn typed_profile_completions_roundtrip_closed_outcomes() {
    for outcome in [
        WmV1ProfileOutcome::Accepted,
        WmV1ProfileOutcome::RejectedIdentity,
        WmV1ProfileOutcome::RejectedState,
    ] {
        let completion = completion(outcome);
        assert_eq!(
            decode_wm_v1_profile_prepared(&encode_wm_v1_profile_prepared(completion).unwrap())
                .unwrap(),
            completion
        );
        assert_eq!(
            decode_wm_v1_profile_active(&encode_wm_v1_profile_active(completion).unwrap()).unwrap(),
            completion
        );
        assert_eq!(
            decode_wm_v1_profile_rolled_back(
                &encode_wm_v1_profile_rolled_back(completion).unwrap()
            )
            .unwrap(),
            completion
        );
    }
}

#[test]
fn typed_profile_identity_reserves_every_null_field() {
    assert_eq!(
        WmV1ProfileIdentity::new(0, 7, [1; WM_V1_PROFILE_DIGEST_BYTES]),
        Err(IpcCodecError::InvalidProfileIdentity("connection_epoch"))
    );
    assert_eq!(
        WmV1ProfileIdentity::new(9, 0, [1; WM_V1_PROFILE_DIGEST_BYTES]),
        Err(IpcCodecError::InvalidProfileIdentity("profile_generation"))
    );
    assert_eq!(
        WmV1ProfileIdentity::new(9, 7, [0; WM_V1_PROFILE_DIGEST_BYTES]),
        Err(IpcCodecError::InvalidProfileIdentity("profile_digest"))
    );
}

#[test]
fn typed_profile_messages_reject_null_transactions() {
    let mut command = command();
    command.transaction = TransactionId::INVALID;
    assert_eq!(
        encode_wm_v1_profile_prepare(command),
        Err(IpcCodecError::InvalidTransaction(0))
    );

    let mut completion = completion(WmV1ProfileOutcome::Accepted);
    completion.transaction = TransactionId::INVALID;
    assert_eq!(
        encode_wm_v1_profile_prepared(completion),
        Err(IpcCodecError::InvalidTransaction(0))
    );
}

#[test]
fn typed_profile_completion_rejects_unknown_outcome() {
    let wire = WmV1ProfileActive {
        connection_epoch: 9,
        profile_generation: 7,
        profile_digest: [1; WM_V1_PROFILE_DIGEST_BYTES],
        outcome: 4,
    };
    let frame = encode_wm_v1_profile_active_frame(TransactionId::from_raw(11), &wire).unwrap();
    assert_eq!(
        decode_wm_v1_profile_active(&frame),
        Err(IpcCodecError::InvalidEnum {
            field: "profile_outcome",
            value: 4,
        })
    );
}
