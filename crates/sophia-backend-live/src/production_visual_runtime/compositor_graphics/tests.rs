#![cfg(test)]

use super::*;

#[test]
fn in_flight_renderer_source_keeps_cpu_content_variants() {
    let surface = SurfaceId::new(7, 1);
    let cpu_handle = 4;
    let cpu_layers = [LiveCpuPresentationLayer {
        surface,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 64,
            height: 48,
        },
        buffer: sophia_renderer_live::LiveCpuBufferSource {
            handle: cpu_handle,
            size: Size {
                width: 64,
                height: 48,
            },
            stride: 64 * 4,
            format: sophia_renderer_live::LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            generation: 1,
            bytes: vec![0x7f; 64 * 48 * 4],
        },
    }];
    let dma_source = BufferSource::DmaBuf { handle: 3 };
    let displayed = crate::LiveRetainedRendererImageLayer {
        image_id: sophia_renderer_live::LiveRendererImageId::from_raw(9),
        size: Size {
            width: 64,
            height: 48,
        },
        format: sophia_renderer_live::LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        placement: sophia_renderer_live::LiveCompositionPlacement {
            target: cpu_layers[0].geometry,
            clip: None,
            transform: Transform::IDENTITY,
            alpha: 1.0,
            sampling: sophia_engine::HeadSamplingClass::Exact,
        },
    };

    let sources =
        retained_surface_sources(surface, dma_source, &cpu_layers, Some(&displayed), None)
            .expect("mixed retained source set");

    assert_eq!(sources.len(), 2);
    assert!(sources.iter().any(|source| {
        source.source == dma_source
            && matches!(
                source.kind,
                sophia_renderer_live::LiveOwnedHeadCompositionSourceKind::RendererImage { .. }
            )
    }));
    assert!(sources.iter().any(|source| {
        source.source == BufferSource::CpuBuffer { handle: cpu_handle }
            && matches!(
                source.kind,
                sophia_renderer_live::LiveOwnedHeadCompositionSourceKind::Cpu(_)
            )
    }));
}
