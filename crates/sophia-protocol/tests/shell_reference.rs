use sophia_protocol::*;
#[path = "support/reference_fixture.rs"]
mod fixture;
#[test]
fn maximum_catalog_and_reference_round_trip_without_descriptor_limit() {
    let tx = TransactionId::from_raw(1);
    let catalog = fixture::catalog(256);
    let frames = encode_shell_shortcut_catalog(tx, &catalog).unwrap();
    assert_eq!(frames.len(), 258);
    assert_eq!(
        decode_shell_shortcut_catalog(&frames).unwrap(),
        (tx, catalog)
    );
    let candidate = fixture::candidate(256);
    let frame = encode_shell_reference_candidate(tx, &candidate).unwrap();
    assert_eq!(
        decode_shell_reference_candidate(&frame).unwrap(),
        (tx, candidate)
    );
    for op in [
        ShellReferenceOperation::Startup,
        ShellReferenceOperation::Toggle,
        ShellReferenceOperation::Next,
        ShellReferenceOperation::Previous,
        ShellReferenceOperation::Dismiss,
    ] {
        let request = ShellReferenceRequest {
            connection_epoch: 5,
            catalog_generation: 7,
            request_generation: 8,
            output: OutputId::from_raw(1),
            output_generation: 2,
            presentation_epoch: 10,
            operation: op,
        };
        assert_eq!(
            decode_shell_reference_request(&encode_shell_reference_request(tx, request).unwrap())
                .unwrap(),
            (tx, request)
        );
    }
    let outcome = ShellReferenceOutcome {
        connection_epoch: 5,
        catalog_generation: 7,
        request_generation: 8,
        candidate_generation: 9,
        presentation_epoch: 10,
        page: 2,
        pages: 5,
        kind: ShellV1CandidateOutcomeKind::Presented,
    };
    assert_eq!(
        decode_shell_reference_outcome(&encode_shell_reference_outcome(tx, outcome).unwrap())
            .unwrap(),
        (tx, outcome)
    );
}
#[test]
fn partial_mixed_replayed_and_malformed_records_fail_closed() {
    let frames =
        encode_shell_shortcut_catalog(TransactionId::from_raw(1), &fixture::catalog(3)).unwrap();
    for i in 0..frames.len() {
        let mut changed = frames.clone();
        changed.remove(i);
        assert!(decode_shell_shortcut_catalog(&changed).is_err());
        let mut changed = frames.clone();
        changed[i][24] ^= 1;
        assert!(decode_shell_shortcut_catalog(&changed).is_err());
        let mut changed = frames.clone();
        changed[i][8] ^= 1;
        assert!(decode_shell_shortcut_catalog(&changed).is_err());
    }
    let tx = TransactionId::from_raw(1);
    let c = fixture::candidate(2);
    let frame = encode_shell_reference_candidate(tx, &c).unwrap();
    for length in 0..frame.len() {
        assert!(decode_shell_reference_candidate(&frame[..length]).is_err());
    }
    let mut c = c.clone();
    c.entries[1].slot = c.entries[0].slot;
    assert!(encode_shell_reference_candidate(tx, &c).is_err());
    let mut c = fixture::candidate(1);
    c.entries[0].label = "a\u{202e}b".into();
    assert!(encode_shell_reference_candidate(tx, &c).is_err());
    let mut c = fixture::candidate(1);
    c.style.columns = 0;
    assert!(encode_shell_reference_candidate(tx, &c).is_err());
    assert!(encode_shell_shortcut_catalog(tx, &fixture::catalog(257)).is_err());
    assert!(encode_shell_reference_candidate(tx, &fixture::candidate(257)).is_err());
}
#[test]
fn independent_golden_corpus_matches_the_rust_codec() {
    let mut catalog = Vec::new();
    for line in include_str!("../../../protocol/golden/sophia-shell-reference.frames").lines() {
        let (name, hex) = line.split_once('|').unwrap();
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        match name {
            "catalog" => catalog.push(bytes),
            "candidate" => assert_eq!(
                bytes,
                encode_shell_reference_candidate(
                    TransactionId::from_raw(1),
                    &fixture::candidate(2)
                )
                .unwrap()
            ),
            "request" => {
                decode_shell_reference_request(&bytes).unwrap();
            }
            "prepared" | "presented" => {
                let (tx, o) = decode_shell_reference_outcome(&bytes).unwrap();
                assert_eq!(bytes, encode_shell_reference_outcome(tx, o).unwrap());
            }
            _ => panic!("unknown fixture"),
        }
    }
    assert_eq!(
        decode_shell_shortcut_catalog(&catalog).unwrap().1,
        fixture::catalog(2)
    );
}
