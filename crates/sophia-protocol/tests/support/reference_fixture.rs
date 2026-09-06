use sophia_protocol::*;
pub fn catalog(count: usize) -> ShellShortcutCatalog {
    ShellShortcutCatalog {
        connection_epoch: 5,
        generation: 7,
        entries: (1..=count)
            .map(|i| ShellShortcut {
                slot: i as u16,
                chord: format!("Super+{i}"),
                action: format!("policy:action-{i}"),
                label: Some(format!("Action {i}")),
                group: None,
            })
            .collect(),
    }
}
pub fn candidate(count: usize) -> ShellReferenceCandidate {
    ShellReferenceCandidate {
        connection_epoch: 5,
        catalog_generation: 7,
        request_generation: 8,
        candidate_generation: 9,
        output: OutputId::from_raw(1),
        visible: true,
        page: 0,
        style: ShellReferenceStyle {
            body_size: 14,
            title_size: 16,
            padding: 24,
            row_gap: 10,
            key_gap: 28,
            column_gap: 32,
            border: 4,
            margin: 48,
            columns: 2,
            colors: [
                0xdd111318, 0xff62a8ff, 0xfff4f7fb, 0xffaab3c2, 0xff262a33, 0xffffffff,
            ],
            title: "Important Hotkeys".into(),
        },
        entries: catalog(count)
            .entries
            .into_iter()
            .map(|e| ShellReferenceEntry {
                slot: e.slot,
                key: e.chord,
                label: e.label.unwrap(),
            })
            .collect(),
    }
}
