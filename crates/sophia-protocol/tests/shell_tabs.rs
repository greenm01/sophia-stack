use sophia_protocol::*;

fn snapshot() -> ShellTabSnapshot {
    ShellTabSnapshot {
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
    }
}
#[test]
fn complete_transfer_and_candidate_round_trip() {
    let snapshot = snapshot();
    let tx = TransactionId::from_raw(8);
    let frames = encode_shell_tab_snapshot(tx, &snapshot).unwrap();
    assert_eq!(decode_shell_tab_snapshot(&frames).unwrap(), (tx, snapshot));
    let c = ShellTabCandidate {
        connection_epoch: 5,
        snapshot_generation: 6,
        candidate_generation: 7,
        groups: vec![1],
    };
    assert_eq!(
        decode_shell_tab_candidate(&encode_shell_tab_candidate(tx, &c).unwrap()).unwrap(),
        (tx, c)
    );
}
#[test]
fn malformed_and_mixed_transfers_fail_closed() {
    let frames = encode_shell_tab_snapshot(TransactionId::from_raw(8), &snapshot()).unwrap();
    for index in 0..frames.len() {
        let mut changed = frames.clone();
        changed.remove(index);
        assert!(decode_shell_tab_snapshot(&changed).is_err());
        let mut changed = frames.clone();
        changed[index][24] ^= 1;
        assert!(decode_shell_tab_snapshot(&changed).is_err());
    }
    let mut changed = frames.clone();
    changed[1][24 + 34] = 2;
    assert!(decode_shell_tab_snapshot(&changed).is_err());
    let mut changed = frames.clone();
    changed[1][24 + 38] = 1;
    assert!(decode_shell_tab_snapshot(&changed).is_err());
    let mut changed = frames.clone();
    changed.swap(1, 2);
    assert!(decode_shell_tab_snapshot(&changed).is_err());
}
#[test]
fn bounds_and_occurrence_identity_are_enforced() {
    let mut s = snapshot();
    s.groups[0].selected_slot = Some(3);
    assert!(validate_shell_tab_snapshot(&s).is_err());
    let mut s = snapshot();
    s.groups.push(s.groups[0].clone());
    assert!(validate_shell_tab_snapshot(&s).is_err());
    let mut s = snapshot();
    s.groups[0].entries[1].slot = 1;
    assert!(validate_shell_tab_snapshot(&s).is_err());
    let mut s = snapshot();
    s.groups[0].entries[0].action.recipient_epoch = 4;
    assert!(validate_shell_tab_snapshot(&s).is_err());
}
