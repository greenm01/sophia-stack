#![cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]

use sophia_backend_live::{
    LivePresentBufferDisposition, LivePresentProtocolFeedback, LiveProductionAuthorityBatch,
    LiveProductionAuthorityGroup, LiveProductionCursorPresentation, LiveProductionCycleRequest,
    LiveProductionDmaBufRegistration, LiveProductionFenceRegistration,
    LiveProductionPresentDisposition, LiveProductionPresentSubmission, LiveProductionVisualRuntime,
};
use sophia_engine::HeadlessOutput;
use sophia_protocol::{
    AuthorityKind, BufferHandle, BufferSource, DRM_FORMAT_MOD_INVALID, DmaBufDescriptor,
    DmaBufPlaneDescriptor, FenceHandle, LayerSnapshot, OutputId, Rect, Region,
    ResizeSyncCapability, Size, SurfaceId, SurfaceTransaction, SurfaceTransactionReadiness,
    TransactionId, Transform,
};
use sophia_renderer_live::{LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveProductionCpuScene};
use std::fs::File;
use std::os::fd::OwnedFd;
use std::sync::Arc;

#[test]
fn same_surface_present_flood_keeps_one_newest_deferred_candidate() {
    const SUCCESSORS: u64 = 512;
    let size = Size {
        width: 64,
        height: 48,
    };
    let output = HeadlessOutput {
        id: OutputId::from_raw(1),
        size,
        scale: 1,
    };
    let surface = SurfaceId::new(110, 1);
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
        source: BufferSource::None,
        damage: Region::single(geometry),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 0,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    }];
    let handles = [
        BufferHandle::from_raw(111),
        BufferHandle::from_raw(112),
        BufferHandle::from_raw(113),
    ];
    let group =
        |transaction: TransactionId, handle: BufferHandle, acquire_fence: Option<FenceHandle>| {
            LiveProductionAuthorityGroup {
                transaction,
                transactions: vec![SurfaceTransaction {
                    transaction,
                    authority: AuthorityKind::SophiaX,
                    surface,
                    namespace: None,
                    target_geometry: geometry,
                    target_buffer: BufferSource::DmaBuf {
                        handle: handle.raw(),
                    },
                    damage: Region::single(geometry),
                    readiness: SurfaceTransactionReadiness::Ready,
                    timeout_msec: 250,
                    previous_committed_generation: 0,
                }],
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
    let acquire_handle = FenceHandle::from_raw(114);
    let acquire_fence = sophia_xshmfence::allocate().unwrap();
    let initial_transaction = TransactionId::from_raw(1_000);
    let initial = LiveProductionAuthorityBatch {
        groups: vec![group(initial_transaction, handles[0], Some(acquire_handle))],
        dma_buf_registrations: handles.into_iter().map(registration).collect(),
        fence_registrations: vec![LiveProductionFenceRegistration {
            handle: acquire_handle,
            initially_triggered: false,
            fd: Arc::new(acquire_fence),
        }],
        released_dma_bufs: Vec::new(),
        released_fences: Vec::new(),
    };
    let mut scene = LiveProductionCpuScene::new(size);
    let mut runtime = LiveProductionVisualRuntime::new(&[output], None, None).unwrap();
    let mut run = |batch: &LiveProductionAuthorityBatch| {
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
                staged_cpu_buffer_handles: &[],
            })
            .unwrap();
    };
    run(&initial);
    for index in 0..SUCCESSORS {
        let transaction = TransactionId::from_raw(2_000 + index);
        let batch = LiveProductionAuthorityBatch {
            groups: vec![group(
                transaction,
                handles[1 + usize::try_from(index % 2).unwrap()],
                None,
            )],
            dma_buf_registrations: Vec::new(),
            fence_registrations: Vec::new(),
            released_dma_bufs: Vec::new(),
            released_fences: Vec::new(),
        };
        run(&batch);
    }

    let diagnostics = runtime.diagnostics();
    assert_eq!(diagnostics.pending_supersessions, 511);
    assert_eq!(diagnostics.surface_content_supersessions, 511);
    assert_eq!(diagnostics.scheduler_supersessions, 0);
    assert_eq!(diagnostics.max_surface_content_deferred, 1);
    assert_eq!(diagnostics.max_latest_deferred_per_surface, 1);
    assert_eq!(diagnostics.max_pending_queued, 1);
    assert_eq!(diagnostics.max_total_queued, 1);
    assert_eq!(diagnostics.live_sources, 3);
    assert_eq!(diagnostics.live_presentations, 1);
    assert_eq!(diagnostics.max_live_presentations, 2);

    let mut feedback = Vec::new();
    runtime.drain_present_feedback_into(&mut feedback).unwrap();
    assert_eq!(feedback.len(), 511);
    for (index, outcome) in feedback.iter().enumerate() {
        let transaction = TransactionId::from_raw(2_000 + index as u64);
        assert_eq!(
            outcome.feedback,
            vec![
                LivePresentProtocolFeedback::Complete {
                    transaction,
                    ust: 0,
                    msc: 0,
                    disposition: LivePresentBufferDisposition::Skipped,
                },
                LivePresentProtocolFeedback::Idle { transaction },
            ]
        );
    }

    runtime.shutdown_presentations().unwrap();
    assert_eq!(runtime.diagnostics().present_rejections, 513);
    assert_eq!(runtime.diagnostics().native_suspend_present_rejections, 0);
    assert_eq!(runtime.diagnostics().shutdown_present_rejections, 2);
    assert_eq!(runtime.diagnostics().other_present_rejections, 0);
    feedback.clear();
    runtime.drain_present_feedback_into(&mut feedback).unwrap();
    assert_eq!(feedback.len(), 2);
    assert_eq!(
        feedback
            .iter()
            .map(|outcome| outcome.feedback[0])
            .collect::<Vec<_>>(),
        vec![
            LivePresentProtocolFeedback::Complete {
                transaction: initial_transaction,
                ust: 0,
                msc: 0,
                disposition: LivePresentBufferDisposition::Skipped,
            },
            LivePresentProtocolFeedback::Complete {
                transaction: TransactionId::from_raw(2_000 + SUCCESSORS - 1),
                ust: 0,
                msc: 0,
                disposition: LivePresentBufferDisposition::Skipped,
            },
        ]
    );
    assert_eq!(runtime.diagnostics().live_sources, 0);
    assert_eq!(runtime.diagnostics().live_presentations, 0);
}
