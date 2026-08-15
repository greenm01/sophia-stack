use super::*;
use sophia_engine::{
    CompositorBackendTickInput, HeadlessCompositorBackendAssembly, HeadlessOutput,
};
use sophia_protocol::{
    BufferSource, CommittedSurfaceState, OutputId, Rect, Region, Size, SurfaceId,
};
use sophia_renderer_live::{
    LiveRendererImportHealth, LiveRendererImportPathStatus, LiveRendererRuntimeObservation,
    LiveRendererSelectionObservation,
};
use std::sync::mpsc;

#[test]
fn mirror_shutdown_drain_accepts_callback_without_running_invalid_surface_projection() {
    let output = HeadlessOutput {
        id: OutputId::from_raw(1),
        size: Size {
            width: 640,
            height: 480,
        },
        scale: 1,
    };
    let (sender, receiver) = mpsc::sync_channel(1);
    sender
        .try_send(LivePageFlipCallback {
            output: output.id,
            connector_id: 11,
            frame_serial: 18,
        })
        .expect("test channel should accept the pending physical callback");
    let assembly = HeadlessCompositorBackendAssembly::new(output);
    let renderer = LiveRendererRuntimeObservation {
        health: LiveRendererImportHealth::CpuFallback,
        xpixmap: LiveRendererImportPathStatus::Disabled,
        dmabuf: LiveRendererImportPathStatus::Disabled,
        selection: LiveRendererSelectionObservation::CpuFallback,
    };
    let mut runtime =
        LiveBackendRuntimeAssembly::from_ready_headless_scanout(assembly, output, renderer)
            .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 1));
    runtime
        .assembly_mut()
        .replace_committed_surfaces(vec![CommittedSurfaceState {
            surface: SurfaceId::new(91, 1),
            committed_generation: 1,
            geometry: Rect {
                x: 0,
                y: 0,
                width: 64,
                height: 48,
            },
            buffer: BufferSource::CpuBuffer { handle: 7 },
            damage: Region::empty(),
        }]);
    let before = runtime.page_flip_observation();

    let drained = runtime.drain_mirror_page_flip_callback_queue();

    assert_eq!(drained.accepted, 1);
    assert_eq!(drained.accepted_callbacks[0].connector_id, 11);
    assert_eq!(drained.accepted_callbacks[0].frame_serial, 18);
    // A physical mirror-head callback is not a logical-output presentation
    // until the native group coordinator has joined every required head.
    assert_eq!(runtime.page_flip_observation(), before);
    let error = runtime
        .run_tick(CompositorBackendTickInput::default())
        .expect_err("the fixture must retain the invalid-surface Engine failure");
    assert!(error.to_string().contains("invalid surface"));
}
