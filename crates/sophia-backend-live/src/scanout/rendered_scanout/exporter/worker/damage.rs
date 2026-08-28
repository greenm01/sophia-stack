fn reusable_cpu_buffer_damage(
    previous_checksum: u64,
    previous: Option<&sophia_engine::OutputFrameDamageSnapshot>,
    checksum: u64,
    current: Option<&sophia_engine::OutputFrameDamageSnapshot>,
    size: sophia_protocol::Size,
) -> Vec<sophia_protocol::Rect> {
    if previous_checksum == checksum {
        return Vec::new();
    }
    let damage = previous
        .zip(current)
        .and_then(|(previous, current)| {
            sophia_engine::output_frame_damage(Some(previous), current).ok()
        })
        .and_then(|damage| {
            sophia_engine::plan_output_repaint(
                size,
                &damage,
                sophia_engine::OutputRepaintPolicy::default(),
            )
            .ok()
        })
        .map(|repaint| match repaint {
            sophia_engine::OutputRepaintPlan::Skip => Vec::new(),
            sophia_engine::OutputRepaintPlan::Partial { damage, .. }
            | sophia_engine::OutputRepaintPlan::Full { damage, .. } => damage.rects,
        });
    damage.unwrap_or_else(|| {
        vec![sophia_protocol::Rect {
            x: 0,
            y: 0,
            width: size.width,
            height: size.height,
        }]
    })
}
