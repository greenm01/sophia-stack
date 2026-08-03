{
    let SessionLoopMetrics {
        batches,
        transactions,
        cpu_buffer_updates,
        cpu_buffer_replacements,
        cpu_buffer_patch_updates,
        cpu_buffer_patch_rects,
        cpu_buffer_payload_bytes,
        dma_buf_registrations_observed: _,
        fence_registrations_observed: _,
        present_submissions_observed: _,
        software_present_submissions_observed: _,
        cpu_compositions,
        coalesced_batches,
        backend_ticks,
        runtime_committed,
        runtime_surfaces,
        physical_events,
        physical_keys_routed,
        key_repeats_routed,
        physical_pointer_events,
        physical_pointer_routed,
        physical_pointer_buttons_routed,
        session_ticks,
        max_compose,
        max_child_reap,
        max_input_phase,
        protocol_error_count,
        expected_protocol_error_count,
        cursor_moves_coalesced,
        cursor_max_motion_to_submit,
    } = metrics;

    if let (Some(runtime), Some(native_scanout)) = (runtime.as_mut(), native_scanout.as_mut()) {
        let report =
            runtime.suspend_native_scanout(native_scanout, &outputs, Duration::from_secs(2))?;
        let evicted_renderer_images = native_scanout.clear_renderer_images()?;
        println!(
            "sophia_live_session_native_suspend schema=2 outcome={} drained={} abandoned_scanouts={} skipped_present={}",
            report.outcome.reduced_name(),
            report.outcome.drained(),
            report.abandoned_scanouts,
            report
                .skipped_present
                .map_or_else(|| "none".to_owned(), |transaction| transaction.raw().to_string()),
        );
        println!(
            "sophia_live_renderer_images schema=1 status=cleared evicted={evicted_renderer_images}"
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
    if let (Some(ingress_msec), Some(presented_ust_usec)) =
        (input_raw_ingress_msec, input_presented_ust_usec)
    {
        let ingress_ust_usec = ingress_msec
            .checked_mul(1_000)
            .ok_or("physical input ingress timestamp overflowed microseconds")?;
        let full_chain_usec = presented_ust_usec.checked_sub(ingress_ust_usec).ok_or(
            "physical input and page-flip timestamps were not in the same monotonic clock domain",
        )?;
        let full_chain = Duration::from_micros(full_chain_usec);
        let submit_to_page_flip = input_submit_to_page_flip
            .ok_or("physical input frame retired without submit-to-page-flip timing")?;
        let input_to_submit = full_chain.saturating_sub(submit_to_page_flip);
        let queue_dwell = input_queue_dwell
            .ok_or("physical input frame retired without per-event queue-dwell timing")?;
        let dwell_to_submit = input_to_submit.saturating_sub(queue_dwell);
        input_presented_latency = Some(full_chain);
        println!(
            "sophia_live_input_latency schema=1 status=complete source=libinput_to_kernel_page_flip ingress_msec={} queue_dwell_msec={} dwell_to_submit_msec={} submit_to_page_flip_msec={} full_chain_msec={}",
            ingress_msec,
            queue_dwell.as_millis(),
            dwell_to_submit.as_millis(),
            submit_to_page_flip.as_millis(),
            full_chain.as_millis(),
        );
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
    if config.expect_physical_text.is_some()
        && native_scanout.as_ref().is_some_and(|native| {
            native.kernel_page_flip_timestamp_missing != 0
                || native.pending_kernel_page_flip_timestamps() != 0
        })
    {
        return Err(
            "physical input proof observed fallback or pending kernel page-flip timestamps".into(),
        );
    }
    if config.expect_physical_text.is_some()
        && native_scanout.is_some()
        && (input_raw_ingress_msec.is_none() || input_presented_ust_usec.is_none())
    {
        return Err(
            "physical input proof did not correlate libinput ingress to its presented frame".into(),
        );
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
    let recovery_extent_count = layout.recovery_extent_count();
    if recovery_extent_count != 0 || layout.constraint_relayout_required() {
        return Err(format!(
            "persistent live session ended with temporary layout constraints: recovery_extents={recovery_extent_count} constraint_relayout_pending={}",
            layout.constraint_relayout_required(),
        )
        .into());
    }
    if config.firefox_full_proof_requested() {
        if config.firefox_m10_proof && !firefox_m8_proof.complete() {
            return Err(format!(
                "Firefox M10 promotion proof incomplete: stages={}/{}",
                firefox_m8_proof.completed(),
                firefox_m8_proof.stage_count(),
            )
            .into());
        }
        if config.firefox_m8_proof
            && (!firefox_m8_proof.complete()
                || selection_owner_changes < 2
                || selection_conversions < 2)
        {
            return Err(format!(
                "Firefox M8 proof incomplete: stages={}/{} selection_owner_changes={} selection_conversions={}",
                firefox_m8_proof.completed(),
                firefox_m8_proof.stage_count(),
                selection_owner_changes,
                selection_conversions,
            )
            .into());
        }
        if config.firefox_m10_proof {
            println!(
                "sophia_firefox_promotion schema=1 status=complete stages={} selection_gates=focused content=redacted",
                firefox_m8_proof.completed(),
            );
        } else {
            println!(
                "sophia_firefox_m8 schema=1 status=complete stages={} selection_owner_changes={} selection_conversions={} content=redacted",
                firefox_m8_proof.completed(),
                selection_owner_changes,
                selection_conversions,
            );
        }
    }
    if config.firefox_m10_rendering_proof {
        if !firefox_m10_rendering_page_ready {
            return Err("Firefox M10 rendering proof did not observe its ready document".into());
        }
        println!(
            "sophia_firefox_rendering schema=1 status=complete page_ready=true recovery_extents=0 content=redacted"
        );
    }
    if config.firefox_m10_dialog_proof {
        if !firefox_m10_dialog_proof.complete() || physical_pointer_buttons_routed < 4 {
            return Err(format!(
                "Firefox M10 dialog proof incomplete: checkpoints={}/{} pointer_buttons={physical_pointer_buttons_routed}",
                firefox_m10_dialog_proof.completed,
                FirefoxM10DialogProof::CHECKPOINTS.len(),
            )
            .into());
        }
        println!(
            "sophia_firefox_dialog schema=1 status=complete checkpoints=3 pointer_buttons={physical_pointer_buttons_routed} recovery_extents=0 content=redacted"
        );
    }
    if config.firefox_m10_primary_proof {
        if !firefox_m10_primary_proof.complete()
            || selection_owner_changes < 2
            || selection_conversions < 2
        {
            return Err(format!(
                "Firefox M10 PRIMARY proof incomplete: checkpoints={}/{} selection_owner_changes={} selection_conversions={}",
                firefox_m10_primary_proof.completed,
                FirefoxM10PrimaryProof::CHECKPOINTS.len(),
                selection_owner_changes,
                selection_conversions,
            )
            .into());
        }
        println!(
            "sophia_firefox_primary schema=1 status=complete checkpoints=3 selection_owner_changes={selection_owner_changes} selection_conversions={selection_conversions} content=redacted"
        );
    }
    if config.firefox_m10_proof {
        if !firefox_m10_kitty_proof.complete() {
            return Err(format!(
                "Firefox M10 Kitty proof incomplete: checkpoints={}/{}",
                firefox_m10_kitty_proof.completed(),
                FirefoxM10KittyProof::CHECKPOINTS.len(),
            )
            .into());
        }
        println!(
            "sophia_firefox_m10 schema=3 status=complete kitty_checkpoints={} selection_gates=focused content=redacted",
            firefox_m10_kitty_proof.completed(),
        );
    }
    if config.firefox_m10_selection_proof {
        if firefox_m8_proof.completed() < 4
            || !firefox_m10_selection_kitty_proof.complete()
            || selection_owner_changes < 4
            || selection_conversions < 4
        {
            return Err(format!(
                "Firefox M10 selection proof incomplete: stages={}/4 checkpoints={}/3 selection_owner_changes={} selection_conversions={}",
                firefox_m8_proof.completed().min(4),
                firefox_m10_selection_kitty_proof.completed(),
                selection_owner_changes,
                selection_conversions,
            )
            .into());
        }
        println!(
            "sophia_firefox_selection schema=1 status=complete stages=4 kitty_checkpoints=3 selection_owner_changes={} selection_conversions={} content=redacted",
            selection_owner_changes,
            selection_conversions,
        );
    }
    if config.firefox_m10_lifecycle_proof {
        if !firefox_m8_page_ready_reported || !firefox_m10_kitty_proof.lifecycle_complete() {
            return Err(format!(
                "Firefox M10 lifecycle proof incomplete: page_ready={} checkpoints={}/6",
                firefox_m8_page_ready_reported,
                firefox_m10_kitty_proof.completed().min(6),
            )
            .into());
        }
        println!(
            "sophia_firefox_lifecycle schema=1 status=complete page_ready=true kitty_checkpoints=6 content=redacted"
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
            || pending_wm_update.is_some()
            || wm_session
                .as_ref()
                .is_some_and(|wm| wm.pending_request_count() != 0)
            || !committed_session_actions.is_empty()
            || session_launches.pending_len() != 0
            || session_launches.admission().is_some()
            || !input_delivery.pending.is_empty()
            || wm_session.as_ref().is_some_and(|wm| wm.degraded))
    {
        return Err(format!(
            "normal session ended with pending work: wm_layout={} wm_update={} wm_requests={} actions={} launches={} admission={} input={} degraded={}",
            usize::from(layout.pending.is_some()),
            usize::from(pending_wm_update.is_some()),
            wm_session
                .as_ref()
                .map_or(0, LiveWmSession::pending_request_count),
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
    let native_resources = native_scanout.as_ref().map_or_else(
        sophia_backend_live::LivePersistentRenderMetrics::default,
        LiveProductionNativeScanout::persistent_render_metrics,
    );
    let native_target_creations = native_resources.target_creations;
    let native_target_recreations = native_resources.target_recreations;
    let native_pipeline_creations = native_resources.pipeline_creations;
    let native_frame_surface_creations = native_resources.frame_surface_creations;
    let native_uploads = native_resources.uploads;
    let native_max_target_create = native_resources.max_target_create;
    let native_max_frame_surface_create = native_resources.max_frame_surface_create;
    let native_max_render = native_resources.max_render;
    let native_max_upload = native_resources.max_upload;
    println!(
        "sophia_live_native_resources schema=4 status=complete target_creations={} pipeline_creations={} frame_surface_creations={} cpu_target_creations={} dmabuf_target_creations={} composition_target_creations={} composition_target_reuses={} generation_replacements={} recovery_replacements={} import_cache_imports={} import_cache_hits={} import_cache_evictions={} import_cache_live_entries={} import_cache_descriptor_mismatches={} import_cache_capacity_rejections={} worker_requests={} worker_completions={} worker_failures={} worker_soft_stalls={} worker_hard_stalls={} worker_release_enqueue_failures={} max_worker_request_msec={}",
        native_resources.target_creations,
        native_resources.pipeline_creations,
        native_resources.frame_surface_creations,
        native_resources.cpu_target_creations,
        native_resources.dmabuf_target_creations,
        native_resources.composition_target_creations,
        native_resources.composition_target_reuses,
        native_resources.generation_replacements,
        native_resources.recovery_replacements,
        native_resources.import_cache_imports,
        native_resources.import_cache_hits,
        native_resources.import_cache_evictions,
        native_resources.import_cache_live_entries,
        native_resources.import_cache_descriptor_mismatches,
        native_resources.import_cache_capacity_rejections,
        native_resources.worker_requests,
        native_resources.worker_completions,
        native_resources.worker_failures,
        native_resources.worker_soft_stalls,
        native_resources.worker_hard_stalls,
        native_resources.worker_release_enqueue_failures,
        native_resources.max_worker_request.as_millis(),
    );
    if let Some(native_scanout) = native_scanout.as_ref() {
        println!(
            "sophia_live_page_flip_clock schema=1 status=complete source=kernel_monotonic timestamps={} fallbacks={} pending={}",
            native_scanout.kernel_page_flip_timestamps,
            native_scanout.kernel_page_flip_timestamp_missing,
            native_scanout.pending_kernel_page_flip_timestamps(),
        );
    }
    println!(
        "sophia_live_rendering_efficiency schema=1 status=complete cpu_updates={} cpu_replacements={} cpu_patch_updates={} cpu_patch_rects={} cpu_payload_bytes={} exact_pixel_metric_frames={} damage_scoped_metric_frames={} composition_target_reuses={}",
        cpu_buffer_updates,
        cpu_buffer_replacements,
        cpu_buffer_patch_updates,
        cpu_buffer_patch_rects,
        cpu_buffer_payload_bytes,
        scene.exact_pixel_metric_frames(),
        scene.damage_scoped_metric_frames(),
        native_resources.composition_target_reuses,
    );
    println!(
        "sophia_live_session_scheduler schema=1 authority_batches={batches} cpu_compositions={cpu_compositions} coalesced_batches={coalesced_batches}"
    );
    println!(
        "sophia_live_owner_timing schema=2 status=complete max_child_reap_msec={} max_input_phase_msec={}",
        max_child_reap.as_millis(),
        max_input_phase.as_millis(),
    );
    if let Some(wm) = wm_session.as_ref() {
        println!(
            "sophia_live_wm_transport schema=2 status=complete peak_depth={} pending={} rejected={} action_coalesced={} stale_responses={} max_queue_dwell_msec={} max_round_trip_msec={}",
            wm.request_peak_depth,
            wm.pending_request_count(),
            wm.request_rejections,
            wm.action_requests_coalesced,
            wm.stale_responses,
            wm.max_queue_dwell.as_millis(),
            wm.max_request().as_millis(),
        );
    }
    println!(
        "sophia_live_session_cursor schema=3 moves_coalesced={} max_motion_to_submit_msec={} max_update_msec={} deferred_primary_in_flight={} buttons_routed={} hardware_updates={} hidden_updates={} hardware_failures={}",
        cursor_moves_coalesced,
        cursor_max_motion_to_submit.as_millis(),
        native_scanout
            .as_ref()
            .map_or(0, |scanout| scanout.max_cursor_update.as_millis()),
        native_scanout
            .as_ref()
            .map_or(0, |scanout| scanout.cursor_deferred_primary_in_flight),
        physical_pointer_buttons_routed,
        native_scanout
            .as_ref()
            .map_or(0, |scanout| scanout.cursor_updates),
        native_scanout
            .as_ref()
            .map_or(0, |scanout| scanout.cursor_hidden_updates),
        native_scanout
            .as_ref()
            .map_or(0, |scanout| scanout.cursor_update_failures),
    );
    println!(
        "sophia_live_session_health schema=1 status=clean protocol_errors={} pending_wm={} pending_actions={} pending_input={} wm_degraded={}",
        protocol_error_count,
        usize::from(layout.pending.is_some())
            .saturating_add(usize::from(pending_wm_update.is_some()))
            .saturating_add(
                wm_session
                    .as_ref()
                    .map_or(0, LiveWmSession::pending_request_count),
            ),
        committed_session_actions.len(),
        input_delivery.pending.len(),
        wm_session.as_ref().is_some_and(|wm| wm.degraded),
    );
    println!(
        "sophia_live_layout_health schema=1 status=clean recovery_extents={} constraint_relayout_pending={}",
        recovery_extent_count,
        layout.constraint_relayout_required(),
    );
    println!(
        "sophia_live_session_protocol_errors schema=1 expected={} unexpected={}",
        expected_protocol_error_count, protocol_error_count,
    );
    println!(
        "sophia_live_selection schema=1 status=complete owner_changes={} conversions={} content=redacted",
        selection_owner_changes, selection_conversions,
    );

    let present_observation = &present_observer;
    if let Some(cadence) = present_observation.retained_cadence.summary() {
        println!(
            "sophia_live_present_cadence schema=1 status=complete samples={} advancing_intervals={} nonadvancing={} overflowed=false mean_fps={:.3} p95_frame_msec={:.3}",
            cadence.samples,
            cadence.advancing_intervals,
            cadence.nonadvancing,
            cadence.mean_fps,
            cadence.p95_frame_msec,
        );
    } else {
        println!(
            "sophia_live_present_cadence schema=1 status=unavailable samples={} advancing_intervals={} nonadvancing={} overflowed={}",
            present_observation
                .retained_cadence
                .intervals_usec
                .len()
                .saturating_add(usize::from(
                    present_observation.retained_cadence.first_ust.is_some()
                )),
            present_observation.retained_cadence.intervals_usec.len(),
            present_observation.retained_cadence.nonadvancing,
            present_observation.retained_cadence.overflowed,
        );
    }
    println!(
        "sophia_live_session schema=16 status=bounded_complete display={} elapsed_msec={} startup_ready_msec={} session_ticks={} authority_batches={} authority_transactions={} authority_queue_capacity={} authority_batches_dropped=0 backend_ticks={} runtime_committed={} runtime_surfaces={} cpu_layers={} cpu_nonzero_pixel_bytes={} cpu_max_nonzero_pixel_bytes={} cpu_nonzero_frames={} cpu_checksum={} cpu_max_compose_msec={} injected_input={} input_events_expected={} input_events_flushed={} input_flush_latency_msec={} input_pixel_change={} input_text_match={} input_presented_latency_msec={} input_dispatch_max_gap_msec={} input_queue_max_depth={} input_queue_dwell_max_msec={} physical_events={} physical_keys_routed={} pointer_pixel_change={} physical_pointer_events={} physical_pointer_routed={} pointer_proof={} native_presentation={} native_submissions={} native_submit_deferred={} native_submit_failures={} native_retirements={} native_retire_failures={} native_max_in_flight_ticks={} native_max_submit_to_page_flip_msec={} native_max_upload_msec={} native_max_target_create_msec={} native_max_frame_surface_create_msec={} native_max_render_msec={} native_target_creations={} native_target_recreations={} native_pipeline_creations={} native_frame_surface_creations={} native_frame_uploads={} native_callback_accepted={} native_callback_rejected={} native_callback_queue_saturated={} native_nonzero_exports={} native_mixed_exports={} native_export_attempts={} native_in_flight={} native_cleanup_pending={} physical_input={} wm_policy={} wm_requests={} wm_committed={} wm_restarts={} wm_degraded={} namespace_profile={} output_update={} output_notifications={} surface_resize={} present_complete_copy={} present_complete_flip={} present_complete_skip={} present_idle={} present_idle_fence_triggers={} present_disconnect_sources={} present_disconnect_fences={} present_disconnect_failures={} present_live_sources={} present_live_fences={} present_live_transactions={} present_acquire_waits={} present_controlled_rejections={}",
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
        native_max_target_create.as_millis(),
        native_max_frame_surface_create.as_millis(),
        native_max_render.as_millis(),
        native_target_creations,
        native_target_recreations,
        native_pipeline_creations,
        native_frame_surface_creations,
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
        present_observation.complete_copy,
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
                    .complete_copy
                    .saturating_add(present_observation.complete_flip)
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
            !independent_native_output_presented(
                head.submissions,
                head.retirements,
                head.callback_accepted,
                head.initial_modeset_submission.is_some(),
                head.nonzero_exports,
            )
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
    let control_metrics = session_controls.metrics();
    println!(
        "sophia_live_session_control schema=1 status=complete enqueued={} dispatched={} delivered={} rejected={} timed_out={} unexpected={} pending={} peak_depth={} max_queue_dwell_msec={} max_ack_msec={}",
        control_metrics.enqueued,
        control_metrics.dispatched,
        control_metrics.delivered,
        control_metrics.rejected,
        control_metrics.timed_out,
        control_metrics.unexpected,
        session_controls.pending_len(),
        control_metrics.peak_depth,
        control_metrics.max_queue_dwell.as_millis(),
        control_metrics.max_acknowledgement_latency.as_millis(),
    );
    if session_controls.pending_len() != 0
        || control_metrics.enqueued != control_metrics.dispatched
        || control_metrics.dispatched != control_metrics.delivered
        || control_metrics.rejected != 0
        || control_metrics.timed_out != 0
        || control_metrics.unexpected != 0
    {
        return Err("persistent session controls did not drain cleanly".into());
    }
    let key_metrics = client_keys.metrics();
    let repeat_metrics = key_repeat.metrics();
    println!(
        "sophia_live_session_keys schema=2 status=complete pending={} release_barrier_pending={} peak_pressed={} synthetic_releases={} state_only_releases={} orphan_releases_suppressed={} removed_surface_keys={} repeat_active_seats={} repeat_armed={} repeat_routed={} repeat_pulses={} repeat_coalesced={} repeat_cancelled={} repeat_capacity_exhausted={}",
        client_keys.pending_len(),
        client_key_release_barrier.len(),
        key_metrics.peak_pressed,
        key_metrics.synthetic_releases,
        key_metrics.state_only_releases,
        key_metrics.orphan_releases_suppressed,
        key_metrics.removed_surface_keys,
        key_repeat.active_seats(),
        repeat_metrics.armed,
        key_repeats_routed,
        repeat_metrics.pulses,
        repeat_metrics.coalesced,
        repeat_metrics.cancelled,
        repeat_metrics.seat_capacity_exhausted,
    );
    let keyboard_coverage = keyboard_coverage.snapshot();
    println!(
        "sophia_live_keyboard_coverage schema=1 status=complete shifted_positions={} shifted_positions_required={} virtual_terminals={} virtual_terminals_required={} content=redacted",
        keyboard_coverage.shifted_positions,
        keyboard_coverage.shifted_positions_required,
        keyboard_coverage.virtual_terminals,
        keyboard_coverage.virtual_terminals_required,
    );
    if client_keys.pending_len() != 0
        || !client_key_release_barrier.is_empty()
        || key_repeat.active_seats() != 0
        || repeat_metrics.seat_capacity_exhausted != 0
        || repeat_metrics.pulses != u64::try_from(key_repeats_routed).unwrap_or(u64::MAX)
    {
        return Err("persistent client key state did not drain cleanly".into());
    }
    Ok(())
}
