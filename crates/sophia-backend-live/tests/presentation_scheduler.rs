#![cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]

use std::fs::File;
use std::os::fd::OwnedFd;
use std::time::{Duration, Instant};

use sophia_backend_live::{
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LivePresentationResourceSession,
    LiveProductionAuthorityBatch, LiveProductionAuthorityGroup, LiveProductionPresentDisposition,
    LiveProductionPresentGate, LiveProductionPresentScheduler, LiveProductionPresentSubmission,
    LiveProductionSubmittedPresent, LiveRetainedDmaBufLayer,
};
use sophia_engine::{HeadlessEngine, ProductionSessionCoordinator};
use sophia_protocol::{
    AuthorityKind, BufferHandle, BufferSource, CommittedSurfaceState, DRM_FORMAT_MOD_INVALID,
    DmaBufDescriptor, DmaBufPlaneDescriptor, Rect, Region, Size, SurfaceId, SurfaceTransaction,
    SurfaceTransactionReadiness, TransactionId, Transform,
};
use sophia_renderer_live::{
    LiveCompositionPlacement, LiveOwnedDmaBufPlane, LiveOwnedMultiPlaneDmaBufFrame,
    LiveRendererImageId,
};

fn fd() -> OwnedFd {
    File::open("/dev/null").unwrap().into()
}

fn descriptor(handle: BufferHandle) -> DmaBufDescriptor {
    DmaBufDescriptor {
        handle,
        size: Size {
            width: 64,
            height: 48,
        },
        format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        modifier: DRM_FORMAT_MOD_INVALID,
        plane_count: 1,
        planes: [
            Some(DmaBufPlaneDescriptor {
                offset: 0,
                stride: 256,
            }),
            None,
            None,
            None,
        ],
    }
}

fn scheduler_batch(
    transaction: TransactionId,
    surface: SurfaceId,
    handle: BufferHandle,
) -> LiveProductionAuthorityBatch {
    LiveProductionAuthorityBatch {
        groups: vec![LiveProductionAuthorityGroup {
            transaction,
            transactions: vec![SurfaceTransaction {
                transaction,
                authority: AuthorityKind::SophiaX,
                surface,
                namespace: None,
                target_geometry: Rect {
                    x: 0,
                    y: 0,
                    width: 64,
                    height: 48,
                },
                target_buffer: BufferSource::DmaBuf {
                    handle: handle.raw(),
                },
                damage: Region::single(Rect {
                    x: 0,
                    y: 0,
                    width: 64,
                    height: 48,
                }),
                readiness: SurfaceTransactionReadiness::Ready,
                timeout_msec: 250,
                previous_committed_generation: 0,
            }],
            removed_surfaces: Vec::new(),
            present_submissions: vec![LiveProductionPresentSubmission {
                transaction,
                surface,
                buffer: handle,
                x_offset: 0,
                y_offset: 0,
                acquire_fence: None,
                idle_fence: None,
                layout_disposition: LiveProductionPresentDisposition::Immediate,
            }],
            software_present_submissions: Vec::new(),
        }],
        dma_buf_registrations: Vec::new(),
        fence_registrations: Vec::new(),
        released_dma_bufs: Vec::new(),
        released_fences: Vec::new(),
    }
}

fn scheduler_batch_with_disposition(
    transaction: TransactionId,
    surface: SurfaceId,
    handle: BufferHandle,
    disposition: LiveProductionPresentDisposition,
) -> LiveProductionAuthorityBatch {
    let mut batch = scheduler_batch(transaction, surface, handle);
    batch.groups[0].present_submissions[0].layout_disposition = disposition;
    batch
}

fn in_flight_present(
    transaction: TransactionId,
    surface: SurfaceId,
) -> LiveProductionSubmittedPresent {
    let geometry = Rect {
        x: 0,
        y: 0,
        width: 64,
        height: 48,
    };
    let buffer = BufferSource::DmaBuf { handle: 900 };
    let production = ProductionSessionCoordinator::new(HeadlessEngine::default())
        .with_committed_surfaces(vec![CommittedSurfaceState {
            surface,
            committed_generation: 1,
            geometry,
            buffer,
            damage: Region::empty(),
        }]);
    let prepared = production.prepare_present_transaction(&SurfaceTransaction {
        transaction,
        authority: AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry: geometry,
        target_buffer: buffer,
        damage: Region::empty(),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 1,
    });
    LiveProductionSubmittedPresent {
        transaction,
        surface,
        prepared,
        displayed_layer: LiveRetainedDmaBufLayer {
            image_id: LiveRendererImageId::from_raw(901),
            frame: LiveOwnedMultiPlaneDmaBufFrame {
                width: 64,
                height: 48,
                format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                modifier: DRM_FORMAT_MOD_INVALID,
                plane_count: 1,
                planes: [
                    Some(LiveOwnedDmaBufPlane {
                        fd: fd(),
                        offset: 0,
                        stride: 256,
                    }),
                    None,
                    None,
                    None,
                ],
            },
            placement: LiveCompositionPlacement {
                target: geometry,
                clip: None,
                transform: Transform::IDENTITY,
                alpha: 1.0,
            },
        },
    }
}

#[test]
fn asynchronous_present_retains_one_transaction_until_scanout_submission() {
    let transaction = TransactionId::from_raw(902);
    let surface = SurfaceId::new(903, 1);
    let mut scheduler = LiveProductionPresentScheduler::default();
    let mut resources = LivePresentationResourceSession::default();

    scheduler.mark_rendering(in_flight_present(transaction, surface));

    assert!(scheduler.has_rendering());
    assert!(scheduler.has_in_flight());
    assert_eq!(
        scheduler.poll_gate(&mut resources, Instant::now()).unwrap(),
        LiveProductionPresentGate::SubmittedInFlight
    );
    assert_eq!(
        scheduler.promote_rendering_to_submitted(),
        Some(transaction)
    );
    assert!(!scheduler.has_rendering());
    assert!(scheduler.has_submitted());
    assert_eq!(
        scheduler
            .take_submitted()
            .map(|present| present.transaction),
        Some(transaction)
    );
    assert!(!scheduler.has_in_flight());
}

#[test]
fn production_present_scheduler_owns_delay_and_controlled_rejection_gates() {
    let handle = BufferHandle::from_raw(37);
    let transaction = TransactionId::from_raw(38);
    let surface = SurfaceId::new(39, 1);
    let mut resources = LivePresentationResourceSession::default();
    resources
        .register_source(descriptor(handle), vec![fd()])
        .unwrap();
    let mut scheduler = LiveProductionPresentScheduler::default().with_controls(
        Some(Duration::from_millis(50)),
        true,
        false,
    );
    let now = Instant::now();
    scheduler
        .enqueue_group(
            &scheduler_batch(transaction, surface, handle).groups[0],
            &[],
            Vec::new(),
            &mut resources,
            now,
        )
        .unwrap();
    assert_eq!(
        scheduler.front().map(|queued| queued.surface),
        Some(surface)
    );

    assert_eq!(
        scheduler.poll_gate(&mut resources, now).unwrap(),
        LiveProductionPresentGate::WaitingAcquire
    );
    assert_eq!(scheduler.acquire_waits(), 1);
    assert_eq!(
        scheduler
            .poll_gate(&mut resources, now + Duration::from_millis(50))
            .unwrap(),
        LiveProductionPresentGate::Reject(transaction)
    );
    assert_eq!(scheduler.controlled_rejections(), 1);
    assert!(!scheduler.has_queued());
}

#[test]
fn queued_present_rebases_offset_and_clip_to_atomic_layout() {
    let handle = BufferHandle::from_raw(47);
    let transaction = TransactionId::from_raw(48);
    let surface = SurfaceId::new(49, 1);
    let mut batch = scheduler_batch(transaction, surface, handle);
    batch.groups[0].present_submissions[0].x_offset = 3;
    batch.groups[0].present_submissions[0].y_offset = -4;
    let mut resources = LivePresentationResourceSession::default();
    resources
        .register_source(descriptor(handle), vec![fd()])
        .unwrap();
    let mut scheduler = LiveProductionPresentScheduler::default();
    scheduler
        .enqueue_group(
            &batch.groups[0],
            &[],
            Vec::new(),
            &mut resources,
            Instant::now(),
        )
        .unwrap();
    let geometry = Rect {
        x: 1280,
        y: 720,
        width: 1280,
        height: 720,
    };

    scheduler.reproject_surface(surface, geometry);

    let queued = scheduler.front().unwrap();
    assert_eq!(queued.surface_clip, geometry);
    assert_eq!(queued.candidate.target_geometry, geometry);
    assert_eq!(
        queued.target,
        Rect {
            x: 1283,
            y: 716,
            ..geometry
        }
    );
}

#[test]
fn queued_present_owns_only_its_exact_surface_transaction() {
    let kitty_handle = BufferHandle::from_raw(402);
    let kitty_transaction = TransactionId::from_raw(403);
    let kitty_surface = SurfaceId::new(404, 1);
    let mut batch = scheduler_batch(kitty_transaction, kitty_surface, kitty_handle);
    batch.groups.insert(
        0,
        LiveProductionAuthorityGroup {
            transaction: TransactionId::from_raw(198),
            transactions: vec![SurfaceTransaction {
                transaction: TransactionId::from_raw(198),
                authority: AuthorityKind::SophiaX,
                surface: SurfaceId::new(405, 1),
                namespace: None,
                target_geometry: Rect {
                    x: 0,
                    y: 0,
                    width: 2560,
                    height: 14,
                },
                target_buffer: BufferSource::CpuBuffer { handle: 406 },
                damage: Region::empty(),
                readiness: SurfaceTransactionReadiness::Ready,
                timeout_msec: 250,
                previous_committed_generation: 1,
            }],
            removed_surfaces: Vec::new(),
            present_submissions: Vec::new(),
            software_present_submissions: Vec::new(),
        },
    );
    let mut resources = LivePresentationResourceSession::default();
    resources
        .register_source(descriptor(kitty_handle), vec![fd()])
        .unwrap();
    let mut scheduler = LiveProductionPresentScheduler::default();

    let rejected = scheduler
        .enqueue_group(
            &batch.groups[1],
            &[],
            Vec::new(),
            &mut resources,
            Instant::now(),
        )
        .unwrap();

    assert!(rejected.is_empty());
    let queued = scheduler.front().expect("Kitty Present should be queued");
    assert_eq!(queued.candidate.transaction, kitty_transaction);
    assert_eq!(queued.candidate.surface, kitty_surface);
}

#[test]
fn dma_buf_authority_group_without_matching_present_fails_closed() {
    let transaction = TransactionId::from_raw(501);
    let surface = SurfaceId::new(502, 1);
    let handle = BufferHandle::from_raw(503);
    let mut batch = scheduler_batch(transaction, surface, handle);
    batch.groups[0].present_submissions.clear();

    assert_eq!(
        batch.validate(),
        Err("production DMA-BUF transactions and Presents are not exact pairs")
    );
}

#[test]
fn mismatched_present_candidate_is_a_controlled_rejection() {
    let handle = BufferHandle::from_raw(407);
    let transaction = TransactionId::from_raw(408);
    let surface = SurfaceId::new(409, 1);
    let mut batch = scheduler_batch(transaction, surface, handle);
    batch.groups[0].transactions[0].transaction = TransactionId::from_raw(410);
    let mut resources = LivePresentationResourceSession::default();
    resources
        .register_source(descriptor(handle), vec![fd()])
        .unwrap();
    let mut scheduler = LiveProductionPresentScheduler::default();

    let rejected = scheduler.enqueue_group(
        &batch.groups[0],
        &[],
        Vec::new(),
        &mut resources,
        Instant::now(),
    );

    assert!(rejected.is_err());
    assert!(!scheduler.has_queued());
    assert_eq!(scheduler.controlled_rejections(), 0);
    assert_eq!(resources.presentation_count(), 0);
}

#[test]
fn newly_queued_present_uses_the_committed_presentation_layout() {
    let handle = BufferHandle::from_raw(50);
    let transaction = TransactionId::from_raw(51);
    let surface = SurfaceId::new(52, 1);
    let mut batch = scheduler_batch(transaction, surface, handle);
    batch.groups[0].transactions[0].target_geometry = Rect {
        x: 80,
        y: 60,
        width: 1280,
        height: 1426,
    };
    let geometry = Rect {
        x: 0,
        y: 14,
        width: 1280,
        height: 1426,
    };
    let layout = [sophia_protocol::LayerSnapshot {
        surface,
        authority_local_id: None,
        namespace: None,
        stack_rank: 0,
        geometry,
        source: batch.groups[0].transactions[0].target_buffer,
        damage: Region::empty(),
        opacity: 1.0,
        crop: None,
        transform: sophia_protocol::Transform::IDENTITY,
        generation: 1,
        resize_sync: sophia_protocol::ResizeSyncCapability::ImplicitOnly,
    }];
    let mut resources = LivePresentationResourceSession::default();
    resources
        .register_source(descriptor(handle), vec![fd()])
        .unwrap();
    let mut scheduler = LiveProductionPresentScheduler::default();

    scheduler
        .enqueue_group(
            &batch.groups[0],
            &layout,
            Vec::new(),
            &mut resources,
            Instant::now(),
        )
        .unwrap();

    let queued = scheduler.front().unwrap();
    assert_eq!(queued.target, geometry);
    assert_eq!(queued.surface_clip, geometry);
}

#[test]
fn aborting_layout_epoch_drains_only_layout_deferred_presents() {
    let retained_handle = BufferHandle::from_raw(57);
    let deferred_handle = BufferHandle::from_raw(58);
    let retained_transaction = TransactionId::from_raw(59);
    let deferred_transaction = TransactionId::from_raw(60);
    let surface = SurfaceId::new(61, 1);
    let mut resources = LivePresentationResourceSession::default();
    resources
        .register_source(descriptor(retained_handle), vec![fd()])
        .unwrap();
    resources
        .register_source(descriptor(deferred_handle), vec![fd()])
        .unwrap();
    let mut scheduler = LiveProductionPresentScheduler::default();
    scheduler
        .enqueue_group(
            &scheduler_batch(retained_transaction, surface, retained_handle).groups[0],
            &[],
            Vec::new(),
            &mut resources,
            Instant::now(),
        )
        .unwrap();
    let mut deferred_batch = scheduler_batch(deferred_transaction, surface, deferred_handle);
    deferred_batch.groups[0].present_submissions[0].layout_disposition =
        LiveProductionPresentDisposition::StageLayout {
            epoch: TransactionId::from_raw(1),
        };
    scheduler
        .enqueue_group(
            &deferred_batch.groups[0],
            &[],
            Vec::new(),
            &mut resources,
            Instant::now(),
        )
        .unwrap();

    assert_eq!(
        scheduler.drain_layout_deferred_transactions(),
        [deferred_transaction]
    );
    assert_eq!(
        scheduler
            .front()
            .map(|queued| queued.submission.transaction),
        Some(retained_transaction)
    );
}

#[test]
fn unrelated_present_remains_eligible_while_layout_surface_is_staged() {
    let staged_handle = BufferHandle::from_raw(62);
    let immediate_handle = BufferHandle::from_raw(63);
    let staged_transaction = TransactionId::from_raw(64);
    let immediate_transaction = TransactionId::from_raw(65);
    let staged_surface = SurfaceId::new(66, 1);
    let immediate_surface = SurfaceId::new(67, 1);
    let mut resources = LivePresentationResourceSession::default();
    resources
        .register_source(descriptor(staged_handle), vec![fd()])
        .unwrap();
    resources
        .register_source(descriptor(immediate_handle), vec![fd()])
        .unwrap();
    let staged = scheduler_batch_with_disposition(
        staged_transaction,
        staged_surface,
        staged_handle,
        LiveProductionPresentDisposition::StageLayout {
            epoch: TransactionId::from_raw(1),
        },
    );
    let immediate = scheduler_batch(immediate_transaction, immediate_surface, immediate_handle);
    let mut scheduler = LiveProductionPresentScheduler::default();
    scheduler
        .enqueue_group(
            &staged.groups[0],
            &[],
            Vec::new(),
            &mut resources,
            Instant::now(),
        )
        .unwrap();
    scheduler
        .enqueue_group(
            &immediate.groups[0],
            &[],
            Vec::new(),
            &mut resources,
            Instant::now(),
        )
        .unwrap();

    assert!(scheduler.has_layout_deferred());
    assert!(scheduler.has_eligible());
    assert_eq!(
        scheduler.poll_gate(&mut resources, Instant::now()).unwrap(),
        LiveProductionPresentGate::Ready(immediate_transaction)
    );
    assert_eq!(
        scheduler
            .front()
            .map(|queued| queued.submission.transaction),
        Some(immediate_transaction)
    );
}

#[test]
fn layout_epoch_keeps_only_the_newest_present_per_surface() {
    let first_handle = BufferHandle::from_raw(67);
    let second_handle = BufferHandle::from_raw(68);
    let first_transaction = TransactionId::from_raw(69);
    let second_transaction = TransactionId::from_raw(70);
    let surface = SurfaceId::new(71, 1);
    let mut resources = LivePresentationResourceSession::default();
    resources
        .register_source(descriptor(first_handle), vec![fd()])
        .unwrap();
    resources
        .register_source(descriptor(second_handle), vec![fd()])
        .unwrap();
    let mut scheduler = LiveProductionPresentScheduler::default();
    let epoch = TransactionId::from_raw(1);
    let first_batch = scheduler_batch_with_disposition(
        first_transaction,
        surface,
        first_handle,
        LiveProductionPresentDisposition::StageLayout { epoch },
    );
    let second_batch = scheduler_batch_with_disposition(
        second_transaction,
        surface,
        second_handle,
        LiveProductionPresentDisposition::StageLayout { epoch },
    );
    let first_superseded = scheduler
        .enqueue_group(
            &first_batch.groups[0],
            &[],
            Vec::new(),
            &mut resources,
            Instant::now(),
        )
        .unwrap();
    let second_superseded = scheduler
        .enqueue_group(
            &second_batch.groups[0],
            &[],
            Vec::new(),
            &mut resources,
            Instant::now(),
        )
        .unwrap();

    assert!(first_superseded.is_empty());
    assert_eq!(second_superseded, [first_transaction]);
    assert_eq!(
        scheduler
            .front()
            .map(|queued| queued.submission.transaction),
        Some(second_transaction)
    );
    assert_eq!(
        scheduler.drain_layout_deferred_transactions(),
        [second_transaction]
    );
}

#[test]
fn wrong_size_epoch_present_is_rejected_without_evicting_matching_candidate() {
    let matching_handle = BufferHandle::from_raw(77);
    let rejected_handle = BufferHandle::from_raw(78);
    let matching_transaction = TransactionId::from_raw(79);
    let rejected_transaction = TransactionId::from_raw(80);
    let surface = SurfaceId::new(81, 1);
    let mut resources = LivePresentationResourceSession::default();
    resources
        .register_source(descriptor(matching_handle), vec![fd()])
        .unwrap();
    resources
        .register_source(descriptor(rejected_handle), vec![fd()])
        .unwrap();
    let mut scheduler = LiveProductionPresentScheduler::default();
    let epoch = TransactionId::from_raw(2);
    let matching_batch = scheduler_batch_with_disposition(
        matching_transaction,
        surface,
        matching_handle,
        LiveProductionPresentDisposition::StageLayout { epoch },
    );
    scheduler
        .enqueue_group(
            &matching_batch.groups[0],
            &[],
            Vec::new(),
            &mut resources,
            Instant::now(),
        )
        .unwrap();

    let rejected_batch = scheduler_batch_with_disposition(
        rejected_transaction,
        surface,
        rejected_handle,
        LiveProductionPresentDisposition::RejectLayoutMismatch,
    );
    let rejected = scheduler
        .enqueue_group(
            &rejected_batch.groups[0],
            &[],
            Vec::new(),
            &mut resources,
            Instant::now(),
        )
        .unwrap();

    assert_eq!(rejected, [rejected_transaction]);
    assert_eq!(
        scheduler
            .front()
            .map(|queued| queued.submission.transaction),
        Some(matching_transaction)
    );
}
