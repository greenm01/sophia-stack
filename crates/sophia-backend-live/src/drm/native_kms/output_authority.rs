use crate::prelude::*;
use crate::{LibdrmNativeOutputCapability, LibdrmNativeOutputTiming, NativeMirrorGrouping};
use sophia_engine::{HeadRenderTarget, HeadlessOutput, RenderHeadId};
use sophia_protocol::{
    DisplayHeadId, DisplayModeId, MAX_OUTPUT_AUTHORITY_MODES_PER_HEAD, OutputAuthoritySnapshot,
    OutputGroupMember, OutputHeadDescriptor, OutputHeadMapping, OutputLogicalGroupState,
    OutputModeDescriptor, OutputTopologyCandidate, OutputTransformSet, OutputVrrPolicy, Rect,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveOutputAuthorityProjectionError {
    InvalidEpoch,
    Empty,
    NativeCapability(String),
    MissingOpaqueHead(String),
    MissingOutput(OutputId),
    MissingCapability(DisplayHeadId),
    MissingMode(DisplayHeadId),
    ModeExtentUnsupported(DisplayHeadId),
    InvalidLogicalGeometry(OutputId),
    InvalidSnapshot(sophia_protocol::OutputTopologyCandidateError),
    InvalidCandidate(sophia_protocol::OutputTopologyCandidateError),
    OutputIdentityExhausted,
    InvalidMirrorGrouping(crate::NativeMirrorGroupingError),
}

impl core::fmt::Display for LiveOutputAuthorityProjectionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LiveOutputAuthorityProjectionError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveOutputAuthorityHeadTarget {
    pub head: RenderHeadId,
    pub output: OutputId,
    pub timing: LibdrmNativeOutputTiming,
    pub native_size: Size,
    pub transform: sophia_protocol::OutputTransform,
    pub mapping: OutputHeadMapping,
    pub vrr: OutputVrrPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveResolvedOutputTopology {
    pub primary_output: OutputId,
    pub outputs: Vec<HeadlessOutput>,
    pub targets: Vec<LiveOutputAuthorityHeadTarget>,
    pub mirror_grouping: NativeMirrorGrouping,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveLogicalOutputAllocator {
    next: u64,
}

impl LiveLogicalOutputAllocator {
    pub fn after(outputs: impl IntoIterator<Item = OutputId>) -> Option<Self> {
        let maximum = outputs.into_iter().map(OutputId::raw).max().unwrap_or(0);
        Some(Self {
            next: maximum.checked_add(1)?.max(1),
        })
    }

    pub fn mint(&mut self) -> Option<OutputId> {
        let output = OutputId::from_raw(self.next);
        self.next = self.next.checked_add(1)?;
        output.is_valid().then_some(output)
    }
}

pub fn project_live_output_authority_snapshot(
    capabilities: &[LibdrmNativeOutputCapability],
    outputs: &[HeadlessOutput],
    topology_epoch: u64,
) -> Result<OutputAuthoritySnapshot, LiveOutputAuthorityProjectionError> {
    if topology_epoch == 0 {
        return Err(LiveOutputAuthorityProjectionError::InvalidEpoch);
    }
    if capabilities.is_empty() || outputs.is_empty() {
        return Err(LiveOutputAuthorityProjectionError::Empty);
    }
    let output_map = outputs
        .iter()
        .map(|output| (output.id, *output))
        .collect::<BTreeMap<_, _>>();
    let mut sorted = capabilities.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|capability| {
        capability
            .head()
            .map_or(u64::MAX, sophia_engine::RenderHeadId::raw)
    });

    let mut heads = Vec::with_capacity(sorted.len());
    let mut members_by_output: BTreeMap<OutputId, Vec<OutputGroupMember>> = BTreeMap::new();
    for (head_index, capability) in sorted.into_iter().enumerate() {
        let head = capability.head().ok_or_else(|| {
            LiveOutputAuthorityProjectionError::MissingOpaqueHead(
                capability.connector_name().to_owned(),
            )
        })?;
        if !output_map.contains_key(&capability.output()) {
            return Err(LiveOutputAuthorityProjectionError::MissingOutput(
                capability.output(),
            ));
        }
        let modes = bounded_modes(capability, head)?;
        let current_mode = modes
            .iter()
            .find(|mode| {
                Some(mode.pixel_size) == timing_size(capability.selected_mode())
                    && mode.refresh_millihz == capability.selected_mode().refresh_millihz
            })
            .map(|mode| mode.mode)
            .ok_or_else(|| {
                LiveOutputAuthorityProjectionError::MissingMode(DisplayHeadId::from_raw(head.raw()))
            })?;
        heads.push(OutputHeadDescriptor {
            head: DisplayHeadId::from_raw(head.raw()),
            generation: 1,
            // Human-readable but connector-neutral. The exclusive output
            // authority identifies targets by the opaque DisplayHeadId; DRM
            // connector names remain private to candidate resolution below.
            label: format!("Display {}", head_index.saturating_add(1)),
            connected: true,
            enabled: true,
            current_mode: Some(current_mode),
            transforms: OutputTransformSet::NORMAL,
            vrr_capable: capability.vrr_configurable(),
            modes,
        });
        members_by_output
            .entry(capability.output())
            .or_default()
            .push(OutputGroupMember {
                head: DisplayHeadId::from_raw(head.raw()),
                mapping: OutputHeadMapping::Fit,
            });
    }

    let mut logical_x = 0i32;
    let mut groups = Vec::with_capacity(outputs.len());
    for output in outputs {
        let scale = i32::try_from(output.scale)
            .ok()
            .filter(|scale| *scale > 0)
            .ok_or(LiveOutputAuthorityProjectionError::InvalidLogicalGeometry(
                output.id,
            ))?;
        let width = ceil_div_positive(output.size.width, scale).ok_or(
            LiveOutputAuthorityProjectionError::InvalidLogicalGeometry(output.id),
        )?;
        let height = ceil_div_positive(output.size.height, scale).ok_or(
            LiveOutputAuthorityProjectionError::InvalidLogicalGeometry(output.id),
        )?;
        let logical = Rect {
            x: logical_x,
            y: 0,
            width,
            height,
        };
        logical_x = logical_x.checked_add(width).ok_or(
            LiveOutputAuthorityProjectionError::InvalidLogicalGeometry(output.id),
        )?;
        groups.push(OutputLogicalGroupState {
            output: output.id,
            generation: 1,
            logical,
            members: members_by_output
                .remove(&output.id)
                .ok_or(LiveOutputAuthorityProjectionError::MissingOutput(output.id))?,
        });
    }
    if let Some(output) = members_by_output.keys().next().copied() {
        return Err(LiveOutputAuthorityProjectionError::MissingOutput(output));
    }
    let snapshot = OutputAuthoritySnapshot {
        topology_epoch,
        primary_output: outputs[0].id,
        heads,
        groups,
    };
    snapshot
        .validate()
        .map_err(LiveOutputAuthorityProjectionError::InvalidSnapshot)?;
    Ok(snapshot)
}

pub fn resolve_live_output_topology_candidate(
    snapshot: &OutputAuthoritySnapshot,
    capabilities: &[LibdrmNativeOutputCapability],
    candidate: &OutputTopologyCandidate,
    allocator: &mut LiveLogicalOutputAllocator,
) -> Result<LiveResolvedOutputTopology, LiveOutputAuthorityProjectionError> {
    candidate
        .validate_against(snapshot)
        .map_err(LiveOutputAuthorityProjectionError::InvalidCandidate)?;
    let capabilities = capabilities
        .iter()
        .filter_map(|capability| capability.head().map(|head| (head, capability)))
        .collect::<BTreeMap<_, _>>();
    let snapshot_heads = snapshot
        .heads
        .iter()
        .map(|head| (head.head, head))
        .collect::<BTreeMap<_, _>>();
    let targets = candidate
        .heads
        .iter()
        .map(|target| {
            let head = RenderHeadId::from_raw(target.head.raw());
            let capability = capabilities.get(&head).ok_or(
                LiveOutputAuthorityProjectionError::MissingCapability(target.head),
            )?;
            let descriptor = snapshot_heads.get(&target.head).ok_or(
                LiveOutputAuthorityProjectionError::MissingCapability(target.head),
            )?;
            let mode = descriptor
                .modes
                .iter()
                .find(|mode| mode.mode == target.mode)
                .ok_or(LiveOutputAuthorityProjectionError::MissingMode(target.head))?;
            let timing = LibdrmNativeOutputTiming::new(
                u32::try_from(mode.pixel_size.width)
                    .map_err(|_| LiveOutputAuthorityProjectionError::MissingMode(target.head))?,
                u32::try_from(mode.pixel_size.height)
                    .map_err(|_| LiveOutputAuthorityProjectionError::MissingMode(target.head))?,
                mode.refresh_millihz,
            );
            if !capability.modes().contains(&timing) {
                return Err(LiveOutputAuthorityProjectionError::MissingMode(target.head));
            }
            Ok((target, head, timing, mode.pixel_size))
        })
        .collect::<Result<Vec<_>, LiveOutputAuthorityProjectionError>>()?;

    // Identity allocation is part of candidate resolution. Keep it private
    // until every later native projection step succeeds so a rejected
    // candidate cannot consume stable logical-output identities.
    let mut candidate_allocator = allocator.clone();
    let mut output_ids = Vec::with_capacity(candidate.groups.len());
    for group in &candidate.groups {
        output_ids.push(if group.output.is_valid() {
            group.output
        } else {
            candidate_allocator
                .mint()
                .ok_or(LiveOutputAuthorityProjectionError::OutputIdentityExhausted)?
        });
    }
    let group_for_head = candidate
        .groups
        .iter()
        .enumerate()
        .flat_map(|(index, group)| {
            group
                .members
                .iter()
                .map(move |member| (member.head, (index, member.mapping)))
        })
        .collect::<BTreeMap<_, _>>();
    let targets = targets
        .into_iter()
        .map(|(proposal, head, timing, native_size)| {
            let (group, mapping) = group_for_head[&proposal.head];
            LiveOutputAuthorityHeadTarget {
                head,
                output: output_ids[group],
                timing,
                native_size,
                transform: proposal.transform,
                mapping,
                vrr: proposal.vrr,
            }
        })
        .collect::<Vec<_>>();
    let outputs = candidate
        .groups
        .iter()
        .enumerate()
        .map(|(index, group)| HeadlessOutput {
            id: output_ids[index],
            size: Size {
                width: group.logical.width,
                height: group.logical.height,
            },
            scale: 1,
        })
        .collect::<Vec<_>>();
    let names = capabilities
        .values()
        .map(|capability| {
            (
                DisplayHeadId::from_raw(capability.head().unwrap().raw()),
                capability.connector_name(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mirror_grouping = NativeMirrorGrouping::new(candidate.groups.iter().filter_map(|group| {
        (group.members.len() > 1).then(|| {
            group
                .members
                .iter()
                .map(|member| names[&member.head].to_owned())
                .collect::<Vec<_>>()
        })
    }))
    .map_err(LiveOutputAuthorityProjectionError::InvalidMirrorGrouping)?;
    let primary_output = output_ids[usize::from(candidate.primary_group_index)];
    *allocator = candidate_allocator;
    Ok(LiveResolvedOutputTopology {
        primary_output,
        outputs,
        targets,
        mirror_grouping,
    })
}

impl LiveResolvedOutputTopology {
    pub fn head_render_targets(&self, target_generation: u64) -> Vec<HeadRenderTarget> {
        self.targets
            .iter()
            .map(|target| HeadRenderTarget {
                head: target.head,
                output: target.output,
                target_generation,
                native_size: target.native_size,
                scale: 1,
                refresh_millihz: target.timing.refresh_millihz,
                transform: target.transform,
                mapping: target.mapping,
            })
            .collect()
    }
}

fn bounded_modes(
    capability: &LibdrmNativeOutputCapability,
    head: RenderHeadId,
) -> Result<Vec<OutputModeDescriptor>, LiveOutputAuthorityProjectionError> {
    let selected = capability.selected_mode();
    let preferred = capability.preferred_mode();
    let mut timings = Vec::new();
    timings.push(selected);
    if let Some(preferred) = preferred
        && !timings.contains(&preferred)
    {
        timings.push(preferred);
    }
    for timing in capability.modes() {
        if timings.len() == MAX_OUTPUT_AUTHORITY_MODES_PER_HEAD {
            break;
        }
        if !timings.contains(timing) {
            timings.push(*timing);
        }
    }
    timings
        .into_iter()
        .enumerate()
        .map(|(index, timing)| {
            let raw = head
                .raw()
                .checked_mul(256)
                .and_then(|base| base.checked_add(u64::try_from(index).ok()?.checked_add(1)?))
                .ok_or(LiveOutputAuthorityProjectionError::MissingMode(
                    DisplayHeadId::from_raw(head.raw()),
                ))?;
            Ok(OutputModeDescriptor {
                mode: DisplayModeId::from_raw(raw),
                pixel_size: timing_size(timing).ok_or(
                    LiveOutputAuthorityProjectionError::ModeExtentUnsupported(
                        DisplayHeadId::from_raw(head.raw()),
                    ),
                )?,
                refresh_millihz: timing.refresh_millihz,
                preferred: preferred == Some(timing),
            })
        })
        .collect()
}

fn timing_size(timing: LibdrmNativeOutputTiming) -> Option<Size> {
    Some(Size {
        width: i32::try_from(timing.width).ok()?,
        height: i32::try_from(timing.height).ok()?,
    })
}

fn ceil_div_positive(value: i32, divisor: i32) -> Option<i32> {
    (value > 0 && divisor > 0).then(|| value.checked_add(divisor - 1)?.checked_div(divisor))?
}
