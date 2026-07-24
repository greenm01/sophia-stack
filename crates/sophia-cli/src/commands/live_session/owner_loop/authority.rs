{
        let authority_batch = initial_authority_batch
            .take()
            .or_else(|| pending_authority_batches.pop_front())
            .map_or_else(
                || {
                    authority_receiver.recv_timeout(if cursor_dirty {
                        Duration::from_millis(1)
                    } else {
                        Duration::from_millis(25)
                    })
                },
                Ok,
            );
        match authority_batch {
            Ok(batch) => {
                let drain_started = Instant::now();
                while pending_authority_batches.len() < 64
                    && drain_started.elapsed() < Duration::from_millis(2)
                {
                    match authority_receiver.try_recv() {
                        Ok(queued) => pending_authority_batches.push_back(queued),
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            return Err(
                                "persistent X authority transaction channel disconnected".into()
                            );
                        }
                    }
                }
                let defer_cpu_frame = runtime.is_some()
                    && software_batch_may_coalesce(&batch)
                    && pending_authority_batches
                        .front()
                        .is_some_and(software_batch_may_coalesce);
                for error in &batch.protocol_errors {
                    metrics.protocol_error_count = metrics.protocol_error_count.saturating_add(1);
                    first_protocol_error.get_or_insert(*error);
                }
                metrics.expected_protocol_error_count = metrics.expected_protocol_error_count
                    .saturating_add(batch.expected_protocol_errors.len());
                if config.firefox_m8_proof {
                    if batch.selection_owner_change {
                        firefox_m8_selection_owner_changes =
                            firefox_m8_selection_owner_changes.saturating_add(1);
                        println!(
                            "sophia_firefox_m8 schema=1 status=selection_observed kind=owner_change count={firefox_m8_selection_owner_changes} content=redacted"
                        );
                    }
                    if batch.selection_conversion {
                        firefox_m8_selection_conversions =
                            firefox_m8_selection_conversions.saturating_add(1);
                        println!(
                            "sophia_firefox_m8 schema=1 status=selection_observed kind=conversion count={firefox_m8_selection_conversions} content=redacted"
                        );
                    }
                    for metadata in &batch.metadata {
                        if !firefox_m8_page_ready_reported
                            && metadata.property_name == "_NET_WM_NAME"
                            && metadata.byte_len == 36
                        {
                            firefox_m8_page_ready_reported = true;
                            println!(
                                "sophia_firefox_m8 schema=1 status=page_ready title_bytes=36 content=redacted"
                            );
                        }
                        for (stage, index, title_bytes) in
                            firefox_m8_proof.observe(&metadata.property_name, metadata.byte_len)
                        {
                            println!(
                                "sophia_firefox_m8 schema=1 status=stage_complete stage={stage} index={index} title_bytes={} content=redacted",
                                title_bytes,
                            );
                        }
                    }
                }
                let has_engine_work = !batch.transactions.is_empty()
                    || !batch.removed_surfaces.is_empty()
                    || !batch.cpu_buffer_updates.is_empty()
                    || !batch.dma_buf_registrations.is_empty()
                    || !batch.fence_registrations.is_empty()
                    || !batch.present_submissions.is_empty()
                    || !batch.released_dma_bufs.is_empty()
                    || !batch.released_fences.is_empty();
                if !has_engine_work {
                    continue;
                }
                last_authority_update = Instant::now();
                metrics.batches = metrics.batches.saturating_add(1);
                metrics.transactions =
                    metrics
                        .transactions
                        .saturating_add(authority_transaction_count(&batch.transactions));
                metrics.cpu_buffer_updates =
                    metrics
                        .cpu_buffer_updates
                        .saturating_add(batch.cpu_buffer_updates.len());
                metrics.dma_buf_registrations_observed = metrics.dma_buf_registrations_observed
                    .saturating_add(batch.dma_buf_registrations.len());
                metrics.fence_registrations_observed =
                    metrics.fence_registrations_observed.saturating_add(batch.fence_registrations.len());
                metrics.present_submissions_observed =
                    metrics.present_submissions_observed.saturating_add(batch.present_submissions.len());
                let removed_surfaces = batch.removed_surfaces.clone();
                if let Some(wm_session) = wm_session.as_mut() {
                    for surface in &removed_surfaces {
                        wm_session.notify_surface_removed(*surface)?;
                    }
                }
                let _ = layout.observe_authority_batch(&batch);
                let mut wm_update = layout.resolve_pending();
                if !resize_proof_complete
                    && let Some((transaction, surface, size)) = resize_proof
                    && layout.pending.is_none()
                    && layout.resize.committed_size(surface) == Some(size)
                {
                    println!(
                        "sophia_live_resize schema=1 status=committed transaction={} surface={} width={} height={} configure_ack=true pixels=true",
                        transaction.raw(),
                        surface.index(),
                        size.width,
                        size.height,
                    );
                    resize_proof_complete = true;
                }
                if wm_update.is_none() {
                    wm_update = layout.expire_pending(control_sender, control_ack_receiver)?;
                }
                if layout.pending.is_none()
                    && let Some(wm_session) = wm_session.as_mut()
                    && let Some(proposal) = wm_session.poll_restart(&layout, output)? {
                        wm_update = layout.stage(proposal, control_sender, control_ack_receiver)?;
                    }
                if resize_proof.is_none()
                    && let Some(size) = config.inject_surface_resize
                    && layout.layers.len() >= if config.secondary_terminal { 2 } else { 1 }
                    && layout.pending.is_none()
                {
                    let surface = layout
                        .layers
                        .keys()
                        .next()
                        .copied()
                        .ok_or("surface resize proof has no target")?;
                    let transaction = TransactionId::from_raw(2_000_000);
                    let mut layers = layout.layers.values().cloned().collect::<Vec<_>>();
                    let layer = layers
                        .iter_mut()
                        .find(|layer| layer.surface == surface)
                        .ok_or("surface resize proof lost its target")?;
                    layer.geometry.width = size.width;
                    layer.geometry.height = size.height;
                    let proposal = LiveWmProposal {
                        transaction,
                        layers,
                        requested_sizes: BTreeMap::from([(surface, size)]),
                        focus: None,
                        timeout: Duration::from_secs(2),
                        update: WmTransactionUpdate {
                            commit: TransactionCommit {
                                transaction,
                                outcome: TransactionOutcome::Committed,
                                applied_surfaces: vec![surface],
                            },
                            ipc_error: None,
                        },
                        moved_surfaces: 0,
                        effects: None,
                    };
                    wm_update = layout.stage(proposal, control_sender, control_ack_receiver)?;
                    resize_proof = Some((transaction, surface, size));
                    println!(
                        "sophia_live_resize schema=1 status=requested transaction={} surface={} width={} height={}",
                        transaction.raw(),
                        surface.index(),
                        size.width,
                        size.height,
                    );
                }
                let wm_update = wm_update.map(|mut result| {
                    if result.update.commit.outcome == TransactionOutcome::Committed
                        && let Some(effects) = result.effects.take()
                        && let Some(wm_session) = wm_session.as_mut()
                    {
                        wm_session.workspace_state = effects.workspace_state;
                        wm_session.mark_committed();
                        if let Some(action) = effects.session_action {
                            committed_session_actions.push_back((
                                effects.transaction,
                                action.0,
                                action.1,
                            ));
                        }
                    }
                    result.update
                });
                let batch = layout.projected_batch(&batch);
                let production_batch = production_authority_batch(&batch);
                if runtime.is_none() {
                    runtime = Some(
                        LiveProductionVisualRuntime::new(&outputs, native_scanout.as_mut(), None)?
                            .with_m4_proof_controls(
                                config.m4_first_acquire_delay,
                                config.m4_reject_first_present,
                                config.m4_diagnose_first_mixed_export,
                            ),
                    );
                }
                let runtime = runtime
                    .as_mut()
                    .expect("persistent backend runtime was initialized above");
                let raised_surface = focus.focused_surface(seat);
                let updates = batch
                    .cpu_buffer_updates
                    .iter()
                    .map(renderer_cpu_buffer_update)
                    .collect::<Vec<_>>();
                let cursor_presentation = if native_scanout.is_some() {
                    LiveProductionCursorPresentation::HardwarePlane
                } else {
                    LiveProductionCursorPresentation::Software(pointer.position)
                };
                let (_tick, report, committed_surfaces, composed, compose_elapsed) =
                    if batch.present_submissions.is_empty() {
                        let (submission, committed_surfaces) = runtime.run_cpu_production_cycle(
                            &production_batch,
                            &mut scene,
                            updates,
                            raised_surface,
                            cursor_presentation,
                            defer_cpu_frame,
                            &outputs,
                            if defer_cpu_frame {
                                None
                            } else {
                                native_scanout.as_mut()
                            },
                            wm_update,
                        )?;
                        (
                            submission.tick,
                            submission.composition,
                            committed_surfaces,
                            submission.composed,
                            submission.compose_elapsed,
                        )
                    } else {
                        let (submission, committed_surfaces) = runtime.run_gpu_production_cycle(
                            &production_batch,
                            &mut scene,
                            updates,
                            raised_surface,
                            cursor_presentation,
                            defer_cpu_frame,
                            &outputs,
                            if defer_cpu_frame {
                                None
                            } else {
                                native_scanout.as_mut()
                            },
                            wm_update,
                        )?;
                        (
                            submission.tick,
                            submission.composition,
                            committed_surfaces,
                            submission.composed,
                            submission.compose_elapsed,
                        )
                    };
                if composed {
                    metrics.max_compose = metrics.max_compose.max(compose_elapsed);
                    metrics.cpu_compositions = metrics.cpu_compositions.saturating_add(1);
                } else {
                    metrics.coalesced_batches = metrics.coalesced_batches.saturating_add(1);
                }
                if let (Some(surface), Some(before_surface)) =
                    (input_surface, input_surface_generation)
                    && scene
                        .surface_buffer_generation(&committed_surfaces, surface)
                        .is_some_and(|generation| generation != before_surface)
                {
                    input_surface_pixel_change = true;
                }
                if let Some(before_frame) = injection_checksum
                    && report.checksum != before_frame
                    && (config.expect_physical_text.is_none()
                        || physical_sequence_completed_at.is_some())
                {
                    input_pixel_change = true;
                }
                if let Some(before_frame) = pointer_checksum
                    && report.checksum != before_frame
                    && metrics.physical_pointer_routed > 0
                {
                    pointer_pixel_change = true;
                }
                metrics.backend_ticks = metrics.backend_ticks.saturating_add(1);
                metrics.runtime_committed = record_runtime_commits(
                    metrics.runtime_committed,
                    authority_transaction_count(&batch.transactions),
                );
                metrics.runtime_surfaces =
                    u64::try_from(runtime.committed_surfaces().len()).unwrap_or(u64::MAX);
                for surface in removed_surfaces {
                    if config.application_proof_requested()
                        && metrics.physical_pointer_buttons_routed == 0
                        && Some(surface) == input_surface
                    {
                        application_surface_missing_since.get_or_insert_with(Instant::now);
                    }
                    if config.application_proof_requested() && Some(surface) == input_surface {
                        application_surface_gone_at.get_or_insert_with(Instant::now);
                    }
                    focus.clear_surface(surface);
                }
                if let Some(surface) = input_surface
                    && runtime
                        .committed_surfaces()
                        .iter()
                        .any(|committed| committed.surface == surface)
                {
                    application_surface_missing_since = None;
                    application_surface_gone_at = None;
                }
                reconcile_initial_session_focus(InitialSessionFocusContext {
                    runtime,
                    focus: &mut focus,
                    seat,
                    wm_session_present: wm_session.is_some(),
                    layout: &layout,
                    control_sender,
                    next_focus_control_transaction: &mut next_focus_control_transaction,
                    focused_client_control: &mut focused_client_control,
                })?;
                if let Some((transaction, surface)) = layout.focus_to_apply.take() {
                    let decision = focus.focus_surface(seat, surface, runtime.committed_surfaces());
                    if decision == InputFocusDecision::Focused && wm_session.is_some() {
                        let client = layout
                            .client_routes
                            .client_for_surface(surface)
                            .ok_or("WM focus has no X11 client route")?;
                        control_sender.try_send(XAuthorityClientControlCommand {
                            client,
                            command: XAuthorityControlCommand::FocusSurface {
                                transaction,
                                surface,
                            },
                        })?;
                        let acknowledgement =
                            control_ack_receiver.recv_timeout(Duration::from_millis(500))?;
                        if acknowledgement.client != client
                            || acknowledgement.acknowledgement.transaction != transaction
                            || acknowledgement.acknowledgement.surface != surface
                            || acknowledgement.acknowledgement.outcome
                                != XAuthorityControlOutcome::Delivered
                        {
                            return Err("X Authority rejected WM focus reconciliation".into());
                        }
                    }
                    println!(
                        "sophia_live_wm schema=1 status=focus_reconciled transaction={} target=surface surface={surface:?} outcome={decision:?}",
                        transaction.raw()
                    );
                    if decision == InputFocusDecision::Focused {
                        println!(
                            "sophia_live_wm schema=1 status=focus_committed transaction={} target=surface",
                            transaction.raw()
                        );
                    }
                }
                if !focus_ready_reported && focus.focused_surface(seat).is_some() {
                    println!("sophia_live_session_input_pipeline schema=1 status=focus_ready");
                    std::io::stdout().flush()?;
                    focus_ready_reported = true;
                    focus_ready_at = Some(Instant::now());
                }
                if let Some(surface) = focus.focused_surface(seat) {
                    let cpu_visual_detail =
                        scene.surface_has_visual_detail(runtime.committed_surfaces(), surface);
                    if !startup_content_ready && cpu_visual_detail {
                        startup_content_ready = true;
                        startup_required_submissions = native_scanout.as_ref().map(|native| {
                            native
                                .heads
                                .iter()
                                .map(|head| {
                                    head.submissions
                                        .max(head.presented_submissions.saturating_add(1))
                                })
                                .collect()
                        });
                        println!(
                            "sophia_live_session_startup schema=1 status=content_ready source=cpu_visual_detail"
                        );
                        std::io::stdout().flush()?;
                    }
                    if !terminal_content_ready && cpu_visual_detail {
                        terminal_content_ready = true;
                        startup_ready_msec = Some(started.elapsed().as_millis());
                        println!(
                            "sophia_live_session_input_pipeline schema=1 status=terminal_content_ready"
                        );
                        std::io::stdout().flush()?;
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                let _ = layout.expire_pending(control_sender, control_ack_receiver)?;
                if layout.pending.is_none()
                    && let Some(wm_session) = wm_session.as_mut()
                    && let Some(proposal) = wm_session.poll_restart(&layout, output)?
                {
                    let _ = layout.stage(proposal, control_sender, control_ack_receiver)?;
                }
                if layout.pending.is_none()
                    && last_authority_update.elapsed()
                        >= Duration::from_millis(config.input_quiet_msec)
                    && let Some(wm_session) = wm_session.as_mut()
                    && let Some(surface) = layout.take_next_unmanaged_surface() {
                        let proposal = wm_session.request_manage(surface, &layout, output)?;
                        if layout
                            .stage(proposal, control_sender, control_ack_receiver)?
                            .is_some()
                        {
                            wm_session.mark_committed();
                        }
                    }
                if let (Some(runtime), Some(native_scanout)) =
                    (runtime.as_mut(), native_scanout.as_mut())
                {
                    let service = runtime.service_native(native_scanout)?;
                    if let Some(retired) = service.retired_present {
                        let stable = runtime.stable_present(native_scanout, retired.transaction);
                        retired_present_surfaces.insert(retired.surface, retired.transaction);
                        println!(
                            "sophia_live_session_present schema=1 status=retired transaction={} surface={}",
                            retired.transaction.raw(),
                            retired.surface.index(),
                        );
                        println!(
                            "sophia_live_session_scanout schema=1 status={} kind=mixed transaction={} pending_primary={}",
                            if stable { "stable" } else { "superseded" },
                            retired.transaction.raw(),
                            !stable,
                        );
                    }
                    if let Some(tick) = service.tick {
                        metrics.backend_ticks = metrics.backend_ticks.saturating_add(1);
                        let _ = tick;
                    }
                    metrics.runtime_surfaces =
                        u64::try_from(runtime.committed_surfaces().len()).unwrap_or(u64::MAX);
                    reconcile_initial_session_focus(InitialSessionFocusContext {
                        runtime,
                        focus: &mut focus,
                        seat,
                        wm_session_present: wm_session.is_some(),
                        layout: &layout,
                        control_sender,
                        next_focus_control_transaction: &mut next_focus_control_transaction,
                        focused_client_control: &mut focused_client_control,
                    })?;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("persistent X authority transaction channel disconnected".into());
            }
        }

        if !physical_input_completion_reported
            && input_pixel_change
            && input_text_match
            && let (Some(text), Some(proof)) = (
                config.expect_physical_text.as_deref(),
                physical_text_proof.as_ref(),
            )
            && proof.is_complete()
        {
            println!(
                "sophia_live_session_input schema=2 status=complete source=physical text={} expected_events={} matched_events={} pixel_change=true",
                text,
                proof.expected_events(),
                proof.matched_events(),
            );
            std::io::stdout().flush()?;
            physical_input_completion_reported = true;
        }

        if wm_session.is_none() {
            while let Ok(acknowledgement) = control_ack_receiver.try_recv() {
                let Some((transaction, surface)) = focused_client_control else {
                    continue;
                };
                if acknowledgement.acknowledgement.transaction != transaction
                    || acknowledgement.acknowledgement.surface != surface
                {
                    continue;
                }
                if acknowledgement.acknowledgement.outcome != XAuthorityControlOutcome::Delivered {
                    return Err(format!(
                        "initial X11 focus control was rejected: {:?}",
                        acknowledgement.acknowledgement.outcome
                    )
                    .into());
                }
                focused_client_control = None;
                focused_client_ready = true;
                println!(
                    "sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control"
                );
                std::io::stdout().flush()?;
            }
        }
}
