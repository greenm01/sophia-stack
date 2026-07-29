#![cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]

use sophia_backend_live::{
    LivePresentCompletionMode, LivePresentProtocolFeedback, LiveProductionAuthorityBatch,
    LiveProductionAuthorityGroup, LiveProductionCursorPresentation, LiveProductionCycleRequest,
    LiveProductionSoftwarePresentSubmission, LiveProductionVisualRuntime,
};
use sophia_engine::HeadlessOutput;
use sophia_protocol::{
    AuthorityKind, BufferSource, LayerSnapshot, OutputId, Rect, Region, ResizeSyncCapability, Size,
    SurfaceId, SurfaceTransaction, SurfaceTransactionReadiness, TransactionId, Transform,
};
use sophia_renderer_live::{
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveCpuBufferSource, LiveCpuBufferUpdate,
    LiveProductionCpuScene,
};

#[test]
fn headless_cpu_present_routes_complete_then_idle_without_a_dmabuf() {
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
        target_buffer: BufferSource::CpuBuffer { handle: 72 },
        damage: Region::single(geometry),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };
    let batch = LiveProductionAuthorityBatch {
        groups: vec![LiveProductionAuthorityGroup {
            transaction,
            transactions: vec![surface_transaction.clone()],
            removed_surfaces: Vec::new(),
            present_submissions: Vec::new(),
            software_present_submissions: vec![LiveProductionSoftwarePresentSubmission {
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
        source: surface_transaction.target_buffer,
        damage: surface_transaction.damage.clone(),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 1,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    }];
    let mut scene = LiveProductionCpuScene::new(size);
    let mut runtime = LiveProductionVisualRuntime::new(&[output], None, None).unwrap();

    let (submission, committed) = runtime
        .run_cpu_production_cycle(LiveProductionCycleRequest {
            batch: &batch,
            scene: &mut scene,
            updates: vec![LiveCpuBufferUpdate::Replace(LiveCpuBufferSource {
                handle: 72,
                size,
                stride: 8,
                format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                generation: 1,
                bytes: vec![0xff; 8],
            })],
            raised_surface: None,
            focused_surface: Some(surface),
            cursor_presentation: LiveProductionCursorPresentation::Software(None),
            defer_frame: false,
            output_descriptors: &[output],
            native_scanout: None,
            wm_update: None,
            presentation_layout: &layout,
            chrome_surfaces: &[surface],
        })
        .unwrap();

    assert!(submission.composed);
    assert_eq!(committed.len(), 1);
    let mut outcomes = Vec::new();
    runtime.drain_present_feedback_into(&mut outcomes).unwrap();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(
        outcomes[0].feedback,
        [
            LivePresentProtocolFeedback::Complete {
                transaction,
                ust: 0,
                msc: 0,
                mode: LivePresentCompletionMode::Flip,
            },
            LivePresentProtocolFeedback::Idle { transaction },
        ]
    );
    assert_eq!(runtime.diagnostics().live_presentations, 0);
}
