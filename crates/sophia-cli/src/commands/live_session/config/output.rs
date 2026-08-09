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
