use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sophia_backend_live::LibdrmNativeOutputCapability;
use sophia_config::{
    DesktopOutputReconcileError, DesktopOutputScaleCapabilities, DesktopOutputState,
    DesktopOutputTiming, DesktopOutputTopologyConnector, DesktopOutputTopologySnapshot,
    DesktopOutputTransform, DesktopOutputTransformSet, DesktopOutputVrrMode,
    validate_desktop_output_topology_snapshot,
};
use sophia_engine::HeadlessOutput;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeOutputTopologyProjectionError {
    Empty,
    DuplicateOutput(u64),
    MissingCapability(u64),
    UnexpectedCapability(u64),
    PixelSizeMismatch(u64),
    ScaleUnsupported(u64),
    PositionExhausted,
    InvalidTopology(DesktopOutputReconcileError),
}

impl fmt::Display for NativeOutputTopologyProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("native output topology is empty"),
            Self::DuplicateOutput(output) => {
                write!(formatter, "native output {output} is duplicated")
            }
            Self::MissingCapability(output) => {
                write!(formatter, "native output {output} has no DRM capability")
            }
            Self::UnexpectedCapability(output) => {
                write!(formatter, "DRM capability {output} has no Engine output")
            }
            Self::PixelSizeMismatch(output) => {
                write!(
                    formatter,
                    "native output {output} disagrees with its selected mode"
                )
            }
            Self::ScaleUnsupported(output) => {
                write!(
                    formatter,
                    "native output {output} scale is outside supported bounds"
                )
            }
            Self::PositionExhausted => {
                formatter.write_str("native output logical position exhausted")
            }
            Self::InvalidTopology(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for NativeOutputTopologyProjectionError {}

pub fn project_native_output_topology(
    capabilities: &[LibdrmNativeOutputCapability],
    outputs: &[HeadlessOutput],
) -> Result<DesktopOutputTopologySnapshot, NativeOutputTopologyProjectionError> {
    if capabilities.is_empty() || outputs.is_empty() {
        return Err(NativeOutputTopologyProjectionError::Empty);
    }
    let mut capabilities_by_output = BTreeMap::new();
    for capability in capabilities {
        let output = capability.output().raw();
        if capabilities_by_output.insert(output, capability).is_some() {
            return Err(NativeOutputTopologyProjectionError::DuplicateOutput(output));
        }
    }

    let mut seen_outputs = BTreeSet::new();
    let mut logical_x = 0i32;
    let mut connectors = Vec::with_capacity(outputs.len());
    for output in outputs {
        let output_id = output.id.raw();
        if !seen_outputs.insert(output_id) {
            return Err(NativeOutputTopologyProjectionError::DuplicateOutput(
                output_id,
            ));
        }
        let capability = capabilities_by_output.remove(&output_id).ok_or(
            NativeOutputTopologyProjectionError::MissingCapability(output_id),
        )?;
        let selected = timing(capability.selected_mode());
        if u32::try_from(output.size.width).ok() != Some(selected.width)
            || u32::try_from(output.size.height).ok() != Some(selected.height)
        {
            return Err(NativeOutputTopologyProjectionError::PixelSizeMismatch(
                output_id,
            ));
        }
        let scale_milli = output
            .scale
            .checked_mul(1_000)
            .filter(|scale| (1_000..=8_000).contains(scale))
            .ok_or(NativeOutputTopologyProjectionError::ScaleUnsupported(
                output_id,
            ))?;
        let position = (logical_x, 0);
        logical_x = logical_x
            .checked_add(logical_extent(selected.width, scale_milli)?)
            .ok_or(NativeOutputTopologyProjectionError::PositionExhausted)?;
        connectors.push(DesktopOutputTopologyConnector {
            connector: capability.connector_name().to_owned(),
            connected: true,
            modes: capability.modes().iter().copied().map(timing).collect(),
            preferred_mode: capability.preferred_mode().map(timing),
            scales: DesktopOutputScaleCapabilities {
                minimum_milli: 1_000,
                maximum_milli: 8_000,
                step_milli: 1_000,
                automatic_milli: scale_milli,
            },
            transforms: DesktopOutputTransformSet::NORMAL,
            vrr_capable: capability.vrr_configurable(),
            current: DesktopOutputState {
                connector: capability.connector_name().to_owned(),
                enabled: true,
                mode: selected,
                scale_milli,
                position,
                transform: DesktopOutputTransform::Normal,
                vrr: DesktopOutputVrrMode::Disabled,
            },
        });
    }
    if let Some(output) = capabilities_by_output.keys().next().copied() {
        return Err(NativeOutputTopologyProjectionError::UnexpectedCapability(
            output,
        ));
    }
    let topology = DesktopOutputTopologySnapshot { connectors };
    validate_desktop_output_topology_snapshot(&topology)
        .map_err(NativeOutputTopologyProjectionError::InvalidTopology)?;
    Ok(topology)
}

fn timing(timing: sophia_backend_live::LibdrmNativeOutputTiming) -> DesktopOutputTiming {
    DesktopOutputTiming::new(timing.width, timing.height, timing.refresh_millihz)
}

fn logical_extent(
    pixels: u32,
    scale_milli: u32,
) -> Result<i32, NativeOutputTopologyProjectionError> {
    let extent = u64::from(pixels)
        .checked_mul(1_000)
        .and_then(|value| value.checked_add(u64::from(scale_milli) - 1))
        .map(|value| value / u64::from(scale_milli))
        .and_then(|value| i32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or(NativeOutputTopologyProjectionError::PositionExhausted)?;
    Ok(extent)
}
