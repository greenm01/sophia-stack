use sophia_renderer_native_egl::{
    native_composition_pixel_metrics, native_composition_pixel_metrics_from_rows,
};

#[test]
fn pixel_metrics_distinguish_rgb_and_alpha_populations() {
    let metrics =
        native_composition_pixel_metrics(&[0, 0, 0, 0, 1, 2, 3, 1, 4, 5, 6, 255, 0, 0, 0, 255]);

    assert_eq!(metrics.pixels, 4);
    assert_eq!(metrics.nonzero_rgb_pixels, 2);
    assert_eq!(metrics.alpha_zero_pixels, 1);
    assert_eq!(metrics.alpha_partial_pixels, 1);
    assert_eq!(metrics.alpha_opaque_pixels, 2);
    assert_ne!(metrics.checksum, 0);
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

    assert!(source.contains(".blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA)"));
    assert!(!source.contains(".blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA)"));
    assert!(source.contains("vec4(color.rgb * opacity, color.a * opacity)"));
}
