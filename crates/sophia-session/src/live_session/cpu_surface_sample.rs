/// Committed CPU content is observable without native scanout. These records
/// describe Engine state, never claim that a frame reached physical glass.
fn log_cpu_surface_sample(
    scene: &LiveProductionCpuScene,
    surfaces: &[CommittedSurfaceState],
    sequence: u32,
) {
    for committed in surfaces.iter().take(32) {
        let geometry = committed.geometry;
        if let Some(generation) = scene.surface_buffer_generation(surfaces, committed.surface) {
            crate::session_println!(
                "sophia_live_cpu_surface schema=1 seq={} surface={}:{} x={} y={} width={} height={} buffer_generation={} visual_detail={} truncated={}",
                sequence, committed.surface.index(), committed.surface.generation(), geometry.x, geometry.y, geometry.width, geometry.height,
                generation, scene.surface_has_visual_detail(surfaces, committed.surface), surfaces.len() > 32,
            );
        }
    }
}
