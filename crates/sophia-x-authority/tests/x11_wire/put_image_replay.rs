// Wire-level regressions for replayable core `PutImage`.
//
// These reproduce the operation order a real xterm issues at startup, which is
// what signed mirror gate attempt 0019 could not satisfy: an accepted
// `PutImage` poisoned the journal, so a 750-density head could only select the
// canonical 1000-density handle.

fn put_image_dispatch(
    namespace: NamespaceId,
    sequence: u16,
    opcode: u8,
    bytes: &[u8],
    runtime: &mut XAuthorityRuntime,
    atoms: &mut XAtomTable,
    properties: &mut XPropertyTable,
) -> XDispatchResult {
    let request = decode_x11_core_request(
        context(
            namespace,
            9_500 + u64::from(sequence),
            XByteOrder::LittleEndian,
        ),
        bytes,
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, sequence, XByteOrder::LittleEndian, opcode),
        request,
        runtime,
        atoms,
        properties,
    )
}

/// A vertical red/blue split whose boundary does not align with a 0.75-density
/// pixel edge. Area-coverage replay must therefore produce boundary pixels
/// carrying both channels, which plain sampling of either source color cannot.
fn split_image(width: i32, height: i32, split_column: i32) -> Vec<u8> {
    let mut pixels = Vec::new();
    for _ in 0..height {
        for x in 0..width {
            let pixel: u32 = if x < split_column {
                0x00ff_0000
            } else {
                0x0000_00ff
            };
            pixels.extend_from_slice(&pixel.to_le_bytes());
        }
    }
    pixels
}

fn density_requirement(
    surface: SurfaceId,
    committed_content_generation: u64,
    densities: &[u32],
) -> SurfaceRasterRequirements {
    SurfaceRasterRequirements {
        surface,
        committed_content_generation,
        requirement_generation: 1,
        logical_extent: Size {
            width: 80,
            height: 40,
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

/// Drives the traced xterm startup order and returns the runtime plus the
/// content generation committed by the last drawing request.
fn xterm_startup_sequence(
    namespace: NamespaceId,
    window: u32,
    gc: u32,
    image: &[u8],
) -> (XAuthorityRuntime, u64) {
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let order = XByteOrder::LittleEndian;

    put_image_dispatch(
        namespace,
        1,
        1,
        &create_window_request(order, window, 0, 0, 80, 40),
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    put_image_dispatch(
        namespace,
        2,
        55,
        // GXcopy with a full plane mask: the unconditional write that lets an
        // upload be retained as replayable content.
        &create_gc_values_request(
            order,
            gc,
            window,
            u32::from(X_GX_COPY),
            u32::MAX,
            0x00ff_ffff,
            0,
            0,
            0,
        ),
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    // Startup upload, before any text or vector drawing.
    put_image_dispatch(
        namespace,
        3,
        72,
        &put_image_request(
            order,
            window,
            gc,
            PutImageGeometry {
                width: 8,
                height: 4,
                dst_x: 0,
                dst_y: 0,
            },
            image,
        ),
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    put_image_dispatch(
        namespace,
        4,
        76,
        &image_text8_request(order, window, gc, 4, 16, b"AaZz"),
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    put_image_dispatch(
        namespace,
        5,
        74,
        &poly_text8_request(order, window, gc, 4, 30, b"xterm"),
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    put_image_dispatch(
        namespace,
        6,
        65,
        &poly_line_request(order, window, gc, &[(2, 34), (70, 34)]),
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    // Same-drawable scroll, below the uploaded region.
    put_image_dispatch(
        namespace,
        7,
        62,
        &copy_area_request(order, window, window, gc, 0, 20, 0, 16, 40, 10),
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    // One generation per accepted drawing request, after the creating one.
    (runtime, 6)
}

#[test]
fn xterm_startup_sequence_replays_exact_authority_rasters_at_both_densities() {
    let namespace = NamespaceId::from_raw(147);
    let window = 0x220b01;
    let surface = SurfaceId::new(window, 1);
    let image = split_image(8, 4, 3);
    let (mut runtime, generation) = xterm_startup_sequence(namespace, window, 0x220b02, &image);

    let response = expect_satisfied_raster(
        runtime
            .apply_surface_raster_requirements(
                TransactionId::from_raw(9_600),
                &density_requirement(surface, generation, &[750, 1_000]),
            )
            .unwrap(),
        "an accepted PutImage must no longer poison the journal",
    );

    // Both requested densities must publish authority-owned content. This is
    // the condition gate attempt 0019 failed.
    assert_eq!(response.transaction.content.variants().len(), 2);
    assert!(
        response
            .transaction
            .content
            .variants()
            .iter()
            .all(|variant| variant.fidelity
                == sophia_protocol::SurfaceContentFidelity::AuthorityRaster),
        "no variant may be published as sampled compatibility content"
    );
    let canonical = response.transaction.content.canonical_variant();
    let derived = response
        .transaction
        .content
        .variants()
        .iter()
        .find(|variant| variant.density_millis == 750)
        .expect("the 0.75 density class must have its own variant");
    assert_eq!(
        canonical.pixel_size,
        Size {
            width: 80,
            height: 40
        }
    );
    assert_eq!(
        derived.pixel_size,
        Size {
            width: 60,
            height: 30
        },
        "the derived store must be native size, not a scaled canonical handle"
    );
    assert_ne!(
        canonical.source, derived.source,
        "each density must own a distinct backing handle"
    );

    let [XAuthorityCpuBufferUpdate::Replace(store)] = response.cpu_buffer_updates.as_slice() else {
        panic!("late density must publish one immutable derived replacement");
    };
    assert_eq!(
        store.size,
        Size {
            width: 60,
            height: 30
        }
    );

    // The uploaded region projects to the derived store's top-left corner. A
    // boundary pixel must mix both source colors; sampling either one alone
    // could not produce that, so this proves the pixels were replayed.
    let pixels = xrgb_pixels(&store.bytes);
    let blended = (0..3).any(|y| {
        (0..6).any(|x| {
            let pixel = pixels[y * 60 + x];
            let red = (pixel >> 16) & 0xff;
            let blue = pixel & 0xff;
            red > 0 && blue > 0
        })
    });
    assert!(
        blended,
        "0.75-density replay must area-average the retained client pixels"
    );
}

#[test]
fn put_image_replay_keeps_fully_covered_pixels_exact() {
    let namespace = NamespaceId::from_raw(148);
    let window = 0x220c01;
    let surface = SurfaceId::new(window, 1);
    let image = split_image(8, 4, 3);
    let (mut runtime, generation) = xterm_startup_sequence(namespace, window, 0x220c02, &image);

    let response = expect_satisfied_raster(
        runtime
            .apply_surface_raster_requirements(
                TransactionId::from_raw(9_700),
                &density_requirement(surface, generation, &[750]),
            )
            .unwrap(),
        "the 0.75 density class must replay from the journal",
    );
    let [XAuthorityCpuBufferUpdate::Replace(store)] = response.cpu_buffer_updates.as_slice() else {
        panic!("the 0.75 class must publish one derived replacement");
    };
    let pixels = xrgb_pixels(&store.bytes);

    // A destination pixel whose source coverage lies wholly inside one color
    // must carry that color exactly. Only the pixels straddling the boundary
    // may blend, so averaging cannot smear across the whole image.
    assert_eq!(
        pixels[0], 0x00ff_0000,
        "a fully red-covered destination pixel must stay exactly red"
    );
    assert_eq!(
        pixels[4], 0x0000_00ff,
        "a fully blue-covered destination pixel must stay exactly blue"
    );
}

#[test]
fn a_later_commit_makes_the_previous_requirement_stale() {
    let namespace = NamespaceId::from_raw(149);
    let window = 0x220d01;
    let gc = 0x220d02;
    let surface = SurfaceId::new(window, 1);
    let image = split_image(8, 4, 3);
    let (mut runtime, generation) = xterm_startup_sequence(namespace, window, gc, &image);

    let requirement = density_requirement(surface, generation, &[750]);
    expect_satisfied_raster(
        runtime
            .apply_surface_raster_requirements(TransactionId::from_raw(9_800), &requirement)
            .unwrap(),
        "the first requirement must be satisfiable",
    );

    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    put_image_dispatch(
        namespace,
        8,
        76,
        &image_text8_request(XByteOrder::LittleEndian, window, gc, 4, 16, b"more"),
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert_eq!(
        expect_raster_fallback(
            runtime
                .apply_surface_raster_requirements(TransactionId::from_raw(9_801), &requirement)
                .unwrap(),
            "a requirement built against superseded content must fail closed",
        ),
        XRasterFallbackCause::StaleContentGeneration,
    );
}

#[test]
fn wire_xy_pixmap_upload_is_classified_unsupported_at_dispatch() {
    let namespace = NamespaceId::from_raw(150);
    let window = 0x220e01;
    let gc = 0x220e02;
    let surface = SurfaceId::new(window, 1);
    let order = XByteOrder::LittleEndian;
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    put_image_dispatch(
        namespace,
        1,
        1,
        &create_window_request(order, window, 0, 0, 80, 40),
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    put_image_dispatch(
        namespace,
        2,
        55,
        &create_gc_values_request(
            order,
            gc,
            window,
            u32::from(X_GX_COPY),
            u32::MAX,
            0x00ff_ffff,
            0,
            0,
            0,
        ),
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    // XYPixmap reaches the journal as an unsupported upload, which proves the
    // dispatch forwards the wire format rather than assuming ZPixmap.
    put_image_dispatch(
        namespace,
        3,
        72,
        &put_image_request_with_format(
            order,
            1,
            window,
            gc,
            PutImageGeometry {
                width: 8,
                height: 4,
                dst_x: 0,
                dst_y: 0,
            },
            &split_image(8, 4, 3),
        ),
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert_eq!(
        expect_raster_fallback(
            runtime
                .apply_surface_raster_requirements(
                    TransactionId::from_raw(9_900),
                    &density_requirement(surface, 2, &[750]),
                )
                .unwrap(),
            "an XYPixmap upload must not be retained as replayable",
        ),
        XRasterFallbackCause::UnsupportedPutImage,
    );
}
