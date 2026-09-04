{
startup_native_recovery_attempted = true;
let mut current = native_scanout
    .take()
    .ok_or("startup native recovery lost the active scanout")?;
let suspended = runtime
    .as_mut()
    .ok_or("startup native recovery lost the visual runtime")?
    .suspend_native_scanout(&mut current, &outputs, Duration::from_millis(100))?;
let renderer_handoff = capture_renderer_image_handoff(
    runtime
        .as_ref()
        .ok_or("startup native recovery lost the visual runtime")?,
    &mut current,
    output.id,
)?;
drop(current);
let mut replacement = LiveProductionNativeScanout::new_with_seat_mirroring_mapping_and_cursor(
    &seat_controller
        .as_ref()
        .ok_or("startup native recovery lost the seat controller")?
        .device_opener(),
    mirror_grouping,
    initial_head_mapping,
    config.cursor_resolution.asset.clone(),
)?;
if replacement.outputs() != outputs {
    suspended_renderer_images = Some(renderer_handoff);
    schedule_output_topology_rebuild!("startup_recovery", false);
    startup_topology_recovery_pending = true;
    drop(replacement);
    tracing::warn!(
        "sophia_live_session_startup schema=3 status=recovery_deferred reason=output_topology_changed"
    );
} else {
    let runtime = runtime
        .as_mut()
        .ok_or("startup native recovery lost the visual runtime")?;
    let restored_renderer_images = resume_native_scanout_from_scene(
        runtime,
        &mut replacement,
        &outputs,
        &mut scene,
        Some(renderer_handoff),
    )?;
    *native_scanout = Some(replacement);
    let _ = reduce_session_startup(
        &mut startup_readiness,
        SessionStartupEvent::NativeRecovered,
    );
    crate::session_println!(
        "sophia_live_session_startup schema=3 status=recovered attempt=1 reason={} outcome={} drained={} abandoned_scanouts={}",
        recovery_reason
            .expect("startup recovery requires an admitted transport reason")
            .reduced_name(),
        suspended.outcome.reduced_name(),
        suspended.outcome.drained(),
        suspended.abandoned_scanouts,
    );
    crate::session_println!(
        "sophia_live_renderer_handoff schema=1 status=restored images={restored_renderer_images} source=startup_recovery"
    );
    std::io::stdout().flush()?;
}
retired_present_surfaces.clear();
startup_surface_presentations.clear();
startup_content_ready = false;
startup_required_submissions = None;
input_content_surface = None;
startup_outputs_ready_reported = false;
}
