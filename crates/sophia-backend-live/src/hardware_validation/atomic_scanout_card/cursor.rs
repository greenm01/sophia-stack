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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyHardwareCursorController<Crtc> {
    initialized: bool,
    active_crtcs: Vec<Crtc>,
}

impl<Crtc: Copy + Debug + Eq> Default for LegacyHardwareCursorController<Crtc> {
    fn default() -> Self {
        Self {
            initialized: false,
            active_crtcs: Vec::new(),
        }
    }
}

impl<Crtc: Copy + Debug + Eq> LegacyHardwareCursorController<Crtc> {
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn active_crtc(&self) -> Option<Crtc> {
        self.active_crtcs.first().copied()
    }

    pub fn active_crtcs(&self) -> &[Crtc] {
        &self.active_crtcs
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
        self.active_crtcs.clear();
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
        self.update_many(device, target.as_slice())
    }

    /// Shows the same logical cursor on every physical CRTC in a mirror group.
    pub fn update_many<D>(
        &mut self,
        device: &mut D,
        targets: &[LegacyHardwareCursorTarget<Crtc>],
    ) -> io::Result<ClassicHardwareCursorUpdate>
    where
        D: LegacyHardwareCursorDevice<Crtc = Crtc>,
    {
        if !self.initialized {
            return Err(io::Error::other(
                "legacy hardware cursor was updated before initialization",
            ));
        }
        for previous in self.active_crtcs.iter().copied().collect::<Vec<_>>() {
            if !targets.iter().any(|target| target.crtc == previous) {
                device.hide_cursor(previous)?;
                self.active_crtcs.retain(|active| *active != previous);
            }
        }
        let mut seen = Vec::with_capacity(targets.len());
        for target in targets.iter().copied() {
            if seen.contains(&target.crtc) {
                continue;
            }
            if !self.active_crtcs.contains(&target.crtc) {
                device.install_cursor(target.crtc)?;
                self.active_crtcs.push(target.crtc);
            }
            device.move_cursor(target.crtc, target.x, target.y)?;
            seen.push(target.crtc);
        }
        self.active_crtcs
            .sort_by_key(|active| seen.iter().position(|target| target == active));
        Ok(if self.active_crtcs.is_empty() {
            ClassicHardwareCursorUpdate::Hidden
        } else {
            ClassicHardwareCursorUpdate::Visible
        })
    }

    pub fn hide_for_teardown<D>(&mut self, device: &mut D) -> io::Result<()>
    where
        D: LegacyHardwareCursorDevice<Crtc = Crtc>,
    {
        for crtc in self.active_crtcs.drain(..) {
            device.hide_cursor(crtc)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassicHardwareCursorUpdate {
    Visible,
    Hidden,
    /// The newest position is owned by the atomic transaction queue.
    ///
    /// This is acceptance, not presentation. The backend will either carry
    /// it on the next primary commit or issue a cursor-only commit as soon as
    /// the CRTC retires. Callers must not keep resubmitting the same motion.
    Queued,
    Deferred,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyHardwareCursorAdmission {
    InitializeThenUpdate,
    Update,
    DeferredInitialization,
    /// The cursor has moved, the plane is initialized, and the CRTC is busy.
    ///
    /// Only the atomic path can answer this. An ioctl moves a cursor whenever
    /// it likes -- direct-scanout archive `0004` counted fifteen updates
    /// issued while a page flip was outstanding -- but the kernel serializes
    /// atomic commits per CRTC, so the same move must wait for the commit in
    /// flight and go out when it frees.
    ///
    /// Waiting is not dropping. The position supersedes any other still
    /// pending, and `CursorPlaneTransactionOwner.tla` requires it to reach a
    /// plane: a cursor left waiting for the next client frame freezes on an
    /// idle desktop, which is why a cursor-only commit exists at all.
    DeferredUpdate,
}

/// Which cursor path the session is driving.
///
/// The two answer differently in exactly one case, and keeping them in one
/// truth table is what makes that visible. The legacy path is what archive
/// `0004` proved and stays byte-for-byte as it was.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HardwareCursorPath {
    /// `drmModeSetCursor`/`drmModeMoveCursor`, outside the atomic queue.
    LegacyIoctl,
    /// A cursor plane in the per-output atomic request.
    AtomicPlane,
}

/// Whether a cursor update may proceed, wait, or must initialize first.
///
/// The legacy row that says "update anyway while a flip is in flight" is the
/// one an atomic commit cannot have; everything else the two paths share.
pub fn hardware_cursor_admission(
    path: HardwareCursorPath,
    initialized: bool,
    primary_in_flight: bool,
) -> LegacyHardwareCursorAdmission {
    match (path, initialized, primary_in_flight) {
        // Initialization touches every CRTC, which is not something to do
        // underneath an in-flight flip on either path.
        (_, false, true) => LegacyHardwareCursorAdmission::DeferredInitialization,
        (_, false, false) => LegacyHardwareCursorAdmission::InitializeThenUpdate,
        (HardwareCursorPath::AtomicPlane, true, true) => {
            LegacyHardwareCursorAdmission::DeferredUpdate
        }
        (_, true, _) => LegacyHardwareCursorAdmission::Update,
    }
}

/// The legacy path's admission, unchanged.
pub fn legacy_hardware_cursor_admission(
    initialized: bool,
    primary_in_flight: bool,
) -> LegacyHardwareCursorAdmission {
    hardware_cursor_admission(
        HardwareCursorPath::LegacyIoctl,
        initialized,
        primary_in_flight,
    )
}

/// What a startup probe of the cursor plane concluded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CursorPlaneProbe {
    /// The driver accepted a test commit carrying the cursor plane.
    Accepted,
    /// It refused, or the card has no cursor plane to offer.
    Refused,
}

/// Which cursor path a session should drive, given what the card offered.
///
/// Probed once rather than per frame. The compositor owns the cursor buffer,
/// so its format, size, and modifier are fixed after the first success --
/// unlike direct scanout, where the *client* owns the buffer and every
/// eligibility edge needs its own test. Position is the only thing that
/// changes afterwards, and position is what a cursor plane is for.
///
/// A refusal is not a failure. The legacy ioctl is a working path -- archive
/// `0004` moved a cursor over directly scanned frames on it with no failures
/// -- so a card that will not take a cursor plane keeps it.
pub const fn cursor_path_for_probe(probe: CursorPlaneProbe) -> HardwareCursorPath {
    match probe {
        CursorPlaneProbe::Accepted => HardwareCursorPath::AtomicPlane,
        CursorPlaneProbe::Refused => HardwareCursorPath::LegacyIoctl,
    }
}
