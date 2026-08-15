use sophia_engine::*;
use sophia_protocol::*;

fn variant(variant: u32, density: u32, source: u64) -> SurfaceContentVariant {
    let size = Size {
        width: i32::try_from((800_u64 * u64::from(density) + 999) / 1_000).unwrap(),
        height: i32::try_from((600_u64 * u64::from(density) + 999) / 1_000).unwrap(),
    };
    SurfaceContentVariant {
        variant,
        source: BufferSource::CpuBuffer { handle: source },
        pixel_size: size,
        density_millis: density,
        transform: SurfaceRasterTransform::Normal,
        fidelity: SurfaceContentFidelity::AuthorityRaster,
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: size.width,
            height: size.height,
        }),
    }
}

fn scene() -> OutputSceneSnapshot {
    let output = OutputId::from_raw(1);
    let surface = SurfaceId::new(4, 1);
    let geometry = Rect {
        x: 100,
        y: 120,
        width: 800,
        height: 600,
    };
    OutputSceneSnapshot {
        output,
        scene_generation: 12,
        logical_viewport: Rect {
            x: 0,
            y: 0,
            width: 2560,
            height: 1440,
        },
        surfaces: vec![OutputSceneSurface {
            surface,
            committed_generation: 8,
            geometry,
            clip: geometry,
            opacity_millis: 1_000,
            content: SurfaceContentSet::new(
                Size {
                    width: 800,
                    height: 600,
                },
                vec![variant(1, 1_000, 41), variant(2, 750, 42)],
            )
            .unwrap(),
            damage: Region::single(geometry),
        }],
        display_list: CompositorDisplayList {
            output,
            commands: vec![
                CompositorDisplayCommand::Surface { surface },
                CompositorDisplayCommand::Border(CompositorBorder {
                    node: CompositorNodeId::SurfaceChrome {
                        surface,
                        role: SurfaceChromeRole::Frame,
                    },
                    generation: 3,
                    outer: Rect {
                        x: 96,
                        y: 116,
                        width: 808,
                        height: 608,
                    },
                    inner: geometry,
                    color: CompositorRgb8 {
                        red: 0,
                        green: 80,
                        blue: 255,
                    },
                }),
            ],
        },
        cursor: Some(OutputSceneCursor {
            geometry: Rect {
                x: 640,
                y: 360,
                width: 24,
                height: 24,
            },
            source: BufferSource::CpuBuffer { handle: 99 },
            generation: 5,
        }),
        logical_damage: Region::single(geometry),
        logical_content_checksum: 0x1234,
    }
}

fn target(head: u64, width: i32, height: i32, mapping: OutputHeadMapping) -> HeadRenderTarget {
    HeadRenderTarget {
        head: RenderHeadId::from_raw(head),
        output: OutputId::from_raw(1),
        target_generation: 7,
        native_size: Size { width, height },
        scale: 1,
        refresh_millihz: 60_000,
        transform: OutputTransform::Normal,
        mapping,
    }
}

#[test]
fn unequal_mirror_heads_select_native_density_variants_from_one_scene() {
    let snapshot = scene();
    let plans = build_output_head_plans(
        &snapshot,
        &[
            target(1, 2560, 1440, OutputHeadMapping::Fit),
            target(2, 1920, 1080, OutputHeadMapping::Fit),
        ],
    )
    .unwrap();
    assert_eq!(plans.len(), 2);
    assert_eq!(plans[0].layers[0].variant, 1);
    assert_eq!(
        plans[0].layers[0].requested_sampling,
        HeadSamplingClass::Exact
    );
    assert_eq!(plans[0].layers[0].native_geometry.width, 800);
    assert_eq!(plans[1].layers[0].variant, 2);
    assert_eq!(plans[1].layers[0].density_millis, 750);
    assert_eq!(
        plans[1].layers[0].requested_sampling,
        HeadSamplingClass::Exact
    );
    assert_eq!(plans[1].layers[0].native_geometry.width, 600);
    assert_eq!(
        plans[1].native_size,
        Size {
            width: 1920,
            height: 1080
        }
    );
    assert_eq!(
        plans[0].logical_content_checksum,
        plans[1].logical_content_checksum
    );

    let HeadCompositorCommand::Border(smaller_border) = plans[1].compositor[1] else {
        panic!("border remained semantic until the smaller head plan")
    };
    assert_eq!(smaller_border.outer.width, 606);
    assert_eq!(plans[1].cursor.unwrap().geometry.width, 18);
    let damage = head_output_damage_snapshot(&plans[1]);
    assert_eq!(damage.output.size.width, 1_920);
    assert_eq!(damage.surfaces[0].geometry.width, 600);
    assert_eq!(
        damage.surfaces[0].buffer,
        BufferSource::CpuBuffer { handle: 42 }
    );
}

#[test]
fn fit_adds_explicit_bars_and_projects_damage_with_filter_footprint() {
    let mut snapshot = scene();
    snapshot.surfaces[0].content = SurfaceContentSet::singleton(
        BufferSource::CpuBuffer { handle: 41 },
        Size {
            width: 800,
            height: 600,
        },
    );
    let plan =
        build_head_composition_plan(&snapshot, target(2, 1920, 1200, OutputHeadMapping::Fit))
            .unwrap();
    assert_eq!(
        plan.transform.projected_scene,
        Rect {
            x: 0,
            y: 60,
            width: 1920,
            height: 1080,
        }
    );
    assert_eq!(
        plan.compositor
            .iter()
            .filter(|command| matches!(command, HeadCompositorCommand::Background(_)))
            .count(),
        2
    );
    assert_eq!(
        plan.layers[0].requested_sampling,
        HeadSamplingClass::Downsampled
    );
    assert!(plan.repaint.rects[0].width > plan.layers[0].native_geometry.width);
}

#[test]
fn cover_crops_without_changing_the_scene_or_variant_identity() {
    let plan =
        build_head_composition_plan(&scene(), target(2, 1920, 1200, OutputHeadMapping::Cover))
            .unwrap();
    assert!(plan.transform.projected_scene.x < 0);
    assert!(
        plan.compositor
            .iter()
            .all(|command| !matches!(command, HeadCompositorCommand::Background(_)))
    );
    assert_eq!(plan.layers[0].variant, 1);
    assert_eq!(
        plan.layers[0].native_clip.x,
        0.max(plan.layers[0].native_geometry.x)
    );
}

#[test]
fn plan_rejects_a_display_list_surface_missing_from_the_snapshot() {
    let mut snapshot = scene();
    snapshot.display_list.commands[0] = CompositorDisplayCommand::Surface {
        surface: SurfaceId::new(99, 1),
    };
    assert_eq!(
        build_head_composition_plan(&snapshot, target(1, 2560, 1440, OutputHeadMapping::Fit),),
        Err(HeadCompositionPlanError::MissingDisplaySurface)
    );
}

#[test]
fn committed_scene_capture_is_one_immutable_fanout_source() {
    let source = scene();
    let committed = source
        .surfaces
        .iter()
        .map(|surface| CommittedSurfaceState {
            surface: surface.surface,
            committed_generation: surface.committed_generation,
            geometry: surface.geometry,
            content: surface.content.clone(),
            damage: surface.damage.clone(),
        })
        .collect::<Vec<_>>();
    let output = HeadlessOutput {
        id: source.output,
        size: Size {
            width: source.logical_viewport.width,
            height: source.logical_viewport.height,
        },
        scale: 1,
    };
    let captured = output_scene_snapshot_from_committed(
        output,
        44,
        &committed,
        source.display_list.clone(),
        source.cursor,
    )
    .unwrap();
    let plans = build_output_head_plans(
        &captured,
        &[
            target(1, 2560, 1440, OutputHeadMapping::Fit),
            target(2, 1920, 1080, OutputHeadMapping::Fit),
        ],
    )
    .unwrap();
    assert_eq!(plans[0].scene_generation, 44);
    assert_eq!(plans[1].scene_generation, 44);
    assert_eq!(
        plans[0].logical_content_checksum,
        plans[1].logical_content_checksum
    );

    let mut changed = committed;
    changed[0].committed_generation += 1;
    let changed = output_scene_snapshot_from_committed(
        output,
        45,
        &changed,
        source.display_list,
        source.cursor,
    )
    .unwrap();
    assert_ne!(
        captured.logical_content_checksum,
        changed.logical_content_checksum
    );
}

#[test]
fn committed_scene_capture_filters_off_view_surfaces_for_extended_outputs() {
    let source = scene();
    let visible = CommittedSurfaceState {
        surface: source.surfaces[0].surface,
        committed_generation: 8,
        geometry: Rect {
            x: 2_700,
            y: 100,
            width: 800,
            height: 600,
        },
        content: source.surfaces[0].content.clone(),
        damage: source.surfaces[0].damage.clone(),
    };
    let hidden = CommittedSurfaceState {
        surface: SurfaceId::new(9, 1),
        committed_generation: 2,
        geometry: Rect {
            x: 100,
            y: 100,
            width: 800,
            height: 600,
        },
        content: source.surfaces[0].content.clone(),
        damage: source.surfaces[0].damage.clone(),
    };
    let captured = output_scene_snapshot_from_committed_in_view(
        source.output,
        50,
        Rect {
            x: 2_560,
            y: 0,
            width: 1_920,
            height: 1_080,
        },
        &[visible, hidden],
        CompositorDisplayList {
            output: source.output,
            commands: vec![
                CompositorDisplayCommand::Surface {
                    surface: source.surfaces[0].surface,
                },
                CompositorDisplayCommand::Surface {
                    surface: SurfaceId::new(9, 1),
                },
            ],
        },
        None,
    )
    .unwrap();
    assert_eq!(captured.surfaces.len(), 1);
    assert_eq!(captured.surfaces[0].surface, source.surfaces[0].surface);
    assert_eq!(captured.display_list.commands.len(), 1);
}

#[test]
fn unsupported_target_and_surface_transforms_fail_closed() {
    let snapshot = scene();
    let mut rotated_target = target(1, 2_560, 1_440, OutputHeadMapping::Fit);
    rotated_target.transform = OutputTransform::Rotate90;
    assert_eq!(
        build_head_composition_plan(&snapshot, rotated_target),
        Err(HeadCompositionPlanError::UnsupportedTargetTransform)
    );

    let mut rotated_snapshot = scene();
    rotated_snapshot.surfaces[0].content = SurfaceContentSet::new(
        Size {
            width: 800,
            height: 600,
        },
        vec![SurfaceContentVariant {
            variant: 7,
            source: BufferSource::CpuBuffer { handle: 77 },
            pixel_size: Size {
                width: 600,
                height: 800,
            },
            density_millis: 1_000,
            transform: SurfaceRasterTransform::Rotate90,
            fidelity: SurfaceContentFidelity::AuthorityRaster,
            damage: Region::single(Rect {
                x: 0,
                y: 0,
                width: 600,
                height: 800,
            }),
        }],
    )
    .unwrap();
    assert_eq!(
        build_head_composition_plan(
            &rotated_snapshot,
            target(1, 2_560, 1_440, OutputHeadMapping::Fit)
        ),
        Err(HeadCompositionPlanError::UnavailableSurfaceVariant)
    );
}
