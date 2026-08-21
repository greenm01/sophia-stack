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

/// A scene placed inside a border may not paint into that border.
///
/// Every mirror policy until centre-unscaled projected the scene across the
/// whole head, so "the framebuffer" and "where the scene is" were one rect and
/// clipping to either gave the same answer. Placing a smaller scene inside a
/// larger head separates them, and content bounded by the framebuffer then
/// reaches into the margin that is supposed to hold background alone.
#[test]
fn a_scene_smaller_than_its_head_is_bounded_by_the_scene_not_the_framebuffer() {
    let mut snapshot = scene();
    snapshot.logical_viewport = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    // A window running off the right and bottom of the logical output. It is
    // retained because it partially intersects, which is what makes the clip
    // load-bearing rather than decorative.
    let surface = snapshot.surfaces[0].surface;
    let straddling = Rect {
        x: 1400,
        y: 700,
        width: 800,
        height: 600,
    };
    snapshot.surfaces[0].geometry = straddling;
    snapshot.surfaces[0].clip = straddling;
    snapshot.display_list.commands = vec![
        CompositorDisplayCommand::Surface { surface },
        CompositorDisplayCommand::Border(CompositorBorder {
            node: CompositorNodeId::SurfaceChrome {
                surface,
                role: SurfaceChromeRole::Frame,
            },
            generation: 3,
            outer: Rect {
                x: 1396,
                y: 696,
                width: 808,
                height: 608,
            },
            inner: straddling,
            color: CompositorRgb8 {
                red: 0,
                green: 80,
                blue: 255,
            },
        }),
    ];
    snapshot.cursor = Some(OutputSceneCursor {
        geometry: Rect {
            x: 1910,
            y: 1070,
            width: 24,
            height: 24,
        },
        source: BufferSource::CpuBuffer { handle: 99 },
        generation: 5,
    });

    // 1920x1080 placed unscaled and centred in 2560x1440: the scene occupies
    // x 320..2240 and y 180..1260, and the rest is border.
    let plans = build_output_head_plans(
        &snapshot,
        &[target(1, 2560, 1440, OutputHeadMapping::Exact)],
    )
    .unwrap();
    let plan = &plans[0];
    let painted = plan.transform.projected_scene;
    assert_eq!(painted.x, 320);
    assert_eq!(painted.y, 180);
    assert_eq!(painted.width, 1920);
    assert_eq!(painted.height, 1080);

    let contains = |rect: Rect| {
        rect.is_empty()
            || (rect.x >= painted.x
                && rect.y >= painted.y
                && rect.x + rect.width <= painted.x + painted.width
                && rect.y + rect.height <= painted.y + painted.height)
    };

    assert!(
        contains(plan.layers[0].native_clip),
        "surface clip {:?} escapes the scene {painted:?}",
        plan.layers[0].native_clip
    );
    assert!(
        contains(plan.cursor.unwrap().geometry),
        "cursor {:?} escapes the scene {painted:?}",
        plan.cursor.unwrap().geometry
    );

    let border = plan
        .compositor
        .iter()
        .find_map(|command| match command {
            HeadCompositorCommand::Border(border) => Some(*border),
            _ => None,
        })
        .expect("the straddling window keeps its border command");
    for band in compositor_border_bands(CompositorBorder {
        node: border.node,
        generation: border.generation,
        outer: border.outer,
        inner: border.inner,
        color: border.color,
    }) {
        let clipped = intersect_for_test(band.geometry, border.clip);
        assert!(
            contains(clipped),
            "border band {:?} clipped to {clipped:?} escapes the scene {painted:?}",
            band.geometry
        );
    }
}

/// Clipping the bands, never the rects they are derived from.
///
/// The four bands are the difference between `outer` and `inner`. Clip those two
/// first and the subtraction produces a band along the clip edge -- a bright line
/// down the side of a window that is merely running off the screen, which is
/// worse than the overflow it was meant to fix.
#[test]
fn a_window_cut_off_by_the_scene_edge_does_not_gain_a_border_there() {
    let painted = Rect {
        x: 320,
        y: 180,
        width: 1920,
        height: 1080,
    };
    // A window whose right edge is far past the scene: its real right band is
    // outside and must vanish, not be redrawn at the boundary.
    let inner = Rect {
        x: 400,
        y: 300,
        width: 4000,
        height: 400,
    };
    let outer = Rect {
        x: 396,
        y: 296,
        width: 4008,
        height: 408,
    };
    let color = CompositorRgb8 {
        red: 0,
        green: 80,
        blue: 255,
    };

    let bands = compositor_border_bands(CompositorBorder {
        node: CompositorNodeId::SurfaceChrome {
            surface: SurfaceId::new(4, 1),
            role: SurfaceChromeRole::Frame,
        },
        generation: 3,
        outer,
        inner,
        color,
    });
    let right_edge_of_scene = painted.x + painted.width;
    let survivors = bands
        .iter()
        .map(|band| intersect_for_test(band.geometry, painted))
        .filter(|geometry| !geometry.is_empty())
        .collect::<Vec<_>>();

    assert!(
        !survivors.is_empty(),
        "the top, bottom and left bands are visible and must survive"
    );
    for band in &survivors {
        assert!(
            band.x < right_edge_of_scene,
            "a band was placed at the scene's edge: {band:?}"
        );
        // A vertical band flush against the boundary would be the invented one.
        let vertical = band.width <= 8;
        assert!(
            !(vertical && band.x + band.width == right_edge_of_scene),
            "a vertical band ends exactly on the scene edge, which is the \
             spurious border clipping outer/inner first would create: {band:?}"
        );
    }
}

/// Mirrors the renderer's per-band clip so the engine test can assert on it.
fn intersect_for_test(first: Rect, second: Rect) -> Rect {
    let left = first.x.max(second.x);
    let top = first.y.max(second.y);
    let right = (first.x + first.width).min(second.x + second.width);
    let bottom = (first.y + first.height).min(second.y + second.height);
    Rect {
        x: left,
        y: top,
        width: (right - left).max(0),
        height: (bottom - top).max(0),
    }
}
