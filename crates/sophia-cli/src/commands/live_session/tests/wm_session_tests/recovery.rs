#[test]
fn fallback_admission_releases_recovery_before_the_standing_target_commits() {
    let surface = SurfaceId::new(69, 1);
    let fallback = Size {
        width: 480,
        height: 281,
    };
    let target = Size {
        width: 1276,
        height: 709,
    };
    let mut layout = PersistentLiveLayout::default();
    layout.layout_epochs.record_committed(surface, fallback);
    layout.layout_epochs.set_recovery_extent(surface, fallback);
    layout.layout_epochs.set_pending_target(surface, target);

    assert!(layout.release_recovery_extent(surface, "test_fallback_retired"));
    assert_eq!(layout.layout_epochs.recovery_extent(surface), None);
    assert_eq!(layout.layout_epochs.pending_target(surface), Some(target));
    assert!(layout.constraint_relayout_required());

    assert!(!layout.complete_visual_commit(
        dma_candidate(
            TransactionId::from_raw(690),
            surface,
            BufferHandle::from_raw(690),
        ),
        fallback,
    ));
    assert_eq!(layout.layout_epochs.recovery_extent(surface), None);
    assert_eq!(layout.layout_epochs.pending_target(surface), Some(target));

    let target_candidate = dma_candidate(
        TransactionId::from_raw(691),
        surface,
        BufferHandle::from_raw(691),
    );
    layout
        .awaiting_visual_commits
        .arm(ResizeVisualCommit {
            candidate: target_candidate,
            size: target,
            layout_size: target,
        })
        .unwrap();
    assert!(layout.complete_visual_commit(target_candidate, target));
    assert_eq!(layout.layout_epochs.committed_size(surface), Some(target));
    assert_eq!(layout.layout_epochs.pending_target(surface), None);
    assert_eq!(layout.layout_epochs.recovery_extent(surface), None);
    assert_eq!(
        layout.layout_epochs.effective_constraints(surface),
        SurfaceConstraints {
            min_size: None,
            max_size: None,
        }
    );
    assert!(layout.constraint_relayout_required());
}

#[test]
fn inset_present_retires_the_standing_outer_target_and_releases_recovery() {
    let surface = SurfaceId::new(72, 1);
    let buffer = sophia_protocol::BufferHandle::from_raw(720);
    let fallback = Size {
        width: 1280,
        height: 1040,
    };
    let outer = Size {
        width: 1276,
        height: 1422,
    };
    let content = Size {
        width: 1266,
        height: 1412,
    };
    let geometry = Rect {
        x: 642,
        y: 16,
        width: outer.width,
        height: outer.height,
    };
    let transaction_id = TransactionId::from_raw(720);
    let transaction = SurfaceTransaction {
        transaction: transaction_id,
        authority: sophia_protocol::AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry: geometry,
        content: sophia_protocol::SurfaceContentSet::singleton(BufferSource::DmaBuf {
            handle: buffer.raw(),
        }, content),

        damage: Region::single(geometry),
        readiness: sophia_protocol::SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 1,
    };
    let candidate = transaction.key();
    let mut batch = crate::commands::live_session::wm_update_coordinator_batch(transaction_id);
    batch.transactions.push(transaction);
    batch
        .present_submissions
        .push(sophia_x_authority::XAuthorityPresentSubmission {
            transaction: transaction_id,
            surface,
            buffer,
            x_offset: 5,
            y_offset: 5,
            acquire_fence: None,
            idle_fence: None,
        });

    let mut layout = PersistentLiveLayout::default();
    layout.dma_buf_sizes.insert(buffer, content);
    layout.layout_epochs.record_committed(surface, fallback);
    layout
        .layout_epochs
        .set_admission(surface, sophia_engine::SurfaceAdmissionState::Managed);
    layout.layout_epochs.set_recovery_extent(surface, fallback);
    layout.layout_epochs.set_pending_target(surface, outer);
    let fallback_candidate = dma_candidate(
        TransactionId::from_raw(719),
        surface,
        BufferHandle::from_raw(719),
    );
    layout
        .awaiting_visual_commits
        .arm(ResizeVisualCommit {
            candidate: fallback_candidate,
            size: fallback,
            layout_size: fallback,
        })
        .unwrap();
    layout.observe_authority_batch(&batch);

    assert!(
        layout
            .awaiting_visual_commits
            .exact_candidate(candidate, content)
    );
    assert_eq!(layout.awaiting_visual_commits.len(), 2);
    layout.observe_authority_batch(&batch);
    assert_eq!(layout.awaiting_visual_commits.len(), 2);
    assert!(layout.complete_visual_commit(fallback_candidate, fallback));
    assert_eq!(layout.layout_epochs.pending_target(surface), Some(outer));
    assert!(layout.release_recovery_extent(surface, "test_fallback_retired"));
    assert!(layout.complete_visual_commit(candidate, content));
    assert_eq!(layout.layout_epochs.committed_size(surface), Some(outer));
    assert_eq!(layout.layout_epochs.pending_target(surface), None);
    assert_eq!(layout.layout_epochs.recovery_extent(surface), None);
}

#[test]
fn unarmed_target_without_a_recovery_extent_cannot_bypass_the_layout_epoch() {
    let surface = SurfaceId::new(70, 1);
    let target = Size {
        width: 1276,
        height: 709,
    };
    let mut layout = PersistentLiveLayout::default();
    layout.layout_epochs.set_pending_target(surface, target);

    assert!(!layout.complete_visual_commit(
        dma_candidate(
            TransactionId::from_raw(700),
            surface,
            BufferHandle::from_raw(700),
        ),
        target,
    ));
    assert_eq!(layout.layout_epochs.committed_size(surface), None);
    assert_eq!(layout.layout_epochs.pending_target(surface), Some(target));
    assert!(!layout.constraint_relayout_required());
}

/// A recovery extent must not outlive the layout it was captured against.
///
/// It records the pixels a client had already produced on the output it was
/// then on. A topology change can move that surface to a smaller output, where
/// the extent cannot be satisfied at all and constraint reconciliation fails
/// the session rather than shrinking the surface: a mixed mirror-plus-extended
/// commit put a 1280x1440 extent onto a 1920x1080 output and ended a live
/// session.
#[test]
fn a_topology_change_releases_every_recovery_extent() {
    let tall = SurfaceId::new(70, 1);
    let short = SurfaceId::new(71, 1);
    let tall_extent = Size {
        width: 1280,
        height: 1440,
    };
    let short_extent = Size {
        width: 640,
        height: 480,
    };
    let mut layout = PersistentLiveLayout::default();
    layout.layout_epochs.record_committed(tall, tall_extent);
    layout.layout_epochs.set_recovery_extent(tall, tall_extent);
    layout.layout_epochs.record_committed(short, short_extent);
    layout.layout_epochs.set_recovery_extent(short, short_extent);

    assert_eq!(layout.release_recovery_extents_for_topology(), 2);
    assert_eq!(layout.layout_epochs.recovery_extent(tall), None);
    assert_eq!(layout.layout_epochs.recovery_extent(short), None);
    // The release is what schedules the relayout the new topology needs.
    assert!(layout.constraint_relayout_required());

    // Nothing left to release, and no spurious relayout demanded.
    let mut settled = PersistentLiveLayout::default();
    settled.layout_epochs.record_committed(tall, tall_extent);
    assert_eq!(settled.release_recovery_extents_for_topology(), 0);
    assert!(!settled.constraint_relayout_required());
}
