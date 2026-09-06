use sophia_protocol::*;
#[path = "support/launcher_fixture.rs"]
mod fixture;
#[test]
fn bounded_catalog_and_launch_records_round_trip() {
    let tx = TransactionId::from_raw(1);
    let catalog = fixture::catalog(4096);
    let frames = encode_shell_application_catalog(tx, &catalog).unwrap();
    assert_eq!(frames.len(), 4098);
    assert_eq!(
        decode_shell_application_catalog(&frames).unwrap(),
        (tx, catalog)
    );
    let candidate = fixture::candidate();
    assert_eq!(
        decode_shell_launcher_candidate(&encode_shell_launcher_candidate(tx, &candidate).unwrap())
            .unwrap(),
        (tx, candidate)
    );
    for operation in [
        ShellLauncherOperation::Open,
        ShellLauncherOperation::Query,
        ShellLauncherOperation::Next,
        ShellLauncherOperation::Previous,
        ShellLauncherOperation::Dismiss,
    ] {
        let request = ShellLauncherRequest {
            connection_epoch: 5,
            catalog_generation: 7,
            request_generation: 8,
            output: OutputId::from_raw(1),
            output_generation: 1,
            presentation_epoch: 11,
            operation,
            query: "Éditeur".into(),
        };
        assert_eq!(
            decode_shell_launcher_request(&encode_shell_launcher_request(tx, &request).unwrap())
                .unwrap(),
            (tx, request)
        );
    }
    let activation = ShellLauncherActivation {
        connection_epoch: 5,
        catalog_generation: 7,
        request_generation: 8,
        candidate_generation: 9,
        presentation_epoch: 11,
        activation: 12,
        slot: 1,
    };
    assert_eq!(
        decode_shell_launcher_activation(
            &encode_shell_launcher_activation(tx, activation).unwrap()
        )
        .unwrap(),
        (tx, activation)
    );
    for consumed in [false, true] {
        let ack = ShellLauncherActivationAck {
            activation,
            consumed,
        };
        assert_eq!(
            decode_shell_launcher_activation_ack(
                &encode_shell_launcher_activation_ack(tx, ack).unwrap()
            )
            .unwrap(),
            (tx, ack)
        );
    }
    for status in [
        ShellLaunchStatus::Started,
        ShellLaunchStatus::Failed,
        ShellLaunchStatus::Rejected,
    ] {
        let outcome = ShellLaunchOutcome { activation, status };
        assert_eq!(
            decode_shell_launch_outcome(&encode_shell_launch_outcome(tx, outcome).unwrap())
                .unwrap(),
            (tx, outcome)
        );
    }
}
#[test]
fn malformed_catalog_and_candidate_cannot_cross_boundary() {
    let tx = TransactionId::from_raw(1);
    let mut catalog = fixture::catalog(3);
    catalog.entries[1].slot = 1;
    assert!(encode_shell_application_catalog(tx, &catalog).is_err());
    catalog.entries[1].slot = 2;
    catalog.entries[1].label = "fake\u{202e}label".into();
    assert!(encode_shell_application_catalog(tx, &catalog).is_err());
    let mut frames = encode_shell_application_catalog(tx, &fixture::catalog(3)).unwrap();
    frames.swap(0, 1);
    assert!(decode_shell_application_catalog(&frames).is_err());
    let mut candidate = fixture::candidate();
    candidate.entries.push(1);
    assert!(encode_shell_launcher_candidate(tx, &candidate).is_err());
    candidate.entries.pop();
    candidate.selected = 4;
    assert!(encode_shell_launcher_candidate(tx, &candidate).is_err());
    candidate.selected = 1;
    let frame = encode_shell_launcher_candidate(tx, &candidate).unwrap();
    for size in 0..frame.len() {
        assert!(decode_shell_launcher_candidate(&frame[..size]).is_err());
    }
}
