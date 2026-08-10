use std::collections::BTreeSet;
use std::fmt;

use crate::{
    ConfigDigest, ConfigGeneration, DESKTOP_OUTPUT_MAX_NAMED, DesktopOutputCandidate,
    DesktopOutputMode, DesktopOutputScale, DesktopOutputTransform, DesktopOutputVrrMode,
    valid_desktop_output_connector,
};

const OUTPUT_MODE_REFRESH_TOLERANCE_MILLIHZ: u32 = 500;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DesktopOutputTiming {
    pub width: u32,
    pub height: u32,
    pub refresh_millihz: u32,
}

impl DesktopOutputTiming {
    pub const fn new(width: u32, height: u32, refresh_millihz: u32) -> Self {
        Self {
            width,
            height,
            refresh_millihz,
        }
    }

    const fn valid(self) -> bool {
        self.width > 0
            && self.width <= 16_384
            && self.height > 0
            && self.height <= 16_384
            && self.refresh_millihz >= 1_000
            && self.refresh_millihz <= 1_000_000
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopOutputTransformSet(u8);

impl DesktopOutputTransformSet {
    pub const NORMAL: Self = Self(1 << 0);
    pub const ROTATE_90: Self = Self(1 << 1);
    pub const ROTATE_180: Self = Self(1 << 2);
    pub const ROTATE_270: Self = Self(1 << 3);
    pub const FLIPPED: Self = Self(1 << 4);
    pub const FLIPPED_90: Self = Self(1 << 5);
    pub const FLIPPED_180: Self = Self(1 << 6);
    pub const FLIPPED_270: Self = Self(1 << 7);
    pub const ALL: Self = Self(u8::MAX);

    pub const fn contains(self, transform: DesktopOutputTransform) -> bool {
        let required = match transform {
            DesktopOutputTransform::Normal => Self::NORMAL.0,
            DesktopOutputTransform::Rotate90 => Self::ROTATE_90.0,
            DesktopOutputTransform::Rotate180 => Self::ROTATE_180.0,
            DesktopOutputTransform::Rotate270 => Self::ROTATE_270.0,
            DesktopOutputTransform::Flipped => Self::FLIPPED.0,
            DesktopOutputTransform::Flipped90 => Self::FLIPPED_90.0,
            DesktopOutputTransform::Flipped180 => Self::FLIPPED_180.0,
            DesktopOutputTransform::Flipped270 => Self::FLIPPED_270.0,
        };
        self.0 & required != 0
    }
}

impl std::ops::BitOr for DesktopOutputTransformSet {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopOutputScaleCapabilities {
    pub minimum_milli: u32,
    pub maximum_milli: u32,
    pub step_milli: u32,
    pub automatic_milli: u32,
}

impl DesktopOutputScaleCapabilities {
    const fn supports(self, scale_milli: u32) -> bool {
        self.minimum_milli <= self.maximum_milli
            && self.step_milli > 0
            && scale_milli >= self.minimum_milli
            && scale_milli <= self.maximum_milli
            && (scale_milli - self.minimum_milli).is_multiple_of(self.step_milli)
    }

    const fn valid(self) -> bool {
        self.minimum_milli >= 250
            && self.maximum_milli <= 8_000
            && self.supports(self.automatic_milli)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopOutputState {
    pub connector: String,
    pub enabled: bool,
    pub mode: DesktopOutputTiming,
    pub scale_milli: u32,
    pub position: (i32, i32),
    pub transform: DesktopOutputTransform,
    pub vrr: DesktopOutputVrrMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopOutputTopologyConnector {
    pub connector: String,
    pub connected: bool,
    pub modes: Vec<DesktopOutputTiming>,
    pub preferred_mode: Option<DesktopOutputTiming>,
    pub scales: DesktopOutputScaleCapabilities,
    pub transforms: DesktopOutputTransformSet,
    pub vrr_capable: bool,
    pub current: DesktopOutputState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopOutputTopologySnapshot {
    pub connectors: Vec<DesktopOutputTopologyConnector>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopOutputReconciliation {
    pub generation: ConfigGeneration,
    pub digest: ConfigDigest,
    pub outputs: Vec<DesktopOutputState>,
    pub focused_connector: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DesktopOutputReconcileError {
    InvalidCandidate(String),
    InvalidTopology(String),
    UnknownConnector(String),
    DisconnectedConnector(String),
    PreferredModeUnavailable(String),
    ModeUnavailable(String),
    ModeAmbiguous(String),
    ScaleUnsupported(String),
    TransformUnsupported(String),
    VrrUnsupported(String),
    FocusedOutputDisabled(String),
    OutputOverlap { first: String, second: String },
    NoEnabledOutput,
}

impl fmt::Display for DesktopOutputReconcileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCandidate(message) => {
                write!(formatter, "invalid output candidate: {message}")
            }
            Self::InvalidTopology(message) => {
                write!(formatter, "invalid output topology: {message}")
            }
            Self::UnknownConnector(connector) => {
                write!(formatter, "unknown connector {connector:?}")
            }
            Self::DisconnectedConnector(connector) => {
                write!(formatter, "connector {connector:?} is disconnected")
            }
            Self::PreferredModeUnavailable(connector) => {
                write!(formatter, "connector {connector:?} has no preferred mode")
            }
            Self::ModeUnavailable(connector) => {
                write!(formatter, "requested mode is unavailable on {connector:?}")
            }
            Self::ModeAmbiguous(connector) => {
                write!(formatter, "requested mode is ambiguous on {connector:?}")
            }
            Self::ScaleUnsupported(connector) => {
                write!(formatter, "requested scale is unsupported on {connector:?}")
            }
            Self::TransformUnsupported(connector) => {
                write!(
                    formatter,
                    "requested transform is unsupported on {connector:?}"
                )
            }
            Self::VrrUnsupported(connector) => {
                write!(
                    formatter,
                    "requested VRR policy is unsupported on {connector:?}"
                )
            }
            Self::FocusedOutputDisabled(connector) => {
                write!(
                    formatter,
                    "startup-focused connector {connector:?} is disabled"
                )
            }
            Self::OutputOverlap { first, second } => {
                write!(
                    formatter,
                    "enabled outputs {first:?} and {second:?} overlap"
                )
            }
            Self::NoEnabledOutput => formatter.write_str("output candidate disables every output"),
        }
    }
}

impl std::error::Error for DesktopOutputReconcileError {}

pub fn reconcile_desktop_output_candidate(
    candidate: &DesktopOutputCandidate,
    topology: &DesktopOutputTopologySnapshot,
) -> Result<DesktopOutputReconciliation, DesktopOutputReconcileError> {
    validate_candidate(candidate)?;
    validate_topology(topology)?;
    let mut outputs = topology
        .connectors
        .iter()
        .map(|connector| {
            if candidate.inherit_sophia {
                connector.current.clone()
            } else {
                DesktopOutputState {
                    connector: connector.connector.clone(),
                    enabled: false,
                    mode: connector.preferred_mode.unwrap_or(connector.current.mode),
                    scale_milli: connector.scales.automatic_milli,
                    position: (0, 0),
                    transform: DesktopOutputTransform::Normal,
                    vrr: DesktopOutputVrrMode::Disabled,
                }
            }
        })
        .collect::<Vec<_>>();
    let mut focused_connector = None;
    for requested in &candidate.named {
        let Some(index) = topology
            .connectors
            .iter()
            .position(|connector| connector.connector == requested.connector)
        else {
            return Err(DesktopOutputReconcileError::UnknownConnector(
                requested.connector.clone(),
            ));
        };
        let connector = &topology.connectors[index];
        let output = &mut outputs[index];
        let enabled = requested.enabled.unwrap_or(if candidate.inherit_sophia {
            output.enabled
        } else {
            true
        });
        if enabled && !connector.connected {
            return Err(DesktopOutputReconcileError::DisconnectedConnector(
                connector.connector.clone(),
            ));
        }
        output.enabled = enabled;
        if let Some(mode) = requested.mode {
            output.mode = resolve_mode(connector, mode)?;
        }
        if let Some(scale) = requested.scale {
            output.scale_milli = match scale {
                DesktopOutputScale::Automatic => connector.scales.automatic_milli,
                DesktopOutputScale::FixedMilli(scale_milli) => scale_milli,
            };
        }
        if !connector.scales.supports(output.scale_milli) {
            return Err(DesktopOutputReconcileError::ScaleUnsupported(
                connector.connector.clone(),
            ));
        }
        if let Some(position) = requested.position {
            output.position = position;
        }
        if let Some(transform) = requested.transform {
            output.transform = transform;
        }
        if !connector.transforms.contains(output.transform) {
            return Err(DesktopOutputReconcileError::TransformUnsupported(
                connector.connector.clone(),
            ));
        }
        if let Some(vrr) = requested.vrr {
            output.vrr = vrr;
        }
        if output.vrr != DesktopOutputVrrMode::Disabled && !connector.vrr_capable {
            return Err(DesktopOutputReconcileError::VrrUnsupported(
                connector.connector.clone(),
            ));
        }
        if requested.focus_at_startup == Some(true) {
            if !output.enabled {
                return Err(DesktopOutputReconcileError::FocusedOutputDisabled(
                    connector.connector.clone(),
                ));
            }
            focused_connector = Some(connector.connector.clone());
        }
    }
    if !outputs.iter().any(|output| output.enabled) {
        return Err(DesktopOutputReconcileError::NoEnabledOutput);
    }
    reject_overlaps(&outputs)?;
    Ok(DesktopOutputReconciliation {
        generation: candidate.generation,
        digest: candidate.digest,
        outputs,
        focused_connector,
    })
}

pub fn validate_desktop_output_topology_snapshot(
    topology: &DesktopOutputTopologySnapshot,
) -> Result<(), DesktopOutputReconcileError> {
    validate_topology(topology)
}

fn validate_candidate(
    candidate: &DesktopOutputCandidate,
) -> Result<(), DesktopOutputReconcileError> {
    if candidate.named.len() > DESKTOP_OUTPUT_MAX_NAMED {
        return Err(DesktopOutputReconcileError::InvalidCandidate(
            "named output count exceeds its bound".to_owned(),
        ));
    }
    let mut connectors = BTreeSet::new();
    let mut focused = false;
    for output in &candidate.named {
        if !valid_desktop_output_connector(&output.connector)
            || !connectors.insert(output.connector.clone())
        {
            return Err(DesktopOutputReconcileError::InvalidCandidate(
                "connector identities must be valid and unique".to_owned(),
            ));
        }
        if output.position.is_some_and(|(x, y)| {
            !(-1_000_000..=1_000_000).contains(&x) || !(-1_000_000..=1_000_000).contains(&y)
        }) {
            return Err(DesktopOutputReconcileError::InvalidCandidate(
                "output position is outside its supported range".to_owned(),
            ));
        }
        if output.focus_at_startup == Some(true) {
            if focused {
                return Err(DesktopOutputReconcileError::InvalidCandidate(
                    "more than one output requests startup focus".to_owned(),
                ));
            }
            focused = true;
        }
    }
    Ok(())
}

fn validate_topology(
    topology: &DesktopOutputTopologySnapshot,
) -> Result<(), DesktopOutputReconcileError> {
    if topology.connectors.is_empty() || topology.connectors.len() > DESKTOP_OUTPUT_MAX_NAMED {
        return Err(DesktopOutputReconcileError::InvalidTopology(
            "connector count is outside its supported range".to_owned(),
        ));
    }
    let mut names = BTreeSet::new();
    for connector in &topology.connectors {
        if !valid_desktop_output_connector(&connector.connector)
            || !names.insert(connector.connector.clone())
        {
            return Err(DesktopOutputReconcileError::InvalidTopology(
                "connector identities must be valid and unique".to_owned(),
            ));
        }
        if connector.current.connector != connector.connector
            || connector.modes.is_empty()
            || connector.modes.len() > 256
            || !connector.scales.valid()
            || !connector
                .transforms
                .contains(DesktopOutputTransform::Normal)
        {
            return Err(DesktopOutputReconcileError::InvalidTopology(format!(
                "connector {:?} has inconsistent capabilities",
                connector.connector
            )));
        }
        let mut modes = BTreeSet::new();
        if connector
            .modes
            .iter()
            .any(|mode| !mode.valid() || !modes.insert(*mode))
            || connector
                .preferred_mode
                .is_some_and(|mode| !modes.contains(&mode))
            || !modes.contains(&connector.current.mode)
            || !connector.scales.supports(connector.current.scale_milli)
            || !connector.transforms.contains(connector.current.transform)
            || !(-1_000_000..=1_000_000).contains(&connector.current.position.0)
            || !(-1_000_000..=1_000_000).contains(&connector.current.position.1)
            || (connector.current.vrr != DesktopOutputVrrMode::Disabled && !connector.vrr_capable)
            || (connector.current.enabled && !connector.connected)
        {
            return Err(DesktopOutputReconcileError::InvalidTopology(format!(
                "connector {:?} has an invalid current state",
                connector.connector
            )));
        }
    }
    Ok(())
}

fn resolve_mode(
    connector: &DesktopOutputTopologyConnector,
    requested: DesktopOutputMode,
) -> Result<DesktopOutputTiming, DesktopOutputReconcileError> {
    let DesktopOutputMode::Exact {
        width,
        height,
        refresh_millihz,
    } = requested
    else {
        return connector.preferred_mode.ok_or_else(|| {
            DesktopOutputReconcileError::PreferredModeUnavailable(connector.connector.clone())
        });
    };
    let mut matches = connector
        .modes
        .iter()
        .copied()
        .filter(|mode| {
            mode.width == width
                && mode.height == height
                && mode.refresh_millihz.abs_diff(refresh_millihz)
                    <= OUTPUT_MODE_REFRESH_TOLERANCE_MILLIHZ
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|mode| mode.refresh_millihz.abs_diff(refresh_millihz));
    let Some(selected) = matches.first().copied() else {
        return Err(DesktopOutputReconcileError::ModeUnavailable(
            connector.connector.clone(),
        ));
    };
    if matches.get(1).is_some_and(|next| {
        next.refresh_millihz.abs_diff(refresh_millihz)
            == selected.refresh_millihz.abs_diff(refresh_millihz)
    }) {
        return Err(DesktopOutputReconcileError::ModeAmbiguous(
            connector.connector.clone(),
        ));
    }
    Ok(selected)
}

fn reject_overlaps(outputs: &[DesktopOutputState]) -> Result<(), DesktopOutputReconcileError> {
    for (index, first) in outputs
        .iter()
        .enumerate()
        .filter(|(_, output)| output.enabled)
    {
        for second in outputs
            .iter()
            .skip(index + 1)
            .filter(|output| output.enabled)
        {
            if output_rect(first).overlaps(output_rect(second)) {
                return Err(DesktopOutputReconcileError::OutputOverlap {
                    first: first.connector.clone(),
                    second: second.connector.clone(),
                });
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct OutputRect {
    x: i64,
    y: i64,
    width: i64,
    height: i64,
}

impl OutputRect {
    const fn overlaps(self, other: Self) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
}

fn output_rect(output: &DesktopOutputState) -> OutputRect {
    let (width, height) = match output.transform {
        DesktopOutputTransform::Rotate90
        | DesktopOutputTransform::Rotate270
        | DesktopOutputTransform::Flipped90
        | DesktopOutputTransform::Flipped270 => (output.mode.height, output.mode.width),
        DesktopOutputTransform::Normal
        | DesktopOutputTransform::Rotate180
        | DesktopOutputTransform::Flipped
        | DesktopOutputTransform::Flipped180 => (output.mode.width, output.mode.height),
    };
    let logical = |pixels: u32| {
        i64::from(pixels)
            .saturating_mul(1_000)
            .saturating_add(i64::from(output.scale_milli) - 1)
            / i64::from(output.scale_milli)
    };
    OutputRect {
        x: i64::from(output.position.0),
        y: i64::from(output.position.1),
        width: logical(width),
        height: logical(height),
    }
}
