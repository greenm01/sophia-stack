/// How many luminance buckets the histogram spreads the 0..=255 range across.
pub const NATIVE_COMPOSITION_LUMINANCE_BUCKETS: usize = 16;

/// Integer Rec.709-shaped luminance weights, chosen to sum to exactly 256.
///
/// The sum being a power of two is what keeps this exact: the shift below is a
/// division that never rounds, so two runs of the same frame agree bit for bit
/// the way `checksum` does. A float luma would not survive that comparison.
const LUMINANCE_WEIGHTS: (u32, u32, u32) = (54, 183, 19);

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
    /// Total luminance, the one population above that reads intensity.
    ///
    /// The channel buckets are deliberately blind to it — they key on which
    /// channels are lit so a palette stays legible across an intensity
    /// conversion, which is exactly what makes them unable to judge one. A
    /// filter that weights gamma-encoded bytes as though they were light moves
    /// every resampled edge pixel and moves none of those buckets.
    pub luminance_sum: u64,
    /// The distribution behind that sum.
    ///
    /// A mean can stay put while a population splits, and the signature of
    /// gamma-space filtering is a shape: edge pixels piled into the low-mid
    /// buckets that belong nearer the middle. Judge the change on this and keep
    /// the sum for a one-number comparison.
    pub luminance_buckets: [u32; NATIVE_COMPOSITION_LUMINANCE_BUCKETS],
    pub checksum: u64,
}

impl NativeCompositionPixelMetrics {
    /// Mean luminance in thousandths, so a comparison stays integer.
    pub const fn luminance_mean_millis(&self) -> u64 {
        match self.pixels {
            0 => 0,
            pixels => self.luminance_sum.saturating_mul(1_000) / pixels as u64,
        }
    }

    /// The bucket populations as one colon-separated evidence field.
    pub fn luminance_histogram_field(&self) -> String {
        let mut field = String::with_capacity(NATIVE_COMPOSITION_LUMINANCE_BUCKETS * 4);
        for (index, count) in self.luminance_buckets.iter().enumerate() {
            if index > 0 {
                field.push(':');
            }
            field.push_str(&count.to_string());
        }
        field
    }
}

/// The luminance of one pixel, on the same 0..=255 scale as its channels.
pub const fn native_composition_luminance(red: u8, green: u8, blue: u8) -> u8 {
    let (red_weight, green_weight, blue_weight) = LUMINANCE_WEIGHTS;
    let weighted =
        red_weight * red as u32 + green_weight * green as u32 + blue_weight * blue as u32;
    (weighted >> 8) as u8
}

/// How many compositions a context may read back to prove it emits light.
///
/// A readback stalls the pipeline on the whole framebuffer, so this is a proof
/// budget, not a measurement of every frame. What a context measures last is
/// what it keeps.
pub const NATIVE_COMPOSITION_PIXEL_PROOF_ATTEMPTS: usize = 3;

/// Whether this composition is worth spending a proof attempt on.
///
/// A composition with no layers clears to black by construction, so measuring
/// it proves nothing and costs an attempt that a composition with content
/// needed. A live session spent its entire budget on exactly those: the three
/// empty startup compositions of its primary head. Every present afterwards
/// carried the zero those attempts latched, so nothing was ever judged to have
/// put light on a screen and startup readiness never arrived.
pub const fn native_composition_pixel_proof_capture(attempts: usize, layers: usize) -> bool {
    attempts < NATIVE_COMPOSITION_PIXEL_PROOF_ATTEMPTS && layers > 0
}

/// Retains the strongest nonzero-pixel proof a renderer context has observed.
///
/// A requested region trace reads the same finished composition as the
/// bounded full-frame proof. It therefore remains useful evidence after the
/// full-frame budget is exhausted, and a later black frame must not erase an
/// earlier proof that this head emitted light.
pub const fn retain_native_composition_nonzero_proof(
    previous: usize,
    captured_frame: usize,
    traced_region: usize,
) -> usize {
    let observed = if captured_frame > traced_region {
        captured_frame
    } else {
        traced_region
    };
    if previous > observed {
        previous
    } else {
        observed
    }
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
        let luminance = native_composition_luminance(*red, *green, *blue);
        metrics.luminance_sum = metrics.luminance_sum.saturating_add(u64::from(luminance));
        let bucket = usize::from(luminance) / (256 / NATIVE_COMPOSITION_LUMINANCE_BUCKETS);
        metrics.luminance_buckets[bucket] = metrics.luminance_buckets[bucket].saturating_add(1);
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
