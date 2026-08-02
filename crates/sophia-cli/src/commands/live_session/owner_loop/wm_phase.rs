{
    if layout.constraint_relayout_required()
        && layout.pending.is_none()
        && let Some(wm) = wm_session.as_mut()
    {
        match wm.enqueue_relayout(&layout, output)? {
            LiveWmRequestAdmission::Admitted | LiveWmRequestAdmission::Duplicate => {
                layout.redrive_unmet_targets(&mut session_controls)?;
                layout.acknowledge_constraint_relayout();
            }
            LiveWmRequestAdmission::RejectedCapacity => {
                return Err("WM recovery-constraint relayout exceeded owner capacity".into());
            }
        }
    }
    if let Some(wm) = wm_session.as_mut() {
        let _ = wm.poll_restart(&layout, output)?;
    }
    if let Some(runtime) = runtime.as_mut() {
        let style = wm_session
            .as_ref()
            .and_then(|wm| wm.surface_chrome_style())
            .unwrap_or(config.surface_chrome_style);
        synchronize_runtime_surface_chrome_style(runtime, style);
    }
    if pending_wm_update.is_none()
        && layout.pending.is_none()
        && let Some(wm) = wm_session.as_mut()
        && let Some(proposal) = wm.poll_request(&layout, output)?
    {
        let previous_focus = focus.focused_surface(seat);
        if let Some(result) = layout.stage(proposal, &mut session_controls)? {
            pending_wm_update =
                Some(apply_wm_commit_result!(result, previous_focus));
        }
    }
    service_layout_progress!("wm_stage");
}
