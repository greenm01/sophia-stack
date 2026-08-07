#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeCompositionPixelMetrics {
    pub pixels: usize,
    pub nonzero_rgb_pixels: usize,
    pub red_pixels: usize,
    pub green_pixels: usize,
    pub blue_pixels: usize,
    pub yellow_pixels: usize,
    pub cyan_pixels: usize,
    pub magenta_pixels: usize,
    pub gray_pixels: usize,
    pub other_pixels: usize,
    pub alpha_zero_pixels: usize,
    pub alpha_partial_pixels: usize,
    pub alpha_opaque_pixels: usize,
    pub checksum: u64,
}

/// Convert a top-left composition region into OpenGL's bottom-left readback Y.
pub const fn native_composition_gl_read_y(frame_height: u32, top: u32, height: u32) -> Option<u32> {
    match top.checked_add(height) {
        Some(bottom) => frame_height.checked_sub(bottom),
        None => None,
    }
}

pub fn native_composition_pixel_metrics(rgba: &[u8]) -> NativeCompositionPixelMetrics {
    let mut metrics = NativeCompositionPixelMetrics {
        pixels: rgba.len() / 4,
        checksum: 0xcbf2_9ce4_8422_2325,
        ..NativeCompositionPixelMetrics::default()
    };
    for pixel in rgba.chunks_exact(4) {
        let [red, green, blue, _alpha] = pixel else {
            unreachable!("chunks_exact(4) always yields four bytes");
        };
        if *red != 0 || *green != 0 || *blue != 0 {
            metrics.nonzero_rgb_pixels = metrics.nonzero_rgb_pixels.saturating_add(1);
        }
        // These broad, channel-oriented buckets make diagnostic palettes
        // robust to intensity conversion while still exposing channel swaps.
        match (*red != 0, *green != 0, *blue != 0) {
            (true, false, false) => metrics.red_pixels = metrics.red_pixels.saturating_add(1),
            (false, true, false) => metrics.green_pixels = metrics.green_pixels.saturating_add(1),
            (false, false, true) => metrics.blue_pixels = metrics.blue_pixels.saturating_add(1),
            (true, true, false) => metrics.yellow_pixels = metrics.yellow_pixels.saturating_add(1),
            (false, true, true) => metrics.cyan_pixels = metrics.cyan_pixels.saturating_add(1),
            (true, false, true) => {
                metrics.magenta_pixels = metrics.magenta_pixels.saturating_add(1)
            }
            (true, true, true) if red == green && green == blue => {
                metrics.gray_pixels = metrics.gray_pixels.saturating_add(1)
            }
            _ => metrics.other_pixels = metrics.other_pixels.saturating_add(1),
        }
        match pixel[3] {
            0 => metrics.alpha_zero_pixels = metrics.alpha_zero_pixels.saturating_add(1),
            255 => metrics.alpha_opaque_pixels = metrics.alpha_opaque_pixels.saturating_add(1),
            _ => metrics.alpha_partial_pixels = metrics.alpha_partial_pixels.saturating_add(1),
        }
        for byte in pixel {
            metrics.checksum ^= u64::from(*byte);
            metrics.checksum = metrics.checksum.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    metrics
}

pub fn native_composition_pixel_metrics_from_rows(
    bytes: &[u8],
    width: u32,
    height: u32,
    stride: u32,
) -> Option<NativeCompositionPixelMetrics> {
    let row_bytes = usize::try_from(width).ok()?.checked_mul(4)?;
    let row_count = usize::try_from(height).ok()?;
    let stride = usize::try_from(stride).ok()?;
    if row_bytes == 0 || row_count == 0 || stride < row_bytes {
        return None;
    }
    let required_len = stride
        .checked_mul(row_count.saturating_sub(1))?
        .checked_add(row_bytes)?;
    if bytes.len() < required_len {
        return None;
    }

    let pixel_len = row_bytes.checked_mul(row_count)?;
    let mut pixels = Vec::with_capacity(pixel_len);
    for row in bytes.chunks(stride).take(row_count) {
        pixels.extend_from_slice(row.get(..row_bytes)?);
    }
    (pixels.len() == pixel_len).then(|| native_composition_pixel_metrics(&pixels))
}
