//! Reconcile logical output authority with backend timing for frontend publication.

use sophia_protocol::Size;
use std::collections::BTreeMap;

/// Check every shared topology field before attaching backend-owned modelines.
/// The authority protocol names opaque modes and cannot supply DRM timing data.
/// Publish the resolved snapshot so X11 receives the timing actually selected.
pub fn prepare_output_topology_publication(
    authority: &sophia_protocol::OutputAuthoritySnapshot,
    resolved: &sophia_backend_live::LiveResolvedOutputTopology,
    generation: u64,
) -> Result<sophia_protocol::OutputTopologySnapshot, Box<dyn std::error::Error>> {
    let expected = output_topology_from_authority_at_generation(authority, generation)?;
    let publication = output_topology_from_resolved_at_generation(resolved, generation)?;
    let mut nominal = publication.clone();
    for output in &mut nominal.outputs {
        output.timing = None;
    }
    if expected != nominal {
        return Err("candidate authority and native topology projections disagree".into());
    }
    Ok(publication)
}

pub fn output_topology_from_resolved_at_generation(
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
            // Both rates come from the head that scans this output: the
            // nominal one the matcher compares, and the timing it is actually
            // running. Read together so they cannot describe different modes.
            let timing = resolved
                .targets
                .iter()
                .find(|target| target.output == viewport.output && target.head == *primary)
                .map(|target| &target.timing)
                .ok_or("resolved topology output has no enabled head")?;
            Ok(sophia_protocol::OutputTopologyEntry {
                output: output.id,
                logical: viewport.logical,
                pixel_size: output.size,
                scale: output.scale,
                refresh_millihz: timing.refresh_millihz,
                timing: timing.mode,
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

pub fn output_topology_from_authority_at_generation(
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
                timing: None,
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
