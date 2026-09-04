use std::fmt;
use std::sync::Arc;

use sha2::{Digest, Sha256};

pub const MAX_CURSOR_EDGE: u32 = 128;
pub const MAX_CURSOR_PIXEL_BYTES: usize = MAX_CURSOR_EDGE as usize * MAX_CURSOR_EDGE as usize * 4;

/// A compositor-owned semantic cursor role.
///
/// Policy chooses a role. The trusted session resolves it to pixels before a
/// renderer or KMS backend sees it, so neither backend acquires styling policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CursorShape {
    #[default]
    LeftPtr,
    Text,
    Pointer,
    Move,
    Wait,
    Crosshair,
    ResizeHorizontal,
    ResizeVertical,
    ResizeNorthWestSouthEast,
    ResizeNorthEastSouthWest,
}

impl CursorShape {
    pub const fn name(self) -> &'static str {
        match self {
            Self::LeftPtr => "left_ptr",
            Self::Text => "text",
            Self::Pointer => "pointer",
            Self::Move => "move",
            Self::Wait => "wait",
            Self::Crosshair => "crosshair",
            Self::ResizeHorizontal => "ew-resize",
            Self::ResizeVertical => "ns-resize",
            Self::ResizeNorthWestSouthEast => "nwse-resize",
            Self::ResizeNorthEastSouthWest => "nesw-resize",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "left_ptr" | "default" => Some(Self::LeftPtr),
            "text" | "xterm" => Some(Self::Text),
            "pointer" | "hand2" => Some(Self::Pointer),
            "move" | "fleur" => Some(Self::Move),
            "wait" | "watch" => Some(Self::Wait),
            "crosshair" => Some(Self::Crosshair),
            "ew-resize" | "sb_h_double_arrow" => Some(Self::ResizeHorizontal),
            "ns-resize" | "sb_v_double_arrow" => Some(Self::ResizeVertical),
            "nwse-resize" => Some(Self::ResizeNorthWestSouthEast),
            "nesw-resize" => Some(Self::ResizeNorthEastSouthWest),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct CursorAssetDigest([u8; 32]);

impl CursorAssetDigest {
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for CursorAssetDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "CursorAssetDigest({self})")
    }
}

impl fmt::Display for CursorAssetDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CursorAssetError {
    InvalidDimensions,
    InvalidHotspot,
    InvalidPixelLength { expected: usize, actual: usize },
    NonPremultipliedPixel { index: usize },
}

impl fmt::Display for CursorAssetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions => formatter.write_str("cursor dimensions must be 1..=128"),
            Self::InvalidHotspot => formatter.write_str("cursor hotspot is outside its raster"),
            Self::InvalidPixelLength { expected, actual } => write!(
                formatter,
                "cursor pixels have length {actual}; expected {expected}"
            ),
            Self::NonPremultipliedPixel { index } => {
                write!(formatter, "cursor pixel {index} is not premultiplied")
            }
        }
    }
}

impl std::error::Error for CursorAssetError {}

/// Immutable premultiplied ARGB8888 cursor pixels.
///
/// Bytes use the little-endian DRM memory order `[blue, green, red, alpha]`.
/// The digest binds dimensions and hotspot as well as pixels, because two
/// identical rasters with different hotspots are different cursor assets.
#[derive(Clone, Eq, PartialEq)]
pub struct CursorAsset {
    width: u32,
    height: u32,
    hotspot_x: u32,
    hotspot_y: u32,
    generation: u64,
    digest: CursorAssetDigest,
    pixels: Arc<[u8]>,
}

impl fmt::Debug for CursorAsset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CursorAsset")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("hotspot_x", &self.hotspot_x)
            .field("hotspot_y", &self.hotspot_y)
            .field("generation", &self.generation)
            .field("digest", &self.digest)
            .finish_non_exhaustive()
    }
}

impl CursorAsset {
    pub fn new(
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
        generation: u64,
        pixels: Vec<u8>,
    ) -> Result<Self, CursorAssetError> {
        if width == 0 || height == 0 || width > MAX_CURSOR_EDGE || height > MAX_CURSOR_EDGE {
            return Err(CursorAssetError::InvalidDimensions);
        }
        if hotspot_x >= width || hotspot_y >= height {
            return Err(CursorAssetError::InvalidHotspot);
        }
        let expected = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(4))
            .filter(|length| *length <= MAX_CURSOR_PIXEL_BYTES)
            .ok_or(CursorAssetError::InvalidDimensions)?;
        if pixels.len() != expected {
            return Err(CursorAssetError::InvalidPixelLength {
                expected,
                actual: pixels.len(),
            });
        }
        if let Some((index, _)) = pixels
            .chunks_exact(4)
            .enumerate()
            .find(|(_, pixel)| pixel[..3].iter().any(|channel| *channel > pixel[3]))
        {
            return Err(CursorAssetError::NonPremultipliedPixel { index });
        }
        let mut digest = Sha256::new();
        digest.update(width.to_le_bytes());
        digest.update(height.to_le_bytes());
        digest.update(hotspot_x.to_le_bytes());
        digest.update(hotspot_y.to_le_bytes());
        digest.update(&pixels);
        Ok(Self {
            width,
            height,
            hotspot_x,
            hotspot_y,
            generation,
            digest: CursorAssetDigest(digest.finalize().into()),
            pixels: pixels.into(),
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn hotspot(&self) -> (u32, u32) {
        (self.hotspot_x, self.hotspot_y)
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn digest(&self) -> CursorAssetDigest {
        self.digest
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

/// The canonical X11 cursor-font `left_ptr` glyph and mask.
///
/// Source: xorg/font/cursor-misc `cursor.bdf`, glyphs 68 and 69. That font is
/// public domain. The 10x16 mask has a (1, 1) hotspot; source bits are black
/// and mask-only bits are white, matching `XCreateFontCursor(XC_left_ptr)`.
pub fn x11_core_left_ptr_cursor(generation: u64) -> CursorAsset {
    const SOURCE: [u8; 14] = [
        0x80, 0xc0, 0xe0, 0xf0, 0xf8, 0xfc, 0xfe, 0xff, 0xf8, 0xd8, 0x8c, 0x0c, 0x06, 0x06,
    ];
    const MASK: [u16; 16] = [
        0xc003, 0xe000, 0xf000, 0xf800, 0xfc00, 0xfe00, 0xff00, 0xff80, 0xffc0, 0xffc0, 0xfe00,
        0xef00, 0xcf00, 0x0780, 0x0780, 0x0300,
    ];
    let mut pixels = vec![0; 10 * 16 * 4];
    for (y, mask) in MASK.into_iter().enumerate() {
        for x in 0..10 {
            let masked = mask & (1 << (15 - x)) != 0;
            if !masked {
                continue;
            }
            let source = y
                .checked_sub(1)
                .and_then(|row| SOURCE.get(row))
                .is_some_and(|source| (1..=8).contains(&x) && source & (1 << (8 - x)) != 0);
            let offset = (y * 10 + x) * 4;
            pixels[offset..offset + 4].copy_from_slice(if source {
                &[0, 0, 0, 0xff]
            } else {
                &[0xff, 0xff, 0xff, 0xff]
            });
        }
    }
    CursorAsset::new(10, 16, 1, 1, generation, pixels)
        .expect("the embedded X11 left_ptr asset is valid")
}
