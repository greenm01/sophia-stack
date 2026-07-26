use super::*;

impl LiveProductionNativeHead {
    pub(super) fn queue_compositor_display_list(
        &mut self,
        display_list: Option<sophia_engine::CompositorDisplayList>,
    ) {
        let Some(display_list) = display_list else {
            self.compositor_display_lists.discard_pending();
            return;
        };
        if let Err(error) = self.compositor_display_lists.queue(display_list) {
            tracing::warn!(
                "sophia_live_compositor_damage schema=1 status=queue_rejected output={} reason={error}",
                self.output.id.raw(),
            );
            self.compositor_display_lists.discard_pending();
        }
    }
}

pub(super) fn trace_presented_compositor_damage(
    status: &'static str,
    output: OutputId,
    presented: &sophia_engine::CompositorDisplayListPresentation,
) {
    tracing::info!(
        "sophia_live_compositor_damage schema=1 status={} output={} rects={}",
        status,
        output.raw(),
        presented.damage.rects.len(),
    );
    tracing::info!(
        "sophia_live_compositor_repaint schema=1 status={} output={} mode={} rects={} pixels={}",
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
