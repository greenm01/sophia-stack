// What per-head density replay actually buys, measured rather than judged.
//
// Both candidates below use the same area rule. The only difference is the
// source: replay rasterizes from the retained drawing commands, while the
// reference resamples the canonical 1x drawable the way a head would if no
// exact variant existed. That isolates the architecture's contribution from
// the choice of filter.

/// Area-resamples a canonical 1x raster to `density`, the same rational
/// overlap replay uses. This is the honest comparison: a weaker filter would
/// flatter replay by construction.
fn area_resample(bytes: &[u8], size: Size, density: u32) -> Vec<u32> {
    let source_width = usize::try_from(size.width).unwrap_or(0);
    let source_height = usize::try_from(size.height).unwrap_or(0);
    let width = usize::try_from((size.width * i32::try_from(density).unwrap_or(1000)).div_euclid(1000)).unwrap_or(0);
    let height =
        usize::try_from((size.height * i32::try_from(density).unwrap_or(1000)).div_euclid(1000))
            .unwrap_or(0);
    let source = xrgb_pixels(bytes);
    let mut out = vec![0_u32; width * height];
    let density = i64::from(density);
    for y in 0..height {
        for x in 0..width {
            let mut area = 0_i64;
            let mut channels = [0_i64; 3];
            for sy in 0..source_height {
                let overlap_y = span_overlap(y as i64, sy as i64, density);
                if overlap_y == 0 {
                    continue;
                }
                for sx in 0..source_width {
                    let overlap_x = span_overlap(x as i64, sx as i64, density);
                    if overlap_x == 0 {
                        continue;
                    }
                    let pixel = source[sy * source_width + sx];
                    let weight = overlap_x * overlap_y;
                    area += weight;
                    for (index, channel) in channels.iter_mut().enumerate() {
                        let component = i64::from((pixel >> (8 * index as u32)) & 0xff);
                        *channel += component * component * weight;
                    }
                }
            }
            if area <= 0 {
                continue;
            }
            let component = |channel: i64| -> u32 {
                u32::try_from(((channel + area / 2) / area).max(0).isqrt()).unwrap_or(0xff) & 0xff
            };
            out[y * width + x] =
                component(channels[2]) << 16 | component(channels[1]) << 8 | component(channels[0]);
        }
    }
    out
}

fn span_overlap(destination: i64, source: i64, density: i64) -> i64 {
    let destination_left = destination * 1_000;
    let destination_right = destination_left + 1_000;
    let source_left = source * density;
    let source_right = source_left + density;
    (destination_right.min(source_right) - destination_left.max(source_left)).max(0)
}

fn peak_intensity(pixels: &[u32]) -> u32 {
    pixels.iter().map(|pixel| pixel & 0xff).max().unwrap_or(0)
}

fn fully_lit_count(pixels: &[u32]) -> usize {
    pixels.iter().filter(|pixel| *pixel & 0xff == 0xff).count()
}

/// Drives one drawing request and returns the canonical 1x raster alongside
/// the exact 0.75-density store replayed from the journal.
fn canonical_and_exact_750(
    namespace: NamespaceId,
    window: u32,
    request: &[u8],
    opcode: u8,
) -> (Vec<u8>, Size, Vec<u32>) {
    let gc = window + 1;
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
    put_image_dispatch(
        namespace,
        3,
        opcode,
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let canonical = runtime
        .take_cpu_buffer_updates()
        .into_iter()
        .find_map(|update| match update {
            XAuthorityCpuBufferUpdate::Replace(snapshot) if snapshot.size.width == 80 => {
                Some(snapshot)
            }
            _ => None,
        })
        .expect("the draw must publish a canonical replacement");

    let response = expect_satisfied_raster(
        runtime
            .apply_surface_raster_requirements(
                TransactionId::from_raw(9_970),
                &density_requirement(surface, 2, &[750]),
            )
            .unwrap(),
        "the 0.75 class must replay from the journal",
    );
    let [XAuthorityCpuBufferUpdate::Replace(derived)] = response.cpu_buffer_updates.as_slice()
    else {
        panic!("late density must publish one derived replacement");
    };

    (
        canonical.bytes.clone(),
        canonical.size,
        xrgb_pixels(&derived.bytes),
    )
}

#[test]
fn vector_content_replays_sharper_than_resampling_the_canonical() {
    let order = XByteOrder::LittleEndian;
    let window = 0x221001;
    let (canonical, size, exact) = canonical_and_exact_750(
        NamespaceId::from_raw(160),
        window,
        &poly_line_request(order, window, window + 1, &[(2, 10), (70, 10)]),
        65,
    );
    let resampled = area_resample(&canonical, size, 750);

    // Replay rasterizes the line at the target density, so it lands on whole
    // pixels and stays fully lit. Resampling the composed 1x raster spreads
    // the same line across two rows, so nothing reaches full intensity. This
    // is the case per-head composition exists for.
    assert!(
        fully_lit_count(&exact) > 0,
        "a replayed line must stay fully lit at native density"
    );
    assert_eq!(
        fully_lit_count(&resampled),
        0,
        "resampling the canonical raster cannot keep a one-pixel line solid"
    );
    assert!(
        peak_intensity(&exact) > peak_intensity(&resampled),
        "replay must beat resampling on peak stroke intensity, got {} vs {}",
        peak_intensity(&exact),
        peak_intensity(&resampled),
    );
}

#[test]
fn fixed_bitmap_text_is_soft_at_a_fractional_density_either_way() {
    let order = XByteOrder::LittleEndian;
    let window = 0x221101;
    let (canonical, size, exact) = canonical_and_exact_750(
        NamespaceId::from_raw(161),
        window,
        &image_text8_request(order, window, window + 1, 4, 16, b"AAAA"),
        76,
    );
    let resampled = area_resample(&canonical, size, 750);

    // A 6x13 cell becomes 4.5 pixels wide, so every stem is narrower than a
    // pixel and no glyph can land fully lit however it is produced. Replay and
    // resampling agree closely here, which is why per-head density buys little
    // for fixed bitmap text and why the mirror gate's visual criterion is not
    // reachable with this font at this ratio.
    assert_eq!(
        fully_lit_count(&exact),
        0,
        "no stem of a 6x13 glyph can occupy a whole pixel at 0.75"
    );
    let exact_peak = i64::from(peak_intensity(&exact));
    let resampled_peak = i64::from(peak_intensity(&resampled));
    assert!(
        (exact_peak - resampled_peak).abs() <= 24,
        "replay and resampling must land close for bitmap glyphs, got {exact_peak} vs {resampled_peak}"
    );
}
