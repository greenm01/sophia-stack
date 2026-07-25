#![cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]

use std::fs::File;
use std::os::fd::OwnedFd;
use std::time::{Duration, Instant};

use sophia_backend_live::{
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveCpuComposedFrame, LivePresentCompletionMode,
    LivePresentFeedbackError, LivePresentProtocolFeedback, LivePresentationResourceSession,
    LivePresentationSubmission, LiveProductionAuthorityBatch,
    LiveProductionPresentFeedbackCoordinator, LiveProductionPresentGate,
    LiveProductionPresentScheduler, LiveProductionPresentSubmission, LiveResourceReleaseStatus,
    LiveRetainedDmaBufLayer, compose_full_state_mixed_frame,
};
use sophia_engine::{HeadlessEngine, ProductionSessionCoordinator};
use sophia_protocol::{
    AuthorityKind, BufferHandle, BufferSource, CommittedSurfaceState, DRM_FORMAT_MOD_INVALID,
    DmaBufDescriptor, DmaBufPlaneDescriptor, Rect, Region, Size, SurfaceId, SurfaceTransaction,
    SurfaceTransactionReadiness, TransactionId, TransactionOutcome,
};
use sophia_renderer_live::{
    LiveCompositionPlacement, LiveOwnedMixedCompositionLayer, LiveOwnedMultiPlaneDmaBufFrame,
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

#[test]
fn backend_session_builds_mixed_cpu_gpu_frame_and_retires_exactly_once() {
    let handle = BufferHandle::from_raw(7);
    let transaction = TransactionId::from_raw(8);
    let mut session = LivePresentationResourceSession::default();
    session
        .register_source(descriptor(handle), vec![fd()])
        .unwrap();
    session
        .begin(LivePresentationSubmission {
            transaction,
            buffer: handle,
            acquire_fence: None,
            idle_fence: None,
        })
        .unwrap();
    let cpu = LiveCpuComposedFrame {
        size: Size {
            width: 128,
            height: 96,
        },
        stride: 512,
        format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        bytes: vec![1; 128 * 96 * 4],
    };

    let frame = session
        .build_mixed_frame(
            transaction,
            Some(cpu),
            Rect {
                x: 20,
                y: 10,
                width: 64,
                height: 48,
            },
            None,
            1.0,
        )
        .unwrap();
    assert_eq!(frame.layers.len(), 2);
    session.mark_submitted(transaction).unwrap();
    assert_eq!(
        session.release_source(handle),
        LiveResourceReleaseStatus::Deferred
    );
    let retired = session.retire_page_flip(transaction).unwrap();
    assert_eq!(retired.source, BufferSource::DmaBuf { handle: 7 });
    assert!(retired.released_source);
    assert!(session.retire_page_flip(transaction).is_none());
    assert_eq!(session.source_count(), 0);
    assert_eq!(session.presentation_count(), 0);
}

#[test]
fn dma_buf_surface_resize_preserves_pixels_and_clips_without_scaling() {
    let handle = BufferHandle::from_raw(17);
    let transaction = TransactionId::from_raw(18);
    let mut session = LivePresentationResourceSession::default();
    session
        .register_source(descriptor(handle), vec![fd()])
        .unwrap();
    session
        .begin(LivePresentationSubmission {
            transaction,
            buffer: handle,
            acquire_fence: None,
            idle_fence: None,
        })
        .unwrap();
    let surface = Rect {
        x: 64,
        y: 12,
        width: 32,
        height: 48,
    };

    let frame = session
        .build_mixed_frame(transaction, None, surface, None, 1.0)
        .unwrap();
    let LiveOwnedMixedCompositionLayer::DmaBuf { placement, .. } = &frame.layers[0] else {
        panic!("expected a DMA-BUF layer");
    };
    assert_eq!(
        placement.target,
        Rect {
            width: 64,
            ..surface
        }
    );
    assert_eq!(placement.clip, Some(surface));
}

#[test]
fn full_state_composition_keeps_retained_surface_before_current_damage() {
    let placement = |x| LiveCompositionPlacement {
        target: Rect {
            x,
            y: 0,
            width: 64,
            height: 48,
        },
        clip: None,
        transform: sophia_protocol::Transform::IDENTITY,
        alpha: 1.0,
    };
    let frame = || LiveOwnedMultiPlaneDmaBufFrame {
        width: 64,
        height: 48,
        format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        modifier: DRM_FORMAT_MOD_INVALID,
        plane_count: 1,
        planes: [
            Some(sophia_renderer_live::LiveOwnedDmaBufPlane {
                fd: fd(),
                offset: 0,
                stride: 256,
            }),
            None,
            None,
            None,
        ],
    };
    let current = sophia_renderer_live::LiveOwnedMixedCompositionFrame {
        layers: vec![LiveOwnedMixedCompositionLayer::DmaBuf {
            frame: frame(),
            placement: placement(64),
        }],
    };

    let composed = compose_full_state_mixed_frame(
        current,
        vec![LiveRetainedDmaBufLayer {
            frame: frame(),
            placement: placement(0),
        }],
    );

    assert_eq!(composed.layers.len(), 2);
    let targets = composed
        .layers
        .iter()
        .map(|layer| match layer {
            LiveOwnedMixedCompositionLayer::DmaBuf { placement, .. }
            | LiveOwnedMixedCompositionLayer::Cpu { placement, .. } => placement.target.x,
        })
        .collect::<Vec<_>>();
    assert_eq!(targets, [0, 64]);
}

#[test]
fn retained_multi_plane_frame_clone_preserves_metadata_planes() {
    let original = LiveOwnedMultiPlaneDmaBufFrame {
        width: 64,
        height: 48,
        format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        modifier: 7,
        plane_count: 3,
        planes: [
            Some(sophia_renderer_live::LiveOwnedDmaBufPlane {
                fd: fd(),
                offset: 0,
                stride: 256,
            }),
            Some(sophia_renderer_live::LiveOwnedDmaBufPlane {
                fd: fd(),
                offset: 12_288,
                stride: 64,
            }),
            Some(sophia_renderer_live::LiveOwnedDmaBufPlane {
                fd: fd(),
                offset: 15_360,
                stride: 64,
            }),
            None,
        ],
    };

    let cloned = original.try_clone().unwrap();

    assert_eq!(cloned.plane_count, 3);
    assert_eq!(cloned.modifier, 7);
    assert_eq!(cloned.planes[1].as_ref().unwrap().offset, 12_288);
    assert_eq!(cloned.planes[2].as_ref().unwrap().stride, 64);
}

#[test]
fn production_feedback_retires_resources_before_complete_and_idle() {
    let handle = BufferHandle::from_raw(17);
    let transaction = TransactionId::from_raw(18);
    let mut coordinator = LiveProductionPresentFeedbackCoordinator::default();
    coordinator
        .resources_mut()
        .register_source(descriptor(handle), vec![fd()])
        .unwrap();
    coordinator
        .resources_mut()
        .begin(LivePresentationSubmission {
            transaction,
            buffer: handle,
            acquire_fence: None,
            idle_fence: None,
        })
        .unwrap();
    coordinator
        .resources_mut()
        .mark_submitted(transaction)
        .unwrap();
    assert_eq!(
        coordinator.resources_mut().release_source(handle),
        LiveResourceReleaseStatus::Deferred
    );

    let outcome = coordinator.complete_flip(transaction, 22, 33).unwrap();
    assert_eq!(
        outcome.feedback,
        [
            LivePresentProtocolFeedback::Complete {
                transaction,
                ust: 22,
                msc: 33,
                mode: LivePresentCompletionMode::Flip,
            },
            LivePresentProtocolFeedback::Idle { transaction },
        ]
    );
    assert!(!outcome.idle_fence_triggered);
    assert_eq!(coordinator.resources().source_count(), 0);
    assert_eq!(coordinator.resources().presentation_count(), 0);
    assert_eq!(
        coordinator.complete_flip(transaction, 44, 55),
        Err(LivePresentFeedbackError::UnknownPresentation { transaction })
    );
}

#[test]
fn displayed_feedback_delays_idle_until_surface_buffer_replacement() {
    let handle = BufferHandle::from_raw(19);
    let transaction = TransactionId::from_raw(20);
    let mut coordinator = LiveProductionPresentFeedbackCoordinator::default();
    coordinator
        .resources_mut()
        .register_source(descriptor(handle), vec![fd()])
        .unwrap();
    coordinator
        .resources_mut()
        .begin(LivePresentationSubmission {
            transaction,
            buffer: handle,
            acquire_fence: None,
            idle_fence: None,
        })
        .unwrap();
    coordinator
        .resources_mut()
        .mark_submitted(transaction)
        .unwrap();

    let completed = coordinator
        .complete_flip_without_idle(transaction, 22, 23)
        .unwrap();
    assert_eq!(
        completed.feedback,
        [LivePresentProtocolFeedback::Complete {
            transaction,
            ust: 22,
            msc: 23,
            mode: LivePresentCompletionMode::Flip,
        }]
    );
    assert_eq!(coordinator.resources().presentation_count(), 1);

    let idle = coordinator.idle_displayed(transaction).unwrap();
    assert_eq!(
        idle.feedback,
        [LivePresentProtocolFeedback::Idle { transaction }]
    );
    assert_eq!(coordinator.resources().presentation_count(), 0);
}

#[test]
fn production_feedback_emits_nothing_when_skip_has_no_live_presentation() {
    let transaction = TransactionId::from_raw(28);
    let mut coordinator = LiveProductionPresentFeedbackCoordinator::default();

    assert_eq!(
        coordinator.reject_skip(transaction, 0, 0),
        Err(LivePresentFeedbackError::UnknownPresentation { transaction })
    );
}

#[test]
fn stale_prepared_page_flip_settles_as_skip_and_retires_resources_exactly_once() {
    let handle = BufferHandle::from_raw(29);
    let transaction = TransactionId::from_raw(30);
    let surface = SurfaceId::new(31, 1);
    let committed = CommittedSurfaceState {
        surface,
        committed_generation: 1,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 64,
            height: 48,
        },
        buffer: BufferSource::DmaBuf {
            handle: handle.raw(),
        },
        damage: Region::empty(),
    };
    let mut production = ProductionSessionCoordinator::new(HeadlessEngine::default())
        .with_committed_surfaces(vec![committed.clone()]);
    let prepared = production.prepare_full_state_present(
        transaction,
        &[SurfaceTransaction {
            transaction,
            authority: AuthorityKind::SophiaX,
            surface,
            namespace: None,
            target_geometry: committed.geometry,
            target_buffer: committed.buffer,
            damage: Region::empty(),
            readiness: SurfaceTransactionReadiness::Ready,
            timeout_msec: 250,
            previous_committed_generation: 1,
        }],
    );
    production.replace_committed_surfaces(Vec::new());

    let mut coordinator = LiveProductionPresentFeedbackCoordinator::default();
    coordinator
        .resources_mut()
        .register_source(descriptor(handle), vec![fd()])
        .unwrap();
    coordinator
        .resources_mut()
        .begin(LivePresentationSubmission {
            transaction,
            buffer: handle,
            acquire_fence: None,
            idle_fence: None,
        })
        .unwrap();
    coordinator
        .resources_mut()
        .mark_submitted(transaction)
        .unwrap();
    assert_eq!(
        coordinator.resources_mut().release_source(handle),
        LiveResourceReleaseStatus::Deferred
    );

    let report = production
        .settle_prepared_retirement(prepared, |commit| match commit.outcome {
            TransactionOutcome::Committed => coordinator.complete_flip(transaction, 41, 42),
            TransactionOutcome::RejectedStaleSurface
            | TransactionOutcome::RejectedInvalidSurface
            | TransactionOutcome::TimedOut => coordinator.reject_skip(transaction, 41, 42),
        })
        .expect("stale page flip should settle through controlled rejection");

    assert_eq!(
        report.commit.outcome,
        TransactionOutcome::RejectedStaleSurface
    );
    assert!(report.committed_surfaces.is_empty());
    assert_eq!(
        report.evidence.feedback,
        [
            LivePresentProtocolFeedback::Complete {
                transaction,
                ust: 41,
                msc: 42,
                mode: LivePresentCompletionMode::Skip,
            },
            LivePresentProtocolFeedback::Idle { transaction },
        ]
    );
    assert_eq!(coordinator.resources().source_count(), 0);
    assert_eq!(coordinator.resources().presentation_count(), 0);
    assert_eq!(
        coordinator.reject_skip(transaction, 41, 42),
        Err(LivePresentFeedbackError::UnknownPresentation { transaction })
    );
}

fn scheduler_batch(
    transaction: TransactionId,
    surface: SurfaceId,
    handle: BufferHandle,
) -> LiveProductionAuthorityBatch {
    LiveProductionAuthorityBatch {
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
        dma_buf_registrations: Vec::new(),
        fence_registrations: Vec::new(),
        present_submissions: vec![LiveProductionPresentSubmission {
            transaction,
            surface,
            buffer: handle,
            x_offset: 0,
            y_offset: 0,
            acquire_fence: None,
            idle_fence: None,
        }],
        released_dma_bufs: Vec::new(),
        released_fences: Vec::new(),
    }
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
        .enqueue_batch(
            &scheduler_batch(transaction, surface, handle),
            None,
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
    batch.present_submissions[0].x_offset = 3;
    batch.present_submissions[0].y_offset = -4;
    let mut resources = LivePresentationResourceSession::default();
    resources
        .register_source(descriptor(handle), vec![fd()])
        .unwrap();
    let mut scheduler = LiveProductionPresentScheduler::default();
    scheduler
        .enqueue_batch(&batch, None, &mut resources, Instant::now())
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
    assert_eq!(
        queued.target,
        Rect {
            x: 1283,
            y: 716,
            ..geometry
        }
    );
    assert_eq!(queued.transactions[0].target_geometry, geometry);
}
