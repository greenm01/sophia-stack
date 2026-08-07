use sophia_protocol::{
    SOPHIA_WM_CAPABILITY_ACTIONS, SOPHIA_WM_CAPABILITY_BINDINGS, TransactionId, WmV1ClientHello,
    WmV1ProjectionBegin, WmV1ProjectionChunk, WmV1ProjectionEnd, WmV1SnapshotBegin,
    WmV1SnapshotChunk, WmV1SnapshotEnd,
};
use sophia_runtime::{
    PolicyConnectionState, PolicySnapshotAssembler, PolicyTransferError, QueuedPolicyProjection,
};

#[test]
fn negotiation_selects_the_shared_revision_and_capabilities() {
    let mut connection = PolicyConnectionState::default();
    connection.connect(7).unwrap();
    let welcome = connection
        .negotiate(&WmV1ClientHello {
            minimum_revision: 1,
            maximum_revision: 3,
            capabilities: SOPHIA_WM_CAPABILITY_BINDINGS | SOPHIA_WM_CAPABILITY_ACTIONS | (1 << 63),
        })
        .unwrap();

    assert_eq!(welcome.selected_revision, 1);
    assert_eq!(welcome.connection_epoch, 7);
    assert_eq!(
        welcome.capabilities,
        SOPHIA_WM_CAPABILITY_BINDINGS | SOPHIA_WM_CAPABILITY_ACTIONS
    );
    assert_eq!(welcome.max_outputs, 16);
    assert_eq!(welcome.max_surfaces, 1024);
    assert_eq!(welcome.max_bindings, 256);
}

#[test]
fn complete_projection_is_admitted_only_while_its_connection_epoch_is_active() {
    let mut connection = negotiated_connection(1);
    let transaction = TransactionId::from_raw(11);
    connection
        .begin_projection(transaction, projection_begin(1))
        .unwrap();
    connection
        .append_projection_chunk(transaction, projection_chunk(1, 0, 1, 1))
        .unwrap();
    connection
        .append_projection_chunk(transaction, projection_chunk(1, 1, 2, 2))
        .unwrap();
    connection
        .finish_projection(transaction, projection_end(1))
        .unwrap();
    connection.disconnect().unwrap();
    connection.connect(2).unwrap();
    connection
        .negotiate(&WmV1ClientHello {
            minimum_revision: 1,
            maximum_revision: 1,
            capabilities: 0,
        })
        .unwrap();

    assert!(matches!(
        connection.settle_queued(),
        Some(QueuedPolicyProjection::Discarded(queued))
            if queued.connection_epoch == 1 && queued.transaction == transaction
    ));
}

#[test]
fn transaction_identity_cannot_be_reused_within_one_epoch() {
    let mut connection = negotiated_connection(3);
    let transaction = TransactionId::from_raw(19);
    connection
        .begin_projection(transaction, projection_begin(3))
        .unwrap();
    connection
        .append_projection_chunk(transaction, projection_chunk(3, 0, 1, 1))
        .unwrap();
    connection
        .append_projection_chunk(transaction, projection_chunk(3, 1, 2, 2))
        .unwrap();
    connection
        .finish_projection(transaction, projection_end(3))
        .unwrap();
    assert!(matches!(
        connection.settle_queued(),
        Some(QueuedPolicyProjection::Admitted(_))
    ));

    assert_eq!(
        connection.begin_projection(transaction, projection_begin(3)),
        Err(PolicyTransferError::ReusedTransaction)
    );
}

#[test]
fn malformed_chunk_does_not_advance_the_transfer() {
    let mut connection = negotiated_connection(5);
    let transaction = TransactionId::from_raw(23);
    connection
        .begin_projection(transaction, projection_begin(5))
        .unwrap();
    let before = connection.clone();

    assert_eq!(
        connection.append_projection_chunk(transaction, projection_chunk(5, 1, 99, 1)),
        Err(PolicyTransferError::DuplicateOrReorderedChunk)
    );
    assert_eq!(connection, before);
}

#[test]
fn snapshot_assembler_requires_exact_order_and_declared_totals() {
    let transaction = TransactionId::from_raw(29);
    let mut assembler = PolicySnapshotAssembler::new(8).unwrap();
    assembler
        .begin(
            transaction,
            WmV1SnapshotBegin {
                connection_epoch: 8,
                scene_generation: 13,
                chunk_count: 3,
                output_count: 1,
                surface_count: 2,
                binding_count: 1,
            },
        )
        .unwrap();
    for (ordinal, record_kind, item_count) in [(0, 1, 1), (1, 2, 2), (2, 3, 1)] {
        assembler
            .append(
                transaction,
                WmV1SnapshotChunk {
                    connection_epoch: 8,
                    ordinal,
                    record_kind,
                    item_count,
                    data: vec![ordinal as u8 + 1],
                },
            )
            .unwrap();
    }
    let snapshot = assembler
        .finish(
            transaction,
            WmV1SnapshotEnd {
                connection_epoch: 8,
                scene_generation: 13,
                chunk_count: 3,
            },
        )
        .unwrap();

    assert_eq!(snapshot.output_count, 1);
    assert_eq!(snapshot.surface_count, 2);
    assert_eq!(snapshot.binding_count, 1);
    assert_eq!(snapshot.chunks.len(), 3);
}

fn negotiated_connection(epoch: u64) -> PolicyConnectionState {
    let mut connection = PolicyConnectionState::default();
    connection.connect(epoch).unwrap();
    connection
        .negotiate(&WmV1ClientHello {
            minimum_revision: 1,
            maximum_revision: 1,
            capabilities: 0,
        })
        .unwrap();
    connection
}

fn projection_begin(epoch: u64) -> WmV1ProjectionBegin {
    WmV1ProjectionBegin {
        connection_epoch: epoch,
        request_id: 17,
        base_generation: 41,
        chunk_count: 2,
        output_count: 1,
        placement_count: 2,
    }
}

fn projection_chunk(
    epoch: u64,
    ordinal: u16,
    record_kind: u16,
    item_count: u32,
) -> WmV1ProjectionChunk {
    WmV1ProjectionChunk {
        connection_epoch: epoch,
        ordinal,
        record_kind,
        item_count,
        data: vec![record_kind as u8],
    }
}

fn projection_end(epoch: u64) -> WmV1ProjectionEnd {
    WmV1ProjectionEnd {
        connection_epoch: epoch,
        request_id: 17,
        base_generation: 41,
        chunk_count: 2,
    }
}
