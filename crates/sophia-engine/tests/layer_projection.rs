use sophia_engine::layer_templates_from_surface_transactions;
use sophia_protocol::{
    AuthorityKind, BufferSource, NamespaceId, Rect, Region, SurfaceId, SurfaceTransaction,
    SurfaceTransactionReadiness, TransactionId,
};

#[test]
fn authority_transaction_template_preserves_namespace_and_order() {
    let transactions = [3_u32, 7].map(|index| SurfaceTransaction {
        transaction: TransactionId::from_raw(1),
        authority: AuthorityKind::SophiaX,
        surface: SurfaceId::new(index, 1),
        namespace: Some(NamespaceId::from_raw(u64::from(index))),
        target_geometry: Rect {
            x: i32::try_from(index).expect("small test index"),
            y: 0,
            width: 64,
            height: 64,
        },
        presentation_extent: sophia_protocol::Size {
            width: 64,
            height: 64,
        },
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::CpuBuffer {
                handle: u64::from(index),
            },
            sophia_protocol::Size {
                width: 64,
                height: 64,
            },
        ),

        damage: Region::empty(),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    });

    let templates = layer_templates_from_surface_transactions(&transactions);

    assert_eq!(templates.len(), 2);
    assert_eq!(templates[0].namespace, Some(NamespaceId::from_raw(3)));
    assert_eq!(templates[0].stack_rank, 0);
    assert_eq!(templates[1].namespace, Some(NamespaceId::from_raw(7)));
    assert_eq!(templates[1].stack_rank, 1);
    assert_eq!(templates[1].source, BufferSource::None);
}
