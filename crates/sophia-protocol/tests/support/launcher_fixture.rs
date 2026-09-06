use sophia_protocol::*;
pub fn catalog(count: usize) -> ShellApplicationCatalog {
    ShellApplicationCatalog {
        connection_epoch: 5,
        generation: 7,
        entries: (1..=count)
            .map(|slot| ShellApplicationDescriptor {
                slot: slot as u16,
                available: slot != 2,
                label: format!("Application {slot}"),
                keywords: "editor terminal".into(),
            })
            .collect(),
    }
}
pub fn candidate() -> ShellLauncherCandidate {
    ShellLauncherCandidate {
        connection_epoch: 5,
        catalog_generation: 7,
        request_generation: 8,
        candidate_generation: 9,
        output: OutputId::from_raw(1),
        visible: true,
        selected: 1,
        entries: vec![1, 2, 3],
        font_size: 14,
        colors: [0xf0202020, 0xffdddddd, 0xff525f66, 0xffffffff],
    }
}
