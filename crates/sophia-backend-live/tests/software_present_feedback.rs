#![cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]

use sophia_backend_live::{
    LivePresentBufferDisposition, LivePresentProtocolFeedback, LiveProductionAuthorityBatch,
    LiveProductionAuthorityGroup, LiveProductionCursorPresentation, LiveProductionCycleRequest,
    LiveProductionSoftwarePresentSubmission, LiveProductionVisualRuntime,
};
use sophia_engine::HeadlessOutput;
use sophia_protocol::{
    AuthorityKind, BufferSource, LayerSnapshot, OutputId, Rect, Region, ResizeSyncCapability, Size,
    SurfaceId, SurfaceTransaction, SurfaceTransactionReadiness, TransactionId, Transform,
};
use sophia_renderer_live::{
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveCpuBufferPatch, LiveCpuBufferSource,
    LiveCpuBufferUpdate, LiveProductionCpuScene,
};

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
    let batch = LiveProductionAuthorityBatch {
        groups: Vec::new(),
        dma_buf_registrations: Vec::new(),
        fence_registrations: Vec::new(),
        released_dma_bufs: Vec::new(),
        released_fences: Vec::new(),
    };
    let mut scene = LiveProductionCpuScene::new(size);
    let mut runtime = LiveProductionVisualRuntime::new(&[output], None, None).unwrap();
    let run_update =
        |runtime: &mut LiveProductionVisualRuntime, scene: &mut LiveProductionCpuScene, updates| {
            runtime.run_cpu_production_cycle(LiveProductionCycleRequest {
                batch: &batch,
                scene,
                updates,
                raised_surface: None,
                focused_surface: None,
                cursor_presentation: LiveProductionCursorPresentation::Software(None),
                defer_frame: false,
                output_descriptors: &[output],
                native_scanout: None,
                wm_update: None,
                presentation_layout: &[],
                chrome_surfaces: &[],
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
fn staged_cpu_present_survives_until_transaction_release_and_routes_feedback() {
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
    let staging_batch = LiveProductionAuthorityBatch {
        groups: Vec::new(),
        dma_buf_registrations: Vec::new(),
        fence_registrations: Vec::new(),
        released_dma_bufs: Vec::new(),
        released_fences: Vec::new(),
    };
    let staged_handle = [72];

    runtime
        .run_cpu_production_cycle(LiveProductionCycleRequest {
            batch: &staging_batch,
            scene: &mut scene,
            updates: vec![LiveCpuBufferUpdate::Replace(LiveCpuBufferSource {
                handle: 72,
                size,
                stride: 8,
                format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                generation: 1,
                bytes: vec![0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0xff],
            })],
            raised_surface: None,
            focused_surface: None,
            cursor_presentation: LiveProductionCursorPresentation::Software(None),
            defer_frame: false,
            output_descriptors: &[output],
            native_scanout: None,
            wm_update: None,
            presentation_layout: &[],
            chrome_surfaces: &[],
            staged_cpu_buffer_handles: &staged_handle,
        })
        .unwrap();
    assert_eq!(scene.resident_buffer_count(), 1);

    let (submission, committed) = runtime
        .run_cpu_production_cycle(LiveProductionCycleRequest {
            batch: &batch,
            scene: &mut scene,
            updates: Vec::new(),
            raised_surface: None,
            focused_surface: Some(surface),
            cursor_presentation: LiveProductionCursorPresentation::Software(None),
            defer_frame: false,
            output_descriptors: &[output],
            native_scanout: None,
            wm_update: None,
            presentation_layout: &layout,
            chrome_surfaces: &[surface],
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
            LivePresentProtocolFeedback::Complete {
                transaction,
                ust: 0,
                msc: 0,
                disposition: LivePresentBufferDisposition::Copied,
            },
            LivePresentProtocolFeedback::Idle { transaction },
        ]
    );
    assert_eq!(runtime.diagnostics().live_presentations, 0);
}
