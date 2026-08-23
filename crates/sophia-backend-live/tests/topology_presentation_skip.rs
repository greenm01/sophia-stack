#![cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]

//! Forcing presentation quiescence for a topology change.
//!
//! The owner waits for every present owner to settle before applying a new
//! output topology, but it cannot make clients stop drawing, and the owners it
//! waits on advance only while it waits. When that wait expires it skips what
//! is outstanding rather than reporting a stall it never tried to clear. Every
//! present given up that way still owes its client feedback.

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

const SIZE: Size = Size {
    width: 64,
    height: 48,
};

fn geometry() -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: SIZE.width,
        height: SIZE.height,
    }
}

fn layer(surface: SurfaceId) -> LayerSnapshot {
    LayerSnapshot {
        surface,
        authority_local_id: None,
        namespace: None,
        stack_rank: 0,
        geometry: geometry(),
        source_size: Size {
            width: (geometry()).width,
            height: (geometry()).height,
        },
        source: BufferSource::None,
        damage: Region::single(geometry()),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 0,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    }
}

fn group(
    transaction: TransactionId,
    surface: SurfaceId,
    handle: BufferHandle,
    disposition: LiveProductionPresentDisposition,
    acquire_fence: Option<FenceHandle>,
) -> LiveProductionAuthorityGroup {
    LiveProductionAuthorityGroup {
        transaction,
        transactions: vec![SurfaceTransaction {
            transaction,
            authority: AuthorityKind::SophiaX,
            surface,
            namespace: None,
            target_geometry: geometry(),
            presentation_extent: Size {
                width: (geometry()).width,
                height: (geometry()).height,
            },
            content: sophia_protocol::SurfaceContentSet::singleton(
                BufferSource::DmaBuf {
                    handle: handle.raw(),
                },
                SIZE,
            ),
            damage: Region::single(geometry()),
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
            layout_disposition: disposition,
        }],
        software_present_submissions: Vec::new(),
    }
}

fn registration(handle: BufferHandle) -> LiveProductionDmaBufRegistration {
    LiveProductionDmaBufRegistration {
        descriptor: DmaBufDescriptor {
            handle,
            size: SIZE,
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
    }
}

struct Fixture {
    runtime: LiveProductionVisualRuntime,
    scene: LiveProductionCpuScene,
    output: HeadlessOutput,
    layout: [LayerSnapshot; 1],
}

fn fixture(surface: SurfaceId, handles: &[BufferHandle]) -> Fixture {
    let output = HeadlessOutput {
        id: OutputId::from_raw(1),
        size: SIZE,
        scale: 1,
    };
    let mut runtime = LiveProductionVisualRuntime::new(&[output], None).unwrap();
    let mut scene = LiveProductionCpuScene::new(SIZE);
    let layout = [layer(surface)];
    let registrations = LiveProductionAuthorityBatch {
        groups: Vec::new(),
        dma_buf_registrations: handles.iter().copied().map(registration).collect(),
        fence_registrations: Vec::new(),
        released_dma_bufs: Vec::new(),
        released_fences: Vec::new(),
    };
    runtime
        .run_gpu_production_cycle(LiveProductionCycleRequest {
            batch: &registrations,
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
    Fixture {
        runtime,
        scene,
        output,
        layout,
    }
}

impl Fixture {
    fn run(&mut self, batch: &LiveProductionAuthorityBatch) {
        self.runtime
            .run_gpu_production_cycle(LiveProductionCycleRequest {
                batch,
                scene: &mut self.scene,
                raised_surface: None,
                focused_surface: None,
                cursor_presentation: LiveProductionCursorPresentation::Software(None),
                defer_frame: false,
                output_descriptors: &[self.output],
                native_scanout: None,
                wm_update: None,
                presentation_layout: &self.layout,
                chrome_surfaces: &[],
                indicator_publication: None,
                staged_cpu_buffer_handles: &[],
            })
            .unwrap();
    }

    fn batch(&self, group: LiveProductionAuthorityGroup) -> LiveProductionAuthorityBatch {
        LiveProductionAuthorityBatch {
            groups: vec![group],
            dma_buf_registrations: Vec::new(),
            fence_registrations: Vec::new(),
            released_dma_bufs: Vec::new(),
            released_fences: Vec::new(),
        }
    }
}

#[test]
fn topology_skip_settles_runnable_presents_and_reaches_quiescence() {
    let surface = SurfaceId::new(310, 1);
    let handle = BufferHandle::from_raw(311);
    let transaction = TransactionId::from_raw(312);
    let mut fixture = fixture(surface, &[handle]);

    // An acquire fence that never triggers holds the present queued. Its layout
    // state stays runnable, which is precisely what a topology wait blocks on.
    let acquire_handle = FenceHandle::from_raw(313);
    let batch = LiveProductionAuthorityBatch {
        groups: vec![group(
            transaction,
            surface,
            handle,
            LiveProductionPresentDisposition::Immediate,
            Some(acquire_handle),
        )],
        dma_buf_registrations: Vec::new(),
        fence_registrations: vec![LiveProductionFenceRegistration {
            handle: acquire_handle,
            initially_triggered: false,
            fd: Arc::new(sophia_xshmfence::allocate().unwrap()),
        }],
        released_dma_bufs: Vec::new(),
        released_fences: Vec::new(),
    };
    fixture.run(&batch);
    assert!(
        !fixture.runtime.topology_rebind_quiescent(),
        "a queued present is exactly what blocks a rebind"
    );

    let report = fixture.runtime.skip_presentations_for_topology(None);

    assert!(!report.is_empty());
    assert_eq!(
        report
            .skipped_queued
            .saturating_add(usize::from(report.skipped_in_flight.is_some())),
        1
    );
    assert!(
        fixture.runtime.topology_rebind_quiescent(),
        "skipping is only worth doing if it actually reaches quiescence: {}",
        fixture.runtime.topology_rebind_quiescence_report()
    );

    // The client is told its buffer will never reach a screen, rather than
    // being left waiting on feedback that can no longer arrive.
    let mut feedback = Vec::new();
    fixture
        .runtime
        .drain_present_feedback_into(&mut feedback)
        .unwrap();
    let settled = feedback
        .iter()
        .flat_map(|outcome| outcome.feedback.iter())
        .filter(|entry| {
            matches!(
                entry,
                LivePresentProtocolFeedback::Complete {
                    transaction: settled,
                    disposition: LivePresentBufferDisposition::Skipped,
                    ..
                } if *settled == transaction
            )
        })
        .count();
    assert_eq!(settled, 1, "the skipped present owes its client feedback");
    assert_eq!(
        fixture
            .runtime
            .diagnostics()
            .topology_escalation_present_rejections,
        1
    );
}

/// Nothing is skipped when the wait could simply have proceeded.
#[test]
fn topology_skip_is_empty_when_nothing_is_outstanding() {
    let surface = SurfaceId::new(320, 1);
    let handle = BufferHandle::from_raw(321);
    let mut fixture = fixture(surface, &[handle]);

    assert!(fixture.runtime.topology_rebind_quiescent());
    let report = fixture.runtime.skip_presentations_for_topology(None);

    assert!(report.is_empty());
    assert_eq!(
        fixture
            .runtime
            .diagnostics()
            .topology_escalation_present_rejections,
        0
    );
    assert!(fixture.runtime.topology_rebind_quiescent());
}
