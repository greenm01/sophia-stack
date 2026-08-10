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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibdrmNativeOutputCapability {
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

fn native_output_timing(mode: drm::control::Mode) -> LibdrmNativeOutputTiming {
    let (width, height) = mode.size();
    LibdrmNativeOutputTiming {
        width: u32::from(width),
        height: u32::from(height),
        refresh_millihz: mode.vrefresh().saturating_mul(1_000),
    }
}
