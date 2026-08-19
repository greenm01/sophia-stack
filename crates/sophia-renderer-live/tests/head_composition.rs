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
    LiveCpuPresentationLayer, LiveHeadCompositionLoweringError, LiveOwnedDmaBufPlane,
    LiveOwnedHeadCompositionSource, LiveOwnedHeadCompositionSourceKind,
    LiveOwnedMixedCompositionLayer, LiveOwnedMultiPlaneDmaBufFrame, LiveProductionCpuScene,
    LiveRendererImageId, lower_cpu_head_composition_plan, lower_head_composition_plan,
};
use std::os::fd::{AsRawFd, OwnedFd};

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
fn retained_renderer_image_uses_head_plan_geometry_instead_of_primary_geometry() {
    let mut plan = plan();
    plan.layers[0].source = BufferSource::DmaBuf { handle: 77 };
    let source = LiveOwnedHeadCompositionSource {
        surface: SurfaceId::new(3, 1),
        source: BufferSource::DmaBuf { handle: 77 },
        kind: LiveOwnedHeadCompositionSourceKind::RendererImage {
            image_id: LiveRendererImageId::from_raw(9),
            size: Size {
                width: 600,
                height: 450,
            },
            format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        },
    };

    let frame = lower_head_composition_plan(&plan, &[source]).unwrap();
    let LiveOwnedMixedCompositionLayer::RendererImage {
        image_id,
        placement,
        ..
    } = frame.layers[0]
    else {
        panic!("retained DMA-BUF did not lower to its per-head renderer image")
    };
    assert_eq!(image_id, LiveRendererImageId::from_raw(9));
    assert_eq!(placement.target, plan.layers[0].native_geometry);
    assert_eq!(placement.clip, Some(plan.layers[0].native_clip));
}

/// A retained image is a copy, so its size is its own fact.
///
/// The plan measures the surface's committed buffer; a renderer image holds
/// the compositor's copy of an earlier generation under that same identity.
/// The two agree only while the surface has not resized, and a live session
/// ended when they stopped agreeing -- a mirror surface whose committed buffer
/// had grown to 2560x1440 while the copy still held 1280x1440. Placement
/// carries the copy to the head at whatever size the head wants; a mirror
/// member already draws every retained image at a size of its own.
#[test]
fn retained_renderer_image_may_hold_a_generation_of_another_size() {
    let mut plan = plan();
    plan.layers[0].source = BufferSource::DmaBuf { handle: 77 };
    let source = LiveOwnedHeadCompositionSource {
        surface: SurfaceId::new(3, 1),
        source: BufferSource::DmaBuf { handle: 77 },
        kind: LiveOwnedHeadCompositionSourceKind::RendererImage {
            image_id: LiveRendererImageId::from_raw(9),
            size: Size {
                width: 300,
                height: 450,
            },
            format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        },
    };

    let frame = lower_head_composition_plan(&plan, &[source]).unwrap();
    let LiveOwnedMixedCompositionLayer::RendererImage { placement, .. } = frame.layers[0] else {
        panic!("retained copy of another generation did not lower")
    };
    assert_eq!(placement.target, plan.layers[0].native_geometry);
}

/// A buffer the plan measured must be the buffer that arrives.
#[test]
fn cpu_source_of_the_wrong_size_is_still_refused() {
    let plan = plan();
    let source = LiveOwnedHeadCompositionSource {
        surface: SurfaceId::new(3, 1),
        source: BufferSource::CpuBuffer { handle: 42 },
        kind: LiveOwnedHeadCompositionSourceKind::Cpu(
            LiveCpuBufferSource {
                handle: 42,
                size: Size {
                    width: 300,
                    height: 450,
                },
                stride: 1_200,
                format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                generation: 9,
                bytes: vec![0; 1_200 * 450],
            }
            .into(),
        ),
    };

    assert_eq!(
        lower_head_composition_plan(&plan, &[source]).unwrap_err(),
        LiveHeadCompositionLoweringError::SourceSizeMismatch {
            surface: SurfaceId::new(3, 1),
            handle: 42,
            planned: Size {
                width: 600,
                height: 450,
            },
            held: Size {
                width: 300,
                height: 450,
            },
        }
    );
}

#[test]
fn dma_buf_source_duplicates_its_affine_plane_for_each_head_frame() {
    let mut plan = plan();
    plan.layers[0].source = BufferSource::DmaBuf { handle: 88 };
    let fd: OwnedFd = std::fs::File::open("/dev/null").unwrap().into();
    let source_fd = fd.as_raw_fd();
    let source = LiveOwnedHeadCompositionSource {
        surface: SurfaceId::new(3, 1),
        source: BufferSource::DmaBuf { handle: 88 },
        kind: LiveOwnedHeadCompositionSourceKind::DmaBuf {
            image_id: LiveRendererImageId::from_raw(10),
            frame: LiveOwnedMultiPlaneDmaBufFrame {
                width: 600,
                height: 450,
                format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                modifier: 0,
                plane_count: 1,
                planes: [
                    Some(LiveOwnedDmaBufPlane {
                        fd,
                        offset: 0,
                        stride: 2_400,
                    }),
                    None,
                    None,
                    None,
                ],
            },
        },
    };

    let first = lower_head_composition_plan(&plan, std::slice::from_ref(&source)).unwrap();
    let second = lower_head_composition_plan(&plan, std::slice::from_ref(&source)).unwrap();
    let plane_fd = |frame: &sophia_renderer_live::LiveOwnedMixedCompositionFrame| {
        let LiveOwnedMixedCompositionLayer::DmaBuf { frame, .. } = &frame.layers[0] else {
            panic!("DMA-BUF source changed kind")
        };
        frame.planes[0].as_ref().unwrap().fd.as_raw_fd()
    };
    assert_ne!(plane_fd(&first), source_fd);
    assert_ne!(plane_fd(&second), source_fd);
    assert_ne!(plane_fd(&first), plane_fd(&second));
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
