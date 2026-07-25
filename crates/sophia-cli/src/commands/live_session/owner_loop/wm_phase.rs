{
    if let Some(wm) = wm_session.as_mut() {
        let _ = wm.poll_restart(&layout, output)?;
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
}
