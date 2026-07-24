use super::*;

#[test]
fn frame_snapshot_replay_rejects_unknown_surface() {
    let engine = HeadlessEngine::default();
    let request = FramePlanRequest {
        output: engine.output().id,
        frame_serial: 12,
    };
    let mut frame = engine
        .plan_frame(request, vec![test_layer(0, 0, 0, Region::empty())])
        .unwrap();
    frame.commands[0].source = Some(SurfaceId::new(99, 1));
    assert_eq!(
        engine.replay_frame(&frame),
        Err(EngineError::InvalidSurface)
    );
}

#[test]
fn render_frame_reports_cpu_fallback_imports() {
    let engine = HeadlessEngine::default();
    let request = FramePlanRequest {
        output: engine.output().id,
        frame_serial: 13,
    };
    let cpu_layer = test_layer(0, 0, 0, Region::empty());
    let mut dma_layer = test_layer(1, 1, 100, Region::empty());
    dma_layer.source = BufferSource::DmaBuf { handle: 99 };
    let frame = engine
        .plan_frame(request, vec![cpu_layer, dma_layer])
        .unwrap();
    let rendered = engine.render_frame(&frame).unwrap();
    assert_eq!(rendered.replay.frame_serial, 13);
    assert_eq!(rendered.replay.steps.len(), 2);
    assert_eq!(rendered.imports.len(), 2);
    assert_eq!(rendered.imports[0].requested, BufferImportPath::CpuReadback);
    assert_eq!(rendered.imports[0].used, BufferImportPath::CpuReadback);
    assert_eq!(
        rendered.imports[0].handle,
        ImportedBufferHandle::CpuReadback {
            source: rendered.imports[0].source
        }
    );
    assert!(!rendered.imports[0].used_fallback);
    assert_eq!(rendered.imports[1].requested, BufferImportPath::DmaBuf);
    assert_eq!(rendered.imports[1].used, BufferImportPath::CpuReadback);
    assert_eq!(
        rendered.imports[1].handle,
        ImportedBufferHandle::CpuReadback {
            source: BufferSource::DmaBuf { handle: 99 }
        }
    );
    assert!(rendered.imports[1].used_fallback);
}

#[test]
fn import_capable_renderer_uses_native_buffer_handles_when_supported() {
    let engine = HeadlessEngine::default();
    let request = FramePlanRequest {
        output: engine.output().id,
        frame_serial: 15,
    };
    let mut xpixmap_layer = test_layer(0, 0, 0, Region::empty());
    xpixmap_layer.source = BufferSource::XPixmap { pixmap: 44 };
    let mut dmabuf_layer = test_layer(1, 1, 100, Region::empty());
    dmabuf_layer.source = BufferSource::DmaBuf { handle: 99 };
    let renderer = ImportCapableRenderer::new(true, true);
    let frame = engine
        .plan_frame(request, vec![xpixmap_layer, dmabuf_layer])
        .unwrap();
    let rendered = engine.render_frame_with(&renderer, &frame).unwrap();
    assert_eq!(rendered.imports.len(), 2);
    assert_eq!(rendered.imports[0].requested, BufferImportPath::XPixmap);
    assert_eq!(rendered.imports[0].used, BufferImportPath::XPixmap);
    assert_eq!(
        rendered.imports[0].handle,
        ImportedBufferHandle::XPixmap { pixmap: 44 }
    );
    assert!(!rendered.imports[0].used_fallback);
    assert_eq!(rendered.imports[1].requested, BufferImportPath::DmaBuf);
    assert_eq!(rendered.imports[1].used, BufferImportPath::DmaBuf);
    assert_eq!(
        rendered.imports[1].handle,
        ImportedBufferHandle::DmaBuf { handle: 99 }
    );
    assert!(!rendered.imports[1].used_fallback);
}

#[test]
fn import_capable_renderer_falls_back_for_unsupported_handles() {
    let engine = HeadlessEngine::default();
    let request = FramePlanRequest {
        output: engine.output().id,
        frame_serial: 16,
    };
    let mut xpixmap_layer = test_layer(0, 0, 0, Region::empty());
    xpixmap_layer.source = BufferSource::XPixmap { pixmap: 44 };
    let mut dmabuf_layer = test_layer(1, 1, 100, Region::empty());
    dmabuf_layer.source = BufferSource::DmaBuf { handle: 99 };
    let renderer = ImportCapableRenderer::new(false, true);
    let frame = engine
        .plan_frame(request, vec![xpixmap_layer, dmabuf_layer])
        .unwrap();
    let rendered = engine.render_frame_with(&renderer, &frame).unwrap();
    assert_eq!(rendered.imports[0].requested, BufferImportPath::XPixmap);
    assert_eq!(rendered.imports[0].used, BufferImportPath::CpuReadback);
    assert!(rendered.imports[0].used_fallback);
    assert_eq!(rendered.imports[1].requested, BufferImportPath::DmaBuf);
    assert_eq!(rendered.imports[1].used, BufferImportPath::DmaBuf);
    assert!(!rendered.imports[1].used_fallback);
}

#[test]
fn render_frame_reuses_replay_validation() {
    let engine = HeadlessEngine::default();
    let request = FramePlanRequest {
        output: engine.output().id,
        frame_serial: 14,
    };
    let mut frame = engine
        .plan_frame(request, vec![test_layer(0, 0, 0, Region::empty())])
        .unwrap();
    frame.commands[0].source = Some(SurfaceId::new(99, 1));
    assert_eq!(
        engine.render_frame(&frame).map(|report| report.imports),
        Err(EngineError::InvalidSurface)
    );
}
