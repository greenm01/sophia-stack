// Cause-classified sampled-fallback regressions.
//
// Every case here proves the negative direction of the density contract: when
// the semantic journal cannot answer a requirement, the surface must publish
// its canonical raster as sampled compatibility content with a named cause,
// and must never label that content as an authority-owned native variant.

fn fallback_window(
    runtime: &mut XAuthorityRuntime,
    namespace: NamespaceId,
    window: XResourceId,
    surface: SurfaceId,
    transaction: u64,
) {
    runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(transaction),
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window,
            surface,
            geometry: Rect {
                x: 0,
                y: 0,
                width: 40,
                height: 20,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    });
}

fn fallback_requirement(
    surface: SurfaceId,
    committed_content_generation: u64,
    densities: &[u32],
) -> SurfaceRasterRequirements {
    SurfaceRasterRequirements {
        surface,
        committed_content_generation,
        requirement_generation: 1,
        logical_extent: Size {
            width: 40,
            height: 20,
        },
        classes: densities
            .iter()
            .map(|density| SurfaceRasterClass {
                density_millis: *density,
                transform: SurfaceRasterTransform::Normal,
            })
            .collect(),
    }
}

fn opaque_image(width: i32, height: i32, pixel: u32) -> Vec<u8> {
    let mut pixels = Vec::new();
    for _ in 0..width.saturating_mul(height) {
        pixels.extend_from_slice(&pixel.to_le_bytes());
    }
    pixels
}

fn z_pixmap_semantics() -> XPutImageSemantics {
    XPutImageSemantics {
        format: 2,
        depth: 24,
        left_pad: 0,
        byte_order: XByteOrder::LittleEndian,
        gc: XGraphicsContextValues::default(),
    }
}

/// Asserts a content set carries only the canonical 1x variant, so a poisoned
/// journal cannot leak a derived store that claims exact density.
fn assert_canonical_only(runtime: &mut XAuthorityRuntime, surface: SurfaceId, generation: u64) {
    let outcome = runtime
        .apply_surface_raster_requirements(
            TransactionId::from_raw(9_999),
            &fallback_requirement(surface, generation, &[750]),
        )
        .unwrap();
    assert!(
        matches!(outcome, XSurfaceRasterOutcome::SampledFallback { .. }),
        "a poisoned journal must never publish a derived variant"
    );
}

#[test]
fn unsupported_put_image_format_reports_its_cause_and_keeps_canonical_only() {
    let namespace = NamespaceId::from_raw(220);
    let window = XResourceId::new(0x220, 1);
    let surface = SurfaceId::new(220, 1);
    let mut runtime = XAuthorityRuntime::new();
    fallback_window(&mut runtime, namespace, window, surface, 200);

    runtime.begin_dispatch();
    let mut semantics = z_pixmap_semantics();
    // XYPixmap is outside the replayable subset.
    semantics.format = 1;
    runtime.apply_put_image(
        TransactionId::from_raw(201),
        namespace,
        window,
        Region::single(Rect {
            x: 0,
            y: 0,
            width: 8,
            height: 4,
        }),
        Some(&opaque_image(8, 4, 0x00ff_0000)),
        Some(&semantics),
    );

    assert_eq!(
        expect_raster_fallback(
            runtime
                .apply_surface_raster_requirements(
                    TransactionId::from_raw(202),
                    &fallback_requirement(surface, 2, &[750]),
                )
                .unwrap(),
            "an XYPixmap upload has no journal representation",
        ),
        XRasterFallbackCause::UnsupportedPutImage,
    );
    assert_canonical_only(&mut runtime, surface, 2);
}

#[test]
fn clipped_or_non_copy_put_image_is_not_retained_as_replayable() {
    let namespace = NamespaceId::from_raw(221);
    let surface = SurfaceId::new(221, 1);
    for (index, mutate) in [
        // A non-GXcopy function does not reproduce the canonical bytes.
        (0_u64, (|gc: &mut XGraphicsContextValues| gc.function = 6) as fn(&mut _)),
        // A partial plane mask leaves some visible planes untouched.
        (1, |gc: &mut XGraphicsContextValues| gc.plane_mask = 0x0000_00ff),
        // Clipping means the upload did not write every named pixel.
        (2, |gc: &mut XGraphicsContextValues| {
            gc.clip_rectangles = vec![Rect {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            }];
        }),
    ] {
        let window = XResourceId::new(0x230 + index, 1);
        let mut runtime = XAuthorityRuntime::new();
        fallback_window(&mut runtime, namespace, window, surface, 210 + index);

        runtime.begin_dispatch();
        let mut semantics = z_pixmap_semantics();
        mutate(&mut semantics.gc);
        runtime.apply_put_image(
            TransactionId::from_raw(211 + index),
            namespace,
            window,
            Region::single(Rect {
                x: 0,
                y: 0,
                width: 8,
                height: 4,
            }),
            Some(&opaque_image(8, 4, 0x0000_ff00)),
            Some(&semantics),
        );

        assert_eq!(
            expect_raster_fallback(
                runtime
                    .apply_surface_raster_requirements(
                        TransactionId::from_raw(212 + index),
                        &fallback_requirement(surface, 2, &[750]),
                    )
                    .unwrap(),
                "a conditional upload must not be retained as replayable",
            ),
            XRasterFallbackCause::UnsupportedPutImage,
        );
    }
}

#[test]
fn absent_put_image_semantics_reports_unsupported_rather_than_guessing() {
    let namespace = NamespaceId::from_raw(222);
    let window = XResourceId::new(0x222, 1);
    let surface = SurfaceId::new(222, 1);
    let mut runtime = XAuthorityRuntime::new();
    fallback_window(&mut runtime, namespace, window, surface, 220);

    runtime.begin_dispatch();
    runtime.apply_put_image(
        TransactionId::from_raw(221),
        namespace,
        window,
        Region::single(Rect {
            x: 0,
            y: 0,
            width: 8,
            height: 4,
        }),
        Some(&opaque_image(8, 4, 0x0012_3456)),
        None,
    );

    assert_eq!(
        expect_raster_fallback(
            runtime
                .apply_surface_raster_requirements(
                    TransactionId::from_raw(222),
                    &fallback_requirement(surface, 2, &[750]),
                )
                .unwrap(),
            "an unvouched upload must fail closed",
        ),
        XRasterFallbackCause::UnsupportedPutImage,
    );
}

#[test]
fn full_opaque_put_image_establishes_a_new_replayable_baseline() {
    let namespace = NamespaceId::from_raw(223);
    let window = XResourceId::new(0x223, 1);
    let surface = SurfaceId::new(223, 1);
    let mut runtime = XAuthorityRuntime::new();
    fallback_window(&mut runtime, namespace, window, surface, 230);

    // Poison the journal first, so recovery is what the baseline proves.
    runtime.begin_dispatch();
    let mut unsupported = z_pixmap_semantics();
    unsupported.format = 1;
    runtime.apply_put_image(
        TransactionId::from_raw(231),
        namespace,
        window,
        Region::single(Rect {
            x: 0,
            y: 0,
            width: 8,
            height: 4,
        }),
        Some(&opaque_image(8, 4, 0x0000_00ff)),
        Some(&unsupported),
    );
    assert_eq!(
        expect_raster_fallback(
            runtime
                .apply_surface_raster_requirements(
                    TransactionId::from_raw(232),
                    &fallback_requirement(surface, 2, &[750]),
                )
                .unwrap(),
            "the journal must be poisoned before recovery is meaningful",
        ),
        XRasterFallbackCause::UnsupportedPutImage,
    );

    // A partial upload covers only part of the drawable, so it cannot serve as
    // a baseline and the journal stays poisoned.
    runtime.begin_dispatch();
    runtime.apply_put_image(
        TransactionId::from_raw(233),
        namespace,
        window,
        Region::single(Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 10,
        }),
        Some(&opaque_image(40, 10, 0x0011_2233)),
        Some(&z_pixmap_semantics()),
    );
    assert_eq!(
        expect_raster_fallback(
            runtime
                .apply_surface_raster_requirements(
                    TransactionId::from_raw(234),
                    &fallback_requirement(surface, 3, &[750]),
                )
                .unwrap(),
            "a partial upload must not discard the poisoned journal",
        ),
        XRasterFallbackCause::UnsupportedPutImage,
    );

    // A full-window unconditional upload reproduces the canonical drawable on
    // its own, so it may replace the journal.
    runtime.begin_dispatch();
    runtime.apply_put_image(
        TransactionId::from_raw(235),
        namespace,
        window,
        Region::single(Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 20,
        }),
        Some(&opaque_image(40, 20, 0x0044_5566)),
        Some(&z_pixmap_semantics()),
    );
    let response = expect_satisfied_raster(
        runtime
            .apply_surface_raster_requirements(
                TransactionId::from_raw(236),
                &fallback_requirement(surface, 4, &[750]),
            )
            .unwrap(),
        "a full opaque upload must re-establish a replayable baseline",
    );
    assert_eq!(response.transaction.content.variants().len(), 2);
    assert!(
        response
            .transaction
            .content
            .variants()
            .iter()
            .all(|variant| variant.fidelity
                == sophia_protocol::SurfaceContentFidelity::AuthorityRaster)
    );
}

#[test]
fn over_budget_put_image_stream_reports_journal_capacity() {
    const EXTENT: i32 = 600;
    let namespace = NamespaceId::from_raw(224);
    let window = XResourceId::new(0x224, 1);
    let surface = SurfaceId::new(224, 1);
    let mut runtime = XAuthorityRuntime::new();
    runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(240),
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window,
            surface,
            geometry: Rect {
                x: 0,
                y: 0,
                width: EXTENT,
                height: EXTENT,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    });

    // Each retained upload owns its own pixels, so a few large ones cross the
    // 4 MiB payload bound. Every rectangle stops one row short of the window,
    // which keeps any single command from qualifying as a new baseline.
    let pixels = opaque_image(EXTENT, EXTENT - 1, 0x0077_8899);
    let mut generation = 1_u64;
    for index in 0..3 {
        runtime.begin_dispatch();
        runtime.apply_put_image(
            TransactionId::from_raw(241 + index),
            namespace,
            window,
            Region::single(Rect {
                x: 0,
                y: 1,
                width: EXTENT,
                height: EXTENT - 1,
            }),
            Some(&pixels),
            Some(&z_pixmap_semantics()),
        );
        generation = generation.saturating_add(1);
    }

    let requirement = SurfaceRasterRequirements {
        surface,
        committed_content_generation: generation,
        requirement_generation: 1,
        logical_extent: Size {
            width: EXTENT,
            height: EXTENT,
        },
        classes: vec![SurfaceRasterClass {
            density_millis: 750,
            transform: SurfaceRasterTransform::Normal,
        }],
    };
    assert_eq!(
        expect_raster_fallback(
            runtime
                .apply_surface_raster_requirements(TransactionId::from_raw(260), &requirement)
                .unwrap(),
            "an over-budget journal must report capacity, not exact content",
        ),
        XRasterFallbackCause::JournalCapacity,
    );
}

#[test]
fn transform_requirement_reports_transform_mismatch() {
    let namespace = NamespaceId::from_raw(225);
    let window = XResourceId::new(0x225, 1);
    let surface = SurfaceId::new(225, 1);
    let mut runtime = XAuthorityRuntime::new();
    fallback_window(&mut runtime, namespace, window, surface, 270);

    runtime.begin_dispatch();
    runtime.apply_core_draw(
        TransactionId::from_raw(271),
        namespace,
        window,
        Region::single(Rect {
            x: 1,
            y: 1,
            width: 4,
            height: 4,
        }),
    );

    let mut requirements = fallback_requirement(surface, 2, &[750]);
    requirements.classes[0].transform = SurfaceRasterTransform::Rotate90;
    assert_eq!(
        expect_raster_fallback(
            runtime
                .apply_surface_raster_requirements(TransactionId::from_raw(272), &requirements)
                .unwrap(),
            "derived stores render the normal transform only",
        ),
        XRasterFallbackCause::TransformMismatch,
    );
}

#[test]
fn fallback_causes_have_distinct_stable_log_tokens() {
    let causes = [
        XRasterFallbackCause::UnsupportedPutImage,
        XRasterFallbackCause::UnsupportedCrossDrawableCopy,
        XRasterFallbackCause::UnsupportedCommand,
        XRasterFallbackCause::StaleContentGeneration,
        XRasterFallbackCause::JournalCapacity,
        XRasterFallbackCause::BackingCapacity,
        XRasterFallbackCause::TransformMismatch,
    ];
    let tokens: std::collections::BTreeSet<_> =
        causes.iter().map(|cause| cause.as_str()).collect();
    assert_eq!(tokens.len(), causes.len());
    assert_eq!(
        XRasterFallbackCause::UnsupportedPutImage.as_str(),
        "unsupported_put_image"
    );
}

#[test]
fn fallback_coalescer_emits_first_and_power_of_two_cumulative_counts() {
    let mut coalescer = XRasterFallbackCoalescer::default();
    let surface = SurfaceId::new(226, 1);
    let cause = XRasterFallbackCause::UnsupportedPutImage;

    let emitted: Vec<_> = (0..10)
        .filter_map(|_| coalescer.observe(surface, cause))
        .collect();

    assert_eq!(emitted, vec![1, 2, 4, 8]);
    // Suppressed occurrences are still counted, never discarded.
    assert_eq!(coalescer.occurrences(surface, cause), 10);
}

#[test]
fn fallback_coalescer_separates_surfaces_and_causes() {
    let mut coalescer = XRasterFallbackCoalescer::default();
    let first = SurfaceId::new(227, 1);
    let second = SurfaceId::new(228, 1);

    assert_eq!(
        coalescer.observe(first, XRasterFallbackCause::UnsupportedPutImage),
        Some(1)
    );
    assert_eq!(
        coalescer.observe(first, XRasterFallbackCause::JournalCapacity),
        Some(1),
        "a second cause on the same surface is its own series"
    );
    assert_eq!(
        coalescer.observe(second, XRasterFallbackCause::UnsupportedPutImage),
        Some(1),
        "a second surface is its own series"
    );
    assert_eq!(
        coalescer.occurrences(first, XRasterFallbackCause::UnsupportedCommand),
        0
    );
}

#[test]
fn a_requirement_built_against_a_lagging_committed_generation_reports_the_gap() {
    let namespace = NamespaceId::from_raw(229);
    let window = XResourceId::new(0x229, 1);
    let surface = SurfaceId::new(229, 1);
    let mut runtime = XAuthorityRuntime::new();
    fallback_window(&mut runtime, namespace, window, surface, 280);

    // Reproduces the live condition: Engine builds a requirement from the scene
    // it last committed, while the client keeps drawing. The authority advances
    // its generation per draw, so by arrival it sits ahead of the request.
    let committed_by_engine = 2;
    for index in 0..5 {
        runtime.begin_dispatch();
        runtime.apply_core_draw(
            TransactionId::from_raw(281 + index),
            namespace,
            window,
            Region::single(Rect {
                x: 1,
                y: 1,
                width: 4,
                height: 4,
            }),
        );
    }

    let (cause, observed) = expect_raster_fallback_detail(
        runtime
            .apply_surface_raster_requirements(
                TransactionId::from_raw(290),
                &fallback_requirement(surface, committed_by_engine, &[750]),
            )
            .unwrap(),
        "a lagging requirement cannot be satisfied",
    );
    assert_eq!(cause, XRasterFallbackCause::StaleContentGeneration);
    assert!(
        observed > committed_by_engine,
        "the authority must report running ahead of the requested generation, \
         got observed={observed} requested={committed_by_engine}"
    );

    // The same requirement rebuilt against the authority's current generation
    // is satisfiable, which isolates the lag as the whole cause.
    expect_satisfied_raster(
        runtime
            .apply_surface_raster_requirements(
                TransactionId::from_raw(291),
                &fallback_requirement(surface, observed, &[750]),
            )
            .unwrap(),
        "the identical requirement must succeed once its generation matches",
    );
}

#[test]
fn a_requirement_naming_the_wrong_extent_is_distinguished_from_a_stale_generation() {
    let namespace = NamespaceId::from_raw(230);
    let window = XResourceId::new(0x231, 1);
    let surface = SurfaceId::new(230, 1);
    let mut runtime = XAuthorityRuntime::new();
    fallback_window(&mut runtime, namespace, window, surface, 300);

    runtime.begin_dispatch();
    runtime.apply_core_draw(
        TransactionId::from_raw(301),
        namespace,
        window,
        Region::single(Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 20,
        }),
    );

    let mut requirements = fallback_requirement(surface, 2, &[750]);
    requirements.logical_extent = Size {
        width: 41,
        height: 20,
    };
    assert_eq!(
        expect_raster_fallback(
            runtime
                .apply_surface_raster_requirements(TransactionId::from_raw(302), &requirements)
                .unwrap(),
            "an extent disagreement must not be reported as a stale generation",
        ),
        XRasterFallbackCause::LogicalExtentMismatch,
    );
}
