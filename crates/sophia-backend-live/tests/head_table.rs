use sophia_backend_live::{
    LiveNativeHeadRecord, LiveNativeHeadTableError, LiveProductionNativeHeadTable,
};
use sophia_engine::RenderHeadId;
use sophia_protocol::OutputId;

fn record(head: u64, card_index: usize, connector_id: u32, crtc_id: u32) -> LiveNativeHeadRecord {
    LiveNativeHeadRecord {
        head: RenderHeadId::from_raw(head),
        output: OutputId::from_raw(1),
        card_index,
        connector_id,
        crtc_id,
        connector_name: format!("DP-{connector_id}"),
    }
}

#[test]
fn head_table_translates_native_identity_to_heads() {
    let table = LiveProductionNativeHeadTable::from_records([
        record(1, 0, 94, 71),
        record(2, 0, 102, 72),
        record(3, 1, 94, 71),
    ])
    .unwrap();

    assert_eq!(table.len(), 3);
    assert_eq!(table.crtc_to_head(0, 72), Some(RenderHeadId::from_raw(2)));
    // Card identity participates in the key: two cards legitimately reuse
    // one connector id, and collapsing them would route a flip to the wrong
    // screen.
    assert_eq!(
        table.connector_to_head(1, 94),
        Some(RenderHeadId::from_raw(3))
    );
    assert_eq!(
        table.connector_to_head(0, 94),
        Some(RenderHeadId::from_raw(1))
    );
    assert_eq!(table.crtc_to_head(0, 99), None);
    assert_eq!(
        table
            .head(RenderHeadId::from_raw(2))
            .map(|record| record.connector_name.as_str()),
        Some("DP-102")
    );
}

#[test]
fn head_table_rejects_invalid_and_duplicate_identity() {
    let mut table = LiveProductionNativeHeadTable::new();
    table.admit(record(1, 0, 94, 71)).unwrap();

    assert_eq!(
        table.admit(LiveNativeHeadRecord {
            head: RenderHeadId::INVALID,
            ..record(0, 0, 95, 73)
        }),
        Err(LiveNativeHeadTableError::InvalidHead)
    );
    assert_eq!(
        table.admit(record(1, 0, 95, 73)),
        Err(LiveNativeHeadTableError::DuplicateHead {
            head: RenderHeadId::from_raw(1)
        })
    );
    assert_eq!(
        table.admit(record(2, 0, 94, 73)),
        Err(LiveNativeHeadTableError::DuplicateConnector {
            card_index: 0,
            connector_id: 94
        })
    );
    assert_eq!(table.len(), 1);
}

#[test]
fn head_table_removal_forgets_the_physical_mapping() {
    let mut table = LiveProductionNativeHeadTable::from_records([record(1, 0, 94, 71)]).unwrap();

    let removed = table.remove(RenderHeadId::from_raw(1)).unwrap();
    assert_eq!(removed.connector_id, 94);
    assert!(table.is_empty());
    assert_eq!(table.crtc_to_head(0, 71), None);
    assert_eq!(table.remove(RenderHeadId::from_raw(1)), None);
}
