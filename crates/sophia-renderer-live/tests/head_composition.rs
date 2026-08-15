#![cfg(feature = "gbm-probe")]

use sophia_engine::{
    HeadBindingOutcome, HeadCompositionPlan, HeadCompositorCommand, HeadLayerBinding,
    HeadLogicalTransform, HeadSamplingClass, OutputSceneCursor, RenderHeadId,
};
use sophia_protocol::{
    BufferSource, CommittedSurfaceState, OutputHeadMapping, OutputId, OutputTransform, Rect,
    Region, Size, SurfaceContentFidelity, SurfaceContentSet, SurfaceContentVariant, SurfaceId,
    SurfaceRasterTransform,
};
use sophia_renderer_live::{
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveCpuBufferSource, LiveCpuBufferUpdate,
    LiveCpuPresentationLayer, LiveHeadCompositionLoweringError, LiveOwnedMixedCompositionLayer,
    LiveProductionCpuScene, lower_cpu_head_composition_plan,
};

fn plan() -> HeadCompositionPlan {
    let surface = SurfaceId::new(3, 1);
    HeadCompositionPlan {
        output: OutputId::from_raw(1),
        scene_generation: 4,
        head: RenderHeadId::from_raw(7),
        target_generation: 2,
        native_size: Size {
            width: 1_920,
            height: 1_080,
        },
        target_transform: OutputTransform::Normal,
        mapping: OutputHeadMapping::Fit,
        transform: HeadLogicalTransform {
            source: Size {
                width: 2_560,
                height: 1_440,
            },
            projected_scene: Rect {
                x: 0,
                y: 0,
                width: 1_920,
                height: 1_080,
            },
        },
        layers: vec![HeadLayerBinding {
            surface,
            committed_generation: 8,
            variant: 2,
            source: BufferSource::CpuBuffer { handle: 42 },
            source_pixel_size: Size {
                width: 600,
                height: 450,
            },
            density_millis: 750,
            opacity_millis: 1_000,
            native_geometry: Rect {
                x: 75,
                y: 90,
                width: 600,
                height: 450,
            },
            native_clip: Rect {
                x: 75,
                y: 90,
                width: 600,
                height: 450,
            },
            requested_sampling: HeadSamplingClass::Exact,
            outcome: HeadBindingOutcome::Active,
        }],
        compositor: vec![HeadCompositorCommand::Surface { surface }],
        cursor: None::<OutputSceneCursor>,
        repaint: Region::single(Rect {
            x: 75,
            y: 90,
            width: 600,
            height: 450,
        }),
        logical_content_checksum: 0x55,
    }
}

fn source(handle: u64) -> LiveCpuPresentationLayer {
    LiveCpuPresentationLayer {
        surface: SurfaceId::new(3, 1),
        geometry: Rect {
            x: 100,
            y: 120,
            width: 800,
            height: 600,
        },
        buffer: LiveCpuBufferSource {
            handle,
            size: Size {
                width: 600,
                height: 450,
            },
            stride: 2_400,
            format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            generation: 9,
            bytes: vec![0; 2_400 * 450],
        },
    }
}

#[test]
fn lowers_the_selected_variant_at_head_native_geometry() {
    let frame = lower_cpu_head_composition_plan(&plan(), &[source(42)]).unwrap();
    let LiveOwnedMixedCompositionLayer::Cpu { buffer, placement } = &frame.layers[0] else {
        panic!("planned CPU surface did not remain a CPU layer")
    };
    assert_eq!(buffer.handle, 42);
    assert_eq!(placement.target.width, 600);
    assert_eq!(placement.target.height, 450);
    let damage = frame.output_damage_snapshot.unwrap();
    assert_eq!(damage.output.size.width, 1_920);
    assert_eq!(
        damage.surfaces[0].buffer,
        BufferSource::CpuBuffer { handle: 42 }
    );
}

#[test]
fn missing_selected_variant_never_falls_back_to_another_cpu_buffer() {
    assert_eq!(
        lower_cpu_head_composition_plan(&plan(), &[source(41)]).unwrap_err(),
        LiveHeadCompositionLoweringError::MissingCpuSource(42)
    );
}

#[test]
fn production_scene_resolves_all_resident_cpu_variants_by_handle() {
    let mut scene = LiveProductionCpuScene::new(Size {
        width: 800,
        height: 600,
    });
    let buffer = |handle, width, height, generation| LiveCpuBufferSource {
        handle,
        size: Size { width, height },
        stride: u32::try_from(width * 4).unwrap(),
        format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        generation,
        bytes: vec![0; usize::try_from(width * height * 4).unwrap()],
    };
    scene
        .apply_updates([
            LiveCpuBufferUpdate::Replace(buffer(41, 800, 600, 1)),
            LiveCpuBufferUpdate::Replace(buffer(42, 600, 450, 1)),
        ])
        .unwrap();
    let content = SurfaceContentSet::new(
        Size {
            width: 800,
            height: 600,
        },
        vec![
            SurfaceContentVariant {
                variant: 1,
                source: BufferSource::CpuBuffer { handle: 41 },
                pixel_size: Size {
                    width: 800,
                    height: 600,
                },
                density_millis: 1_000,
                transform: SurfaceRasterTransform::Normal,
                fidelity: SurfaceContentFidelity::AuthorityRaster,
                damage: Region::single(Rect {
                    x: 0,
                    y: 0,
                    width: 800,
                    height: 600,
                }),
            },
            SurfaceContentVariant {
                variant: 2,
                source: BufferSource::CpuBuffer { handle: 42 },
                pixel_size: Size {
                    width: 600,
                    height: 450,
                },
                density_millis: 750,
                transform: SurfaceRasterTransform::Normal,
                fidelity: SurfaceContentFidelity::AuthorityRaster,
                damage: Region::single(Rect {
                    x: 0,
                    y: 0,
                    width: 600,
                    height: 450,
                }),
            },
        ],
    )
    .unwrap();
    let surface = SurfaceId::new(3, 1);
    let layers = scene.presentation_variant_layers(
        &[CommittedSurfaceState {
            surface,
            committed_generation: 1,
            geometry: Rect {
                x: 0,
                y: 0,
                width: 800,
                height: 600,
            },
            content,
            damage: Region::empty(),
        }],
        &[surface],
    );
    assert_eq!(
        layers
            .iter()
            .map(|layer| layer.buffer.handle)
            .collect::<Vec<_>>(),
        vec![41, 42]
    );
}
