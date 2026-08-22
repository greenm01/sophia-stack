#[test]
fn broker_v1_candidate_and_descriptor_round_trip() {
    let transaction = TransactionId::from_raw(7);
    let surface = SurfaceId::new(4, 2);
    let request = BrokerV1Request::CandidateReduced {
        connection_epoch: 9,
        candidate: ReducedMetadataCandidate {
            surface,
            label: Some(DisplayLabel {
                text: "Firefox".to_owned(),
                redacted: true,
            }),
            disclosure: MetadataDisclosure::ClassOnly,
            generation: 11,
        },
    };
    let encoded = encode_broker_v1_request_frame(transaction, &request).unwrap();
    assert_eq!(
        decode_broker_v1_request_frame(&encoded).unwrap(),
        (transaction, request)
    );

    let response = BrokerV1Response::EmitDescriptor {
        connection_epoch: 9,
        descriptor: SanitizedChromeMetadata {
            surface,
            label: Some("Firefox".to_owned()),
            label_redacted: true,
            icon: Some(IconTokenId::from_raw(3)),
            trust_level: TrustLevel::Isolated,
            attention: AttentionState::Notice,
            generation: 11,
        },
    };
    let encoded = encode_broker_v1_response_frame(transaction, &response).unwrap();
    assert_eq!(
        decode_broker_v1_response_frame(&encoded).unwrap(),
        (transaction, response)
    );
}

#[test]
fn broker_v1_rejects_unknown_kinds_and_excessive_labels() {
    let request = BrokerV1Request::CandidateReduced {
        connection_epoch: 1,
        candidate: ReducedMetadataCandidate {
            surface: SurfaceId::new(1, 1),
            label: Some(DisplayLabel {
                text: "x".repeat(MAX_CHROME_LABEL_LEN + 1),
                redacted: false,
            }),
            disclosure: MetadataDisclosure::Full,
            generation: 1,
        },
    };
    assert!(matches!(
        encode_broker_v1_request_frame(TransactionId::from_raw(1), &request),
        Err(IpcCodecError::TextTooLarge { .. })
    ));

    let valid = encode_broker_v1_request_frame(
        TransactionId::from_raw(1),
        &BrokerV1Request::SurfaceRemoved {
            connection_epoch: 1,
            surface: SurfaceId::new(1, 1),
        },
    )
    .unwrap();
    let mut unknown = valid;
    unknown[SOPHIA_IPC_HEADER_LEN] = 0xff;
    unknown[SOPHIA_IPC_HEADER_LEN + 1] = 0xff;
    assert!(matches!(
        decode_broker_v1_request_frame(&unknown),
        Err(IpcCodecError::InvalidEnum {
            field: "broker_request_kind",
            ..
        })
    ));
}

#[test]
fn broker_v1_handshakes_reject_transaction_ids() {
    let mut hello = encode_broker_v1_client_hello_frame(BrokerV1ClientHello {
        minimum_revision: 1,
        maximum_revision: 1,
    })
    .unwrap();
    hello[8..16].copy_from_slice(&7_u64.to_le_bytes());
    assert_eq!(
        decode_broker_v1_client_hello_frame(&hello),
        Err(IpcCodecError::InvalidTransaction(7))
    );

    let mut welcome = encode_broker_v1_server_welcome_frame(BrokerV1ServerWelcome {
        selected_revision: 1,
        connection_epoch: 1,
        max_surfaces: SOPHIA_BROKER_MAX_SURFACES,
        max_label_bytes: MAX_CHROME_LABEL_LEN as u16,
    })
    .unwrap();
    welcome[8..16].copy_from_slice(&8_u64.to_le_bytes());
    assert_eq!(
        decode_broker_v1_server_welcome_frame(&welcome),
        Err(IpcCodecError::InvalidTransaction(8))
    );
}
