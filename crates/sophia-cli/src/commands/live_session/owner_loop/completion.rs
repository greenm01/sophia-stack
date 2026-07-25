{
    let SessionLoopMetrics {
        batches,
        transactions,
        cpu_buffer_updates: _,
        dma_buf_registrations_observed: _,
        fence_registrations_observed: _,
        present_submissions_observed: _,
        cpu_compositions,
        coalesced_batches,
        backend_ticks,
        runtime_committed,
        runtime_surfaces,
        physical_events,
        physical_keys_routed,
        physical_pointer_events,
        physical_pointer_routed,
        physical_pointer_buttons_routed,
        session_ticks,
        max_compose,
        protocol_error_count,
        expected_protocol_error_count,
        cursor_moves_coalesced,
        cursor_max_motion_to_submit,
    } = metrics;

    if let (Some(runtime), Some(native_scanout)) = (runtime.as_mut(), native_scanout.as_mut()) {
        let report =
            runtime.suspend_native_scanout(native_scanout, &outputs, Duration::from_secs(2))?;
        println!(
            "sophia_live_session_native_suspend schema=2 outcome={} drained={} abandoned_scanouts={} skipped_present={}",
            report.outcome.reduced_name(),
            report.outcome.drained(),
            report.abandoned_scanouts,
            report
                .skipped_present
                .map_or_else(|| "none".to_owned(), |transaction| transaction.raw().to_string()),
        );
    }
    if let Some(runtime) = runtime.as_mut() {
        let report = runtime.shutdown_presentations();
        present_feedback.clear();
        runtime.drain_present_feedback_into(&mut present_feedback)?;
        for outcome in present_feedback.drain(..) {
            present_observer.observe_feedback(outcome);
        }
        present_observer.observe_disconnect(report);
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

    let report = scene
        .last_report()
        .ok_or("persistent live session received no composable X pixels")?;
    if config.input_proof_requested()
        && input_delivery.events_expected != input_delivery.events_flushed
    {
        return Err(format!(
            "persistent live session completed with unflushed X11 input: expected={} flushed={} pending={}",
            input_delivery.events_expected,
            input_delivery.events_flushed,
            input_delivery.pending.len(),
        )
        .into());
    }
    if config.input_proof_requested() && input_delivery.flush_latency.is_none() {
        return Err("persistent live session input proof never observed flushed X11 input".into());
    }
    if config.input_proof_requested() && !input_pixel_change {
        return Err(format!(
            "persistent live session input did not change composed terminal pixels: baseline={injection_checksum:?} final_frame={} final_buffers={} input_surface={input_surface:?} input_surface_pixel_change={input_surface_pixel_change} batches={batches} transactions={transactions}",
            report.checksum,
            scene.buffer_checksum(),
        )
        .into());
    }
    if config.input_proof_requested() && input_presented_latency.is_none() {
        let native_heads = runtime.as_ref().map_or_else(
            || "none".to_owned(),
            LiveProductionVisualRuntime::native_diagnostic,
        );
        return Err(format!(
            "persistent live session input pixels were not presented: change_submission_baseline={input_change_submission_baseline:?} primary_presented_submissions={} native_submissions={} native_callbacks={} native_heads={native_heads}",
            native_scanout
                .as_ref()
                .and_then(|native| native.heads.first())
                .map_or(0, |head| head.presented_submissions),
            native_scanout.as_ref().map_or(0, |native| native.submissions),
            native_scanout
                .as_ref()
                .map_or(0, |native| native.callback_accepted),
        )
        .into());
    }
    if config.input_proof_requested() && !input_text_match {
        return Err(
            "persistent live session terminal did not receive the expected text and Return".into(),
        );
    }
    if config.expect_physical_text.is_some()
        && (!physical_text_proof
            .as_ref()
            .is_some_and(PhysicalTextProof::is_complete)
            || !physical_input_completion_reported)
    {
        return Err("persistent live session did not complete exact physical text proof".into());
    }
    if config.expect_physical_pointer
        && (!pointer_pixel_change || physical_pointer_buttons_routed == 0)
    {
        return Err(format!(
            "persistent live session pointer input did not change pixels: baseline={pointer_checksum:?} routed={physical_pointer_routed} buttons={physical_pointer_buttons_routed} observed={physical_pointer_events}"
        )
        .into());
    }
    if config.application_proof_requested() {
        let status =
            primary_exit_status.ok_or("application proof ended before the client exited")?;
        if config.require_client_normal_exit && !status.success() {
            return Err(format!("application did not exit normally: {status}").into());
        }
        if let Some(expected) = config.expect_client_stdout.as_deref()
            && client_stdout != expected.as_bytes()
        {
            return Err(format!(
                "application stdout mismatch: expected_bytes={} received_bytes={}",
                expected.len(),
                client_stdout.len()
            )
            .into());
        }
        if session_protocol_errors_are_fatal(false, true, protocol_error_count) {
            return Err(format!("application emitted {protocol_error_count} X protocol errors; first={first_protocol_error:?}").into());
        }
    }
    if session_protocol_errors_are_fatal(
        config.normal_session,
        config.application_proof_requested(),
        protocol_error_count,
    ) {
        return Err(format!(
            "normal session emitted {protocol_error_count} X protocol errors; first={first_protocol_error:?}"
        )
        .into());
    }
    if config.firefox_m8_proof {
        if !firefox_m8_proof.complete()
            || firefox_m8_selection_owner_changes < 2
            || firefox_m8_selection_conversions < 2
        {
            return Err(format!(
                "Firefox M8 proof incomplete: stages={}/{} selection_owner_changes={} selection_conversions={}",
                firefox_m8_proof.completed_stage,
                FirefoxM8StageProof::STAGES.len(),
                firefox_m8_selection_owner_changes,
                firefox_m8_selection_conversions,
            )
            .into());
        }
        println!(
            "sophia_firefox_m8 schema=1 status=complete stages={} selection_owner_changes={} selection_conversions={} content=redacted",
            firefox_m8_proof.completed_stage,
            firefox_m8_selection_owner_changes,
            firefox_m8_selection_conversions,
        );
    }
    if config.inject_surface_resize.is_some() && !resize_proof_complete {
        return Err(
            "persistent live session did not commit configured surface resize pixels".into(),
        );
    }
    if let Some(wm_session) = wm_session.as_ref()
        && wm_session.committed == 0
    {
        return Err("live session ended without a committed external WM layout".into());
    }
    if config.normal_session
        && (layout.pending.is_some()
            || !committed_session_actions.is_empty()
            || session_launches.pending_len() != 0
            || session_launches.admission().is_some()
            || !input_delivery.pending.is_empty()
            || wm_session.as_ref().is_some_and(|wm| wm.degraded))
    {
        return Err(format!(
            "normal session ended with pending work: wm={} actions={} launches={} admission={} input={} degraded={}",
            usize::from(layout.pending.is_some()),
            committed_session_actions.len(),
            session_launches.pending_len(),
            usize::from(session_launches.admission().is_some()),
            input_delivery.pending.len(),
            wm_session.as_ref().is_some_and(|wm| wm.degraded),
        )
        .into());
    }
    println!(
        "sophia_session_launches schema=1 status=complete peak_depth={} rejected={} admission_timeouts={}",
        session_launches.peak_depth(),
        session_launches.rejected(),
        session_launches.timed_out(),
    );
    let input_stats = physical_input
        .as_ref()
        .map_or_else(Default::default, |input| input.stats());
    if let Some(input) = physical_input.as_ref() {
        let policy = input.policy_report();
        println!(
            "sophia_live_session_input_devices schema=1 source={} added={} removed={} active={} keyboards={} pointers={} touch={}",
            if policy.udev_managed { "udev" } else { "paths" },
            policy.devices_added,
            policy.devices_removed,
            policy.active_devices,
            policy.keyboards,
            policy.pointers,
            policy.touch_devices,
        );
    }
    let (
        native_target_creations,
        native_target_recreations,
        native_pipeline_creations,
        native_uploads,
        native_max_upload,
    ) = native_scanout.as_ref().map_or(
        (0, 0, 0, 0, Duration::ZERO),
        LiveProductionNativeScanout::persistent_render_metrics,
    );
    println!(
        "sophia_live_session_scheduler schema=1 authority_batches={batches} cpu_compositions={cpu_compositions} coalesced_batches={coalesced_batches}"
    );
    println!(
        "sophia_live_session_cursor schema=2 moves_coalesced={} max_motion_to_submit_msec={} buttons_routed={} hardware_updates={} hardware_failures={}",
        cursor_moves_coalesced,
        cursor_max_motion_to_submit.as_millis(),
        physical_pointer_buttons_routed,
        native_scanout
            .as_ref()
            .map_or(0, |scanout| scanout.cursor_updates),
        native_scanout
            .as_ref()
            .map_or(0, |scanout| scanout.cursor_update_failures),
    );
    println!(
        "sophia_live_session_health schema=1 status=clean protocol_errors={} pending_wm={} pending_actions={} pending_input={} wm_degraded={}",
        protocol_error_count,
        usize::from(layout.pending.is_some()),
        committed_session_actions.len(),
        input_delivery.pending.len(),
        wm_session.as_ref().is_some_and(|wm| wm.degraded),
    );
    println!(
        "sophia_live_session_protocol_errors schema=1 expected={} unexpected={}",
        expected_protocol_error_count, protocol_error_count,
    );

    let present_observation = &present_observer;
    println!(
        "sophia_live_session schema=14 status=bounded_complete display={} elapsed_msec={} startup_ready_msec={} session_ticks={} authority_batches={} authority_transactions={} authority_queue_capacity={} authority_batches_dropped=0 backend_ticks={} runtime_committed={} runtime_surfaces={} cpu_layers={} cpu_nonzero_pixel_bytes={} cpu_max_nonzero_pixel_bytes={} cpu_nonzero_frames={} cpu_checksum={} cpu_max_compose_msec={} injected_input={} input_events_expected={} input_events_flushed={} input_flush_latency_msec={} input_pixel_change={} input_text_match={} input_presented_latency_msec={} input_dispatch_max_gap_msec={} input_queue_max_depth={} input_queue_dwell_max_msec={} physical_events={} physical_keys_routed={} pointer_pixel_change={} physical_pointer_events={} physical_pointer_routed={} pointer_proof={} native_presentation={} native_submissions={} native_submit_deferred={} native_submit_failures={} native_retirements={} native_retire_failures={} native_max_in_flight_ticks={} native_max_submit_to_page_flip_msec={} native_max_upload_msec={} native_target_creations={} native_target_recreations={} native_pipeline_creations={} native_frame_uploads={} native_callback_accepted={} native_callback_rejected={} native_callback_queue_saturated={} native_nonzero_exports={} native_mixed_exports={} native_export_attempts={} native_in_flight={} native_cleanup_pending={} physical_input={} wm_policy={} wm_requests={} wm_committed={} wm_restarts={} wm_degraded={} namespace_profile={} output_update={} output_notifications={} surface_resize={} present_complete_flip={} present_complete_skip={} present_idle={} present_idle_fence_triggers={} present_disconnect_sources={} present_disconnect_fences={} present_disconnect_failures={} present_live_sources={} present_live_fences={} present_live_transactions={} present_acquire_waits={} present_controlled_rejections={}",
        config.display,
        started.elapsed().as_millis(),
        startup_ready_msec.ok_or("persistent live session never reached startup readiness")?,
        session_ticks,
        batches,
        transactions,
        SESSION_AUTHORITY_CAPACITY,
        backend_ticks,
        runtime_committed,
        runtime_surfaces,
        report.layers_composed,
        report.nonzero_pixel_bytes,
        scene.max_nonzero_pixel_bytes(),
        scene.nonzero_frames(),
        report.checksum,
        max_compose.as_millis(),
        config.inject_text.is_some(),
        input_delivery.events_expected,
        input_delivery.events_flushed,
        input_delivery
            .flush_latency
            .map_or(0, |duration| duration.as_millis()),
        input_pixel_change,
        input_text_match,
        input_presented_latency
            .map(|latency| latency.as_millis().to_string())
            .unwrap_or_else(|| "none".to_owned()),
        input_stats.max_dispatch_gap_msec,
        input_stats.max_queue_depth,
        input_stats.max_queue_dwell_msec,
        physical_events,
        physical_keys_routed,
        pointer_pixel_change,
        physical_pointer_events,
        physical_pointer_routed,
        if config.expect_physical_pointer {
            "enabled"
        } else {
            "disabled"
        },
        if native_scanout.is_some() {
            "enabled"
        } else {
            "disabled"
        },
        native_scanout
            .as_ref()
            .map_or(0, |native| native.submissions),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.submit_deferred),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.submit_failures),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.retirements),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.retire_failures),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.max_in_flight_ticks),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.max_submit_to_page_flip.as_millis()),
        native_max_upload.as_millis(),
        native_target_creations,
        native_target_recreations,
        native_pipeline_creations,
        native_uploads,
        native_scanout
            .as_ref()
            .map_or(0, |native| native.callback_accepted),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.callback_rejected),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.callback_queue_saturated),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.nonzero_exports),
        native_scanout
            .as_ref()
            .map_or(0, LiveProductionNativeScanout::mixed_exports),
        native_scanout
            .as_ref()
            .map_or(0, LiveProductionNativeScanout::export_attempts),
        runtime
            .as_ref()
            .is_some_and(LiveProductionVisualRuntime::native_scanout_in_flight),
        runtime
            .as_ref()
            .is_some_and(LiveProductionVisualRuntime::native_cleanup_pending),
        if physical_input.is_some() {
            "enabled"
        } else {
            "disabled"
        },
        if wm_session.is_some() {
            "external"
        } else {
            "disabled"
        },
        wm_session.as_ref().map_or(0, |wm| wm.requests),
        wm_session.as_ref().map_or(0, |wm| wm.committed),
        wm_session.as_ref().map_or(0, |wm| wm.restarts),
        wm_session.as_ref().is_some_and(|wm| wm.degraded),
        match config.namespace_profile {
            NamespaceProfile::ClassicShared => "classic_shared",
            NamespaceProfile::Confined => "confined",
        },
        if config.inject_output_size.is_some() {
            "applied"
        } else {
            "disabled"
        },
        output_notifications,
        if resize_proof_complete {
            "committed"
        } else {
            "disabled"
        },
        present_observation.complete_flip,
        present_observation.complete_skip,
        present_observation.idle,
        present_observation.idle_fence_triggers,
        present_observation.disconnect_sources,
        present_observation.disconnect_fences,
        present_observation.disconnect_failures,
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.diagnostics().live_sources),
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.diagnostics().live_fences),
        runtime
            .as_ref()
            .map_or(0, |runtime| { runtime.diagnostics().live_presentations }),
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.diagnostics().acquire_waits),
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.diagnostics().controlled_rejections),
    );
    if let Some(runtime) = runtime.as_ref()
        && (present_observation.disconnect_failures != 0
            || runtime.diagnostics().live_sources != 0
            || runtime.diagnostics().live_fences != 0
            || runtime.diagnostics().live_presentations != 0
            || present_observation.idle
                != present_observation
                    .complete_flip
                    .saturating_add(present_observation.complete_skip))
    {
        return Err("persistent Present resources did not retire exactly once".into());
    }
    if let (Some(runtime), Some(native_scanout)) = (runtime.as_ref(), native_scanout.as_ref())
        && (native_scanout.submissions == 0
            || native_scanout.retirements == 0
            || native_scanout.nonzero_exports == 0
            || native_scanout.submit_failures != 0
            || native_scanout.retire_failures != 0
            || native_scanout.callback_rejected != 0
            || native_scanout.callback_queue_saturated != 0
            || native_scanout.vsync_overlap_rejections != 0
            || native_scanout.page_flip_phase_rejections != 0
            || runtime.native_scanout_in_flight()
            || runtime.native_cleanup_pending())
    {
        return Err(format!(
            "persistent native scanout did not submit, retire, and drain cleanly: overlap_rejections={} phase_rejections={}",
            native_scanout.vsync_overlap_rejections,
            native_scanout.page_flip_phase_rejections,
        )
        .into());
    }
    if let Some(native_scanout) = native_scanout.as_ref() {
        println!(
            "sophia_live_vsync schema=1 status=complete outputs={} overlap_rejections={} phase_rejections={} policy=page_flip_paced",
            native_scanout.heads.len(),
            native_scanout.vsync_overlap_rejections,
            native_scanout.page_flip_phase_rejections,
        );
        for head in &native_scanout.heads {
            println!(
                "sophia_live_output schema=1 status=complete output={} checksum={} submissions={} retirements={} callbacks={} nonzero_exports={}",
                head.output.id.raw(),
                head.last_checksum,
                head.submissions,
                head.retirements,
                head.callback_accepted,
                head.nonzero_exports,
            );
        }
        if native_scanout.heads.iter().any(|head| {
            head.submissions == 0
                || head.retirements == 0
                || head.callback_accepted == 0
                || head.nonzero_exports == 0
        }) {
            return Err(
                "one or more native outputs did not present and retire independently".into(),
            );
        }
        let mut checksums = native_scanout
            .heads
            .iter()
            .map(|head| head.last_checksum)
            .collect::<Vec<_>>();
        checksums.sort_unstable();
        checksums.dedup();
        if checksums.len() != native_scanout.heads.len() {
            return Err("native output frames are not independently distinguishable".into());
        }
    }
    if let Some(client) = config.client.as_deref() {
        let client_name = std::path::Path::new(client)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("client");
        println!(
            "sophia_x_application_session schema=1 status=passed class=gtk3_software client={} profile={} child_outcome=normal exit_code=0 stdout_match={} protocol_errors=0 first_error=none physical_text={} pointer_button={} surface_resize={} buffer_path=cpu_shm native_presentation={} cleanup=clean",
            client_name,
            match config.namespace_profile {
                NamespaceProfile::ClassicShared => "classic_shared",
                NamespaceProfile::Confined => "confined",
            },
            config.expect_client_stdout.is_some(),
            physical_text_proof
                .as_ref()
                .is_some_and(PhysicalTextProof::is_complete),
            physical_pointer_buttons_routed > 0,
            if resize_proof_complete {
                "committed"
            } else {
                "disabled"
            },
            if native_scanout.is_some() {
                "enabled"
            } else {
                "disabled"
            },
        );
    }
    Ok(())
}
