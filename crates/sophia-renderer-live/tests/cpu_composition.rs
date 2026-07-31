use sophia_engine::{CompositorDisplayList, CompositorRgb8, HeadlessOutput};
use sophia_protocol::{
    BufferSource, CommittedSurfaceState, OutputId, Point, Rect, Region, Size, SurfaceId,
};
use sophia_renderer_live::{
    DEFAULT_CURSOR_EDGE, DEFAULT_CURSOR_HOTSPOT, DEFAULT_CURSOR_SHAPE,
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveCpuBufferPatch, LiveCpuBufferSource,
    LiveCpuBufferSourceRef, LiveCpuBufferUpdate, LiveCpuCompositionElementRef,
    LiveCpuCompositionLayer, LiveCpuCompositionLayerRef, LiveCpuFrameMetricsMode,
    LiveProductionCpuScene, compose_live_cpu_display_list_frame,
    compose_live_cpu_display_list_frame_with_metrics_reusing, compose_live_cpu_frame,
    compose_live_cpu_frame_ref, compose_live_cpu_frame_ref_with_cursor,
};

#[test]
fn production_scene_discards_late_patch_but_reports_missing_committed_base() {
    let size = Size {
        width: 2,
        height: 1,
    };
    let surface = SurfaceId::new(1, 1);
    let committed = [CommittedSurfaceState {
        surface,
        committed_generation: 1,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 2,
            height: 1,
        },
        buffer: BufferSource::CpuBuffer { handle: 72 },
        damage: Region::empty(),
    }];
    let mut scene = LiveProductionCpuScene::new(size);

    scene
        .apply_production_updates([LiveCpuBufferUpdate::Patch(LiveCpuBufferPatch {
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
        })])
        .unwrap();

    assert_eq!(scene.resident_buffer_count(), 0);
    assert_eq!(scene.missing_committed_buffer_count(&committed), 1);
}

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
fn display_list_composition_reuses_uniquely_owned_frame_storage() {
    let size = Size {
        width: 4,
        height: 4,
    };
    let reusable = Arc::new(vec![0xaa; 64]);
    let allocation = reusable.as_ptr();
    let report = compose_live_cpu_display_list_frame_with_metrics_reusing(
        size,
        &[LiveCpuCompositionElementRef::Solid {
            geometry: Rect {
                x: 1,
                y: 1,
                width: 2,
                height: 2,
            },
            color: CompositorRgb8 {
                red: 1,
                green: 2,
                blue: 3,
            },
        }],
        None,
        LiveCpuFrameMetricsMode::DamageScopedEvidence,
        Some(reusable),
    )
    .unwrap();

    assert_eq!(report.frame.bytes.as_ptr(), allocation);
    assert!(report.frame.bytes[..16].iter().all(|byte| *byte == 0));
}

#[test]
fn display_list_content_identity_is_stable_across_metric_modes() {
    let size = Size {
        width: 4,
        height: 4,
    };
    let elements = [LiveCpuCompositionElementRef::Solid {
        geometry: Rect {
            x: 1,
            y: 1,
            width: 2,
            height: 2,
        },
        color: CompositorRgb8 {
            red: 1,
            green: 2,
            blue: 3,
        },
    }];
    let exact = compose_live_cpu_display_list_frame_with_metrics_reusing(
        size,
        &elements,
        None,
        LiveCpuFrameMetricsMode::ExactPixels,
        None,
    )
    .unwrap();
    let damage_scoped = compose_live_cpu_display_list_frame_with_metrics_reusing(
        size,
        &elements,
        None,
        LiveCpuFrameMetricsMode::DamageScopedEvidence,
        None,
    )
    .unwrap();

    assert_eq!(exact.checksum, damage_scoped.checksum);
    assert_eq!(exact.nonzero_pixel_bytes, 16);
    assert_eq!(damage_scoped.nonzero_pixel_bytes, 1);
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
                generation: 1,
                bytes: &pixels,
            },
        }],
    )
    .unwrap();
    assert_eq!(report.frame.bytes.as_ref(), &pixels);
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
                generation: 2,
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
fn cpu_composition_identity_changes_with_an_immutable_generation() {
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
                generation: 2,
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
        .apply_updates([LiveCpuBufferUpdate::Replace(LiveCpuBufferSource {
            handle: 1,
            size,
            stride: 8,
            format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            generation: 1,
            bytes: vec![0x55; 16],
        })])
        .unwrap();
    scene.reconcile_buffer_residency(&[1]);

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
        .compose_display_list(output, &[], &display_list, None)
        .unwrap();
    let frames = scene.frames_for_outputs(&[output]).unwrap();

    assert_eq!(frames.len(), 1);
    assert_eq!(
        frames[0]
            .output_damage_snapshot
            .as_ref()
            .map(|snapshot| &snapshot.compositor_display_list),
        Some(&display_list),
    );
}

#[test]
fn production_scene_reuses_unchanged_secondary_output_frames() {
    let primary = HeadlessOutput {
        id: OutputId::from_raw(1),
        size: Size {
            width: 4,
            height: 3,
        },
        scale: 1,
    };
    let secondary = HeadlessOutput {
        id: OutputId::from_raw(2),
        size: Size {
            width: 3,
            height: 2,
        },
        scale: 1,
    };
    let display_list = CompositorDisplayList::empty(primary.id);
    let mut scene = LiveProductionCpuScene::new(primary.size);

    scene
        .compose_display_list(primary, &[], &display_list, None)
        .unwrap();
    let first = scene.frames_for_outputs(&[primary, secondary]).unwrap();
    scene
        .compose_display_list(primary, &[], &display_list, None)
        .unwrap();
    let second = scene.frames_for_outputs(&[primary, secondary]).unwrap();

    assert!(Arc::ptr_eq(&first[1].frame.bytes, &second[1].frame.bytes));
    assert_eq!(first[1].checksum, second[1].checksum);

    let resized_secondary = HeadlessOutput {
        size: Size {
            width: 2,
            height: 2,
        },
        ..secondary
    };
    let resized = scene
        .frames_for_outputs(&[primary, resized_secondary])
        .unwrap();
    assert!(!Arc::ptr_eq(
        &second[1].frame.bytes,
        &resized[1].frame.bytes
    ));
    assert_eq!(resized[1].frame.size, resized_secondary.size);
}

#[test]
fn production_scene_metric_warmup_does_not_schedule_unchanged_content() {
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
    let mut checksums = Vec::new();

    for _ in 0..4 {
        checksums.push(
            scene
                .compose_display_list(output, &[], &display_list, None)
                .unwrap()
                .checksum,
        );
    }

    assert!(checksums.windows(2).all(|pair| pair[0] == pair[1]));
    assert_eq!(scene.exact_pixel_metric_frames(), 3);
    assert_eq!(scene.damage_scoped_metric_frames(), 1);
}
use std::sync::Arc;
