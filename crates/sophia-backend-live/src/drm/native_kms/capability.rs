use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LibdrmNativeOutputTiming {
    pub width: u32,
    pub height: u32,
    pub refresh_millihz: u32,
}

impl LibdrmNativeOutputTiming {
    pub const fn new(width: u32, height: u32, refresh_millihz: u32) -> Self {
        Self {
            width,
            height,
            refresh_millihz,
        }
    }

    pub const fn valid(self) -> bool {
        self.width > 0 && self.height > 0 && self.refresh_millihz > 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeModeResolutionStatus {
    Resolved,
    /// The requested timing is not one this connector advertises. A planned
    /// candidate that cannot name a real mode must not reach a commit.
    UnknownTiming,
    /// The request itself carried a zero dimension or refresh.
    InvalidTiming,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeModeResolution {
    pub status: LibdrmNativeModeResolutionStatus,
    /// Index into the connector's mode list, in the order the kernel reported it.
    pub index: Option<usize>,
}

impl LibdrmNativeModeResolution {
    const fn rejected(status: LibdrmNativeModeResolutionStatus) -> Self {
        Self {
            status,
            index: None,
        }
    }
}

/// Resolves a planned timing to a position in a connector's reported mode list.
///
/// A configured candidate names a timing, but a KMS commit needs the mode object
/// that produced it, and the reduction from mode to timing is lossy: several modes
/// can share one width, height, and integer refresh. This returns the **first**
/// match, which is the same choice the capability reader makes when it dedupes
/// reduced timings and keeps the first occurrence. So any timing a capability
/// advertises resolves back to exactly the mode that advertised it.
///
/// Invalid modes are skipped rather than matched, for the same reason the
/// capability reader skips them: a zero dimension or refresh cannot drive a head.
pub fn resolve_native_output_mode_index(
    modes: &[LibdrmNativeOutputTiming],
    requested: LibdrmNativeOutputTiming,
) -> LibdrmNativeModeResolution {
    if !requested.valid() {
        return LibdrmNativeModeResolution::rejected(
            LibdrmNativeModeResolutionStatus::InvalidTiming,
        );
    }
    match modes
        .iter()
        .position(|mode| mode.valid() && *mode == requested)
    {
        Some(index) => LibdrmNativeModeResolution {
            status: LibdrmNativeModeResolutionStatus::Resolved,
            index: Some(index),
        },
        None => {
            LibdrmNativeModeResolution::rejected(LibdrmNativeModeResolutionStatus::UnknownTiming)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibdrmNativeOutputCapability {
    head: sophia_engine::RenderHeadId,
    output: OutputId,
    connector_id: u32,
    connector_name: String,
    modes: Vec<LibdrmNativeOutputTiming>,
    preferred_mode: Option<LibdrmNativeOutputTiming>,
    selected_mode: LibdrmNativeOutputTiming,
    vrr_status: LibdrmNativeVrrPropertyDiscoveryStatus,
}

impl LibdrmNativeOutputCapability {
    pub fn new(
        output: OutputId,
        connector_id: u32,
        connector_name: impl Into<String>,
        modes: impl IntoIterator<Item = LibdrmNativeOutputTiming>,
        preferred_mode: Option<LibdrmNativeOutputTiming>,
        selected_mode: LibdrmNativeOutputTiming,
        vrr_status: LibdrmNativeVrrPropertyDiscoveryStatus,
    ) -> io::Result<Self> {
        let capability = Self {
            head: sophia_engine::RenderHeadId::INVALID,
            output,
            connector_id,
            connector_name: connector_name.into(),
            modes: modes.into_iter().collect(),
            preferred_mode,
            selected_mode,
            vrr_status,
        };
        if capability.connector_name.is_empty()
            || capability.connector_name.len() > 64
            || !capability
                .connector_name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(io::Error::other("DRM connector identity is invalid"));
        }
        if capability.modes.is_empty()
            || capability.modes.len() > 256
            || capability.modes.iter().any(|mode| !mode.valid())
            || capability
                .preferred_mode
                .is_some_and(|mode| !capability.modes.contains(&mode))
            || !capability.modes.contains(&capability.selected_mode)
        {
            return Err(io::Error::other(
                "DRM connector has inconsistent mode capabilities",
            ));
        }
        Ok(capability)
    }

    pub const fn output(&self) -> OutputId {
        self.output
    }

    /// Opaque Engine/backend identity when this capability came from an active
    /// production head. Standalone discovery and test fixtures may not yet have
    /// admitted a head, in which case this is `None`.
    pub const fn head(&self) -> Option<sophia_engine::RenderHeadId> {
        if self.head.is_valid() {
            Some(self.head)
        } else {
            None
        }
    }

    pub fn bind_head(mut self, head: sophia_engine::RenderHeadId) -> io::Result<Self> {
        if !head.is_valid() {
            return Err(io::Error::other(
                "DRM capability cannot bind an invalid head",
            ));
        }
        self.head = head;
        Ok(self)
    }

    pub const fn connector_id(&self) -> u32 {
        self.connector_id
    }

    pub fn connector_name(&self) -> &str {
        &self.connector_name
    }

    pub fn modes(&self) -> &[LibdrmNativeOutputTiming] {
        &self.modes
    }

    pub const fn preferred_mode(&self) -> Option<LibdrmNativeOutputTiming> {
        self.preferred_mode
    }

    pub const fn selected_mode(&self) -> LibdrmNativeOutputTiming {
        self.selected_mode
    }

    pub const fn vrr_status(&self) -> LibdrmNativeVrrPropertyDiscoveryStatus {
        self.vrr_status
    }

    pub const fn vrr_configurable(&self) -> bool {
        matches!(
            self.vrr_status,
            LibdrmNativeVrrPropertyDiscoveryStatus::Discovered
        )
    }
}

pub(crate) fn read_native_output_capability<D>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelection,
    output: OutputId,
) -> io::Result<LibdrmNativeOutputCapability>
where
    D: drm::control::Device + LibdrmNativePropertyLookupDevice,
{
    let connector = device.get_connector(selection.connector, false)?;
    let connector_name = connector.to_string();
    let mut modes = Vec::new();
    let mut preferred_mode = None;
    for mode in connector.modes().iter().copied() {
        let timing = native_output_timing(mode);
        if !timing.valid() {
            continue;
        }
        if preferred_mode.is_none()
            && mode
                .mode_type()
                .contains(drm::control::ModeTypeFlags::PREFERRED)
        {
            preferred_mode = Some(timing);
        }
        if !modes.contains(&timing) {
            modes.push(timing);
        }
    }
    let selected_mode = selection
        .mode
        .map(native_output_timing)
        .filter(|mode| mode.valid())
        .ok_or_else(|| io::Error::other("selected DRM connector has no usable mode"))?;
    let preferred_mode = preferred_mode.or(Some(selected_mode));
    let vrr_status =
        discover_native_vrr_properties(device, selection.connector, selection.crtc).status;
    LibdrmNativeOutputCapability::new(
        output,
        selection.connector_id(),
        connector_name,
        modes,
        preferred_mode,
        selected_mode,
        vrr_status,
    )
}

/// Reads a connector's modes and returns the one matching a planned timing.
///
/// This is the bridge from a configured candidate to a KMS mode object. It reduces
/// each reported mode exactly as `read_native_output_capability` does and defers
/// the choice to `resolve_native_output_mode_index`, so capability advertisement
/// and commit selection cannot disagree. Returning `None` is a fail-closed
/// outcome, not an error: the connector simply does not offer that timing.
#[cfg(feature = "libdrm-events")]
pub fn resolve_native_connector_mode<D>(
    device: &D,
    connector: drm::control::connector::Handle,
    requested: LibdrmNativeOutputTiming,
) -> io::Result<Option<drm::control::Mode>>
where
    D: drm::control::Device,
{
    let reported = device.get_connector(connector, false)?;
    let modes = reported.modes();
    let reduced = modes
        .iter()
        .copied()
        .map(native_output_timing)
        .collect::<Vec<_>>();
    let resolution = resolve_native_output_mode_index(&reduced, requested);
    Ok(resolution.index.map(|index| modes[index]))
}

fn native_output_timing(mode: drm::control::Mode) -> LibdrmNativeOutputTiming {
    let (width, height) = mode.size();
    LibdrmNativeOutputTiming {
        width: u32::from(width),
        height: u32::from(height),
        refresh_millihz: mode.vrefresh().saturating_mul(1_000),
    }
}
