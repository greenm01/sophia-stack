if !startup_ready_reported
    && startup_ready_deadline.is_some_and(|deadline| Instant::now() >= deadline)
{
    let missing_cpu_buffers = runtime.as_ref().map_or(0, |runtime| {
        scene.missing_committed_buffer_count(runtime.committed_surfaces())
    });
    let stage = if layout.pending.is_some() {
        "layout_pending"
    } else if layout.layers.is_empty() {
        "no_surface"
    } else if runtime
        .as_ref()
        .is_none_or(|runtime| runtime.committed_surfaces().is_empty())
    {
        "not_committed"
    } else if focus.focused_surface(seat).is_none() {
        "not_focused"
    } else if !startup_readiness.client_focus_applied {
        "focus_control_pending"
    } else if missing_cpu_buffers != 0 {
        "cpu_buffer_missing"
    } else if !startup_readiness.visual_detail {
        "no_visual_detail"
    } else {
        "not_presented"
    };
    if let Some(native) = native_scanout.as_ref() {
        for head in &native.heads {
            if let Some(report) = head.last_submit_report {
                eprintln!("{}", report.reduced_log_line());
            }
        }
    }
    eprintln!(
        "sophia_live_session_startup schema=3 status=failed stage={stage} elapsed_msec={} authority_batches={batches} transactions={transactions} cpu_buffer_updates={cpu_buffer_updates} cpu_compositions={cpu_compositions} cpu_buffers_resident={} cpu_buffer_bytes={} cpu_buffers_missing={} staged_cpu_buffers={} layout_surfaces={} runtime_surfaces={runtime_surfaces} focus={} focus_control_ready={focused_client_ready} retired_present_surfaces={} dma_buf_registrations={dma_buf_registrations_observed} fence_registrations={fence_registrations_observed} present_submissions={present_submissions_observed} software_present_submissions={software_present_submissions_observed} native_submissions={} native_submit_failures={} native_retirements={} native_callbacks={} native_state={} protocol_errors={protocol_error_count}",
        started.elapsed().as_millis(),
        scene.resident_buffer_count(),
        scene.resident_buffer_bytes(),
        missing_cpu_buffers,
        staged_cpu_buffer_handles.len(),
        layout.layers.len(),
        focus.focused_surface(seat).is_some(),
        retired_present_surfaces.len(),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.submissions),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.submit_failures),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.retirements),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.callback_accepted),
        runtime.as_ref().map_or_else(
            || "none".to_owned(),
            LiveProductionVisualRuntime::native_diagnostic
        ),
        batches = metrics.batches,
        transactions = metrics.transactions,
        cpu_buffer_updates = metrics.cpu_buffer_updates,
        cpu_compositions = metrics.cpu_compositions,
        runtime_surfaces = metrics.runtime_surfaces,
        dma_buf_registrations_observed = metrics.dma_buf_registrations_observed,
        fence_registrations_observed = metrics.fence_registrations_observed,
        present_submissions_observed = metrics.present_submissions_observed,
        software_present_submissions_observed = metrics.software_present_submissions_observed,
        protocol_error_count = metrics.protocol_error_count,
    );
    return Err(format!(
        "startup application was not visibly presented within {} milliseconds: stage={stage}",
        config
            .startup_ready_timeout
            .expect("startup deadline requires a timeout")
            .as_millis()
    )
    .into());
}
