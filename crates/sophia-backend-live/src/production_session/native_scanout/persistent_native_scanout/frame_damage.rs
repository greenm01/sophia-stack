use super::*;

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
