use sophia_protocol::*;
#[path = "../tests/support/launcher_fixture.rs"]
mod fixture;
fn emit(name: &str, frame: Vec<u8>) {
    println!(
        "{name}|{}",
        frame.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
}
fn main() {
    let tx = TransactionId::from_raw(1);
    for frame in encode_shell_application_catalog(tx, &fixture::catalog(3)).unwrap() {
        emit("catalog", frame);
    }
    let request = ShellLauncherRequest {
        connection_epoch: 5,
        catalog_generation: 7,
        request_generation: 8,
        output: OutputId::from_raw(1),
        output_generation: 1,
        presentation_epoch: 0,
        operation: ShellLauncherOperation::Open,
        query: String::new(),
    };
    emit(
        "request",
        encode_shell_launcher_request(tx, &request).unwrap(),
    );
    emit(
        "candidate",
        encode_shell_launcher_candidate(tx, &fixture::candidate()).unwrap(),
    );
    for (name, kind, epoch) in [
        ("prepared", ShellV1CandidateOutcomeKind::Prepared, 0),
        ("presented", ShellV1CandidateOutcomeKind::Presented, 11),
    ] {
        emit(
            name,
            encode_shell_launcher_outcome(
                tx,
                ShellLauncherOutcome {
                    connection_epoch: 5,
                    request_generation: 8,
                    candidate_generation: 9,
                    presentation_epoch: epoch,
                    kind,
                },
            )
            .unwrap(),
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
    emit(
        "activation",
        encode_shell_launcher_activation(tx, activation).unwrap(),
    );
    emit(
        "ack",
        encode_shell_launcher_activation_ack(
            tx,
            ShellLauncherActivationAck {
                activation,
                consumed: true,
            },
        )
        .unwrap(),
    );
    emit(
        "started",
        encode_shell_launch_outcome(
            tx,
            ShellLaunchOutcome {
                activation,
                status: ShellLaunchStatus::Started,
            },
        )
        .unwrap(),
    );
}
