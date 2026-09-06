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
                if failure.is_stale_target_for(completion.key.kind) {
                    if applied_client_focus == Some(completion.key.surface) {
                        applied_client_focus = None;
                    }
                    crate::session_println!(
                        "sophia_live_session_control schema=1 status=stale_target_retired kind={:?} transaction={} surface={}",
                        completion.key.kind,
                        completion.key.transaction.raw(),
                        completion.key.surface.index(),
                    );
                    continue;
                }
                return Err(format!(
                    "X Authority control {:?} failed for surface {:?}: {failure:?} (queue_dwell_msec={} acknowledgement_latency_msec={})",
                    completion.key.kind,
                    completion.key.surface,
                    completion.queue_dwell.as_millis(),
                    completion.acknowledgement_latency.as_millis(),
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
                crate::session_println!(
                    "sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control"
                );
            }
            if completion.key.kind == XAuthorityControlKind::ConfigureSurface
            {
                crate::session_println!(
                    "sophia_live_surface_geometry schema=1 status=frontend_configured transaction={} surface={}",
                    completion.key.transaction.raw(),
                    completion.key.surface.index(),
                );
            }
            if completion.key.kind == XAuthorityControlKind::SetPresentationState
                && layout.acknowledge_presentation_control(
                    completion.key.transaction,
                    completion.key.surface,
                )
            {
                crate::session_println!(
                    "sophia_live_surface_presentation schema=1 status=frontend_configured transaction={} surface={}",
                    completion.key.transaction.raw(),
                    completion.key.surface.index(),
                );
            }
            if completion.key.kind == XAuthorityControlKind::ConfigureSurface
                && layout.layout_epochs.acknowledge_recovery_configure(
                    completion.key.transaction,
                    completion.key.surface,
                )
            {
                crate::session_println!(
                    "sophia_live_resize_epoch schema=1 status=recovery_configure_acknowledged transaction={} surface={}",
                    completion.key.transaction.raw(),
                    completion.key.surface.index(),
                );
            }
            if completion.key.kind == XAuthorityControlKind::AdmitSurface {
                let acknowledged = layout.acknowledge_admission_control(
                    completion.key.transaction,
                    completion.key.surface,
                );
                if acknowledged {
                    crate::session_println!(
                        "sophia_live_surface_admission schema=1 status=frontend_admitted transaction={} surface={}",
                        completion.key.transaction.raw(),
                        completion.key.surface.index(),
                    );
                }
            }
        }
        service_layout_progress!("control");
    }};
}

macro_rules! service_core_config_reload {
    () => {{
        if let Some(watcher) = config_watcher.as_ref() {
            while watcher.try_recv().is_ok() {
                config_reload_pending = true;
            }
        }
        let wm_shortcuts_idle = wm_session
            .as_ref()
            .and_then(|wm| wm.shortcuts.as_ref())
            .is_none_or(WmShortcutRouter::shortcut_idle);
        let input_idle = client_keys.pending_len() == 0
            && input_delivery.pending.is_empty()
            && wm_shortcuts_idle;
        if config_reload_pending && input_idle {
            config_reload_pending = false;
            let path = config
                .core_config_source
                .path
                .as_deref()
                .expect("only file-backed config creates a watcher");
            match sophia_config::read_config_file(path) {
                Ok(bytes) => match config.core_config_state.reload(&bytes) {
                    Ok(report)
                        if report.disposition
                            == sophia_config::ReloadDisposition::Applied =>
                    {
                        let snapshot = config.core_config_state.active().clone();
                        config.applications =
                            PersistentXtermSessionConfig::applications_from_core(&snapshot)?;
                        config.key_repeat_config = snapshot.input.repeat;
                        config.verbose_diagnostics = snapshot.verbose_diagnostics;
                        let repeat = KeyRepeatConfig::new(
                            snapshot.input.repeat.delay_msec,
                            snapshot.input.repeat.interval_msec,
                        )
                        .ok_or("KDL2 key repeat controls must be nonzero")?;
                        key_repeat.cancel_seat(seat);
                        key_repeat = KeyRepeatState::new(repeat);
                        config.surface_chrome_style =
                            PersistentXtermSessionConfig::surface_chrome_style(
                                snapshot.fallback_chrome,
                            );
                        if let Some(wm) = wm_session.as_mut() {
                            wm.set_fallback_chrome(config.surface_chrome_style);
                        }
                        if let Some(runtime) = runtime.as_mut() {
                            let style = wm_session
                                .as_ref()
                                .and_then(|wm| wm.surface_chrome_style())
                                .unwrap_or(config.surface_chrome_style);
                            runtime.set_surface_chrome_style(style);
                        }
                        if report.delta.cursor_changed {
                            // The desktop profile still wins per key, so this
                            // may resolve to the cursor already on screen --
                            // which is why the asset is compared rather than
                            // the config that produced it.
                            match config.reload_cursor(&snapshot.cursor) {
                                Ok(Some(asset)) => {
                                    scene.set_cursor_asset(asset.clone());
                                    if let Some(native) = native_scanout.as_mut()
                                        && let Err(error) =
                                            native.replace_hardware_cursor_asset(asset)
                                    {
                                        crate::session_eprintln!(
                                            "sophia_live_cursor schema=1 status=reload_declined detail={error}"
                                        );
                                    }
                                    crate::session_println!(
                                        "sophia_live_cursor schema=1 status=reloaded theme={} size={}",
                                        config.cursor_resolution.effective_theme,
                                        config.cursor_resolution.effective_nominal_size,
                                    );
                                }
                                Ok(None) => crate::session_println!(
                                    "sophia_live_cursor schema=1 status=reload_unchanged reason=profile_overrides"
                                ),
                                Err(error) => crate::session_eprintln!(
                                    "sophia_live_cursor schema=1 status=reload_declined detail={error}"
                                ),
                            }
                        }
                        if config.verbose_diagnostics {
                            crate::session_println!(
                                "sophia_config_reload_detail schema=2 source={:?} pending_restart=false applications={} repeat_delay_ms={} repeat_interval_ms={} chrome_clearance={}",
                                config.core_config_source.class,
                                config.applications.applications.len(),
                                config.key_repeat_config.delay_msec,
                                config.key_repeat_config.interval_msec,
                                config.surface_chrome_style.clearance(),
                            );
                        }
                        crate::session_println!(
                            "sophia_config_reload schema=2 status=applied generation={} digest={} applications_changed={} repeat_changed={} chrome_changed={} cursor_changed={} diagnostics_changed={}",
                            report.generation.raw(),
                            snapshot.digest,
                            report.delta.applications_changed,
                            report.delta.repeat_changed,
                            report.delta.chrome_changed,
                            report.delta.cursor_changed,
                            report.delta.diagnostics_changed,
                        );
                    }
                    Ok(report)
                        if report.disposition
                            == sophia_config::ReloadDisposition::PendingRestart =>
                    {
                        let pending = config
                            .core_config_state
                            .pending_restart()
                            .expect("pending restart disposition retains candidate");
                        crate::session_println!(
                            "sophia_config_reload schema=2 status=pending_restart generation={} digest={} cursor_changed={} restart_required={}",
                            report.generation.raw(),
                            pending.digest,
                            report.delta.cursor_changed,
                            report.delta.restart_required,
                        );
                    }
                    Ok(report) => {
                        crate::session_println!(
                            "sophia_config_reload schema=1 status=unchanged generation={}",
                            report.generation.raw(),
                        );
                    }
                    Err(error) => {
                        crate::session_eprintln!(
                            "sophia_config_reload schema=1 status=rejected reason=parse error={error}"
                        );
                    }
                },
                Err(error) => {
                    crate::session_eprintln!(
                        "sophia_config_reload schema=1 status=rejected reason=read error={error}"
                    );
                }
            }
            std::io::stdout().flush()?;
        }
    }};
}

macro_rules! track_client_key_flush {
    ($released:expr, $reason:expr, $scope_field:literal, $scope_value:expr) => {{
        let released = $released;
        input_delivery.events_expected = input_delivery
            .events_expected
            .saturating_add(client_key_deliveries.len());
        input_delivery
            .pending
            .extend(client_key_deliveries.iter().copied());
        client_key_release_barrier.extend(client_key_deliveries.iter().copied());
        if released != 0 {
            crate::session_println!(
                concat!(
                    "sophia_live_session_keys schema=1 status=released reason={} ",
                    $scope_field,
                    "={} count={}"
                ),
                $reason,
                $scope_value,
                released,
            );
        }
    }};
}

macro_rules! flush_client_keys {
    ($surface:expr, $reason:expr) => {{
        let surface = $surface;
        key_repeat.cancel_surface(surface);
        let released = flush_client_pressed_keys(
            surface,
            &mut client_keys,
            &mut client_key_scratch,
            &mut client_key_deliveries,
            input_sender,
            &mut routed_input_saturation,
            &mut modifiers,
            &mut input_delivery.next,
            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        )?;
        track_client_key_flush!(released, $reason, "surface", surface.index());
    }};
}

macro_rules! flush_all_client_keys {
    ($reason:expr) => {{
        key_repeat.cancel_seat(seat);
        let released = flush_all_client_pressed_keys(
            &mut client_keys,
            &mut client_key_scratch,
            &mut client_key_deliveries,
            input_sender,
            &mut routed_input_saturation,
            &mut modifiers,
            &mut input_delivery.next,
            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        )?;
        track_client_key_flush!(released, $reason, "scope", "all");
    }};
}

macro_rules! service_runtime_deadline_key_drain {
    () => {{
        let now_msec = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        match runtime_deadline_key_drain.observe(
            now_msec,
            client_keys.pending_len(),
            input_delivery.pending.len(),
            client_key_release_barrier.len(),
            wm_session
                .as_ref()
                .map_or(0, LiveWmSession::in_flight_request_count),
        ) {
            RuntimeDeadlineKeyDrainDecision::BeginRelease => {
                flush_all_client_keys!("runtime_deadline");
                crate::session_println!(
                    "sophia_live_session_keys schema=3 status=deadline_release pending_deliveries={} release_barrier_pending={}",
                    input_delivery.pending.len(),
                    client_key_release_barrier.len(),
                );
                continue;
            }
            RuntimeDeadlineKeyDrainDecision::Waiting => {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            RuntimeDeadlineKeyDrainDecision::Complete => {
                if runtime_deadline_key_drain.is_draining() {
                    crate::session_println!(
                        "sophia_live_session_keys schema=3 status=deadline_drained pending=0 release_barrier_pending=0"
                    );
                }
                begin_session_quiescence!("runtime_deadline");
            }
            RuntimeDeadlineKeyDrainDecision::AbandonedPolicyRequests(requests) => {
                crate::session_println!(
                    "sophia_live_session_keys schema=3 status=deadline_drained pending=0 release_barrier_pending=0 abandoned_policy_requests={requests}"
                );
                begin_session_quiescence!("runtime_deadline");
            }
            RuntimeDeadlineKeyDrainDecision::TimedOut => {
                return Err(format!(
                    "runtime deadline key-release barrier timed out: pressed={} pending_deliveries={} release_barrier_pending={} policy_requests={}",
                    client_keys.pending_len(),
                    input_delivery.pending.len(),
                    client_key_release_barrier.len(),
                    wm_session
                        .as_ref()
                        .map_or(0, LiveWmSession::in_flight_request_count),
                )
                .into());
            }
        }
    }};
}

macro_rules! reconcile_pending_wm_focus {
    ($runtime:expr) => {{
        if let Some((transaction, surface)) = layout.focus_to_apply {
            let decision = focus.focus_surface(seat, surface, $runtime.committed_surfaces());
            layout.focus_to_apply =
                pending_wm_focus_after_engine_decision((transaction, surface), decision);
            match decision {
                InputFocusDecision::Focused => {
                    if wm_session.is_some() {
                        if let Some(previous) = applied_client_focus
                            && previous != surface
                        {
                            keyboard_focus_handoff = KeyboardFocusHandoffState::default();
                            deferred_physical_key_timings.clear();
                            flush_client_keys!(previous, "focus_handoff");
                        }
                        let client = layout
                            .client_routes
                            .client_for_surface(surface)
                            .ok_or("WM focus has no X11 client route")?;
                        session_controls
                            .enqueue(
                                XAuthorityClientControlCommand {
                                    client,
                                    command: XAuthorityControlCommand::FocusSurface {
                                        transaction,
                                        surface,
                                    },
                                },
                                Instant::now(),
                            )
                            .map_err(|error| {
                                format!("failed to queue WM focus reconciliation: {error:?}")
                            })?;
                    }
                    let _ = reduce_session_startup(
                        &mut startup_readiness,
                        SessionStartupEvent::PinSurface(surface),
                    );
                    crate::session_println!(
                        "sophia_live_wm schema=1 status=focus_reconciled transaction={} target=surface surface={surface:?} outcome={decision:?}",
                        transaction.raw()
                    );
                    crate::session_println!(
                        "sophia_live_wm schema=1 status=focus_committed transaction={} target=surface",
                        transaction.raw()
                    );
                }
                // The seat already holds this surface. Startup readiness still
                // counts it, because a replayed focus after a policy restart pins
                // the same surface it would have pinned. Nothing is sent to the
                // client and no handoff runs: no focus moved, so there is no
                // keyboard sequence to flush and nothing for the client to learn.
                // `focus_committed` stays unprinted -- physical verifiers read it
                // as evidence that focus actually changed.
                InputFocusDecision::AlreadyFocused => {
                    let _ = reduce_session_startup(
                        &mut startup_readiness,
                        SessionStartupEvent::PinSurface(surface),
                    );
                    crate::session_println!(
                        "sophia_live_wm schema=1 status=focus_reconciled transaction={} target=surface surface={surface:?} outcome={decision:?}",
                        transaction.raw()
                    );
                }
                InputFocusDecision::UnknownSurface => {}
                InputFocusDecision::InvalidSeat => {
                    return Err("WM focus reconciliation used an invalid seat".into());
                }
            }
        }
        if !focus_ready_reported && focus.focused_surface(seat).is_some() {
            crate::session_println!("sophia_live_session_input_pipeline schema=1 status=focus_ready");
            std::io::stdout().flush()?;
            focus_ready_reported = true;
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
            crate::session_println!(
                "sophia_live_wm schema=1 status=physical_action_committed action={}",
                action.raw(),
            );
        }
        if let Some(mode) = owner_commit.pointer_gesture {
            crate::session_println!(
                "sophia_live_wm schema=4 status=pointer_gesture_committed mode={}",
                match mode {
                    sophia_protocol::WmPointerGestureMode::Move => "move",
                    sophia_protocol::WmPointerGestureMode::Resize => "resize",
                },
            );
        }
        if let Some(action) = owner_commit.session_action {
            committed_session_actions.push_back(action);
        }
        if let Some(projection) = owner_commit.workspace_projection {
            crate::session_println!(
                "sophia_live_wm schema=2 status=workspace_projection_committed transaction={} output={} workspace={} visible_surfaces={} focus={}",
                projection.transaction.raw(),
                projection.output.raw(),
                projection.workspace.raw(),
                projection.visible_surfaces,
                if projection.focus_present { "surface" } else { "none" },
            );
            if let Some((transaction, surface)) = layout.focus_to_apply
                && transaction == projection.transaction
            {
                crate::session_println!(
                    "sophia_live_wm schema=1 status=workspace_focus_restore_queued transaction={} surface={}",
                    transaction.raw(),
                    surface.index(),
                );
            }
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
            keyboard_focus_handoff = KeyboardFocusHandoffState::default();
            deferred_physical_key_timings.clear();
            layout.focus_to_apply = None;
            crate::session_println!(
                "sophia_live_wm schema=1 status=hidden_focus_cleared transaction={}",
                transaction.raw(),
            );
        }
        owner_commit.update
    }};
}

macro_rules! service_layout_progress {
    ($trigger:literal) => {{
        if pending_wm_update.is_none()
            && wm_session
                .as_ref()
                .is_some_and(LiveWmSession::public_settlement_abort_required)
            && let Some(result) = layout.expire_pending(&mut session_controls)?
        {
            let transaction = result.update.commit.transaction;
            pending_wm_update = Some(apply_wm_commit_result!(
                result,
                focus.focused_surface(seat)
            ));
            layout_progress_deferred_reported = false;
            crate::session_println!(
                "sophia_live_layout_progress schema=1 status=aborted trigger={} transaction={} reason=public_transport_lost preserved_layout=true",
                $trigger,
                transaction.raw(),
            );
        }
        if pending_wm_update.is_none() && layout.pending_is_ready() {
            if let Some(wm) = wm_session.as_mut() {
                if !wm.prepare_public_layout_commit(&layout)? {
                    // Reservations can advance the canonical scene while a client
                    // answers a resize. Retire the old epoch through normal recovery
                    // before asking policy for a projection against the new work area.
                    layout.force_pending_timeout();
                    if let Some(result) = layout.expire_pending(&mut session_controls)? {
                        pending_wm_update = Some(apply_wm_commit_result!(
                            result,
                            focus.focused_surface(seat)
                        ));
                    }
                } else if wm.trigger_public_proof_fault(PublicPolicyFaultPoint::Prepared) {
                    let _ = wm.poll_restart(&mut layout, output)?;
                }
            }
        }
        if pending_wm_update.is_none()
            && wm_session
                .as_ref()
                .is_some_and(LiveWmSession::public_settlement_abort_required)
            && let Some(result) = layout.expire_pending(&mut session_controls)?
        {
            let transaction = result.update.commit.transaction;
            pending_wm_update = Some(apply_wm_commit_result!(
                result,
                focus.focused_surface(seat)
            ));
            layout_progress_deferred_reported = false;
            crate::session_println!(
                "sophia_live_layout_progress schema=1 status=aborted trigger={} transaction={} reason=public_transport_lost preserved_layout=true",
                $trigger,
                transaction.raw(),
            );
        }
        match reconcile_live_layout_progress(&mut layout, pending_wm_update.is_none()) {
            LiveLayoutProgress::Committed(result) => {
                let transaction = result.update.commit.transaction;
                pending_wm_update = Some(apply_wm_commit_result!(
                    result,
                    focus.focused_surface(seat)
                ));
                layout_progress_deferred_reported = false;
                crate::session_println!(
                    "sophia_live_layout_progress schema=1 status=committed trigger={} transaction={}",
                    $trigger,
                    transaction.raw(),
                );
            }
            LiveLayoutProgress::DeferredReady => {
                if !layout_progress_deferred_reported {
                    crate::session_println!(
                        "sophia_live_layout_progress schema=1 status=deferred trigger={} reason=wm_update_pending",
                        $trigger,
                    );
                    layout_progress_deferred_reported = true;
                }
            }
            LiveLayoutProgress::Blocked => {
                layout_progress_deferred_reported = false;
            }
        }
    }};
}

include!("physical_input_phase.rs")
}
