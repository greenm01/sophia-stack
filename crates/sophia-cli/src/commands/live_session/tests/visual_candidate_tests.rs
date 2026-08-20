use super::*;
use crate::commands::live_session::PersistentLiveLayout;
use sophia_protocol::{
    BufferHandle, SurfaceConstraints, SurfacePresentationIntent, SurfacePresentationIntentKind,
    TransactionId,
};
use std::collections::BTreeMap;

fn rect(width: i32, height: i32) -> Rect {
    Rect {
        x: 0,
        y: 0,
        width,
        height,
    }
}

#[test]
fn inset_present_content_proves_the_outer_layout_extent_without_scaling() {
    let surface = SurfaceId::new(77, 1);
    let buffer = BufferHandle::from_raw(770);
    let outer = rect(1276, 1422);
    let content = Size {
        width: 1266,
        height: 1412,
    };
    let transaction = SurfaceTransaction {
        transaction: TransactionId::from_raw(77),
        authority: AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry: outer,
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::DmaBuf {
                handle: buffer.raw(),
            },
            content,
        ),

        damage: Region::single(outer),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };
    let dma_buf_sizes = BTreeMap::from([(buffer, content)]);

    assert_eq!(
        live_transaction_observed_size(&transaction, &dma_buf_sizes, &BTreeMap::new()),
        Size {
            width: outer.width,
            height: outer.height,
        }
    );
}

#[test]
fn mismatched_present_content_cannot_prove_the_outer_layout_extent() {
    let buffer = BufferHandle::from_raw(771);
    let stale = Size {
        width: 1280,
        height: 1040,
    };
    let transaction = SurfaceTransaction {
        transaction: TransactionId::from_raw(771),
        authority: AuthorityKind::SophiaX,
        surface: SurfaceId::new(771, 1),
        namespace: None,
        target_geometry: rect(1276, 1422),
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::DmaBuf {
                handle: buffer.raw(),
            },
            sophia_protocol::Size {
                width: 1266,
                height: 1412,
            },
        ),

        damage: Region::empty(),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };
    let dma_buf_sizes = BTreeMap::from([(buffer, stale)]);

    assert_eq!(
        live_transaction_observed_size(&transaction, &dma_buf_sizes, &BTreeMap::new()),
        stale
    );
}

/// The raster size is the buffer's, even when the buffer satisfies its extent.
///
/// `live_transaction_observed_size` reports the logical extent in that case, on
/// purpose: it answers whether the surface reached its configured size. Reusing
/// it as a raster measurement put the placement back into the committed record
/// and a live session ended comparing it against the buffer -- planned
/// 1920x1080, held 1280x1440, one DMA-BUF.
#[test]
fn raster_size_reports_the_buffer_rather_than_the_configured_extent() {
    let buffer = BufferHandle::from_raw(772);
    let raster = Size {
        width: 1266,
        height: 1412,
    };
    let transaction = SurfaceTransaction {
        transaction: TransactionId::from_raw(772),
        authority: AuthorityKind::SophiaX,
        surface: SurfaceId::new(772, 1),
        namespace: None,
        target_geometry: rect(1276, 1422),
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::DmaBuf {
                handle: buffer.raw(),
            },
            raster,
        ),
        damage: Region::empty(),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };
    let dma_buf_sizes = BTreeMap::from([(buffer, raster)]);

    assert_eq!(
        live_transaction_raster_size(&transaction, &dma_buf_sizes, &BTreeMap::new()),
        raster
    );
    assert_eq!(
        live_transaction_observed_size(&transaction, &dma_buf_sizes, &BTreeMap::new()),
        Size {
            width: 1276,
            height: 1422,
        }
    );
}

#[test]
fn unresolved_x_pixmap_is_not_presented_buffer_evidence() {
    let transaction = SurfaceTransaction {
        transaction: TransactionId::from_raw(79),
        authority: AuthorityKind::SophiaX,
        surface: SurfaceId::new(79, 1),
        namespace: None,
        target_geometry: rect(500, 500),
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::XPixmap { pixmap: 0x220001 },
            sophia_protocol::Size {
                width: 500,
                height: 500,
            },
        ),

        damage: Region::single(rect(500, 500)),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };
    let mut batch =
        crate::commands::live_session::wm_update_coordinator_batch(transaction.transaction);
    batch.transactions.push(transaction.clone());

    assert_eq!(
        live_transaction_visual_evidence(&transaction, &batch),
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
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::CpuBuffer { handle: 780 },
            sophia_protocol::Size {
                width: 500,
                height: 500,
            },
        ),

        damage: Region::single(rect(500, 500)),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };
    let mut batch =
        crate::commands::live_session::wm_update_coordinator_batch(transaction.transaction);
    batch.transactions.push(transaction.clone());
    batch.software_present_submissions.push(
        sophia_x_authority::XAuthoritySoftwarePresentSubmission {
            transaction: transaction.transaction,
            surface: transaction.surface,
            acquire_fence: None,
            idle_fence: None,
        },
    );

    assert_eq!(
        live_transaction_visual_evidence(&transaction, &batch),
        sophia_engine::SurfaceVisualEvidence::PresentedBuffer
    );
}

#[test]
fn backing_snapshot_cannot_impersonate_same_transaction_present() {
    let surface = SurfaceId::new(80, 1);
    let transaction = TransactionId::from_raw(80);
    let geometry = rect(500, 500);
    let dma_buffer = BufferHandle::from_raw(800);
    let cpu_handle = 801;
    let dma = SurfaceTransaction {
        transaction,
        authority: AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry: geometry,
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::DmaBuf {
                handle: dma_buffer.raw(),
            },
            sophia_protocol::Size {
                width: geometry.width,
                height: geometry.height,
            },
        ),

        damage: Region::single(geometry),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };
    let backing = SurfaceTransaction {
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::CpuBuffer { handle: cpu_handle },
            dma.target_content_size(),
        ),
        ..dma.clone()
    };
    let mut batch = crate::commands::live_session::wm_update_coordinator_batch(transaction);
    batch.transactions.extend([dma.clone(), backing.clone()]);
    batch
        .present_submissions
        .push(sophia_x_authority::XAuthorityPresentSubmission {
            transaction,
            surface,
            buffer: dma_buffer,
            x_offset: 0,
            y_offset: 0,
            acquire_fence: None,
            idle_fence: None,
        });

    assert_eq!(
        live_transaction_visual_evidence(&dma, &batch),
        sophia_engine::SurfaceVisualEvidence::PresentedBuffer
    );
    assert_eq!(
        live_transaction_visual_evidence(&backing, &batch),
        sophia_engine::SurfaceVisualEvidence::BackingSnapshot
    );

    let mut layout = PersistentLiveLayout::default();
    let extent = Size {
        width: geometry.width,
        height: geometry.height,
    };
    layout.dma_buf_sizes.insert(dma_buffer, extent);
    layout.cpu_buffer_sizes.insert(cpu_handle, extent);
    layout.observe_authority_batch(&batch);
    assert_eq!(
        layout.layout_epochs.safe_observation(surface),
        Some(sophia_engine::SafeSurfaceObservation {
            candidate: Some(dma.key()),
            extent,
            evidence: sophia_engine::SurfaceVisualEvidence::PresentedBuffer,
            sequence: 1,
        })
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
        surface_kind: sophia_protocol::LayoutNodeKind::Toplevel,
        placement_preference: sophia_protocol::SurfacePlacementPreference::Default,
        presentation_owner: None,
        stack_rank: 0,
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
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::DmaBuf {
                handle: present_buffer.raw(),
            },
            sophia_protocol::Size {
                width: initial.width,
                height: initial.height,
            },
        ),

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
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::CpuBuffer {
                handle: backing_handle,
            },
            sophia_protocol::Size {
                width: tiled.width,
                height: tiled.height,
            },
        ),

        damage: Region::single(tiled),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 1,
    });
    layout.observe_authority_batch(&backing);

    let selected = layout.layout_epochs.safe_observation(surface).unwrap();
    assert_eq!(
        selected.candidate,
        Some(sophia_protocol::SurfaceTransactionKey {
            transaction: present_id,
            surface,
            target_buffer: BufferSource::DmaBuf {
                handle: present_buffer.raw(),
            },
        })
    );
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
