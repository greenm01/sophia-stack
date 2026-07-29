use super::*;
use crate::commands::live_session::PersistentLiveLayout;
use sophia_protocol::{
    BufferHandle, SurfaceConstraints, SurfacePresentationIntent, SurfacePresentationIntentKind,
    TransactionId,
};

fn rect(width: i32, height: i32) -> Rect {
    Rect {
        x: 0,
        y: 0,
        width,
        height,
    }
}

#[test]
fn unresolved_x_pixmap_is_not_presented_buffer_evidence() {
    let transaction = SurfaceTransaction {
        transaction: TransactionId::from_raw(79),
        authority: AuthorityKind::SophiaX,
        surface: SurfaceId::new(79, 1),
        namespace: None,
        target_geometry: rect(500, 500),
        target_buffer: BufferSource::XPixmap { pixmap: 0x220001 },
        damage: Region::single(rect(500, 500)),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };

    assert_eq!(
        live_transaction_visual_evidence(&transaction, false),
        sophia_engine::SurfaceVisualEvidence::BackingSnapshot
    );
}

#[test]
fn presented_cpu_snapshot_is_complete_present_evidence() {
    let transaction = SurfaceTransaction {
        transaction: TransactionId::from_raw(78),
        authority: AuthorityKind::SophiaX,
        surface: SurfaceId::new(78, 1),
        namespace: None,
        target_geometry: rect(500, 500),
        target_buffer: BufferSource::CpuBuffer { handle: 780 },
        damage: Region::single(rect(500, 500)),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };

    assert_eq!(
        live_transaction_visual_evidence(&transaction, true),
        sophia_engine::SurfaceVisualEvidence::PresentedBuffer
    );
}

#[test]
fn present_candidate_is_not_replaced_by_later_blank_backing_extent() {
    let surface = SurfaceId::new(81, 1);
    let initial = rect(500, 500);
    let tiled = rect(1276, 1422);
    let mut intent =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(80));
    intent.presentation_intents.push(SurfacePresentationIntent {
        surface,
        kind: SurfacePresentationIntentKind::Request,
        role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
        geometry: initial,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        generation: 1,
    });
    let mut layout = PersistentLiveLayout::default();
    layout.observe_authority_batch(&intent);

    let present_id = TransactionId::from_raw(81);
    let present_buffer = BufferHandle::from_raw(810);
    layout.dma_buf_sizes.insert(
        present_buffer,
        Size {
            width: initial.width,
            height: initial.height,
        },
    );
    let mut present = crate::commands::live_session::wm_update_coordinator_batch(present_id);
    present.transactions.push(SurfaceTransaction {
        transaction: present_id,
        authority: AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry: initial,
        target_buffer: BufferSource::DmaBuf {
            handle: present_buffer.raw(),
        },
        damage: Region::single(initial),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    });
    present
        .present_submissions
        .push(sophia_x_authority::XAuthorityPresentSubmission {
            transaction: present_id,
            surface,
            buffer: present_buffer,
            x_offset: 0,
            y_offset: 0,
            acquire_fence: None,
            idle_fence: None,
        });
    layout.observe_authority_batch(&present);

    let backing_handle = 820;
    layout.cpu_buffer_sizes.insert(
        backing_handle,
        Size {
            width: tiled.width,
            height: tiled.height,
        },
    );
    let mut backing =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(82));
    backing.transactions.push(SurfaceTransaction {
        transaction: TransactionId::from_raw(82),
        authority: AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry: tiled,
        target_buffer: BufferSource::CpuBuffer {
            handle: backing_handle,
        },
        damage: Region::single(tiled),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 1,
    });
    layout.observe_authority_batch(&backing);

    let selected = layout.layout_epochs.safe_observation(surface).unwrap();
    assert_eq!(selected.transaction, Some(present_id));
    assert_eq!(selected.extent.width, initial.width);
    assert_eq!(selected.extent.height, initial.height);
    assert_eq!(
        selected.evidence,
        sophia_engine::SurfaceVisualEvidence::PresentedBuffer
    );
    assert!(
        layout
            .selected_pre_admission_transaction(
                surface,
                Size {
                    width: tiled.width,
                    height: tiled.height,
                },
            )
            .is_none()
    );
    let recovery = layout
        .layout_epochs
        .begin_recovery(
            [(
                surface,
                Size {
                    width: tiled.width,
                    height: tiled.height,
                },
            )],
            [surface],
        )
        .unwrap();
    assert_eq!(recovery[0].size, selected.extent);
}
