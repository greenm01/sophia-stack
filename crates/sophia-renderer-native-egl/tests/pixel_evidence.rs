use sophia_renderer_native_egl::{
    NATIVE_COMPOSITION_LUMINANCE_BUCKETS, NATIVE_COMPOSITION_PIXEL_PROOF_ATTEMPTS,
    native_composition_gl_read_y, native_composition_luminance, native_composition_pixel_metrics,
    native_composition_pixel_metrics_from_rows, native_composition_pixel_proof_capture,
};
#[cfg(feature = "gbm-platform")]
use sophia_renderer_native_egl::{NativeCpuTextureUpload, native_cpu_texture_upload};

#[cfg(feature = "gbm-platform")]
#[test]
fn cpu_texture_upload_reallocates_only_when_layer_extent_changes() {
    assert_eq!(
        native_cpu_texture_upload(2560, 1440, 2560, 24),
        NativeCpuTextureUpload::Reallocate
    );
    assert_eq!(
        native_cpu_texture_upload(2560, 24, 2560, 24),
        NativeCpuTextureUpload::Update
    );
}

#[test]
fn pixel_metrics_distinguish_rgb_and_alpha_populations() {
    let metrics =
        native_composition_pixel_metrics(&[0, 0, 0, 0, 1, 2, 3, 1, 4, 5, 6, 255, 0, 0, 0, 255]);

    assert_eq!(metrics.pixels, 4);
    assert_eq!(metrics.nonzero_rgb_pixels, 2);
    assert_eq!(metrics.other_pixels, 4);
    assert_eq!(metrics.alpha_zero_pixels, 1);
    assert_eq!(metrics.alpha_partial_pixels, 1);
    assert_eq!(metrics.alpha_opaque_pixels, 2);
    assert_ne!(metrics.checksum, 0);
}

#[test]
fn pixel_metrics_distinguish_asymmetric_color_channels() {
    let metrics = native_composition_pixel_metrics(&[
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255, 0, 255, 255, 255, 255, 0,
        255, 255, 128, 128, 128, 255,
    ]);

    assert_eq!(metrics.red_pixels, 1);
    assert_eq!(metrics.green_pixels, 1);
    assert_eq!(metrics.blue_pixels, 1);
    assert_eq!(metrics.yellow_pixels, 1);
    assert_eq!(metrics.cyan_pixels, 1);
    assert_eq!(metrics.magenta_pixels, 1);
    assert_eq!(metrics.gray_pixels, 1);
    assert_eq!(metrics.other_pixels, 0);
}

/// Luminance reads what the channel buckets are built not to see.
///
/// Both fixtures are neutral grays, so every channel-presence bucket assigns
/// them identically and the two are indistinguishable to every population that
/// existed before. They differ only in intensity, which is the whole of what a
/// filter working in the wrong space changes. A metric that cannot separate
/// these cannot judge a gamma correction, which is why one was added rather
/// than the existing populations being reused.
#[test]
fn luminance_separates_frames_the_channel_buckets_report_as_identical() {
    let dark = native_composition_pixel_metrics(&[64, 64, 64, 255, 64, 64, 64, 255]);
    let light = native_composition_pixel_metrics(&[192, 192, 192, 255, 192, 192, 192, 255]);

    assert_eq!(dark.gray_pixels, light.gray_pixels);
    assert_eq!(dark.nonzero_rgb_pixels, light.nonzero_rgb_pixels);
    assert_eq!(dark.other_pixels, light.other_pixels);
    assert_eq!(dark.alpha_opaque_pixels, light.alpha_opaque_pixels);

    assert!(light.luminance_sum > dark.luminance_sum);
    assert_ne!(light.luminance_buckets, dark.luminance_buckets);
    assert_eq!(dark.luminance_mean_millis(), 64_000);
    assert_eq!(light.luminance_mean_millis(), 192_000);
}

/// A mean can hold still while the population behind it splits.
///
/// Gamma-space filtering piles resampled edge pixels into the low-mid buckets
/// instead of spreading them, and a sum alone would report that as no change at
/// all. The histogram is the metric that judges the correction; the sum is for
/// a one-number comparison.
#[test]
fn luminance_histogram_separates_populations_that_share_a_mean() {
    let split = native_composition_pixel_metrics(&[0, 0, 0, 255, 246, 246, 246, 255]);
    let centered = native_composition_pixel_metrics(&[122, 122, 122, 255, 124, 124, 124, 255]);

    assert_eq!(
        split.luminance_mean_millis(),
        centered.luminance_mean_millis()
    );
    assert_ne!(split.luminance_buckets, centered.luminance_buckets);
    assert_eq!(split.luminance_buckets[0], 1);
    assert_eq!(split.luminance_buckets[15], 1);
    assert_eq!(centered.luminance_buckets[7], 2);
}

#[test]
fn luminance_weights_are_exact_and_span_the_channel_range() {
    // The weights sum to 256, so the shift is a division that never rounds and
    // white lands exactly on the top of the range rather than one short of it.
    assert_eq!(native_composition_luminance(255, 255, 255), 255);
    assert_eq!(native_composition_luminance(0, 0, 0), 0);
    assert!(
        native_composition_luminance(0, 255, 0) > native_composition_luminance(255, 0, 0),
        "green carries more luminance than red"
    );
    assert!(
        native_composition_luminance(255, 0, 0) > native_composition_luminance(0, 0, 255),
        "red carries more luminance than blue"
    );

    let saturated = native_composition_pixel_metrics(&[255, 255, 255, 255]);
    assert_eq!(
        saturated.luminance_buckets[NATIVE_COMPOSITION_LUMINANCE_BUCKETS - 1],
        1,
        "the brightest pixel lands in the last bucket, not past it"
    );
}

#[test]
fn luminance_histogram_field_is_one_count_per_bucket() {
    let metrics = native_composition_pixel_metrics(&[0, 0, 0, 255, 255, 255, 255, 255]);
    let field = metrics.luminance_histogram_field();

    let counts = field.split(':').collect::<Vec<_>>();
    assert_eq!(counts.len(), NATIVE_COMPOSITION_LUMINANCE_BUCKETS);
    assert_eq!(counts[0], "1");
    assert_eq!(counts[NATIVE_COMPOSITION_LUMINANCE_BUCKETS - 1], "1");
    assert!(
        !field.contains(' '),
        "the field stays one whitespace-free token"
    );
}

#[test]
fn composition_region_readback_converts_from_top_left_coordinates() {
    assert_eq!(native_composition_gl_read_y(1440, 64, 240), Some(1136));
    assert_eq!(native_composition_gl_read_y(1440, 1400, 41), None);
    assert_eq!(native_composition_gl_read_y(u32::MAX, u32::MAX, 1), None);
}

#[test]
fn pixel_checksum_is_deterministic_and_content_sensitive() {
    let first = native_composition_pixel_metrics(&[0, 0, 0, 255]);
    let same = native_composition_pixel_metrics(&[0, 0, 0, 255]);
    let changed = native_composition_pixel_metrics(&[1, 0, 0, 255]);

    assert_eq!(first.checksum, same.checksum);
    assert_ne!(first.checksum, changed.checksum);
}

#[test]
fn pixel_metrics_ignore_gbm_row_padding() {
    let metrics = native_composition_pixel_metrics_from_rows(
        &[
            1, 2, 3, 255, 0, 0, 0, 255, 99, 99, 99, 99, 4, 5, 6, 255, 0, 0, 0, 255, 88, 88, 88, 88,
        ],
        2,
        2,
        12,
    )
    .unwrap();

    assert_eq!(metrics.pixels, 4);
    assert_eq!(metrics.nonzero_rgb_pixels, 2);
    assert_eq!(metrics.alpha_opaque_pixels, 4);
}

#[test]
fn pixel_metrics_reject_invalid_gbm_rows() {
    assert!(native_composition_pixel_metrics_from_rows(&[0; 16], 2, 2, 4).is_none());
    assert!(native_composition_pixel_metrics_from_rows(&[0; 15], 2, 2, 8).is_none());
    assert!(native_composition_pixel_metrics_from_rows(&[], 0, 2, 8).is_none());
}

#[test]
fn argb_composition_uses_premultiplied_source_over_without_double_alpha() {
    let source = include_str!("../src/gl.rs");
    let shaders = include_str!("../src/gl/shaders.rs");

    assert!(source.contains(".blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA)"));
    assert!(!source.contains(".blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA)"));
    assert!(source.contains("get_uniform_location(program, \"source_is_opaque\")"));
    assert!(shaders.contains("if (source_is_opaque > 0.5)"));
    assert!(shaders.contains("color.a = 1.0"));
    assert!(shaders.contains("color.rgb = min(color.rgb, vec3(color.a))"));
    assert!(shaders.contains("vec4(color.rgb * opacity, color.a * opacity)"));
}

/// The proof budget is spent only where light could be shown.
///
/// An empty composition is a clear to black, so measuring one answers a
/// question nobody asked while consuming an attempt that a composition with
/// content needed. A live session lost all three attempts to its empty startup
/// compositions and then reported zero lit pixels for every frame it ever
/// presented.
#[test]
fn pixel_proof_attempts_are_spent_only_on_compositions_with_layers() {
    assert!(native_composition_pixel_proof_capture(0, 1));
    assert!(native_composition_pixel_proof_capture(
        NATIVE_COMPOSITION_PIXEL_PROOF_ATTEMPTS - 1,
        5
    ));
    assert!(!native_composition_pixel_proof_capture(0, 0));
    assert!(!native_composition_pixel_proof_capture(
        NATIVE_COMPOSITION_PIXEL_PROOF_ATTEMPTS,
        1
    ));
}
