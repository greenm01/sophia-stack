use sophia_protocol::*;
fn main() {
    let transaction = TransactionId::from_raw(8);
    let snapshot = ShellTabSnapshot {
        connection_epoch: 5,
        generation: 6,
        groups: vec![ShellTabGroup {
            slot: 1,
            output: OutputId::from_raw(7),
            focused: true,
            selected_slot: Some(1),
            entries: (1..=2)
                .map(|slot| ShellV1Descriptor {
                    slot,
                    generation: 9,
                    label: Some(DisplayLabel {
                        text: format!("Tab {slot}"),
                        redacted: false,
                    }),
                    trust_level: TrustLevel::Trusted,
                    attention: AttentionState::None,
                    action: ToplevelActionCapabilityRef {
                        token: u64::from(slot) + 40,
                        issuer_epoch: 3,
                        issuer_revocation_epoch: 4,
                        recipient_epoch: 5,
                        target_slot: slot,
                        target_generation: 9,
                    },
                })
                .collect(),
        }],
    };
    for frame in encode_shell_tab_snapshot(transaction, &snapshot).unwrap() {
        print_frame("snapshot", &frame);
    }
    print_frame(
        "candidate",
        &encode_shell_tab_candidate(
            transaction,
            &ShellTabCandidate {
                connection_epoch: 5,
                snapshot_generation: 6,
                candidate_generation: 7,
                groups: vec![1],
            },
        )
        .unwrap(),
    );
}
fn print_frame(kind: &str, bytes: &[u8]) {
    println!(
        "{kind}|{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
}
