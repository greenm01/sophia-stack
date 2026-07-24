{
        let focused_client_ready = focus
            .focused_surface(seat)
            .is_some_and(|surface| applied_client_focus == Some(surface));
        let focused_content_ready = focus
            .focused_surface(seat)
            .is_some_and(|surface| input_content_surface == Some(surface));
        let cpu_baseline_presented = input_baseline_presented_before_wait
            || scene.last_report().is_some_and(|report| {
                report.nonzero_pixel_bytes > 0
                    && native_scanout.as_ref().is_none_or(|native| {
                        native.heads.first().is_some_and(|head| {
                            head.presented_checksum != 0 && head.nonzero_exports > 0
                        })
                    })
            });
        let input_baseline_presented =
            input_baseline_is_presented(focused_content_ready, cpu_baseline_presented);
        let input_start_stable = if config.inject_surface_resize.is_some() {
            resize_proof_complete
        } else if config.expect_physical_text.is_some() {
            layout.pending.is_none()
                && wm_session
                    .as_ref()
                    .is_none_or(|wm_session| wm_session.committed > 0)
        } else {
            last_authority_update.elapsed() >= Duration::from_millis(config.input_quiet_msec)
                || wm_session.as_ref().is_some_and(|wm| {
                    wm.last_committed_at.is_some_and(|committed| {
                        committed.elapsed() >= Duration::from_millis(config.input_quiet_msec)
                    })
                })
        };
        if require_startup_focus
            && physical_input.is_some()
            && input_baseline_presented
            && focus_deadline_started_at.is_none()
        {
            focus_deadline_started_at = Some(Instant::now());
        }
        if injection_checksum.is_none()
            && config.input_proof_requested()
            && input_baseline_presented
            && input_start_stable
            && focused_client_ready
            && focused_content_ready
            && (config.inject_surface_resize.is_none() || resize_proof_complete)
        {
            injection_checksum = scene.last_report().map(|report| report.checksum);
            input_change_submission_baseline = native_scanout
                .as_ref()
                .and_then(|native| native.heads.first())
                .map(|head| head.presented_submissions);
            input_surface = focus.focused_surface(seat);
            input_surface_generation = input_surface.and_then(|surface| {
                runtime.as_ref().and_then(|runtime| {
                    scene.surface_buffer_generation(runtime.committed_surfaces(), surface)
                })
            });
            if let Some(text) = config.inject_text.as_deref() {
                let events = synthetic_text_input_events(text)?;
                let expected = events.len();
                let runtime = runtime
                    .as_ref()
                    .ok_or("synthetic routed input requires an initialized runtime")?;
                let report = route_input_events(
                    events,
                    &focus,
                    runtime.committed_surfaces(),
                    runtime.input_layers(),
                    &layout.client_routes,
                    input_sender,
                    &mut modifiers,
                    &mut emergency_chord,
                    None,
                    &mut pointer,
                    false,
                    false,
                    false,
                    PhysicalInputRoutingMode::Full,
                    &mut next_input_delivery,
                    None,
                )?;
                if report.keys_routed != expected {
                    return Err(format!(
                        "synthetic input did not traverse committed Engine focus: expected={expected} routed={}",
                        report.keys_routed
                    )
                    .into());
                }
                input_events_expected =
                    input_events_expected.saturating_add(report.deliveries.len());
                pending_input_deliveries.extend(report.deliveries.iter().copied());
                input_delivery_wait_started_at = Some(Instant::now());
                input_delivery_source = Some("synthetic");
                input_batch_baseline = Some(metrics.batches);
                input_cpu_update_baseline = Some(metrics.cpu_buffer_updates);
                if !key_routed_reported {
                    println!(
                        "sophia_live_session_input_pipeline schema=1 status=key_routed source=synthetic"
                    );
                    std::io::stdout().flush()?;
                    key_routed_reported = true;
                }
            } else {
                input_batch_baseline = Some(metrics.batches);
                input_cpu_update_baseline = Some(metrics.cpu_buffer_updates);
                physical_input_ready_at = Some(Instant::now());
                println!(
                    "sophia_live_session_input schema=1 status=ready source=physical text={}",
                    config
                        .expect_physical_text
                        .as_deref()
                        .expect("checked above")
                );
                std::io::stdout().flush()?;
            }
        }
        if config.expect_physical_pointer
            && physical_input_completion_reported
            && input_pixel_change
            && pointer_phase_started_at.is_none()
            && pointer_cursor_checksum.is_none()
        {
            let runtime = runtime
                .as_ref()
                .ok_or("pointer proof became ready before the backend runtime")?;
            pointer
                .arm_at_focused_surface_center(focus.focused_surface(seat), runtime.input_layers())
                .ok_or("pointer proof has no focused application surface to place the cursor")?;
            cursor_dirty_since.get_or_insert_with(Instant::now);
            cursor_dirty = true;
        }
        if application_surface_missing_since
            .is_some_and(|started| started.elapsed() >= Duration::from_millis(500))
        {
            return Err(
                "application proof surface disappeared before the required physical pointer selection"
                    .into(),
            );
        }
        // Once the proof surface is gone, the session owns no narrower
        // deadline and the global runtime budget intentionally stays out of
        // input proofs. A toolkit that destroyed its window but never exits
        // would otherwise leave the loop presenting blank frames forever;
        // bound that wait and fail closed with the exact exit-term states.
        if sophia_cli::input_proof::application_exit_overdue(
            config.application_proof_requested(),
            application_surface_gone_at.is_some(),
            primary_child_exited,
        ) && application_surface_gone_at.is_some_and(|gone_at| {
            gone_at.elapsed() >= Duration::from_millis(SESSION_COMPLETION_TIMEOUT_MSEC)
        }) {
            return Err(format!(
                "persistent live session application surface was removed but the client did not exit: presented_latency={} text_match={} completion_reported={} pointer_pixels={} buttons_routed={} child_exited={}",
                input_presented_latency.is_some(),
                input_text_match,
                physical_input_completion_reported,
                pointer_pixel_change,
                metrics.physical_pointer_buttons_routed,
                primary_child_exited,
            )
            .into());
        }
        if execute_committed_session_actions(
            SessionActionExecutionContext {
                config,
                xauthority,
                children: secondary_children,
                layout: &layout,
                focus: &focus,
                seat,
                control_sender,
                control_ack_receiver,
            },
            &mut committed_session_actions,
        )? {
            logout_requested = true;
            let discarded = pending_input_deliveries.len();
            input_events_expected = input_events_expected.saturating_sub(discarded);
            pending_input_deliveries.clear();
            input_delivery_wait_started_at = None;
            if discarded != 0 {
                println!(
                    "sophia_live_session_input_pipeline schema=2 status=logout_discarded pending={discarded}"
                );
                std::io::stdout().flush()?;
            }
        }
        if logout_requested && pending_input_deliveries.is_empty() {
            break;
        }
        if input_presented_latency.is_none()
            && input_pixel_change
            && let Some(started) = input_proof_started_at
            && native_scanout.as_ref().is_none_or(|native| {
                input_change_submission_baseline.is_some_and(|baseline| {
                    native
                        .heads
                        .first()
                        .is_some_and(|head| head.presented_submissions > baseline)
                })
            })
        {
            input_presented_latency = Some(started.elapsed());
        }
        if let Some(runtime) = runtime.as_mut() {
            present_feedback.clear();
            runtime.drain_present_feedback_into(&mut present_feedback)?;
            for outcome in present_feedback.drain(..) {
                present_observer.observe_feedback(outcome);
            }
        }
        if (config.exit_after_input_proof || config.inject_text.is_some())
            && input_presented_latency.is_some()
            && input_text_match
            && (config.expect_physical_text.is_none() || physical_input_completion_reported)
            && (!config.expect_physical_pointer || pointer_pixel_change)
            && (!config.application_proof_requested() || primary_child_exited)
        {
            break;
        }
}
