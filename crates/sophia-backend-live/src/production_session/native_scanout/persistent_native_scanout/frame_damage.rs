use super::*;

/// Projects one logical mirror scene's immutable damage state into a physical
/// head's coordinate space.
///
/// Rendering and damage must use the same target rectangle. Keeping this
/// transformation pure lets every mirror queue path prepare all heads before
/// it reserves or mutates a logical generation.
pub fn project_mirror_output_damage_snapshot(
    snapshot: &sophia_engine::OutputFrameDamageSnapshot,
    source: sophia_protocol::Size,
    destination: sophia_engine::HeadlessOutput,
    fit: crate::NativeMirrorFit,
) -> Result<sophia_engine::OutputFrameDamageSnapshot, &'static str> {
    if snapshot.output.id != destination.id
        || snapshot.compositor_display_list.output != destination.id
    {
        return Err("mirror damage snapshot targets a different logical output");
    }
    if snapshot.output.size != source {
        return Err("mirror damage snapshot source size does not match the projected scene");
    }
    if source.width <= 0
        || source.height <= 0
        || destination.size.width <= 0
        || destination.size.height <= 0
        || destination.scale == 0
    {
        return Err("mirror damage projection has an invalid output shape");
    }
    let target = crate::project_mirror_rect(source, destination.size, fit);
    if target.width <= 0 || target.height <= 0 {
        return Err("mirror damage projection is empty");
    }

    let mut projected = snapshot.clone();
    projected.output = destination;
    for surface in &mut projected.surfaces {
        surface.geometry = crate::project_mirror_child_rect(surface.geometry, source, target);
    }
    for command in &mut projected.compositor_display_list.commands {
        if let sophia_engine::CompositorDisplayCommand::Border(border) = command {
            border.outer = crate::project_mirror_child_rect(border.outer, source, target);
            border.inner = crate::project_mirror_child_rect(border.inner, source, target);
        }
    }
    projected.software_cursor = projected
        .software_cursor
        .map(|cursor| crate::project_mirror_child_rect(cursor, source, target));
    Ok(projected)
}

impl LiveProductionNativeHead {
    pub(super) fn queue_output_damage_snapshot(
        &mut self,
        snapshot: Option<sophia_engine::OutputFrameDamageSnapshot>,
    ) {
        let Some(snapshot) = snapshot else {
            self.output_frames.discard_pending();
            return;
        };
        if let Err(error) = self.output_frames.queue(snapshot) {
            tracing::warn!(
                "sophia_live_output_damage schema=1 status=queue_rejected output={} reason={error}",
                self.output.id.raw(),
            );
            self.output_frames.discard_pending();
        }
    }
}

pub(super) fn trace_presented_output_damage(
    status: &'static str,
    output: OutputId,
    presented: &sophia_engine::OutputFramePresentation,
) {
    tracing::info!(
        "sophia_live_compositor_damage schema=1 status={} output={} rects={}",
        status,
        output.raw(),
        presented.compositor_damage.rects.len(),
    );
    tracing::info!(
        "sophia_live_output_damage schema=1 status={} output={} rects={}",
        status,
        output.raw(),
        presented.damage.rects.len(),
    );
    tracing::info!(
        "sophia_live_output_repaint schema=1 status={} output={} mode={} rects={} pixels={}",
        status,
        output.raw(),
        presented.repaint.reduced_name(),
        presented
            .repaint
            .damage()
            .map_or(0, |damage| damage.rects.len()),
        presented.repaint.damaged_pixels(),
    );
}

pub(super) fn trace_presented_mirror_head_damage(
    output: OutputId,
    head: sophia_engine::RenderHeadId,
    frame: LiveProductionNativeFrameId,
    presented: &sophia_engine::OutputFramePresentation,
) {
    tracing::info!(
        "sophia_live_mirror_head_damage schema=2 status=presented output={} head={} frame={} width={} height={} mode={} rects={} pixels={}",
        output.raw(),
        head.raw(),
        frame.raw(),
        presented.snapshot.output.size.width,
        presented.snapshot.output.size.height,
        presented.repaint.reduced_name(),
        presented
            .repaint
            .damage()
            .map_or(0, |damage| damage.rects.len()),
        presented.repaint.damaged_pixels(),
    );
}
