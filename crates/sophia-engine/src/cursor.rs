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

/// How far the pointer must travel before a movement counts as deliberate.
pub const CURSOR_SHAKE_MIN_DELTA: i32 = 16;
/// How close together two reversals must be to belong to the same shake.
pub const CURSOR_SHAKE_MAX_GAP_MSEC: u64 = 180;
/// How long the whole gesture may take.
pub const CURSOR_SHAKE_WINDOW_MSEC: u64 = 650;
/// How long the enlarged cursor lingers after the pointer stops.
pub const CURSOR_SHAKE_RESTORE_DELAY_MSEC: u64 = 700;
/// How many direction changes make a shake.
pub const CURSOR_SHAKE_TRIGGER_REVERSALS: u32 = 3;

/// What a shake asks the session to do about the cursor.
///
/// An action rather than a size, because the Engine owns the cursor and the
/// detector is only reading the pointer. It says a cursor should be easier to
/// find; it does not resolve a theme or raster anything.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CursorShakeAction {
    Enlarge,
    Restore,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum CursorShakeAxis {
    #[default]
    Undecided,
    Horizontal,
    Vertical,
}

/// Recognises the gesture of shaking the pointer to find it.
///
/// A reversal detector rather than a speed threshold: what distinguishes
/// looking for the cursor from moving it somewhere is changing direction
/// repeatedly, and a threshold on speed alone fires when someone throws the
/// pointer across three monitors on purpose.
///
/// The detector holds no clock. Every decision is a function of the timestamps
/// it is handed, which is what makes the gesture testable without one.
#[derive(Clone, Copy, Debug, Default)]
pub struct CursorShakeDetector {
    last: Option<(i32, i32, u64)>,
    gesture_started_msec: Option<u64>,
    axis: CursorShakeAxis,
    sign: i32,
    reversals: u32,
    enlarged: bool,
    restore_due_msec: u64,
}

impl CursorShakeDetector {
    /// The size an enlarged cursor is drawn at.
    ///
    /// Doubling is enough to catch the eye on a small cursor but not on a
    /// large one, so a flat addition takes over where doubling stops helping.
    /// The ceiling is what the Engine will raster, and a base already at the
    /// ceiling simply does not grow -- the caller sees no size change and can
    /// decline the gesture rather than paint the same cursor twice.
    #[must_use]
    pub const fn enlarged_size(base: u32) -> u32 {
        let doubled = base.saturating_mul(2);
        let stepped = base.saturating_add(24);
        let wanted = if doubled > stepped { doubled } else { stepped };
        if wanted > MAX_CURSOR_EDGE {
            MAX_CURSOR_EDGE
        } else {
            wanted
        }
    }

    #[must_use]
    pub const fn is_enlarged(&self) -> bool {
        self.enlarged
    }

    fn clear_gesture(&mut self) {
        self.gesture_started_msec = None;
        self.axis = CursorShakeAxis::Undecided;
        self.sign = 0;
        self.reversals = 0;
    }

    /// Feeds one pointer position to the detector.
    ///
    /// `enabled` is passed per call rather than stored so that turning the
    /// feature off restores a cursor that is currently enlarged, instead of
    /// leaving it big until something else happens to move.
    pub fn observe_motion(
        &mut self,
        enabled: bool,
        x: i32,
        y: i32,
        now_msec: u64,
    ) -> Option<CursorShakeAction> {
        if !enabled {
            let restore = self.enlarged.then_some(CursorShakeAction::Restore);
            self.enlarged = false;
            self.last = None;
            self.clear_gesture();
            return restore;
        }
        if self.enlarged {
            // Still moving, so the cursor is still being looked for. The
            // countdown starts when the pointer stops, not when it grew.
            self.restore_due_msec = now_msec.saturating_add(CURSOR_SHAKE_RESTORE_DELAY_MSEC);
        }
        let Some((last_x, last_y, last_msec)) = self.last else {
            self.last = Some((x, y, now_msec));
            return None;
        };

        let (dx, dy) = (x - last_x, y - last_y);
        if dx.abs().max(dy.abs()) < CURSOR_SHAKE_MIN_DELTA {
            return None;
        }
        let (axis, sign) = if dx.abs() >= dy.abs() {
            (CursorShakeAxis::Horizontal, dx.signum())
        } else {
            (CursorShakeAxis::Vertical, dy.signum())
        };
        let gap = now_msec.saturating_sub(last_msec);
        self.last = Some((x, y, now_msec));

        // A different axis, or too long a pause, is a new gesture rather than
        // a continuation of this one.
        if gap > CURSOR_SHAKE_MAX_GAP_MSEC || self.axis != axis {
            self.clear_gesture();
            self.axis = axis;
            self.sign = sign;
            self.gesture_started_msec = Some(now_msec);
            return None;
        }
        if self.sign == sign {
            return None;
        }

        match self.gesture_started_msec {
            Some(started) if now_msec.saturating_sub(started) <= CURSOR_SHAKE_WINDOW_MSEC => {
                self.reversals += 1;
            }
            // Reversing after the window closed starts counting again from
            // this reversal, which is what keeps a slow waggle from ever
            // accumulating three.
            _ => {
                self.gesture_started_msec = Some(now_msec);
                self.reversals = 1;
            }
        }
        self.sign = sign;

        if self.reversals < CURSOR_SHAKE_TRIGGER_REVERSALS {
            return None;
        }
        self.restore_due_msec = now_msec.saturating_add(CURSOR_SHAKE_RESTORE_DELAY_MSEC);
        self.clear_gesture();
        if self.enlarged {
            return None;
        }
        self.enlarged = true;
        Some(CursorShakeAction::Enlarge)
    }

    /// Asks whether an enlarged cursor has lingered long enough.
    ///
    /// Separate from motion because the restore is what happens when the
    /// pointer stops, and a detector that only ran on movement could never
    /// observe stopping.
    pub fn tick(&mut self, enabled: bool, now_msec: u64) -> Option<CursorShakeAction> {
        if !self.enlarged {
            return None;
        }
        if enabled && now_msec < self.restore_due_msec {
            return None;
        }
        self.enlarged = false;
        self.clear_gesture();
        Some(CursorShakeAction::Restore)
    }
}
