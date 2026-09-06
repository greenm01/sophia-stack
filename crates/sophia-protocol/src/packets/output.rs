use crate::{OutputId, Rect, Size, SurfaceId};
use std::collections::BTreeSet;

pub const MAX_OUTPUT_TOPOLOGY_ENTRIES: usize = 16;
pub const MAX_SURFACE_OUTPUT_RESERVATIONS: usize = 4;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OutputEdge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AxisSpan {
    pub start: i32,
    pub end: i32,
}

impl AxisSpan {
    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputReservation {
    pub edge: OutputEdge,
    pub depth: i32,
    pub span: AxisSpan,
}

impl OutputReservation {
    pub const fn is_valid(self) -> bool {
        self.depth > 0 && !self.span.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceOutputReservations {
    pub surface: SurfaceId,
    pub reservations: Vec<OutputReservation>,
}

impl SurfaceOutputReservations {
    pub fn is_valid(&self) -> bool {
        self.surface.is_valid()
            && self.reservations.len() <= MAX_SURFACE_OUTPUT_RESERVATIONS
            && self
                .reservations
                .iter()
                .all(|reservation| reservation.is_valid())
            && self
                .reservations
                .iter()
                .map(|reservation| reservation.edge)
                .collect::<BTreeSet<_>>()
                .len()
                == self.reservations.len()
    }
}

/// The scanout timing a display is actually running.
///
/// Kept beside `refresh_millihz` rather than replacing it, because the two
/// answer different questions and confusing them costs a working desktop.
/// `refresh_millihz` is the **nominal** rate: DRM's integer `vrefresh` scaled
/// by a thousand, which is what a person writes in a profile as `@120` and
/// what the mode matcher compares with exact equality. These fields are the
/// **measured** rate: a 120 Hz panel usually runs at 119.9976 Hz, and
/// `clock / (htotal * vtotal)` says so exactly.
///
/// The exactness is the point rather than a detail. `glXGetMscRateOML` hands
/// a client a numerator and a denominator so a rate that is not a whole number
/// survives as a fraction, and Chromium predicts vsync from it. Answering that
/// from a rounded integer would defeat the interface it is answering.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OutputModeTiming {
    /// Pixel clock in kHz, as DRM reports it.
    pub clock_khz: u32,
    pub hdisplay: u16,
    pub hsync_start: u16,
    pub hsync_end: u16,
    pub htotal: u16,
    pub hskew: u16,
    pub vdisplay: u16,
    pub vsync_start: u16,
    pub vsync_end: u16,
    pub vtotal: u16,
    /// DRM mode flags, carried through unmodified.
    pub flags: u32,
}

impl OutputModeTiming {
    /// Whether the timing describes a mode that could be scanned out.
    ///
    /// Totals bound their own active regions and the clock is non-zero, which
    /// is what makes the refresh division below meaningful rather than a
    /// division by chance.
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.clock_khz > 0
            && self.htotal >= self.hdisplay
            && self.vtotal >= self.vdisplay
            && self.hdisplay > 0
            && self.vdisplay > 0
            && self.htotal > 0
            && self.vtotal > 0
    }

    /// The measured refresh in millihertz, or `None` when the timing cannot
    /// describe one.
    #[must_use]
    pub const fn measured_refresh_millihz(self) -> Option<u32> {
        if !self.is_valid() {
            return None;
        }
        // clock is kHz, so the numerator is clock * 1_000_000 millihertz
        // before dividing by the total pixels in a frame.
        let numerator = self.clock_khz as u64 * 1_000_000;
        let denominator = self.htotal as u64 * self.vtotal as u64;
        let millihz = numerator / denominator;
        if millihz > u32::MAX as u64 {
            return None;
        }
        Some(millihz as u32)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputTopologyEntry {
    pub output: OutputId,
    pub logical: Rect,
    pub pixel_size: Size,
    pub scale: u32,
    /// The nominal rate, which the mode matcher compares. See
    /// `OutputModeTiming` for why this is not the measured one.
    pub refresh_millihz: u32,
    /// The measured scanout timing, when the output has one.
    ///
    /// `None` for a headless or synthetic output, which genuinely has no
    /// timing -- a state worth distinguishing from a zero that would read as
    /// measured.
    pub timing: Option<OutputModeTiming>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputTopologySnapshot {
    pub generation: u64,
    pub primary: OutputId,
    pub outputs: Vec<OutputTopologyEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputTopologyError {
    InvalidGeneration,
    InvalidPrimary,
    Empty,
    CapacityExceeded,
    InvalidOutput,
    DuplicateOutput,
    InvalidGeometry,
    InvalidMode,
    RootSizeExceeded,
}

impl OutputTopologySnapshot {
    pub fn validate(&self) -> Result<Size, OutputTopologyError> {
        if self.generation == 0 {
            return Err(OutputTopologyError::InvalidGeneration);
        }
        if self.outputs.is_empty() {
            return Err(OutputTopologyError::Empty);
        }
        if self.outputs.len() > MAX_OUTPUT_TOPOLOGY_ENTRIES {
            return Err(OutputTopologyError::CapacityExceeded);
        }

        let mut ids = BTreeSet::new();
        let mut right = 0i32;
        let mut bottom = 0i32;
        for entry in &self.outputs {
            if !entry.output.is_valid() {
                return Err(OutputTopologyError::InvalidOutput);
            }
            if !ids.insert(entry.output) {
                return Err(OutputTopologyError::DuplicateOutput);
            }
            if entry.logical.is_empty() || entry.logical.x < 0 || entry.logical.y < 0 {
                return Err(OutputTopologyError::InvalidGeometry);
            }
            if entry.pixel_size.width <= 0
                || entry.pixel_size.height <= 0
                || entry.scale == 0
                || entry.refresh_millihz == 0
            {
                return Err(OutputTopologyError::InvalidMode);
            }
            let entry_right = entry
                .logical
                .x
                .checked_add(entry.logical.width)
                .ok_or(OutputTopologyError::RootSizeExceeded)?;
            let entry_bottom = entry
                .logical
                .y
                .checked_add(entry.logical.height)
                .ok_or(OutputTopologyError::RootSizeExceeded)?;
            right = right.max(entry_right);
            bottom = bottom.max(entry_bottom);
        }
        if !ids.contains(&self.primary) {
            return Err(OutputTopologyError::InvalidPrimary);
        }
        if right <= 0 || bottom <= 0 || right > i32::from(u16::MAX) || bottom > i32::from(u16::MAX)
        {
            return Err(OutputTopologyError::RootSizeExceeded);
        }
        Ok(Size {
            width: right,
            height: bottom,
        })
    }

    pub fn root_size(&self) -> Result<Size, OutputTopologyError> {
        self.validate()
    }

    pub fn deterministic() -> Self {
        Self {
            generation: 1,
            primary: OutputId::from_raw(1),
            outputs: vec![OutputTopologyEntry {
                output: OutputId::from_raw(1),
                logical: Rect {
                    x: 0,
                    y: 0,
                    width: 1280,
                    height: 720,
                },
                pixel_size: Size {
                    width: 1280,
                    height: 720,
                },
                scale: 1,
                refresh_millihz: 60_000,
                timing: None,
            }],
        }
    }
}
