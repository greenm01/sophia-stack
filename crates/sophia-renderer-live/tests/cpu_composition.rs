use sophia_engine::{CompositorDisplayList, CompositorRgb8, HeadlessOutput};
use sophia_protocol::{
    BufferSource, CommittedSurfaceState, OutputId, Point, Rect, Region, Size, SurfaceId,
};
use sophia_renderer_live::{
    DEFAULT_CURSOR_EDGE, DEFAULT_CURSOR_HOTSPOT, DEFAULT_CURSOR_SHAPE,
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveCpuBufferSource, LiveCpuBufferSourceRef,
    LiveCpuBufferUpdate, LiveCpuCompositionElementRef, LiveCpuCompositionLayer,
    LiveCpuCompositionLayerRef, LiveProductionCpuScene, compose_live_cpu_display_list_frame,
    compose_live_cpu_frame, compose_live_cpu_frame_ref, compose_live_cpu_frame_ref_with_cursor,
};

#[test]
fn cpu_composition_blits_clipped_xrgb_layers() {
    let source = LiveCpuBufferSource {
        handle: 1,
        size: Size {
            width: 2,
            height: 2,
        },
        stride: 8,
        format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        generation: 1,
        bytes: vec![0xff; 16],
    };
    let report = compose_live_cpu_frame(
        Size {
            width: 3,
            height: 3,
        },
        &[LiveCpuCompositionLayer {
            geometry: Rect {
                x: 2,
                y: 2,
                width: 2,
                height: 2,
            },
            buffer: source,
        }],
    )
    .unwrap();

    assert_eq!(report.layers_input, 1);
    assert_eq!(report.layers_composed, 1);
    assert_eq!(report.nonzero_pixel_bytes, 4);
    assert_ne!(report.checksum, 0);
    assert_eq!(&report.frame.bytes[32..36], &[0xff; 4]);
}

#[test]
fn cpu_display_list_preserves_solid_order_and_clips_to_output() {
    let size = Size {
        width: 3,
        height: 2,
    };
    let pixels = vec![0x11; 3 * 2 * 4];
    let report = compose_live_cpu_display_list_frame(
        size,
        &[
            LiveCpuCompositionElementRef::Layer(LiveCpuCompositionLayerRef {
                geometry: Rect {
                    x: 0,
                    y: 0,
                    width: 3,
                    height: 2,
                },
                buffer: LiveCpuBufferSourceRef {
                    handle: 21,
                    size,
                    stride: 12,
                    format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                    generation: 1,
                    bytes: &pixels,
                },
            }),
            LiveCpuCompositionElementRef::Solid {
                geometry: Rect {
                    x: 2,
                    y: -1,
                    width: 3,
                    height: 3,
                },
                color: CompositorRgb8 {
                    red: 0x70,
                    green: 0xb7,
                    blue: 0xff,
                },
            },
        ],
        None,
    )
    .unwrap();

    assert_eq!(report.layers_input, 2);
    assert_eq!(report.layers_composed, 2);
    assert_eq!(&report.frame.bytes[0..4], &[0x11; 4]);
    assert_eq!(&report.frame.bytes[8..12], &[0xff, 0xb7, 0x70, 0xff]);
    assert_eq!(&report.frame.bytes[20..24], &[0xff, 0xb7, 0x70, 0xff]);
}

#[test]
fn borrowed_fullscreen_composition_preserves_pixels_and_metrics() {
    let pixels = vec![0x5a; 1280 * 720 * 4];
    let report = compose_live_cpu_frame_ref(
        Size {
            width: 1280,
            height: 720,
        },
        &[LiveCpuCompositionLayerRef {
            geometry: Rect {
                x: 0,
                y: 0,
                width: 1280,
                height: 720,
            },
            buffer: LiveCpuBufferSourceRef {
                handle: 7,
                size: Size {
                    width: 1280,
                    height: 720,
                },
                stride: 1280 * 4,
                format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                generation: 2,
                bytes: &pixels,
            },
        }],
    )
    .unwrap();
    assert_eq!(report.frame.bytes, pixels);
    assert_eq!(report.nonzero_pixel_bytes, 1280 * 720 * 4);
    assert_eq!(report.layers_composed, 1);
}

#[test]
fn borrowed_composition_clips_negative_geometry_by_rows() {
    let pixels = vec![0x33; 4 * 4 * 4];
    let report = compose_live_cpu_frame_ref(
        Size {
            width: 4,
            height: 4,
        },
        &[LiveCpuCompositionLayerRef {
            geometry: Rect {
                x: -2,
                y: -1,
                width: 4,
                height: 4,
            },
            buffer: LiveCpuBufferSourceRef {
                handle: 9,
                size: Size {
                    width: 4,
                    height: 4,
                },
                stride: 16,
                format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                generation: 1,
                bytes: &pixels,
            },
        }],
    )
    .unwrap();
    assert_eq!(report.nonzero_pixel_bytes, 2 * 3 * 4);
    assert_eq!(&report.frame.bytes[..8], &[0x33; 8]);
    assert!(report.frame.bytes[8..16].iter().all(|byte| *byte == 0));
}

#[test]
fn cpu_composition_checksum_changes_with_a_single_byte() {
    let size = Size {
        width: 2,
        height: 1,
    };
    let baseline = [0x5a; 8];
    let mut changed = baseline;
    changed[7] ^= 0xff;
    let first = compose_live_cpu_frame_ref(
        size,
        &[LiveCpuCompositionLayerRef {
            geometry: Rect {
                x: 0,
                y: 0,
                width: 2,
                height: 1,
            },
            buffer: LiveCpuBufferSourceRef {
                handle: 13,
                size,
                stride: 8,
                format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                generation: 1,
                bytes: &baseline,
            },
        }],
    )
    .unwrap();
    let second = compose_live_cpu_frame_ref(
        size,
        &[LiveCpuCompositionLayerRef {
            geometry: Rect {
                x: 0,
                y: 0,
                width: 2,
                height: 1,
            },
            buffer: LiveCpuBufferSourceRef {
                handle: 13,
                size,
                stride: 8,
                format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                generation: 1,
                bytes: &changed,
            },
        }],
    )
    .unwrap();
    assert_eq!(first.nonzero_pixel_bytes, 8);
    assert_eq!(second.nonzero_pixel_bytes, 8);
    assert_ne!(first.checksum, second.checksum);
}

#[test]
fn borrowed_composition_draws_a_high_contrast_software_cursor() {
    let size = Size {
        width: 16,
        height: 20,
    };
    let pixels = vec![0x22; 16 * 20 * 4];
    let layer = LiveCpuCompositionLayerRef {
        geometry: Rect {
            x: 0,
            y: 0,
            width: 16,
            height: 20,
        },
        buffer: LiveCpuBufferSourceRef {
            handle: 17,
            size,
            stride: 16 * 4,
            format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            generation: 1,
            bytes: &pixels,
        },
    };
    let baseline = compose_live_cpu_frame_ref(size, &[layer]).unwrap();
    let report =
        compose_live_cpu_frame_ref_with_cursor(size, &[layer], Some(Point { x: 2.8, y: 3.2 }))
            .unwrap();

    let outline = (3 * 16 + 2) * 4;
    let white = (4 * 16 + 3) * 4;
    assert_eq!(&report.frame.bytes[white..white + 4], &[0xff; 4]);
    assert_eq!(&report.frame.bytes[outline..outline + 4], &[0, 0, 0, 0xff]);
    assert_ne!(report.checksum, baseline.checksum);
}

#[test]
fn default_cursor_asset_has_stable_dimensions_and_hotspot() {
    assert_eq!(DEFAULT_CURSOR_EDGE, 16);
    assert_eq!(DEFAULT_CURSOR_HOTSPOT, (0, 0));
    assert_eq!(DEFAULT_CURSOR_SHAPE.len(), DEFAULT_CURSOR_EDGE);
    assert!(DEFAULT_CURSOR_SHAPE.iter().all(|row| {
        row.len() == DEFAULT_CURSOR_EDGE
            && row.iter().all(|pixel| matches!(pixel, b'.' | b'#' | b'W'))
    }));
}

#[test]
fn production_scene_composes_only_the_visible_surface_order() {
    let size = Size {
        width: 2,
        height: 2,
    };
    let surface = SurfaceId::new(1, 1);
    let committed = CommittedSurfaceState {
        surface,
        committed_generation: 1,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        },
        buffer: BufferSource::CpuBuffer { handle: 1 },
        damage: Region::empty(),
    };
    let mut scene = LiveProductionCpuScene::new(size);
    scene
        .apply_updates(
            [LiveCpuBufferUpdate::Replace(LiveCpuBufferSource {
                handle: 1,
                size,
                stride: 8,
                format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                generation: 1,
                bytes: vec![0x55; 16],
            })],
            std::slice::from_ref(&committed),
        )
        .unwrap();

    let hidden = scene
        .compose_visible(std::slice::from_ref(&committed), &[], None, None)
        .unwrap();
    assert_eq!(hidden.layers_composed, 0);
    assert_eq!(hidden.nonzero_pixel_bytes, 0);

    let visible = scene
        .compose_visible(
            std::slice::from_ref(&committed),
            std::slice::from_ref(&surface),
            None,
            None,
        )
        .unwrap();
    assert_eq!(visible.layers_composed, 1);
    assert_eq!(visible.nonzero_pixel_bytes, 16);
}

#[test]
fn production_scene_keeps_display_list_attached_to_composed_primary_pixels() {
    let output = HeadlessOutput {
        id: OutputId::from_raw(1),
        size: Size {
            width: 2,
            height: 2,
        },
        scale: 1,
    };
    let display_list = CompositorDisplayList::empty(output.id);
    let mut scene = LiveProductionCpuScene::new(output.size);

    scene
        .compose_display_list(&[], &display_list, None)
        .unwrap();
    let frames = scene.frames_for_outputs(&[output]).unwrap();

    assert_eq!(frames.len(), 1);
    assert_eq!(
        frames[0].compositor_display_list.as_ref(),
        Some(&display_list)
    );
}
