//! Premultiplied Porter-Duff pixel math for RENDER.
//!
//! Linear, matching Xorg's fb implementation: RENDER compositing is defined
//! on premultiplied values with no gamma correction, and a client
//! hand-computes expected results against that definition. The gamma-aware
//! density blend in `raster_replay` answers a different question -- scaling
//! text without darkening its edges -- and must not be used here.

use sophia_protocol::Rect;

use super::raster_ops::{bytes_mut, clipped_bounds};
use super::update::XAuthorityCpuBufferSnapshot;
use crate::{
    X_RENDER_FORMAT_A1, X_RENDER_FORMAT_A8, X_RENDER_FORMAT_ARGB32, X_RENDER_FORMAT_RGB24,
};

/// The pixel layouts a picture may take, and how each one reads and writes
/// the 32-bit store slot behind it.
///
/// Every drawable's backing is 32 bits per pixel whatever its depth, so a
/// narrow format is a view over the same slot: A8 and A1 keep their channel
/// in the alpha-position byte, and RGB24 has no alpha component at all. The
/// window buffer tag stays `XR24`, which is why an RGB24 write forces the
/// alpha byte to zero rather than storing what the blend produced -- the
/// compositor was promised an opaque buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XRenderPictFormatKind {
    Argb32,
    Rgb24,
    A8,
    A1,
}

impl XRenderPictFormatKind {
    pub fn from_format_id(id: u32) -> Option<Self> {
        match id {
            X_RENDER_FORMAT_ARGB32 => Some(Self::Argb32),
            X_RENDER_FORMAT_RGB24 => Some(Self::Rgb24),
            X_RENDER_FORMAT_A8 => Some(Self::A8),
            X_RENDER_FORMAT_A1 => Some(Self::A1),
            _ => None,
        }
    }

    pub const fn depth(self) -> u8 {
        match self {
            Self::Argb32 => 32,
            Self::Rgb24 => 24,
            Self::A8 => 8,
            Self::A1 => 1,
        }
    }

    /// One store slot as a premultiplied `[b, g, r, a]` sample.
    pub(crate) fn read(self, slot: [u8; 4]) -> [u8; 4] {
        match self {
            Self::Argb32 => slot,
            // No alpha component means opaque, not transparent: a picture
            // over an RGB24 drawable composites onto its colors.
            Self::Rgb24 => [slot[0], slot[1], slot[2], 0xff],
            Self::A8 | Self::A1 => [0, 0, 0, slot[3]],
        }
    }

    /// The store slot for one premultiplied `[b, g, r, a]` result. A format
    /// without a channel discards it, which is the protocol's definition of
    /// compositing onto that format rather than a loss.
    pub(crate) fn write(self, pixel: [u8; 4]) -> [u8; 4] {
        match self {
            Self::Argb32 => pixel,
            Self::Rgb24 => [pixel[0], pixel[1], pixel[2], 0],
            Self::A8 => [0, 0, 0, pixel[3]],
            Self::A1 => [0, 0, 0, if pixel[3] >= 128 { 0xff } else { 0 }],
        }
    }
}

/// The operators with an implementation behind them: the original Porter-Duff
/// twelve plus Add and Saturate. The Disjoint, Conjoint and PDF ranges are
/// declined at dispatch, and no measured client sends them.
pub(crate) fn render_operator_is_implemented(op: u8) -> bool {
    op <= 13
}

/// `value * factor / 255`, rounded, the fixed-point multiply every operator
/// factor is applied with.
fn mul_div_255(value: u8, factor: u8) -> u8 {
    ((u16::from(value) * u16::from(factor) + 127) / 255) as u8
}

/// One premultiplied Porter-Duff blend: `src * Fa + dst * Fb`, with the
/// factors the operator defines. Both pixels and the result are premultiplied
/// `[b, g, r, a]`.
pub(crate) fn render_blend_pixel(op: u8, src: [u8; 4], dst: [u8; 4]) -> [u8; 4] {
    let src_alpha = src[3];
    let dst_alpha = dst[3];
    let (src_factor, dst_factor) = match op {
        0 => (0, 0),                              // Clear
        1 => (255, 0),                            // Src
        2 => (0, 255),                            // Dst
        3 => (255, 255 - src_alpha),              // Over
        4 => (255 - dst_alpha, 255),              // OverReverse
        5 => (dst_alpha, 0),                      // In
        6 => (0, src_alpha),                      // InReverse
        7 => (255 - dst_alpha, 0),                // Out
        8 => (0, 255 - src_alpha),                // OutReverse
        9 => (dst_alpha, 255 - src_alpha),        // Atop
        10 => (255 - dst_alpha, src_alpha),       // AtopReverse
        11 => (255 - dst_alpha, 255 - src_alpha), // Xor
        12 => (255, 255),                         // Add
        // Saturate: as much source as the destination has room for.
        13 => {
            let src_factor = if src_alpha == 0 {
                255
            } else {
                (u32::from(255 - dst_alpha) * 255 / u32::from(src_alpha)).min(255) as u8
            };
            (src_factor, 255)
        }
        // Dispatch validates the operator before any pixel is touched.
        _ => (0, 255),
    };
    let mut out = [0u8; 4];
    for channel in 0..4 {
        out[channel] = mul_div_255(src[channel], src_factor)
            .saturating_add(mul_div_255(dst[channel], dst_factor));
    }
    out
}

/// Whether a destination point is inside the picture's clip list, already
/// translated by its clip origin. An empty list clips nothing.
fn render_point_in_clip(x: usize, y: usize, clip: &[Rect]) -> bool {
    if clip.is_empty() {
        return true;
    }
    let x = i32::try_from(x).unwrap_or(i32::MAX);
    let y = i32::try_from(y).unwrap_or(i32::MAX);
    clip.iter().any(|rect| {
        x >= rect.x
            && y >= rect.y
            && x < rect.x.saturating_add(rect.width)
            && y < rect.y.saturating_add(rect.height)
    })
}

/// Fill one rectangle with one premultiplied color through an operator,
/// honouring the destination format and clip list.
pub(super) fn render_fill_rect(
    buffer: &mut XAuthorityCpuBufferSnapshot,
    rect: Rect,
    op: u8,
    color: [u8; 4],
    clip: &[Rect],
    format: XRenderPictFormatKind,
) {
    let Some((left, top, right, bottom)) = clipped_bounds(buffer.size, rect) else {
        return;
    };
    let stride = usize::try_from(buffer.stride).unwrap_or(0);
    let bytes = bytes_mut(buffer);
    for y in top..bottom {
        for x in left..right {
            if !render_point_in_clip(x, y, clip) {
                continue;
            }
            let offset = y.saturating_mul(stride).saturating_add(x.saturating_mul(4));
            if let Some(slot) = bytes.get_mut(offset..offset.saturating_add(4)) {
                let existing: [u8; 4] = slot.try_into().unwrap_or([0; 4]);
                let blended = render_blend_pixel(op, color, format.read(existing));
                slot.copy_from_slice(&format.write(blended));
            }
        }
    }
}
