use sophia_protocol::*;

fn groups() -> Vec<PolicyTranslationGroup> {
    vec![PolicyTranslationGroup {
        output: OutputId::from_raw(1),
        group: 7,
        x: -1268,
        y: 0,
        members: vec![SurfaceId::new(1, 1), SurfaceId::new(2, 1)],
    }]
}

#[test]
fn shared_translation_round_trip_and_malformed_members_fail_closed() {
    let chunks = encode_wm_translation_groups(&groups(), 3, 4).unwrap();
    assert_eq!(chunks[0].ordinal, 4);
    assert_eq!(decode_wm_translation_groups(&chunks).unwrap(), groups());
    let mut corrupt = chunks.clone();
    corrupt[0].data[28] = 1;
    assert!(decode_wm_translation_groups(&corrupt).is_err());
    let mut corrupt = chunks.clone();
    corrupt[1].data[8] = 8;
    assert!(decode_wm_translation_groups(&corrupt).is_err());
    let mut corrupt = chunks.clone();
    corrupt[1].data[20..24].fill(0);
    assert!(decode_wm_translation_groups(&corrupt).is_err());
    let mut corrupt = chunks.clone();
    corrupt.pop();
    assert!(decode_wm_translation_groups(&corrupt).is_err());
    for index in 0..chunks.len() {
        let mut corrupt = chunks.clone();
        corrupt[index].data.pop();
        assert!(decode_wm_translation_groups(&corrupt).is_err());
    }
}
