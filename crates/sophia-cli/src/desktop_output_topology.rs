use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sophia_backend_live::LibdrmNativeOutputCapability;
use sophia_config::{
    ConfigDigest, ConfigGeneration, DesktopOutputReconcileError, DesktopOutputReconciliation,
    DesktopOutputScaleCapabilities, DesktopOutputState, DesktopOutputTiming,
    DesktopOutputTopologyConnector, DesktopOutputTopologySnapshot, DesktopOutputTransform,
    DesktopOutputTransformSet, DesktopOutputVrrMode, validate_desktop_output_reconciliation,
    validate_desktop_output_topology_snapshot,
};
use sophia_engine::HeadlessOutput;
use sophia_protocol::OutputId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeOutputActivationTarget {
    output: OutputId,
    rollback: DesktopOutputState,
    requested: DesktopOutputState,
}

impl NativeOutputActivationTarget {
    pub const fn output(&self) -> OutputId {
        self.output
    }

    pub const fn rollback(&self) -> &DesktopOutputState {
        &self.rollback
    }

    pub const fn requested(&self) -> &DesktopOutputState {
        &self.requested
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeOutputActivationPlan {
    generation: ConfigGeneration,
    digest: ConfigDigest,
    targets: Vec<NativeOutputActivationTarget>,
    focused_output: Option<OutputId>,
}

impl NativeOutputActivationPlan {
    pub const fn generation(&self) -> ConfigGeneration {
        self.generation
    }

    pub const fn digest(&self) -> ConfigDigest {
        self.digest
    }

    pub fn targets(&self) -> &[NativeOutputActivationTarget] {
        &self.targets
    }

    pub const fn focused_output(&self) -> Option<OutputId> {
        self.focused_output
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeOutputTopologyProjectionError {
    Empty,
    DuplicateOutput(u64),
    MissingCapability(u64),
    UnexpectedCapability(u64),
    PixelSizeMismatch(u64),
    /// Connectors sharing one logical output disagree on their selected mode. A
    /// mirror group scans out one framebuffer, and no plane scaling exists here.
    MirrorModeMismatch(u64),
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
            Self::MirrorModeMismatch(output) => {
                write!(
                    formatter,
                    "native output {output} mirrors connectors with different modes"
                )
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeOutputActivationPlanError {
    InvalidOutput(u64),
    DuplicateOutput(u64),
    DuplicateConnector(String),
    MissingCapability(String),
    UnexpectedCapability(String),
    CapabilityDrift(String),
    InvalidReconciliation(DesktopOutputReconcileError),
}

impl fmt::Display for NativeOutputActivationPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOutput(output) => write!(formatter, "native output {output} is invalid"),
            Self::DuplicateOutput(output) => {
                write!(formatter, "native output {output} is duplicated")
            }
            Self::DuplicateConnector(connector) => {
                write!(formatter, "native connector {connector:?} is duplicated")
            }
            Self::MissingCapability(connector) => {
                write!(
                    formatter,
                    "native connector {connector:?} has no DRM capability"
                )
            }
            Self::UnexpectedCapability(connector) => {
                write!(
                    formatter,
                    "DRM connector {connector:?} has no admitted output"
                )
            }
            Self::CapabilityDrift(connector) => {
                write!(
                    formatter,
                    "native connector {connector:?} changed after topology projection"
                )
            }
            Self::InvalidReconciliation(error) => error.fmt(formatter),
        }
    }
}

pub fn prepare_native_output_activation_plan(
    capabilities: &[LibdrmNativeOutputCapability],
    topology: &DesktopOutputTopologySnapshot,
    reconciliation: &DesktopOutputReconciliation,
) -> Result<NativeOutputActivationPlan, NativeOutputActivationPlanError> {
    validate_desktop_output_reconciliation(reconciliation, topology)
        .map_err(NativeOutputActivationPlanError::InvalidReconciliation)?;

    let mut output_ids = BTreeSet::new();
    let mut capabilities_by_connector = BTreeMap::new();
    for capability in capabilities {
        let output = capability.output().raw();
        if output == 0 {
            return Err(NativeOutputActivationPlanError::InvalidOutput(output));
        }
        if !output_ids.insert(output) {
            return Err(NativeOutputActivationPlanError::DuplicateOutput(output));
        }
        let connector = capability.connector_name();
        if capabilities_by_connector
            .insert(connector, capability)
            .is_some()
        {
            return Err(NativeOutputActivationPlanError::DuplicateConnector(
                connector.to_owned(),
            ));
        }
    }
    let mut requested_by_connector = reconciliation
        .outputs
        .iter()
        .map(|output| (output.connector.as_str(), output))
        .collect::<BTreeMap<_, _>>();
    let mut targets = Vec::with_capacity(topology.connectors.len());
    for connector in &topology.connectors {
        let capability = capabilities_by_connector
            .remove(connector.connector.as_str())
            .ok_or_else(|| {
                NativeOutputActivationPlanError::MissingCapability(connector.connector.clone())
            })?;
        if !capability_matches_topology(capability, connector) {
            return Err(NativeOutputActivationPlanError::CapabilityDrift(
                connector.connector.clone(),
            ));
        }
        let requested = requested_by_connector
            .remove(connector.connector.as_str())
            .ok_or_else(|| invalid_reconciliation("output is absent from reconciliation"))?;
        targets.push(NativeOutputActivationTarget {
            output: capability.output(),
            rollback: connector.current.clone(),
            requested: requested.clone(),
        });
    }
    if let Some(connector) = capabilities_by_connector.keys().next() {
        return Err(NativeOutputActivationPlanError::UnexpectedCapability(
            (*connector).to_owned(),
        ));
    }
    let focused_output = reconciliation
        .focused_connector
        .as_deref()
        .map(|focused| {
            targets
                .iter()
                .find(|target| target.requested().connector == focused)
                .map(NativeOutputActivationTarget::output)
                .ok_or_else(|| invalid_reconciliation("focused output is absent from plan"))
        })
        .transpose()?;
    Ok(NativeOutputActivationPlan {
        generation: reconciliation.generation,
        digest: reconciliation.digest,
        targets,
        focused_output,
    })
}

impl std::error::Error for NativeOutputTopologyProjectionError {}

impl std::error::Error for NativeOutputActivationPlanError {}

pub fn project_native_output_topology(
    capabilities: &[LibdrmNativeOutputCapability],
    outputs: &[HeadlessOutput],
) -> Result<DesktopOutputTopologySnapshot, NativeOutputTopologyProjectionError> {
    if capabilities.is_empty() || outputs.is_empty() {
        return Err(NativeOutputTopologyProjectionError::Empty);
    }
    // Capabilities sharing one logical output are a mirror group: N connectors
    // driving one `SnapshotOutput`. Grouping rather than rejecting is what admits
    // mirroring at all; the members are validated against each other below.
    let mut capabilities_by_output: BTreeMap<u64, Vec<&LibdrmNativeOutputCapability>> =
        BTreeMap::new();
    for capability in capabilities {
        capabilities_by_output
            .entry(capability.output().raw())
            .or_default()
            .push(capability);
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
        let group = capabilities_by_output.remove(&output_id).ok_or(
            NativeOutputTopologyProjectionError::MissingCapability(output_id),
        )?;
        let capability = group[0];
        let selected = timing(capability.selected_mode());
        // Every head in a group scans out one framebuffer, and no plane scaling
        // exists on this path, so a member on a different mode could only be
        // letterboxed. Refusing here keeps that from reaching the commit.
        if group
            .iter()
            .any(|member| timing(member.selected_mode()) != selected)
        {
            return Err(NativeOutputTopologyProjectionError::MirrorModeMismatch(
                output_id,
            ));
        }
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
        // Each member keeps its own connector row, because the topology describes
        // hardware and the hardware is N connectors. They share one position, which
        // is what makes them one logical output rather than N side by side.
        for member in group {
            connectors.push(DesktopOutputTopologyConnector {
                connector: member.connector_name().to_owned(),
                connected: true,
                modes: member.modes().iter().copied().map(timing).collect(),
                preferred_mode: member.preferred_mode().map(timing),
                scales: DesktopOutputScaleCapabilities {
                    minimum_milli: 1_000,
                    maximum_milli: 8_000,
                    step_milli: 1_000,
                    automatic_milli: scale_milli,
                },
                transforms: DesktopOutputTransformSet::NORMAL,
                vrr_capable: member.vrr_configurable(),
                current: DesktopOutputState {
                    connector: member.connector_name().to_owned(),
                    enabled: true,
                    mode: selected,
                    scale_milli,
                    position,
                    transform: DesktopOutputTransform::Normal,
                    vrr: DesktopOutputVrrMode::Disabled,
                },
            });
        }
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

fn capability_matches_topology(
    capability: &LibdrmNativeOutputCapability,
    connector: &DesktopOutputTopologyConnector,
) -> bool {
    let capability_modes = capability
        .modes()
        .iter()
        .copied()
        .map(timing)
        .collect::<BTreeSet<_>>();
    let topology_modes = connector.modes.iter().copied().collect::<BTreeSet<_>>();
    capability.connector_name() == connector.connector
        && capability_modes == topology_modes
        && capability.preferred_mode().map(timing) == connector.preferred_mode
        && timing(capability.selected_mode()) == connector.current.mode
        && capability.vrr_configurable() == connector.vrr_capable
}

fn invalid_reconciliation(message: &str) -> NativeOutputActivationPlanError {
    NativeOutputActivationPlanError::InvalidReconciliation(
        DesktopOutputReconcileError::InvalidReconciliation(message.to_owned()),
    )
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
