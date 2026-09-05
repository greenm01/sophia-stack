use sophia_engine::*;
use sophia_protocol::*;

fn scene(content: SurfaceContentSet) -> OutputSceneSnapshot {
    let output = OutputId::from_raw(1);
    let surface = SurfaceId::new(7, 1);
    let geometry = Rect {
        x: 100,
        y: 100,
        width: 800,
        height: 600,
    };
    OutputSceneSnapshot {
        output,
        scene_generation: 9,
        logical_viewport: Rect {
            x: 0,
            y: 0,
            width: 2560,
            height: 1440,
        },
        surfaces: vec![OutputSceneSurface {
            surface,
            committed_generation: 4,
            geometry,
            clip: geometry,
            opacity_millis: 1_000,
            content,
            damage: Region::single(geometry),
        }],
        display_list: CompositorDisplayList {
            output,
            commands: vec![CompositorDisplayCommand::Surface { surface }],
        },
        cursor: None,
        logical_damage: Region::single(geometry),
        logical_content_checksum: 1,
    }
}

fn target(head: u64, size: Size) -> HeadRenderTarget {
    HeadRenderTarget {
        head: RenderHeadId::from_raw(head),
        output: OutputId::from_raw(1),
        target_generation: 1,
        native_size: size,
        scale: 1,
        refresh_millihz: 60_000,
        transform: OutputTransform::Normal,
        mapping: OutputHeadMapping::Fit,
    }
}

#[test]
fn unequal_mirror_heads_emit_one_deduplicated_density_union() {
    let logical = Size {
        width: 800,
        height: 600,
    };
    let snapshot = scene(SurfaceContentSet::singleton(
        BufferSource::CpuBuffer { handle: 11 },
        logical,
    ));
    let targets = [
        target(
            1,
            Size {
                width: 2560,
                height: 1440,
            },
        ),
        target(
            2,
            Size {
                width: 1920,
                height: 1080,
            },
        ),
    ];
    let mut tracker = SurfaceRasterRequirementTracker::new();
    let first = tracker
        .reconcile(std::slice::from_ref(&snapshot), &targets)
        .unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].logical_extent, logical);
    assert_eq!(
        first[0].classes,
        vec![
            SurfaceRasterClass {
                density_millis: 750,
                transform: SurfaceRasterTransform::Normal,
            },
            SurfaceRasterClass {
                density_millis: 1_000,
                transform: SurfaceRasterTransform::Normal,
            },
        ]
    );
    assert!(tracker.reconcile(&[snapshot], &targets).unwrap().is_empty());
}

#[test]
fn stale_raster_response_cannot_consume_current_requirement() {
    let logical = Size {
        width: 800,
        height: 600,
    };
    let snapshot = scene(SurfaceContentSet::singleton(
        BufferSource::CpuBuffer { handle: 11 },
        logical,
    ));
    let mut tracker = SurfaceRasterRequirementTracker::new();
    let requirement = tracker
        .reconcile(
            &[snapshot],
            &[target(
                2,
                Size {
                    width: 1920,
                    height: 1080,
                },
            )],
        )
        .unwrap()
        .remove(0);
    // Content older than the demand cannot answer it: those pixels predate the
    // classes that were asked about.
    assert!(!tracker.accept_response(SurfaceRasterResponseIdentity {
        transaction: TransactionId::from_raw(50),
        surface: requirement.surface,
        source_content_generation: requirement.committed_content_generation - 1,
        requirement_generation: requirement.requirement_generation,
    }));
    // A response must still name the exact edge it answers, whatever content
    // generation it carries.
    assert!(!tracker.accept_response(SurfaceRasterResponseIdentity {
        transaction: TransactionId::from_raw(51),
        surface: requirement.surface,
        source_content_generation: requirement.committed_content_generation,
        requirement_generation: requirement.requirement_generation + 1,
    }));
    // Content newer than the demand does answer it. An authority replies from
    // its current state, which leads the committed generation Engine asked
    // from whenever the client kept drawing, and that reply commits through the
    // ordinary ordered chain.
    assert!(tracker.accept_response(SurfaceRasterResponseIdentity {
        transaction: TransactionId::from_raw(52),
        surface: requirement.surface,
        source_content_generation: requirement.committed_content_generation + 1,
        requirement_generation: requirement.requirement_generation,
    }));
}

#[test]
fn canonical_variant_reserves_one_slot_across_many_density_classes() {
    let snapshot = scene(SurfaceContentSet::singleton(
        BufferSource::CpuBuffer { handle: 11 },
        Size {
            width: 800,
            height: 600,
        },
    ));
    let targets = [
        target(
            1,
            Size {
                width: 1536,
                height: 864,
            },
        ),
        target(
            2,
            Size {
                width: 1792,
                height: 1008,
            },
        ),
        target(
            3,
            Size {
                width: 2048,
                height: 1152,
            },
        ),
        target(
            4,
            Size {
                width: 2304,
                height: 1296,
            },
        ),
    ];
    let mut tracker = SurfaceRasterRequirementTracker::new();
    let requirements = tracker.reconcile(&[snapshot], &targets).unwrap();
    assert_eq!(requirements.len(), 1);
    assert_eq!(
        requirements[0].classes.len(),
        MAX_SURFACE_CONTENT_VARIANTS - 1
    );
    assert_eq!(
        requirements[0]
            .classes
            .iter()
            .map(|class| class.density_millis)
            .collect::<Vec<_>>(),
        vec![600, 700, 800]
    );
}

#[test]
fn an_unchanged_demand_keeps_one_outstanding_requirement_edge() {
    let logical = Size {
        width: 800,
        height: 600,
    };
    let targets = [target(
        2,
        Size {
            width: 1920,
            height: 1080,
        },
    )];
    let mut tracker = SurfaceRasterRequirementTracker::new();
    let requirement = tracker
        .reconcile(
            &[scene(SurfaceContentSet::singleton(
                BufferSource::CpuBuffer { handle: 11 },
                logical,
            ))],
            &targets,
        )
        .unwrap()
        .remove(0);

    // The client keeps drawing, so Engine's committed vantage advances while
    // the demand itself — extent and classes — is unchanged. That must not
    // mint a new edge: a fresh edge per frame would strand every reply in
    // flight against an edge that no longer exists.
    for advanced in [5_u64, 6, 7] {
        let mut snapshot = scene(SurfaceContentSet::singleton(
            BufferSource::CpuBuffer { handle: 11 },
            logical,
        ));
        snapshot.surfaces[0].committed_generation = advanced;
        assert!(
            tracker.reconcile(&[snapshot], &targets).unwrap().is_empty(),
            "an unchanged demand must not re-issue while one edge is outstanding"
        );
    }

    // A reply produced from newer content still answers the original edge.
    assert!(tracker.accept_response(SurfaceRasterResponseIdentity {
        transaction: TransactionId::from_raw(70),
        surface: requirement.surface,
        source_content_generation: requirement.committed_content_generation + 3,
        requirement_generation: requirement.requirement_generation,
    }));
}

#[test]
fn renderer_content_raises_no_raster_requirement() {
    // A DMA-BUF surface carries pixels and no semantic form, so no authority
    // can re-rasterize it at another density. Demanding one costs a round trip
    // and returns fallback at best; at worst it asks an authority a question
    // about a surface it does not own pixels for.
    let logical = Size {
        width: 800,
        height: 600,
    };
    let snapshot = scene(SurfaceContentSet::singleton(
        BufferSource::DmaBuf { handle: 77 },
        logical,
    ));
    let mut tracker = SurfaceRasterRequirementTracker::new();
    assert!(
        tracker
            .reconcile(
                &[snapshot],
                &[target(
                    2,
                    Size {
                        width: 1920,
                        height: 1080,
                    },
                )],
            )
            .unwrap()
            .is_empty(),
        "a renderer surface must not be asked for a density variant"
    );
}

#[test]
fn a_mixed_scene_demands_only_its_cpu_backed_surface() {
    let logical = Size {
        width: 800,
        height: 600,
    };
    let mut snapshot = scene(SurfaceContentSet::singleton(
        BufferSource::CpuBuffer { handle: 11 },
        logical,
    ));
    let cpu_surface = snapshot.surfaces[0].surface;
    let mut renderer_surface = snapshot.surfaces[0].clone();
    renderer_surface.surface = SurfaceId::new(9, 1);
    renderer_surface.content =
        SurfaceContentSet::singleton(BufferSource::DmaBuf { handle: 78 }, logical);
    snapshot.surfaces.push(renderer_surface);

    let requirements = SurfaceRasterRequirementTracker::new().reconcile(
        &[snapshot],
        &[target(
            2,
            Size {
                width: 1920,
                height: 1080,
            },
        )],
    );
    let requirements = requirements.unwrap();
    assert_eq!(requirements.len(), 1);
    assert_eq!(requirements[0].surface, cpu_surface);
}
