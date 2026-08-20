use std::collections::BTreeMap;

use sophia_cli::resize_transaction::{
    PendingLayoutGeometryAuthority, PendingLayoutObservationMerge, ResizeRollbackCoordinator,
    ResizeVisualCommit, ResizeVisualCommitTracker, merge_unrequested_layout_observation,
    present_pixels_conflict_with_requested_sizes, project_authority_batch_onto_layout,
};

fn visual_candidate(
    transaction: TransactionId,
    surface: SurfaceId,
) -> sophia_protocol::SurfaceTransactionKey {
    sophia_protocol::SurfaceTransactionKey {
        transaction,
        surface,
        target_buffer: BufferSource::DmaBuf {
            handle: transaction.raw(),
        },
    }
}

#[test]
fn resize_target_is_not_visually_committed_until_its_exact_present_retires() {
    let surface = SurfaceId::new(70, 1);
    let transaction = TransactionId::from_raw(700);
    let launch = size(1280, 1040);
    let target = size(1276, 1422);
    let mut coordinator = ResizeRollbackCoordinator::default();
    let mut visual = ResizeVisualCommitTracker::default();
    coordinator.record_committed(surface, launch);
    coordinator.set_pending_target(surface, target);
    let candidate = visual_candidate(transaction, surface);
    visual
        .arm(ResizeVisualCommit {
            candidate,
            size: target,
            layout_size: target,
        })
        .unwrap();

    assert_eq!(coordinator.committed_size(surface), Some(launch));
    assert_eq!(coordinator.pending_target(surface), Some(target));
    assert!(visual.surface_awaiting(surface));
    assert!(visual.exact_candidate(candidate, target));
    assert!(!visual.exact_candidate(candidate, launch));
    assert!(!visual.exact_candidate(
        visual_candidate(TransactionId::from_raw(701), surface),
        target,
    ));
    assert_eq!(visual.complete(candidate, launch), None);

    let committed = visual.complete(candidate, target).unwrap();
    coordinator.record_committed(committed.candidate.surface, committed.layout_size);
    assert_eq!(coordinator.committed_size(surface), Some(target));
    assert_eq!(coordinator.pending_target(surface), None);
    assert!(visual.is_empty());
}

#[test]
fn removing_a_surface_cancels_only_its_awaiting_visual_commits() {
    let first = SurfaceId::new(71, 1);
    let second = SurfaceId::new(72, 1);
    let target = size(1276, 1422);
    let mut visual = ResizeVisualCommitTracker::default();
    for (raw, surface) in [(701, first), (702, second)] {
        visual
            .arm(ResizeVisualCommit {
                candidate: visual_candidate(TransactionId::from_raw(raw), surface),
                size: target,
                layout_size: target,
            })
            .unwrap();
    }

    assert_eq!(visual.remove_surface(first), 1);
    assert!(!visual.surface_awaiting(first));
    assert!(visual.surface_awaiting(second));
    assert_eq!(visual.len(), 1);
}

#[test]
fn one_layout_transaction_tracks_multiple_surface_retirements_independently() {
    let transaction = TransactionId::from_raw(920);
    let firefox = SurfaceId::new(92, 1);
    let kitty = SurfaceId::new(93, 1);
    let size = Size {
        width: 1276,
        height: 1422,
    };
    let mut tracker = ResizeVisualCommitTracker::default();
    let firefox_candidate = visual_candidate(transaction, firefox);
    let kitty_candidate = visual_candidate(transaction, kitty);
    tracker
        .arm(ResizeVisualCommit {
            candidate: firefox_candidate,
            size,
            layout_size: size,
        })
        .unwrap();
    tracker
        .arm(ResizeVisualCommit {
            candidate: kitty_candidate,
            size,
            layout_size: size,
        })
        .unwrap();

    assert_eq!(tracker.len(), 2);
    assert_eq!(
        tracker.complete(firefox_candidate, size),
        Some(ResizeVisualCommit {
            candidate: firefox_candidate,
            size,
            layout_size: size,
        })
    );
    assert!(tracker.surface_awaiting(kitty));
    assert_eq!(tracker.len(), 1);
}

#[test]
fn recovery_frame_and_one_standing_target_successor_are_bounded_independently() {
    let surface = SurfaceId::new(95, 1);
    let fallback = size(1280, 1040);
    let target = size(1276, 1422);
    let mut tracker = ResizeVisualCommitTracker::default();
    tracker
        .arm(ResizeVisualCommit {
            candidate: visual_candidate(TransactionId::from_raw(950), surface),
            size: fallback,
            layout_size: fallback,
        })
        .unwrap();
    tracker
        .arm(ResizeVisualCommit {
            candidate: visual_candidate(TransactionId::from_raw(951), surface),
            size: target,
            layout_size: target,
        })
        .unwrap();

    assert!(tracker.surface_layout_awaiting(surface, fallback));
    assert!(tracker.surface_layout_awaiting(surface, target));
    assert_eq!(tracker.len(), 2);
    // A repeated Present for the target is recognized before callers arm it;
    // the per-surface recovery chain therefore stays at two candidates.
    assert!(tracker.surface_layout_awaiting(surface, target));
}

#[test]
fn inset_content_retirement_commits_its_distinct_outer_layout_extent() {
    let surface = SurfaceId::new(94, 1);
    let candidate = visual_candidate(TransactionId::from_raw(940), surface);
    let source_size = size(1266, 1412);
    let layout_size = size(1276, 1422);
    let mut tracker = ResizeVisualCommitTracker::default();
    tracker
        .arm(ResizeVisualCommit {
            candidate,
            size: source_size,
            layout_size,
        })
        .unwrap();

    assert_eq!(tracker.complete(candidate, layout_size), None);
    assert_eq!(
        tracker.complete(candidate, source_size),
        Some(ResizeVisualCommit {
            candidate,
            size: source_size,
            layout_size,
        })
    );
}
use sophia_protocol::{
    AuthorityKind, BufferHandle, BufferSource, LayerSnapshot, Rect, Region, ResizeSyncCapability,
    Size, SurfaceId, SurfaceTransaction, SurfaceTransactionReadiness, TransactionId, Transform,
};
use sophia_x_authority::{
    XAuthorityCpuBufferSnapshot, XAuthorityCpuBufferUpdate, XAuthorityObservedTransactionBatch,
    XAuthorityPresentSubmission, XResourceId,
};

fn size(width: i32, height: i32) -> Size {
    Size { width, height }
}

fn layer(surface: SurfaceId, generation: u64) -> LayerSnapshot {
    LayerSnapshot {
        surface,
        authority_local_id: None,
        namespace: None,
        stack_rank: 0,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        },
        source: BufferSource::CpuBuffer {
            handle: surface.index() as u64,
        },
        source_size: Size {
            width: 640,
            height: 480,
        },
        damage: Region::empty(),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    }
}

#[test]
fn successful_resize_advances_the_committed_size() {
    let surface = SurfaceId::new(1, 1);
    let mut coordinator = ResizeRollbackCoordinator::default();
    coordinator.record_committed(surface, size(800, 600));
    assert!(coordinator.accept_observation(surface, size(1024, 768)));
    coordinator.record_committed(surface, size(1024, 768));
    assert_eq!(coordinator.committed_size(surface), Some(size(1024, 768)));
    assert!(!coordinator.rollback_pending(surface));
}

#[test]
fn timeout_builds_a_compensating_configure_from_committed_state() {
    let surface = SurfaceId::new(2, 1);
    let mut coordinator = ResizeRollbackCoordinator::default();
    coordinator.record_committed(surface, size(800, 600));
    let rollback = coordinator
        .begin_rollback([(surface, size(1024, 768))])
        .unwrap();
    assert_eq!(rollback.len(), 1);
    assert_eq!(rollback[0].surface, surface);
    assert_eq!(rollback[0].size, size(800, 600));
    assert!(rollback[0].transaction.raw() >= 1 << 63);
    assert!(coordinator.rollback_pending(surface));
    assert!(!coordinator.request_allowed(surface, size(1024, 768)));
    assert!(coordinator.request_allowed(surface, size(1280, 720)));
}

#[test]
fn late_abandoned_pixels_are_fenced_until_rollback_confirmation() {
    let surface = SurfaceId::new(3, 1);
    let mut coordinator = ResizeRollbackCoordinator::default();
    coordinator.record_committed(surface, size(800, 600));
    coordinator
        .begin_rollback([(surface, size(1024, 768))])
        .unwrap();
    assert!(!coordinator.accept_observation(surface, size(1024, 768)));
    assert!(!coordinator.request_allowed(surface, size(1024, 768)));
    assert!(coordinator.accept_observation(surface, size(800, 600)));
    assert!(!coordinator.rollback_pending(surface));
    assert!(coordinator.request_allowed(surface, size(1024, 768)));
}

#[test]
fn disconnect_cleans_committed_and_rollback_state() {
    let surface = SurfaceId::new(4, 1);
    let mut coordinator = ResizeRollbackCoordinator::default();
    coordinator.record_committed(surface, size(800, 600));
    coordinator
        .begin_rollback([(surface, size(1024, 768))])
        .unwrap();
    coordinator.remove(surface);
    assert_eq!(coordinator.committed_size(surface), None);
    assert!(!coordinator.rollback_pending(surface));
    assert!(coordinator.request_allowed(surface, size(1024, 768)));
    assert!(coordinator.rollback_surfaces().next().is_none());
}

#[test]
fn resize_projection_preserves_generation_chain_and_cpu_updates() {
    let surface = SurfaceId::new(5, 1);
    let transaction = SurfaceTransaction {
        transaction: TransactionId::from_raw(118),
        authority: AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry: Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 800,
        },
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::CpuBuffer { handle: 9 },
            sophia_protocol::Size {
                width: 640,
                height: 800,
            },
        ),

        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 800,
        }),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 90,
    };
    let update = XAuthorityCpuBufferUpdate::Replace(XAuthorityCpuBufferSnapshot {
        handle: 9,
        drawable: XResourceId::new(9, 1),
        size: size(640, 800),
        stride: 2_560,
        format: u32::from_le_bytes(*b"XR24"),
        generation: 91,
        bytes: vec![1; 640 * 800 * 4],
    });
    let batch = XAuthorityObservedTransactionBatch {
        client: None,
        admission: None,
        transaction: transaction.transaction,
        transactions: vec![transaction.clone()],
        surface_presentations: Vec::new(),
        presentation_intents: Vec::new(),
        removed_surfaces: Vec::new(),
        surface_output_reservations: Vec::new(),
        cpu_buffer_updates: vec![update.clone()],
        raster_responses: Vec::new(),
        dma_buf_registrations: Vec::new(),
        fence_registrations: Vec::new(),
        present_submissions: Vec::new(),
        software_present_submissions: Vec::new(),
        released_dma_bufs: Vec::new(),
        released_fences: Vec::new(),
        protocol_errors: Vec::new(),
        expected_protocol_errors: Vec::new(),
        metadata: Vec::new(),
        selection_owner_change: false,
        selection_conversion: false,
    };
    let committed_geometry = Rect {
        x: 20,
        y: 30,
        width: 1280,
        height: 800,
    };
    let layers = BTreeMap::from([(
        surface,
        LayerSnapshot {
            surface,
            authority_local_id: None,
            namespace: None,
            stack_rank: 0,
            geometry: committed_geometry,
            source_size: Size {
                width: (committed_geometry).width,
                height: (committed_geometry).height,
            },
            source: transaction.target_buffer(),
            damage: Region::empty(),
            opacity: 1.0,
            crop: None,
            transform: Transform::IDENTITY,
            generation: 90,
            resize_sync: ResizeSyncCapability::ImplicitOnly,
        },
    )]);

    let projected = project_authority_batch_onto_layout(batch, &layers);

    assert_eq!(projected.transaction, TransactionId::from_raw(118));
    assert_eq!(projected.transactions.len(), 1);
    assert_eq!(projected.transactions[0].previous_committed_generation, 90);
    assert_eq!(
        projected.transactions[0].target_geometry,
        committed_geometry
    );
    assert_eq!(projected.cpu_buffer_updates, vec![update]);
}

#[test]
fn wrong_size_present_conflicts_but_matching_pixels_can_enter_resize_quarantine() {
    let surface = SurfaceId::new(6, 1);
    let handle = BufferHandle::from_raw(12);
    let requested = BTreeMap::from([(surface, size(2560, 1440))]);
    let mut buffers = BTreeMap::from([(handle, size(1280, 1440))]);
    let mut batch = XAuthorityObservedTransactionBatch {
        client: None,
        admission: None,
        transaction: TransactionId::from_raw(119),
        transactions: Vec::new(),
        surface_presentations: Vec::new(),
        presentation_intents: Vec::new(),
        removed_surfaces: Vec::new(),
        surface_output_reservations: Vec::new(),
        cpu_buffer_updates: Vec::new(),
        raster_responses: Vec::new(),
        dma_buf_registrations: Vec::new(),
        fence_registrations: Vec::new(),
        present_submissions: Vec::new(),
        software_present_submissions: Vec::new(),
        released_dma_bufs: Vec::new(),
        released_fences: Vec::new(),
        protocol_errors: Vec::new(),
        expected_protocol_errors: Vec::new(),
        metadata: Vec::new(),
        selection_owner_change: false,
        selection_conversion: false,
    };
    batch.present_submissions.push(XAuthorityPresentSubmission {
        transaction: TransactionId::from_raw(120),
        surface,
        buffer: handle,
        x_offset: 0,
        y_offset: 0,
        acquire_fence: None,
        idle_fence: None,
    });

    assert!(present_pixels_conflict_with_requested_sizes(
        &requested, &buffers, &batch
    ));

    buffers.insert(handle, size(2560, 1440));
    assert!(!present_pixels_conflict_with_requested_sizes(
        &requested, &buffers, &batch
    ));
}

#[test]
fn pending_layout_retains_surface_admitted_during_resize() {
    let existing = SurfaceId::new(7, 1);
    let admitted = SurfaceId::new(8, 1);
    let mut pending = vec![layer(existing, 1)];
    let requested = BTreeMap::from([(existing, size(1280, 720))]);

    assert_eq!(
        merge_unrequested_layout_observation(
            &mut pending,
            &requested,
            layer(admitted, 1),
            PendingLayoutGeometryAuthority::Layout,
        ),
        PendingLayoutObservationMerge::Inserted
    );
    assert!(
        pending
            .iter()
            .any(|candidate| candidate.surface == admitted)
    );
}

#[test]
fn pending_layout_updates_unowned_pixels_without_overwriting_resize_owned_state() {
    let surface = SurfaceId::new(9, 1);
    let resized = SurfaceId::new(10, 1);
    let mut pending = vec![layer(surface, 1), layer(resized, 1)];
    pending[0].stack_rank = 7;
    pending[0].geometry.x = 1280;
    let requested = BTreeMap::from([(resized, size(1280, 720))]);
    let mut observed = layer(surface, 2);
    observed.geometry.x = 80;
    observed.source = BufferSource::CpuBuffer { handle: 99 };

    assert_eq!(
        merge_unrequested_layout_observation(
            &mut pending,
            &requested,
            observed,
            PendingLayoutGeometryAuthority::Layout,
        ),
        PendingLayoutObservationMerge::Merged
    );
    let merged = pending
        .iter()
        .find(|candidate| candidate.surface == surface)
        .unwrap();
    assert_eq!(merged.generation, 2);
    assert_eq!(merged.source, BufferSource::CpuBuffer { handle: 99 });
    assert_eq!(merged.geometry.x, 1280);
    assert_eq!(merged.stack_rank, 7);
    assert_eq!(
        merge_unrequested_layout_observation(
            &mut pending,
            &requested,
            layer(resized, 2),
            PendingLayoutGeometryAuthority::Layout,
        ),
        PendingLayoutObservationMerge::ResizeOwned
    );
    assert_eq!(
        pending
            .iter()
            .find(|candidate| candidate.surface == resized)
            .unwrap()
            .generation,
        1
    );
}

#[test]
fn pending_layout_accepts_authority_owned_client_positioned_geometry() {
    let surface = SurfaceId::new(11, 1);
    let mut pending = vec![layer(surface, 1)];
    pending[0].geometry.x = 20;
    let mut observed = layer(surface, 2);
    observed.geometry.x = 40;

    assert_eq!(
        merge_unrequested_layout_observation(
            &mut pending,
            &BTreeMap::new(),
            observed,
            PendingLayoutGeometryAuthority::Observation,
        ),
        PendingLayoutObservationMerge::Merged
    );
    assert_eq!(pending[0].geometry.x, 40);
    assert_eq!(pending[0].generation, 2);
}
