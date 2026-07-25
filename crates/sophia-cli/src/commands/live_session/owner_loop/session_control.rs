{
macro_rules! service_session_controls {
    () => {{
        session_control_completions.clear();
        client_key_release_barrier
            .retain(|delivery| input_delivery.pending.contains(delivery));
        session_controls
            .service_when(
                control_sender,
                control_ack_receiver,
                Instant::now(),
                &mut session_control_completions,
                client_key_release_barrier.is_empty(),
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

macro_rules! flush_client_keys {
    ($surface:expr, $reason:expr) => {{
        let surface = $surface;
        let released = flush_client_pressed_keys(
            surface,
            &mut client_keys,
            &mut client_key_scratch,
            &mut client_key_deliveries,
            input_sender,
            &mut modifiers,
            &mut input_delivery.next,
            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        )?;
        input_delivery.events_expected = input_delivery
            .events_expected
            .saturating_add(client_key_deliveries.len());
        input_delivery
            .pending
            .extend(client_key_deliveries.iter().copied());
        client_key_release_barrier.extend(client_key_deliveries.iter().copied());
        if released != 0 {
            println!(
                "sophia_live_session_keys schema=1 status=released reason={} surface={} count={released}",
                $reason,
                surface.index(),
            );
        }
    }};
}

macro_rules! apply_wm_commit_result {
    ($result:expr, $previous_focus:expr) => {{
        let owner_commit = wm_session
            .as_mut()
            .ok_or("WM commit completed without a live WM session")?
            .apply_commit_result($result, $previous_focus, output.id)?;
        if let Some(action) = owner_commit.physical_action {
            println!(
                "sophia_live_wm schema=1 status=physical_action_committed action={}",
                action.raw(),
            );
        }
        if let Some(action) = owner_commit.session_action {
            committed_session_actions.push_back(action);
        }
        if let Some((transaction, surface)) = owner_commit.clear_focus {
            let client = layout
                .client_routes
                .client_for_surface(surface)
                .ok_or("hidden WM focus has no X11 client route")?;
            flush_client_keys!(surface, "clear_focus");
            session_controls
                .enqueue(
                    XAuthorityClientControlCommand {
                        client,
                        command: XAuthorityControlCommand::ClearFocus {
                            transaction,
                            surface,
                        },
                    },
                    Instant::now(),
                )
                .map_err(|error| {
                    format!("failed to queue hidden-focus clearing: {error:?}")
                })?;
            focus.clear_focus(seat);
            applied_client_focus = None;
            layout.focus_to_apply = None;
            println!(
                "sophia_live_wm schema=1 status=hidden_focus_cleared transaction={}",
                transaction.raw(),
            );
        }
        if let Some((transaction, surface)) = owner_commit.restore_focus {
            layout.focus_to_apply = Some((transaction, surface));
            println!(
                "sophia_live_wm schema=1 status=workspace_focus_restore_queued transaction={} surface={}",
                transaction.raw(),
                surface.index(),
            );
        }
        owner_commit.update
    }};
}

include!("physical_input_phase.rs")
}
