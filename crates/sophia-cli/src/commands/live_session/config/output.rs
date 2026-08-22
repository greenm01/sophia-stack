use super::*;

pub(super) fn output_topology_from_engine_outputs(
    outputs: &[sophia_engine::HeadlessOutput],
) -> Result<sophia_protocol::OutputTopologySnapshot, Box<dyn std::error::Error>> {
    output_topology_from_engine_outputs_at_generation(outputs, 1)
}

pub(super) fn output_topology_from_engine_outputs_at_generation(
    outputs: &[sophia_engine::HeadlessOutput],
    generation: u64,
) -> Result<sophia_protocol::OutputTopologySnapshot, Box<dyn std::error::Error>> {
    let primary = outputs
        .first()
        .ok_or("live session requires at least one Engine output")?
        .id;
    let mut logical_x = 0i32;
    let entries = outputs
        .iter()
        .map(|output| {
            let scale = output.scale.max(1);
            let scale_i32 = i32::try_from(scale).unwrap_or(i32::MAX);
            let logical_size = Size {
                width: output.size.width.saturating_div(scale_i32).max(1),
                height: output.size.height.saturating_div(scale_i32).max(1),
            };
            let logical = Rect {
                x: logical_x,
                y: 0,
                width: logical_size.width,
                height: logical_size.height,
            };
            logical_x = logical_x.saturating_add(logical_size.width);
            sophia_protocol::OutputTopologyEntry {
                output: output.id,
                logical,
                pixel_size: output.size,
                scale,
                refresh_millihz: 60_000,
            }
        })
        .collect();
    let snapshot = sophia_protocol::OutputTopologySnapshot {
        generation,
        primary,
        outputs: entries,
    };
    snapshot
        .validate()
        .map_err(|error| -> Box<dyn std::error::Error> {
            format!("invalid live Engine output topology: {error:?}").into()
        })?;
    Ok(snapshot)
}

pub(super) fn output_topology_from_resolved_at_generation(
    resolved: &sophia_backend_live::LiveResolvedOutputTopology,
    generation: u64,
) -> Result<sophia_protocol::OutputTopologySnapshot, Box<dyn std::error::Error>> {
    let outputs = resolved
        .logical_viewports
        .iter()
        .map(|viewport| {
            let output = resolved
                .outputs
                .iter()
                .find(|output| output.id == viewport.output)
                .ok_or("resolved topology viewport has no logical output")?;
            let primary = resolved
                .primary_heads
                .get(&viewport.output)
                .ok_or("resolved topology output has no primary head")?;
            let refresh_millihz = resolved
                .targets
                .iter()
                .find(|target| target.output == viewport.output && target.head == *primary)
                .map(|target| target.timing.refresh_millihz)
                .ok_or("resolved topology output has no enabled head")?;
            Ok(sophia_protocol::OutputTopologyEntry {
                output: output.id,
                logical: viewport.logical,
                pixel_size: output.size,
                scale: output.scale,
                refresh_millihz,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    let snapshot = sophia_protocol::OutputTopologySnapshot {
        generation,
        primary: resolved.primary_output,
        outputs,
    };
    snapshot
        .validate()
        .map_err(|error| -> Box<dyn std::error::Error> {
            format!("invalid resolved live output topology: {error:?}").into()
        })?;
    Ok(snapshot)
}

pub(super) fn output_topology_from_authority_at_generation(
    authority: &sophia_protocol::OutputAuthoritySnapshot,
    generation: u64,
) -> Result<sophia_protocol::OutputTopologySnapshot, Box<dyn std::error::Error>> {
    authority
        .validate()
        .map_err(|error| format!("invalid authority output topology: {error:?}"))?;
    let heads = authority
        .heads
        .iter()
        .map(|head| (head.head, head))
        .collect::<BTreeMap<_, _>>();
    let outputs = authority
        .groups
        .iter()
        .map(|group| {
            let member = group
                .members
                .first()
                .ok_or("authority output group has no primary head")?;
            let refresh_millihz = heads
                .get(&member.head)
                .and_then(|head| {
                    let current = head.current_mode?;
                    head.modes
                        .iter()
                        .find(|mode| mode.mode == current)
                        .map(|mode| mode.refresh_millihz)
                })
                .ok_or("authority output group has no enabled mode")?;
            Ok(sophia_protocol::OutputTopologyEntry {
                output: group.output,
                logical: group.logical,
                pixel_size: Size {
                    width: group.logical.width,
                    height: group.logical.height,
                },
                scale: 1,
                refresh_millihz,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    let snapshot = sophia_protocol::OutputTopologySnapshot {
        generation,
        primary: authority.primary_output,
        outputs,
    };
    snapshot
        .validate()
        .map_err(|error| -> Box<dyn std::error::Error> {
            format!("invalid X frontend output topology: {error:?}").into()
        })?;
    Ok(snapshot)
}

pub(super) fn resolved_output_bounds(
    resolved: &sophia_backend_live::LiveResolvedOutputTopology,
) -> Vec<(sophia_protocol::OutputId, Rect)> {
    resolved
        .logical_viewports
        .iter()
        .map(|viewport| (viewport.output, viewport.logical))
        .collect()
}

pub(super) fn wm_output_bounds(
    outputs: &[sophia_engine::HeadlessOutput],
) -> Vec<(sophia_protocol::OutputId, Rect)> {
    let mut x = 0;
    outputs
        .iter()
        .map(|output| {
            let scale = i32::try_from(output.scale.max(1)).unwrap_or(i32::MAX);
            let bounds = Rect {
                x,
                y: 0,
                width: output.size.width.saturating_div(scale).max(1),
                height: output.size.height.saturating_div(scale).max(1),
            };
            x = x.saturating_add(bounds.width);
            (output.id, bounds)
        })
        .collect()
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PreparedOutputProfile {
    slot: sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopOutputCandidate>,
}

impl PreparedOutputProfile {
    pub(super) fn new(
        candidate: sophia_config::DesktopOutputCandidate,
    ) -> Result<Self, sophia_config::DesktopProfileCandidateSlotError> {
        Ok(Self {
            slot: sophia_config::DesktopProfileCandidateSlot::with_candidate(candidate)?,
        })
    }

    pub(super) fn candidate(&self) -> &sophia_config::DesktopOutputCandidate {
        debug_assert_eq!(
            self.slot().participant().phase(),
            sophia_config::DesktopProfileParticipantPhase::Prepared
        );
        self.slot()
            .candidate()
            .expect("trusted startup retains its prepared output candidate")
    }

    pub(super) const fn slot(
        &self,
    ) -> &sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopOutputCandidate> {
        &self.slot
    }

    pub(super) const fn slot_mut(
        &mut self,
    ) -> &mut sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopOutputCandidate> {
        &mut self.slot
    }
}
