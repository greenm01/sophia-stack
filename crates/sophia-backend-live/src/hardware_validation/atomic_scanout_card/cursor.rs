use std::{fmt::Debug, io};

pub const LEGACY_HARDWARE_CURSOR_FALLBACK_EDGE: u32 = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyHardwareCursorDimensions {
    pub width: u32,
    pub height: u32,
}

pub fn resolve_legacy_hardware_cursor_dimensions(
    driver_width: Option<u64>,
    driver_height: Option<u64>,
) -> LegacyHardwareCursorDimensions {
    let valid_dimension = |value: Option<u64>| {
        value
            .filter(|value| *value != 0)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(LEGACY_HARDWARE_CURSOR_FALLBACK_EDGE)
    };
    LegacyHardwareCursorDimensions {
        width: valid_dimension(driver_width),
        height: valid_dimension(driver_height),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyHardwareCursorTarget<Crtc> {
    pub crtc: Crtc,
    pub x: i32,
    pub y: i32,
}

/// The narrow legacy KMS cursor surface used by the state controller.
///
/// Keeping atomic commit methods out of this interface is intentional. A
/// standalone cursor-plane atomic commit competes with primary page flips on
/// the same DRM device. A future all-atomic implementation must instead give a
/// single per-output transaction owner both primary and cursor plane state,
/// including a scheduled cursor-only transaction when primary content is idle.
pub trait LegacyHardwareCursorDevice {
    type Crtc: Copy + Debug + Eq;

    fn hide_cursor(&mut self, crtc: Self::Crtc) -> io::Result<()>;
    fn install_cursor(&mut self, crtc: Self::Crtc) -> io::Result<()>;
    fn move_cursor(&mut self, crtc: Self::Crtc, x: i32, y: i32) -> io::Result<()>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyHardwareCursorController<Crtc> {
    initialized: bool,
    active_crtc: Option<Crtc>,
}

impl<Crtc: Copy + Debug + Eq> Default for LegacyHardwareCursorController<Crtc> {
    fn default() -> Self {
        Self {
            initialized: false,
            active_crtc: None,
        }
    }
}

impl<Crtc: Copy + Debug + Eq> LegacyHardwareCursorController<Crtc> {
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn active_crtc(&self) -> Option<Crtc> {
        self.active_crtc
    }

    pub fn initialize<D>(&mut self, device: &mut D, crtcs: &[Crtc]) -> io::Result<()>
    where
        D: LegacyHardwareCursorDevice<Crtc = Crtc>,
    {
        if self.initialized {
            return Ok(());
        }
        for crtc in crtcs.iter().copied() {
            device.hide_cursor(crtc)?;
        }
        self.active_crtc = None;
        self.initialized = true;
        Ok(())
    }

    pub fn update<D>(
        &mut self,
        device: &mut D,
        target: Option<LegacyHardwareCursorTarget<Crtc>>,
    ) -> io::Result<ClassicHardwareCursorUpdate>
    where
        D: LegacyHardwareCursorDevice<Crtc = Crtc>,
    {
        if !self.initialized {
            return Err(io::Error::other(
                "legacy hardware cursor was updated before initialization",
            ));
        }
        let Some(target) = target else {
            let Some(previous) = self.active_crtc else {
                return Ok(ClassicHardwareCursorUpdate::Hidden);
            };
            device.hide_cursor(previous)?;
            self.active_crtc = None;
            return Ok(ClassicHardwareCursorUpdate::Hidden);
        };

        if self.active_crtc != Some(target.crtc) {
            if let Some(previous) = self.active_crtc {
                device.hide_cursor(previous)?;
                self.active_crtc = None;
            }
            device.install_cursor(target.crtc)?;
            self.active_crtc = Some(target.crtc);
        }
        device.move_cursor(target.crtc, target.x, target.y)?;
        Ok(ClassicHardwareCursorUpdate::Visible)
    }

    pub fn hide_for_teardown<D>(&mut self, device: &mut D) -> io::Result<()>
    where
        D: LegacyHardwareCursorDevice<Crtc = Crtc>,
    {
        let Some(crtc) = self.active_crtc else {
            return Ok(());
        };
        device.hide_cursor(crtc)?;
        self.active_crtc = None;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassicHardwareCursorUpdate {
    Visible,
    Hidden,
    Deferred,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyHardwareCursorAdmission {
    InitializeThenUpdate,
    Update,
    DeferredInitialization,
}

pub fn legacy_hardware_cursor_admission(
    initialized: bool,
    primary_in_flight: bool,
) -> LegacyHardwareCursorAdmission {
    match (initialized, primary_in_flight) {
        (false, true) => LegacyHardwareCursorAdmission::DeferredInitialization,
        (false, false) => LegacyHardwareCursorAdmission::InitializeThenUpdate,
        (true, _) => LegacyHardwareCursorAdmission::Update,
    }
}
