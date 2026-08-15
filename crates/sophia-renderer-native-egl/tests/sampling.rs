use sophia_renderer_native_egl::{NativeCompositionSampling, native_composition_sampling};
use sophia_x_authority::x_fixed_glyph_rows;

#[test]
fn composition_sampling_preserves_exact_pixels_and_reconstructs_reductions() {
    assert_eq!(
        native_composition_sampling((2560, 1440), (2560, 1440)),
        NativeCompositionSampling::ExactNearest,
    );
    assert_eq!(
        native_composition_sampling((2560, 1440), (1920, 1080)),
        NativeCompositionSampling::SharpDownscale,
    );
    assert_eq!(
        native_composition_sampling((1920, 1200), (1920, 1080)),
        NativeCompositionSampling::SharpDownscale,
    );
    assert_eq!(
        native_composition_sampling((1280, 720), (2560, 1440)),
        NativeCompositionSampling::LinearUpscale,
    );
}

#[test]
fn sampling_names_are_stable_evidence_values() {
    assert_eq!(
        NativeCompositionSampling::ExactNearest.reduced_name(),
        "exact_nearest"
    );
    assert_eq!(
        NativeCompositionSampling::SharpDownscale.reduced_name(),
        "sharp_downscale"
    );
    assert_eq!(
        NativeCompositionSampling::LinearUpscale.reduced_name(),
        "linear_upscale"
    );
}

#[test]
fn sharp_reconstruction_preserves_mixed_case_bitmap_strokes_at_three_quarters_scale() {
    let width = 24usize;
    let height = 13usize;
    let mut source = vec![0.0f32; width * height];
    for (glyph, byte) in [b'A', b'a', b'Z', b'z'].into_iter().enumerate() {
        for (row, bits) in x_fixed_glyph_rows(byte).into_iter().enumerate() {
            for column in 0..6 {
                if bits & (1 << (5 - column)) != 0 {
                    source[row * width + glyph * 6 + column] = 1.0;
                }
            }
        }
    }

    let sharp = resample(&source, (width, height), (18, 10), Sample::Sharp);
    let linear = resample(&source, (width, height), (18, 10), Sample::Linear);
    let nearest = resample(&source, (width, height), (18, 10), Sample::Nearest);

    assert!(sharp.iter().any(|value| *value > 0.0 && *value < 1.0));
    assert!(sharp.iter().any(|value| *value > 0.85));
    assert_ne!(quantized(&sharp), quantized(&linear));
    assert_ne!(quantized(&sharp), quantized(&nearest));
    for range in [0..4, 4..9, 9..13, 13..18] {
        assert!(
            (0..10).any(|row| range.clone().any(|column| sharp[row * 18 + column] > 0.2)),
            "every mixed-case glyph retains visible coverage"
        );
    }

    let shader = include_str!("../src/gl/shaders.rs");
    assert!(shader.contains("SHARP_DOWNSCALE_FRAGMENT_SHADER"));
    assert!(shader.contains("for (int row = -1; row <= 2; row++)"));
    assert!(shader.contains("for (int column = -1; column <= 2; column++)"));
}

#[derive(Clone, Copy)]
enum Sample {
    Nearest,
    Linear,
    Sharp,
}

fn resample(
    source: &[f32],
    source_size: (usize, usize),
    target_size: (usize, usize),
    sample: Sample,
) -> Vec<f32> {
    let mut target = vec![0.0; target_size.0 * target_size.1];
    for y in 0..target_size.1 {
        for x in 0..target_size.0 {
            let source_x = (x as f32 + 0.5) * source_size.0 as f32 / target_size.0 as f32 - 0.5;
            let source_y = (y as f32 + 0.5) * source_size.1 as f32 / target_size.1 as f32 - 0.5;
            target[y * target_size.0 + x] = match sample {
                Sample::Nearest => texel(source, source_size, source_x.round(), source_y.round()),
                Sample::Linear => bilinear(source, source_size, source_x, source_y),
                Sample::Sharp => catmull_sample(source, source_size, source_x, source_y),
            };
        }
    }
    target
}

fn texel(source: &[f32], size: (usize, usize), x: f32, y: f32) -> f32 {
    let x = (x as isize).clamp(0, size.0 as isize - 1) as usize;
    let y = (y as isize).clamp(0, size.1 as isize - 1) as usize;
    source[y * size.0 + x]
}

fn bilinear(source: &[f32], size: (usize, usize), x: f32, y: f32) -> f32 {
    let left = x.floor();
    let top = y.floor();
    let fraction_x = x - left;
    let fraction_y = y - top;
    let top_value = texel(source, size, left, top) * (1.0 - fraction_x)
        + texel(source, size, left + 1.0, top) * fraction_x;
    let bottom_value = texel(source, size, left, top + 1.0) * (1.0 - fraction_x)
        + texel(source, size, left + 1.0, top + 1.0) * fraction_x;
    top_value * (1.0 - fraction_y) + bottom_value * fraction_y
}

fn catmull_sample(source: &[f32], size: (usize, usize), x: f32, y: f32) -> f32 {
    let origin_x = x.floor();
    let origin_y = y.floor();
    let fraction_x = x - origin_x;
    let fraction_y = y - origin_y;
    let mut sum = 0.0;
    let mut total = 0.0;
    for row in -1..=2 {
        let weight_y = catmull_rom(row as f32 - fraction_y);
        for column in -1..=2 {
            let weight = weight_y * catmull_rom(column as f32 - fraction_x);
            sum += texel(
                source,
                size,
                origin_x + column as f32,
                origin_y + row as f32,
            ) * weight;
            total += weight;
        }
    }
    (sum / total.max(0.0001)).clamp(0.0, 1.0)
}

fn catmull_rom(value: f32) -> f32 {
    let x = value.abs();
    if x <= 1.0 {
        ((1.5 * x - 2.5) * x) * x + 1.0
    } else if x < 2.0 {
        ((-0.5 * x + 2.5) * x - 4.0) * x + 2.0
    } else {
        0.0
    }
}

fn quantized(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .map(|value| (value * 255.0).round() as u8)
        .collect()
}
