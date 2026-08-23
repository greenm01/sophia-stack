#![cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]

use sophia_backend_live::{
    LivePresentBufferDisposition, LivePresentProtocolFeedback, LiveProductionAuthorityBatch,
    LiveProductionAuthorityGroup, LiveProductionCursorPresentation, LiveProductionCycleRequest,
    LiveProductionDmaBufRegistration, LiveProductionFenceRegistration, LiveProductionNativeFrameId,
    LiveProductionNativeRetirementOwner, LiveProductionNativeSubmissionOwner,
    LiveProductionPresentDisposition, LiveProductionPresentSubmission,
    LiveProductionScanoutContent, LiveProductionSoftwarePresentFrameObservation,
    LiveProductionSoftwarePresentFramePhase, LiveProductionSoftwarePresentFrameTransition,
    LiveProductionSoftwarePresentSubmission, LiveProductionVisualRuntime,
    reduce_live_production_native_retirement_owner, reduce_live_production_native_submission_owner,
    reduce_software_present_frame_observation,
};
use sophia_engine::HeadlessOutput;
use sophia_protocol::{
    AuthorityKind, BufferHandle, BufferSource, DRM_FORMAT_MOD_INVALID, DmaBufDescriptor,
    DmaBufPlaneDescriptor, FenceHandle, LayerSnapshot, OutputId, Rect, Region,
    ResizeSyncCapability, Size, SurfaceId, SurfaceTransaction, SurfaceTransactionReadiness,
    TransactionId, Transform,
};
use sophia_renderer_live::{
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveCpuBufferPatch, LiveCpuBufferSource,
    LiveCpuBufferUpdate, LiveProductionCpuScene,
};
use std::fs::File;
use std::os::fd::OwnedFd;
use std::sync::Arc;

#[test]
fn software_present_feedback_requires_its_own_native_frame() {
    let owned = LiveProductionNativeFrameId::from_raw(41);
    let unrelated = LiveProductionNativeFrameId::from_raw(42);

    assert_eq!(
        reduce_software_present_frame_observation(
            owned,
            LiveProductionSoftwarePresentFramePhase::Pending,
            LiveProductionSoftwarePresentFrameObservation::NativeSubmitted(unrelated),
        ),
        LiveProductionSoftwarePresentFrameTransition::Unrelated
    );
    assert_eq!(
        reduce_software_present_frame_observation(
            owned,
            LiveProductionSoftwarePresentFramePhase::Pending,
            LiveProductionSoftwarePresentFrameObservation::NativeRetired(unrelated),
        ),
        LiveProductionSoftwarePresentFrameTransition::Unrelated
    );
    assert_eq!(
        reduce_software_present_frame_observation(
            owned,
            LiveProductionSoftwarePresentFramePhase::Pending,
            LiveProductionSoftwarePresentFrameObservation::NativeRetired(owned),
        ),
        LiveProductionSoftwarePresentFrameTransition::InvalidRetirement
    );
    assert_eq!(
        reduce_software_present_frame_observation(
            owned,
            LiveProductionSoftwarePresentFramePhase::Pending,
            LiveProductionSoftwarePresentFrameObservation::NativeSubmitted(owned),
        ),
        LiveProductionSoftwarePresentFrameTransition::Submitted
    );
    assert_eq!(
        reduce_software_present_frame_observation(
            owned,
            LiveProductionSoftwarePresentFramePhase::Submitted,
            LiveProductionSoftwarePresentFrameObservation::NativeSubmitted(owned),
        ),
        LiveProductionSoftwarePresentFrameTransition::AlreadySubmitted
    );
    assert_eq!(
        reduce_software_present_frame_observation(
            owned,
            LiveProductionSoftwarePresentFramePhase::Submitted,
            LiveProductionSoftwarePresentFrameObservation::NativeRetired(owned),
        ),
        LiveProductionSoftwarePresentFrameTransition::Retired
    );
}

#[test]
fn older_software_frame_may_retire_after_next_dma_frame_submits() {
    let retired = LiveProductionNativeFrameId::from_raw(30);
    let successor = LiveProductionNativeFrameId::from_raw(31);
    let software = LiveProductionScanoutContent::RetainedMixed {
        frame: retired,
        nonzero_rgb_pixels: 985,
    };

    assert_eq!(
        reduce_live_production_native_retirement_owner(retired, software, Some(successor)),
        LiveProductionNativeRetirementOwner::IndependentFrame
    );
    assert_eq!(
        reduce_live_production_native_retirement_owner(
            retired,
            LiveProductionScanoutContent::MixedPresent {
                frame: retired,
                transaction: TransactionId::from_raw(699),
                nonzero_rgb_pixels: 985,
            },
            Some(successor),
        ),
        LiveProductionNativeRetirementOwner::InvalidDmaOwnership
    );
}

#[test]
fn ordinary_head_composition_submission_does_not_advance_a_present_cohort() {
    let frame = LiveProductionNativeFrameId::from_raw(43);
    let transaction = TransactionId::from_raw(700);
    let ordinary = LiveProductionScanoutContent::HeadComposition {
        frame,
        logical_content_checksum: 91,
        nonzero_rgb_pixels: 1_024,
    };
    let present = LiveProductionScanoutContent::MixedPresent {
        frame,
        transaction,
        nonzero_rgb_pixels: 1_024,
    };

    assert_eq!(
        reduce_live_production_native_submission_owner(ordinary, None),
        LiveProductionNativeSubmissionOwner::IndependentFrame
    );
    assert_eq!(
        reduce_live_production_native_submission_owner(present, Some((frame, transaction))),
        LiveProductionNativeSubmissionOwner::SubmittedDmaPresent
    );
    assert_eq!(
        reduce_live_production_native_submission_owner(ordinary, Some((frame, transaction))),
        LiveProductionNativeSubmissionOwner::InvalidDmaOwnership
    );
    assert_eq!(
        reduce_live_production_native_submission_owner(present, None),
        LiveProductionNativeSubmissionOwner::InvalidDmaOwnership
    );
}

#[test]
fn recent_cpu_update_residency_bridges_patch_gaps_and_remains_bounded() {
    let size = Size {
        width: 2,
        height: 1,
    };
    let output = HeadlessOutput {
        id: OutputId::from_raw(1),
        size,
        scale: 1,
    };
    let mut scene = LiveProductionCpuScene::new(size);
    let mut runtime = LiveProductionVisualRuntime::new(&[output], None).unwrap();
    let run_update = |runtime: &mut LiveProductionVisualRuntime,
                      scene: &mut LiveProductionCpuScene,
                      updates: Vec<LiveCpuBufferUpdate>| {
        let groups = if let Some(update) = updates.first() {
            let transaction = TransactionId::from_raw(
                update
                    .handle()
                    .saturating_add(update.generation())
                    .saturating_add(1),
            );
            let surface = SurfaceId::new(update.handle() as u32, 1);
            vec![LiveProductionAuthorityGroup {
                transaction,
                transactions: vec![SurfaceTransaction {
                    transaction,
                    authority: AuthorityKind::SophiaX,
                    surface,
                    namespace: None,
                    target_geometry: Rect {
                        x: 0,
                        y: 0,
                        width: size.width,
                        height: size.height,
                    },
                    presentation_extent: sophia_protocol::Size {
                        width: size.width,
                        height: size.height,
                    },
                    content: sophia_protocol::SurfaceContentSet::singleton(
                        BufferSource::CpuBuffer {
                            handle: update.handle(),
                        },
                        size,
                    ),

                    damage: Region::empty(),
                    readiness: SurfaceTransactionReadiness::Ready,
                    timeout_msec: 250,
                    previous_committed_generation: 0,
                }],
                cpu_buffer_updates: updates,
                removed_surfaces: vec![surface],
                present_submissions: Vec::new(),
                software_present_submissions: Vec::new(),
            }]
        } else {
            Vec::new()
        };
        let batch = LiveProductionAuthorityBatch {
            groups,
            dma_buf_registrations: Vec::new(),
            fence_registrations: Vec::new(),
            released_dma_bufs: Vec::new(),
            released_fences: Vec::new(),
        };
        runtime.run_cpu_production_cycle(LiveProductionCycleRequest {
            batch: &batch,
            scene,
            raised_surface: None,
            focused_surface: None,
            cursor_presentation: LiveProductionCursorPresentation::Software(None),
            defer_frame: false,
            output_descriptors: &[output],
            native_scanout: None,
            wm_update: None,
            presentation_layout: &[],
            chrome_surfaces: &[],
            indicator_publication: None,
            staged_cpu_buffer_handles: &[],
        })
    };

    run_update(
        &mut runtime,
        &mut scene,
        vec![LiveCpuBufferUpdate::Replace(LiveCpuBufferSource {
            handle: 72,
            size,
            stride: 8,
            format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            generation: 1,
            bytes: vec![0; 8],
        })],
    )
    .unwrap();
    assert_eq!(scene.resident_buffer_count(), 1);

    run_update(
        &mut runtime,
        &mut scene,
        vec![LiveCpuBufferUpdate::Patch(LiveCpuBufferPatch {
            handle: 72,
            size,
            stride: 8,
            format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            generation: 2,
            rect: Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            bytes: vec![1, 2, 3, 4],
        })],
    )
    .unwrap();
    assert_eq!(scene.resident_buffer_count(), 1);

    run_update(&mut runtime, &mut scene, Vec::new()).unwrap();
    assert_eq!(scene.resident_buffer_count(), 1);

    for handle in 100..116 {
        run_update(
            &mut runtime,
            &mut scene,
            vec![LiveCpuBufferUpdate::Replace(LiveCpuBufferSource {
                handle,
                size,
                stride: 8,
                format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                generation: 1,
                bytes: vec![0; 8],
            })],
        )
        .unwrap();
    }
    assert_eq!(scene.resident_buffer_count(), 16);

    run_update(&mut runtime, &mut scene, Vec::new()).unwrap();
    assert_eq!(scene.resident_buffer_count(), 16);
}

#[test]
fn software_present_applies_grouped_pixels_and_routes_feedback() {
    let size = Size {
        width: 2,
        height: 1,
    };
    let output = HeadlessOutput {
        id: OutputId::from_raw(1),
        size,
        scale: 1,
    };
    let transaction = TransactionId::from_raw(70);
    let surface = SurfaceId::new(71, 1);
    let geometry = Rect {
        x: 0,
        y: 0,
        width: 2,
        height: 1,
    };
    let surface_transaction = SurfaceTransaction {
        transaction,
        authority: AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry: geometry,
        presentation_extent: Size {
            width: (geometry).width,
            height: (geometry).height,
        },
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::CpuBuffer { handle: 72 },
            size,
        ),

        damage: Region::single(geometry),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };
    let batch = LiveProductionAuthorityBatch {
        groups: vec![LiveProductionAuthorityGroup {
            transaction,
            transactions: vec![surface_transaction.clone()],
            cpu_buffer_updates: vec![LiveCpuBufferUpdate::Replace(LiveCpuBufferSource {
                handle: 72,
                size,
                stride: 8,
                format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                generation: 1,
                bytes: vec![0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0xff],
            })],
            removed_surfaces: Vec::new(),
            present_submissions: Vec::new(),
            software_present_submissions: vec![LiveProductionSoftwarePresentSubmission {
                candidate: surface_transaction.key(),
                source_size: size,
                transaction,
                surface,
                acquire_fence: None,
                idle_fence: None,
            }],
        }],
        dma_buf_registrations: Vec::new(),
        fence_registrations: Vec::new(),
        released_dma_bufs: Vec::new(),
        released_fences: Vec::new(),
    };
    let layout = [LayerSnapshot {
        surface,
        authority_local_id: None,
        namespace: None,
        stack_rank: 0,
        geometry,
        source_size: Size {
            width: geometry.width,
            height: geometry.height,
        },
        source: surface_transaction.target_buffer(),
        damage: surface_transaction.damage.clone(),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 1,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    }];
    let mut scene = LiveProductionCpuScene::new(size);
    let mut runtime = LiveProductionVisualRuntime::new(&[output], None).unwrap();
    let (submission, committed) = runtime
        .run_cpu_production_cycle(LiveProductionCycleRequest {
            batch: &batch,
            scene: &mut scene,
            raised_surface: None,
            focused_surface: Some(surface),
            cursor_presentation: LiveProductionCursorPresentation::Software(None),
            defer_frame: false,
            output_descriptors: &[output],
            native_scanout: None,
            wm_update: None,
            presentation_layout: &layout,
            chrome_surfaces: &[surface],
            indicator_publication: None,
            staged_cpu_buffer_handles: &[],
        })
        .unwrap();

    assert!(submission.composed);
    assert_eq!(committed.len(), 1);
    assert!(scene.surface_has_visual_detail(&committed, surface));
    let mut outcomes = Vec::new();
    runtime.drain_present_feedback_into(&mut outcomes).unwrap();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(
        outcomes[0].feedback,
        [
            LivePresentProtocolFeedback::Idle { transaction },
            LivePresentProtocolFeedback::Complete {
                transaction,
                ust: 0,
                msc: 0,
                disposition: LivePresentBufferDisposition::Copied,
            },
        ]
    );
    assert_eq!(runtime.diagnostics().live_presentations, 0);
    let mut retired = Vec::new();
    runtime
        .drain_retired_software_presents_into(&mut retired)
        .unwrap();
    assert_eq!(
        retired,
        [sophia_backend_live::LiveProductionRetiredSoftwarePresent {
            candidate: surface_transaction.key(),
            source_size: size,
            frame: sophia_backend_live::LiveProductionNativeFrameId::from_raw(0),
            native_submission: 0,
            ust_usec: 0,
            msc: 0,
        }]
    );
}

#[test]
fn gpu_owner_batch_registers_its_separate_software_present_group() {
    let size = Size {
        width: 64,
        height: 48,
    };
    let output = HeadlessOutput {
        id: OutputId::from_raw(1),
        size,
        scale: 1,
    };
    let cpu_transaction = TransactionId::from_raw(80);
    let cpu_surface = SurfaceId::new(81, 1);
    let cpu_geometry = Rect {
        x: 0,
        y: 0,
        width: size.width,
        height: size.height,
    };
    let cpu_candidate = SurfaceTransaction {
        transaction: cpu_transaction,
        authority: AuthorityKind::SophiaX,
        surface: cpu_surface,
        namespace: None,
        target_geometry: cpu_geometry,
        presentation_extent: Size {
            width: (cpu_geometry).width,
            height: (cpu_geometry).height,
        },
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::CpuBuffer { handle: 82 },
            size,
        ),

        damage: Region::single(cpu_geometry),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };
    let dma_transaction = TransactionId::from_raw(90);
    let dma_surface = SurfaceId::new(91, 1);
    let dma_handle = BufferHandle::from_raw(92);
    let dma_candidate = SurfaceTransaction {
        transaction: dma_transaction,
        authority: AuthorityKind::SophiaX,
        surface: dma_surface,
        namespace: None,
        target_geometry: cpu_geometry,
        presentation_extent: Size {
            width: (cpu_geometry).width,
            height: (cpu_geometry).height,
        },
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::DmaBuf {
                handle: dma_handle.raw(),
            },
            size,
        ),

        damage: Region::single(cpu_geometry),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };
    let batch = LiveProductionAuthorityBatch {
        groups: vec![
            LiveProductionAuthorityGroup {
                transaction: cpu_transaction,
                transactions: vec![cpu_candidate.clone()],
                cpu_buffer_updates: Vec::new(),
                removed_surfaces: Vec::new(),
                present_submissions: Vec::new(),
                software_present_submissions: vec![LiveProductionSoftwarePresentSubmission {
                    candidate: cpu_candidate.key(),
                    source_size: size,
                    transaction: cpu_transaction,
                    surface: cpu_surface,
                    acquire_fence: None,
                    idle_fence: None,
                }],
            },
            LiveProductionAuthorityGroup {
                transaction: dma_transaction,
                transactions: vec![dma_candidate],
                cpu_buffer_updates: Vec::new(),
                removed_surfaces: Vec::new(),
                present_submissions: vec![LiveProductionPresentSubmission {
                    transaction: dma_transaction,
                    surface: dma_surface,
                    buffer: dma_handle,
                    x_offset: 0,
                    y_offset: 0,
                    acquire_fence: None,
                    idle_fence: None,
                    layout_disposition: LiveProductionPresentDisposition::Immediate,
                }],
                software_present_submissions: Vec::new(),
            },
        ],
        dma_buf_registrations: vec![LiveProductionDmaBufRegistration {
            descriptor: DmaBufDescriptor {
                handle: dma_handle,
                size,
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
            },
            plane_fds: vec![Arc::new(OwnedFd::from(
                File::open("/dev/null").expect("DMA-BUF fixture FD"),
            ))],
        }],
        fence_registrations: Vec::new(),
        released_dma_bufs: Vec::new(),
        released_fences: Vec::new(),
    };
    let mut runtime = LiveProductionVisualRuntime::new(&[output], None).unwrap();

    runtime
        .run_batch(&batch, &[], None, None, Vec::new(), None)
        .unwrap();

    let diagnostics = runtime.diagnostics();
    assert_eq!(diagnostics.software_present_frames_waiting, 1);
    assert_eq!(diagnostics.software_present_frames_submitted, 0);
    assert_eq!(diagnostics.live_presentations, 1);
    runtime.shutdown_presentations().unwrap();
    assert_eq!(runtime.diagnostics().live_presentations, 0);
}

#[test]
fn deferred_successor_present_retains_resources_until_stream_admission() {
    let size = Size {
        width: 64,
        height: 48,
    };
    let output = HeadlessOutput {
        id: OutputId::from_raw(1),
        size,
        scale: 1,
    };
    let surface = SurfaceId::new(101, 1);
    let geometry = Rect {
        x: 0,
        y: 0,
        width: size.width,
        height: size.height,
    };
    let layout = [LayerSnapshot {
        surface,
        authority_local_id: None,
        namespace: None,
        stack_rank: 0,
        geometry,
        source_size: Size {
            width: geometry.width,
            height: geometry.height,
        },
        source: BufferSource::None,
        damage: Region::single(geometry),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 0,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    }];
    let first_transaction = TransactionId::from_raw(102);
    let first_handle = BufferHandle::from_raw(103);
    let acquire_handle = FenceHandle::from_raw(104);
    let acquire_fence = sophia_xshmfence::allocate().unwrap();
    let present_batch =
        |transaction: TransactionId, handle: BufferHandle, acquire_fence: Option<FenceHandle>| {
            let candidate = SurfaceTransaction {
                transaction,
                authority: AuthorityKind::SophiaX,
                surface,
                namespace: None,
                target_geometry: geometry,
                presentation_extent: Size {
                    width: (geometry).width,
                    height: (geometry).height,
                },
                content: sophia_protocol::SurfaceContentSet::singleton(
                    BufferSource::DmaBuf {
                        handle: handle.raw(),
                    },
                    size,
                ),

                damage: Region::single(geometry),
                readiness: SurfaceTransactionReadiness::Ready,
                timeout_msec: 250,
                previous_committed_generation: 0,
            };
            LiveProductionAuthorityGroup {
                transaction,
                transactions: vec![candidate],
                cpu_buffer_updates: Vec::new(),
                removed_surfaces: Vec::new(),
                present_submissions: vec![LiveProductionPresentSubmission {
                    transaction,
                    surface,
                    buffer: handle,
                    x_offset: 0,
                    y_offset: 0,
                    acquire_fence,
                    idle_fence: None,
                    layout_disposition: LiveProductionPresentDisposition::Immediate,
                }],
                software_present_submissions: Vec::new(),
            }
        };
    let registration = |handle| LiveProductionDmaBufRegistration {
        descriptor: DmaBufDescriptor {
            handle,
            size,
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
        },
        plane_fds: vec![Arc::new(OwnedFd::from(
            File::open("/dev/null").expect("DMA-BUF fixture FD"),
        ))],
    };
    let first = LiveProductionAuthorityBatch {
        groups: vec![present_batch(
            first_transaction,
            first_handle,
            Some(acquire_handle),
        )],
        dma_buf_registrations: vec![registration(first_handle)],
        fence_registrations: vec![LiveProductionFenceRegistration {
            handle: acquire_handle,
            initially_triggered: false,
            fd: Arc::new(acquire_fence),
        }],
        released_dma_bufs: Vec::new(),
        released_fences: Vec::new(),
    };
    let successor_transaction = TransactionId::from_raw(105);
    let successor_handle = BufferHandle::from_raw(106);
    let successor = LiveProductionAuthorityBatch {
        groups: vec![present_batch(successor_transaction, successor_handle, None)],
        dma_buf_registrations: vec![registration(successor_handle)],
        fence_registrations: Vec::new(),
        released_dma_bufs: vec![successor_handle],
        released_fences: Vec::new(),
    };
    let mut scene = LiveProductionCpuScene::new(size);
    let mut runtime = LiveProductionVisualRuntime::new(&[output], None).unwrap();
    for batch in [&first, &successor] {
        runtime
            .run_gpu_production_cycle(LiveProductionCycleRequest {
                batch,
                scene: &mut scene,
                raised_surface: None,
                focused_surface: None,
                cursor_presentation: LiveProductionCursorPresentation::Software(None),
                defer_frame: false,
                output_descriptors: &[output],
                native_scanout: None,
                wm_update: None,
                presentation_layout: &layout,
                chrome_surfaces: &[],
                indicator_publication: None,
                staged_cpu_buffer_handles: &[],
            })
            .unwrap();
    }

    let diagnostics = runtime.diagnostics();
    assert_eq!(diagnostics.live_presentations, 1);
    assert_eq!(diagnostics.live_sources, 2);
    runtime.shutdown_presentations().unwrap();
    assert_eq!(runtime.diagnostics().live_sources, 0);
}
