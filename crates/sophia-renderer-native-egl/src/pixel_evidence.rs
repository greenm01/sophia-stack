#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeCompositionPixelMetrics {
    pub pixels: usize,
    pub nonzero_rgb_pixels: usize,
    pub alpha_zero_pixels: usize,
    pub alpha_partial_pixels: usize,
    pub alpha_opaque_pixels: usize,
    pub checksum: u64,
}

pub fn native_composition_pixel_metrics(rgba: &[u8]) -> NativeCompositionPixelMetrics {
    let mut metrics = NativeCompositionPixelMetrics {
        pixels: rgba.len() / 4,
        checksum: 0xcbf2_9ce4_8422_2325,
        ..NativeCompositionPixelMetrics::default()
    };
    for pixel in rgba.chunks_exact(4) {
        if pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0 {
            metrics.nonzero_rgb_pixels = metrics.nonzero_rgb_pixels.saturating_add(1);
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
