#![cfg(feature = "gbm-probe")]

use sophia_engine::DirectScanoutVerdict;
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
        // This fixture is a composed multi-layer plan; the renderer's own
        // tests do not exercise eligibility, which Engine decides.
        direct_scanout: DirectScanoutVerdict::default(),
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
    assert_eq!(placement.sampling, HeadSamplingClass::Exact);
    assert_eq!(frame.trace.unwrap().head, RenderHeadId::from_raw(7));
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
    assert_eq!(placement.sampling, HeadSamplingClass::Upsampled);
    assert_eq!(
        frame.output_damage_snapshot.unwrap().surfaces[0].source_size,
        Size {
            width: 300,
            height: 450,
        },
    );
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

/// The lowerer clips each band, and clips them one at a time.
///
/// This exercises the real lowering rather than a reimplementation of it, which
/// matters because the arithmetic is easy to mirror correctly in a test while
/// the production path does something else. The window here runs far off the
/// right of the scene: its left, top and bottom bands are partly visible and
/// must be trimmed to the scene, and its right band is entirely outside and must
/// disappear rather than reappear against the boundary.
#[test]
fn border_bands_are_clipped_individually_to_the_scene_they_belong_to() {
    let scene = Rect {
        x: 320,
        y: 180,
        width: 1_920,
        height: 1_080,
    };
    let inner = Rect {
        x: 400,
        y: 300,
        width: 4_000,
        height: 400,
    };
    let outer = Rect {
        x: 396,
        y: 296,
        width: 4_008,
        height: 408,
    };

    let mut plan = plan();
    plan.native_size = Size {
        width: 2_560,
        height: 1_440,
    };
    plan.transform.projected_scene = scene;
    plan.layers.clear();
    plan.compositor = vec![HeadCompositorCommand::Border(
        sophia_engine::HeadCompositorBorder {
            node: sophia_engine::CompositorNodeId::SurfaceChrome {
                surface: SurfaceId::new(3, 1),
                role: sophia_engine::SurfaceChromeRole::Frame,
            },
            generation: 3,
            outer,
            inner,
            color: sophia_engine::CompositorRgb8 {
                red: 0,
                green: 80,
                blue: 255,
            },
            clip: scene,
        },
    )];

    let frame = lower_cpu_head_composition_plan(&plan, &[]).unwrap();
    let bands = frame
        .layers
        .iter()
        .filter_map(|layer| match layer {
            LiveOwnedMixedCompositionLayer::Solid { geometry, .. } => Some(*geometry),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(
        !bands.is_empty(),
        "the visible bands of a straddling window must still be drawn"
    );
    let right_edge = scene.x + scene.width;
    for band in &bands {
        assert!(
            band.x >= scene.x
                && band.y >= scene.y
                && band.x + band.width <= right_edge
                && band.y + band.height <= scene.y + scene.height,
            "band {band:?} escapes the scene {scene:?}"
        );
        // Clipping outer and inner before subtracting them would produce a
        // vertical band flush against the boundary: a border down the side of a
        // window that is only running off the screen.
        assert!(
            !(band.width <= 8 && band.x + band.width == right_edge),
            "band {band:?} is the invented border at the scene's edge"
        );
    }
    // The real right band lies wholly outside and must simply be gone.
    assert!(
        bands.iter().all(|band| band.x < inner.x + inner.width),
        "a band survived from beyond the window's own right edge: {bands:?}"
    );

    // A window lying entirely outside this scene contributes nothing at all.
    //
    // This is the case that separates clipping the bands from clipping the rects
    // they come from. Clipping `outer` and `inner` here leaves both degenerate,
    // and their difference is still positive, so that approach emits a band at
    // the window's original off-screen coordinates while reporting itself as
    // clipped. Only clipping the result removes it.
    let elsewhere = Rect {
        x: 3_000,
        y: 300,
        width: 400,
        height: 400,
    };
    let HeadCompositorCommand::Border(mut border) = plan.compositor[0] else {
        unreachable!("the fixture holds exactly one border command");
    };
    border.inner = elsewhere;
    border.outer = Rect {
        x: elsewhere.x - 4,
        y: elsewhere.y - 4,
        width: elsewhere.width + 8,
        height: elsewhere.height + 8,
    };
    plan.compositor = vec![HeadCompositorCommand::Border(border)];

    let frame = lower_cpu_head_composition_plan(&plan, &[]).unwrap();
    let escaped = frame
        .layers
        .iter()
        .filter_map(|layer| match layer {
            LiveOwnedMixedCompositionLayer::Solid { geometry, .. } => Some(*geometry),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        escaped.is_empty(),
        "a window outside the scene still drew bands: {escaped:?}"
    );
}

/// A lowered frame that Engine proved needs no composition: one client
/// DMA-BUF covering the head exactly, opaque, unscaled, unclipped.
fn direct_frame() -> sophia_renderer_live::LiveOwnedMixedCompositionFrame {
    let fd: OwnedFd = std::fs::File::open("/dev/null").unwrap().into();
    sophia_renderer_live::LiveOwnedMixedCompositionFrame {
        layers: vec![LiveOwnedMixedCompositionLayer::DmaBuf {
            image_id: LiveRendererImageId::from_raw(7),
            frame: LiveOwnedMultiPlaneDmaBufFrame {
                width: 640,
                height: 480,
                format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                modifier: 0x0100_0000_0000_0001,
                plane_count: 1,
                planes: [
                    Some(LiveOwnedDmaBufPlane {
                        fd,
                        offset: 0,
                        stride: 2_560,
                    }),
                    None,
                    None,
                    None,
                ],
            },
            placement: sophia_renderer_live::LiveCompositionPlacement {
                target: Rect {
                    x: 0,
                    y: 0,
                    width: 640,
                    height: 480,
                },
                clip: None,
                transform: sophia_protocol::Transform::IDENTITY,
                alpha: 1.0,
                sampling: HeadSamplingClass::Exact,
            },
        }],
        output_damage_snapshot: None,
        trace: None,
        direct_scanout: DirectScanoutVerdict::Eligible,
    }
}

const DIRECT_HEAD: Size = Size {
    width: 640,
    height: 480,
};

fn direct_layer_placement(
    frame: &mut sophia_renderer_live::LiveOwnedMixedCompositionFrame,
) -> &mut sophia_renderer_live::LiveCompositionPlacement {
    let LiveOwnedMixedCompositionLayer::DmaBuf { placement, .. } = &mut frame.layers[0] else {
        panic!("direct fixture changed layer kind")
    };
    placement
}

fn direct_layer_frame(
    frame: &mut sophia_renderer_live::LiveOwnedMixedCompositionFrame,
) -> &mut LiveOwnedMultiPlaneDmaBufFrame {
    let LiveOwnedMixedCompositionLayer::DmaBuf { frame, .. } = &mut frame.layers[0] else {
        panic!("direct fixture changed layer kind")
    };
    frame
}

#[test]
fn proven_full_head_client_buffer_becomes_a_plane_descriptor() {
    let frame = direct_frame();
    let buffer = frame.direct_scanout_buffer(DIRECT_HEAD).unwrap();

    // The descriptor describes the client's buffer, not a compositor one: its
    // format, its stride, and its modifier reach AddFB2 unchanged.
    assert_eq!(buffer.descriptor.size, DIRECT_HEAD);
    assert_eq!(buffer.descriptor.format, LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888);
    assert_eq!(buffer.descriptor.pitch, 2_560);
    assert_eq!(buffer.descriptor.plane_count, 1);
    assert_eq!(buffer.descriptor.modifier, Some(0x0100_0000_0000_0001));
    assert!(buffer.descriptor.is_valid_scanout_buffer());
    assert_eq!(buffer.image_id, LiveRendererImageId::from_raw(7));
    assert!(buffer.planes[0].is_some());
    assert!(buffer.planes[1].is_none());

    // Duplicated, not taken: the frame it came from is still whole, which is
    // what lets it be composed instead if the driver refuses the buffer.
    let original = direct_layer_frame(&mut { frame }).planes[0]
        .as_ref()
        .map(|plane| plane.fd.as_raw_fd());
    assert!(original.is_some());
    assert_ne!(
        original.unwrap(),
        buffer.planes[0].as_ref().unwrap().fd.as_raw_fd()
    );
}

#[test]
fn an_unproven_frame_is_refused_however_it_is_shaped() {
    // Structurally identical to the eligible fixture. Only the proof differs,
    // so this is exactly the claim that the backend does not decide
    // eligibility for itself.
    let mut frame = direct_frame();
    frame.direct_scanout = DirectScanoutVerdict::default();
    assert_eq!(
        frame.direct_scanout_buffer(DIRECT_HEAD).unwrap_err(),
        sophia_renderer_live::LiveDirectScanoutRefusal::NotProven(
            DirectScanoutVerdict::default()
        )
    );
}

#[test]
fn a_proof_that_disagrees_with_the_pixels_refuses_rather_than_flips() {
    // A verdict claiming eligibility over a frame that lowered to chrome as
    // well. Trusting the stamp here would put a client's buffer on the plane
    // and drop everything drawn over it.
    let mut frame = direct_frame();
    frame.layers.push(LiveOwnedMixedCompositionLayer::Solid {
        geometry: Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 24,
        },
        color: sophia_engine::CompositorRgb8 {
            red: 0,
            green: 0,
            blue: 0,
        },
    });
    assert_eq!(
        frame.direct_scanout_buffer(DIRECT_HEAD).unwrap_err(),
        sophia_renderer_live::LiveDirectScanoutRefusal::LayerCount(2)
    );
}

#[test]
fn translucency_scaling_clipping_and_partial_cover_each_refuse() {
    use sophia_renderer_live::LiveDirectScanoutRefusal as Refusal;

    let mut translucent = direct_frame();
    direct_layer_placement(&mut translucent).alpha = 0.5;
    assert_eq!(
        translucent.direct_scanout_buffer(DIRECT_HEAD).unwrap_err(),
        Refusal::LayerTranslucent
    );

    let mut resampled = direct_frame();
    direct_layer_placement(&mut resampled).sampling = HeadSamplingClass::Upsampled;
    assert_eq!(
        resampled.direct_scanout_buffer(DIRECT_HEAD).unwrap_err(),
        Refusal::LayerResampled
    );

    let mut clipped = direct_frame();
    direct_layer_placement(&mut clipped).clip = Some(Rect {
        x: 0,
        y: 0,
        width: 640,
        height: 240,
    });
    assert_eq!(
        clipped.direct_scanout_buffer(DIRECT_HEAD).unwrap_err(),
        Refusal::LayerClipped
    );

    let mut partial = direct_frame();
    direct_layer_placement(&mut partial).target = Rect {
        x: 0,
        y: 0,
        width: 640,
        height: 400,
    };
    assert_eq!(
        partial.direct_scanout_buffer(DIRECT_HEAD).unwrap_err(),
        Refusal::LayerNotHeadSized
    );

    // A clip that names the whole head clips nothing and is not a refusal.
    let mut whole = direct_frame();
    direct_layer_placement(&mut whole).clip = Some(Rect {
        x: 0,
        y: 0,
        width: 640,
        height: 480,
    });
    assert!(whole.direct_scanout_buffer(DIRECT_HEAD).is_ok());
}

#[test]
fn argb_is_not_opaque_enough_to_scan_out() {
    // ARGB8888 is an accepted scanout format for a compositor buffer, whose
    // alpha the compositor controls. A client's alpha is part of the image,
    // and nothing behind it would be drawn on a plane.
    let mut frame = direct_frame();
    direct_layer_frame(&mut frame).format =
        sophia_renderer_live::LIVE_RENDERER_SCANOUT_FORMAT_ARGB8888;
    assert_eq!(
        frame.direct_scanout_buffer(DIRECT_HEAD).unwrap_err(),
        sophia_renderer_live::LiveDirectScanoutRefusal::FormatNotOpaque(
            sophia_renderer_live::LIVE_RENDERER_SCANOUT_FORMAT_ARGB8888
        )
    );
}

#[test]
fn a_buffer_smaller_than_the_head_refuses_even_when_its_placement_covers() {
    // The placement says the layer fills the head, but the buffer behind it
    // does not. A plane scans the buffer, not the placement.
    let mut frame = direct_frame();
    direct_layer_frame(&mut frame).height = 400;
    assert_eq!(
        frame.direct_scanout_buffer(DIRECT_HEAD).unwrap_err(),
        sophia_renderer_live::LiveDirectScanoutRefusal::BufferSizeMismatch
    );
}

#[test]
fn a_cpu_layer_has_no_buffer_to_hand_the_plane() {
    let mut frame = direct_frame();
    frame.layers[0] = LiveOwnedMixedCompositionLayer::Solid {
        geometry: Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        },
        color: sophia_engine::CompositorRgb8 {
            red: 1,
            green: 2,
            blue: 3,
        },
    };
    assert_eq!(
        frame.direct_scanout_buffer(DIRECT_HEAD).unwrap_err(),
        sophia_renderer_live::LiveDirectScanoutRefusal::LayerNotDmaBuf
    );
}

#[test]
fn lowering_carries_the_plans_verdict_onto_the_frame_it_produces() {
    // The verdict has to survive lowering, because the backend reads it from
    // the frame and never sees the plan.
    let mut plan = plan();
    plan.direct_scanout = DirectScanoutVerdict::Eligible;
    let lowered = lower_cpu_head_composition_plan(&plan, &[source(42)]).unwrap();
    assert_eq!(lowered.direct_scanout, DirectScanoutVerdict::Eligible);

    // And the default that arrives with no plan behind it composes, so a
    // frame built anywhere else cannot be mistaken for a proven one.
    assert_eq!(
        sophia_renderer_live::LiveOwnedMixedCompositionFrame::default().direct_scanout,
        DirectScanoutVerdict::default()
    );
}
