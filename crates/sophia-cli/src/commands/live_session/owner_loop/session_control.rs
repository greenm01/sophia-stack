macro_rules! service_session_controls {
    () => {{
        session_control_completions.clear();
        session_controls
            .service(
                control_sender,
                control_ack_receiver,
                Instant::now(),
                &mut session_control_completions,
            )
            .map_err(|error| format!("session control service failed: {error:?}"))?;
        for completion in session_control_completions.drain(..) {
            if let Some(failure) = completion.failure {
                return Err(format!(
                    "X Authority control {:?} failed for surface {:?}: {failure:?}",
                    completion.key.kind, completion.key.surface
                )
                .into());
            }
            if completion.key.kind == XAuthorityControlKind::FocusSurface
                && focus.focused_surface(seat) == Some(completion.key.surface)
            {
                applied_client_focus = Some(completion.key.surface);
                let _ = reduce_session_startup(
                    &mut startup_readiness,
                    SessionStartupEvent::PinSurface(completion.key.surface),
                );
                let _ = reduce_session_startup(
                    &mut startup_readiness,
                    SessionStartupEvent::ClientFocusApplied(completion.key.surface),
                );
                println!(
                    "sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control"
                );
            }
        }
    }};
}
