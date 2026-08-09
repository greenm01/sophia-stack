{
        let native_frame_service_request = match (runtime.as_ref(), native_scanout.as_ref()) {
            (Some(runtime), Some(native_scanout)) => {
                Some(runtime.native_output_service_request(native_scanout)?)
            }
            _ => None,
        };
        if native_frame_service_request
            .as_ref()
            .is_some_and(native_frame_service_requires_owner_progress)
        {
            native_frame_service_deadline_armed = true;
            native_frame_idle_service_cycles = 0;
        }
        let native_frame_service_preemption = native_frame_service_request
            .as_ref()
            .is_some_and(|request| {
                native_frame_service_should_preempt_authority(
                    request,
                    native_frame_service_preempted_previous_cycle,
                    session_controls.pending_len() != 0,
                    native_frame_control_priority_cycles,
                    native_frame_service_deadline_armed
                        && last_native_frame_service.elapsed() >= Duration::from_millis(16),
                )
            });
        let wm_only_cycle = initial_authority_batch.is_none()
            && pending_authority_batches.is_empty()
            && pending_wm_update.is_some();
        let authority_batch = if native_frame_service_preemption {
            // Renderer completion and KMS retirement are not X Authority
            // events. Give their bounded polling path a turn even when queued
            // or newly arriving metadata-only X batches keep the authority
            // channel continuously readable. Never take two such preemptions
            // consecutively so X control and focus traffic also makes bounded
            // progress under a continuously active renderer.
            std::thread::sleep(Duration::from_millis(1));
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        } else {
            initial_authority_batch
                .take()
                .or_else(|| pending_authority_batches.pop_front())
                .or_else(|| {
                    pending_wm_update.as_ref().map(|update| {
                        wm_update_coordinator_batch(update.commit.transaction)
                    })
                })
                .or_else(|| {
                    runtime.as_ref().and_then(|runtime| {
                        runtime
                            .released_surface_content_transaction()
                            .map(wm_update_coordinator_batch)
                    })
                })
                .map_or_else(
                    || {
                        authority_receiver.recv_timeout(authority_wait_timeout(
                            physical_input.is_some(),
                            cursor_updates.dirty,
                            session_controls.pending_len() != 0,
                        ))
                    },
                    Ok,
                )
        };
        native_frame_service_preempted_previous_cycle = native_frame_service_preemption;
        if session_controls.pending_len() == 0 {
            native_frame_control_priority_cycles = 0;
        } else if native_frame_service_preemption {
            native_frame_control_priority_cycles = 0;
        } else {
            native_frame_control_priority_cycles =
                native_frame_control_priority_cycles.saturating_add(1);
        }
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
                if batch.selection_owner_change {
                    selection_owner_changes = selection_owner_changes.saturating_add(1);
                    if config.firefox_proof_requested() {
                        println!(
                            "sophia_firefox_m8 schema=1 status=selection_observed kind=owner_change count={selection_owner_changes} content=redacted"
                        );
                    }
                }
                if batch.selection_conversion {
                    selection_conversions = selection_conversions.saturating_add(1);
                    if config.firefox_proof_requested() {
                        println!(
                            "sophia_firefox_m8 schema=1 status=selection_observed kind=conversion count={selection_conversions} content=redacted"
                        );
                    }
                }
                if config.firefox_proof_requested() {
                    for metadata in &batch.metadata {
                        if config.firefox_m10_rendering_proof
                            && !firefox_m10_rendering_page_ready
                            && metadata.property_name == "_NET_WM_NAME"
                            && metadata.byte_len == 249
                        {
                            firefox_m10_rendering_page_ready = true;
                            println!(
                                "sophia_firefox_rendering schema=1 status=page_ready title_bytes=249 content=redacted"
                            );
                        }
                        if config.firefox_m10_dialog_proof
                            && let Some(checkpoint) = firefox_m10_dialog_proof
                                .observe(&metadata.property_name, metadata.byte_len)
                        {
                            println!(
                                "sophia_firefox_dialog schema=1 status=checkpoint checkpoint={checkpoint} title_bytes={} content=redacted",
                                metadata.byte_len,
                            );
                        }
                        if config.firefox_m10_primary_proof
                            && metadata.property_name == "_NET_WM_NAME"
                        {
                            println!(
                                "sophia_firefox_primary schema=1 status=title_observed title_bytes={} content=redacted",
                                metadata.byte_len
                            );
                            if let Some(checkpoint) = firefox_m10_primary_proof
                                .observe(&metadata.property_name, metadata.byte_len)
                            {
                                println!(
                                    "sophia_firefox_primary schema=1 status=checkpoint checkpoint={checkpoint} title_bytes={} content=redacted",
                                    metadata.byte_len,
                                );
                            }
                        }
                        if !firefox_m8_page_ready_reported
                            && metadata.property_name == "_NET_WM_NAME"
                            && metadata.byte_len == 36
                        {
                            firefox_m8_page_ready_reported = true;
                            println!(
                                "sophia_firefox_m8 schema=1 status=page_ready title_bytes=36 content=redacted"
                            );
                        }
                        if !firefox_m8_navigation_ready_reported
                            && firefox_m8_proof.navigation_ready(
                                &metadata.property_name,
                                metadata.byte_len,
                            )
                        {
                            firefox_m8_navigation_ready_reported = true;
                            println!(
                                "sophia_firefox_m8 schema=1 status=navigation_ready content=redacted"
                            );
                        }
                        if !firefox_m8_dialog_ready_reported
                            && firefox_m8_proof.dialog_ready(
                                &metadata.property_name,
                                metadata.byte_len,
                            )
                        {
                            firefox_m8_dialog_ready_reported = true;
                            println!(
                                "sophia_firefox_m8 schema=1 status=dialog_ready content=redacted"
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
                        if (config.firefox_m10_proof || config.firefox_m10_lifecycle_proof)
                            && let Some((terminal, checkpoint)) = firefox_m10_kitty_proof
                                .observe(&metadata.property_name, metadata.byte_len)
                        {
                            println!(
                                "sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal={terminal} checkpoint={checkpoint} content=redacted"
                            );
                        }
                        if config.firefox_m10_selection_proof
                            && let Some(checkpoint) = firefox_m10_selection_kitty_proof
                                .observe(&metadata.property_name, metadata.byte_len)
                        {
                            println!(
                                "sophia_firefox_selection schema=1 status=kitty_checkpoint checkpoint={checkpoint} content=redacted"
                            );
                        }
                    }
                }
                let has_engine_work = !batch.transactions.is_empty()
                    || !batch.surface_presentations.is_empty()
                    || !batch.presentation_intents.is_empty()
                    || !batch.removed_surfaces.is_empty()
                    || !batch.surface_output_reservations.is_empty()
                    || !batch.cpu_buffer_updates.is_empty()
                    || !batch.dma_buf_registrations.is_empty()
                    || !batch.fence_registrations.is_empty()
                    || !batch.present_submissions.is_empty()
                    || !batch.released_dma_bufs.is_empty()
                    || !batch.released_fences.is_empty()
                    || runtime
                        .as_ref()
                        .is_some_and(LiveProductionVisualRuntime::has_released_surface_content);
                if !has_engine_work && pending_wm_update.is_none() {
                    continue;
                }
                if !wm_only_cycle {
                    last_authority_update = Instant::now();
                    metrics.batches = metrics.batches.saturating_add(1);
                    metrics.transactions = metrics.transactions.saturating_add(
                        authority_transaction_count(&batch.transactions),
                    );
                    metrics.cpu_buffer_updates = metrics
                        .cpu_buffer_updates
                        .saturating_add(batch.cpu_buffer_updates.len());
                    metrics.cpu_buffer_replacements =
                        metrics.cpu_buffer_replacements.saturating_add(
                            batch
                                .cpu_buffer_updates
                                .iter()
                                .filter(|update| update.is_replacement())
                                .count(),
                        );
                    metrics.cpu_buffer_patch_updates =
                        metrics.cpu_buffer_patch_updates.saturating_add(
                            batch
                                .cpu_buffer_updates
                                .iter()
                                .filter(|update| !update.is_replacement())
                                .count(),
                        );
                    metrics.cpu_buffer_patch_rects =
                        metrics.cpu_buffer_patch_rects.saturating_add(
                            batch
                                .cpu_buffer_updates
                                .iter()
                                .map(sophia_x_authority::XAuthorityCpuBufferUpdate::patch_rects)
                                .sum::<usize>(),
                        );
                    metrics.cpu_buffer_payload_bytes =
                        metrics.cpu_buffer_payload_bytes.saturating_add(
                            batch
                                .cpu_buffer_updates
                                .iter()
                                .map(sophia_x_authority::XAuthorityCpuBufferUpdate::payload_bytes)
                                .sum::<usize>(),
                        );
                    metrics.dma_buf_registrations_observed = metrics
                        .dma_buf_registrations_observed
                        .saturating_add(batch.dma_buf_registrations.len());
                    metrics.fence_registrations_observed = metrics
                        .fence_registrations_observed
                        .saturating_add(batch.fence_registrations.len());
                    metrics.present_submissions_observed = metrics
                        .present_submissions_observed
                        .saturating_add(batch.present_submissions.len());
                    metrics.software_present_submissions_observed = metrics
                        .software_present_submissions_observed
                        .saturating_add(batch.software_present_submissions.len());
                }
                let removed_surfaces = batch.removed_surfaces.clone();
                for surface in &removed_surfaces {
                    let seats = application_route_leases
                        .leases()
                        .filter(|lease| {
                            lease.target_surface == *surface
                                && !matches!(
                                    lease.phase,
                                    sophia_engine::ApplicationRouteLeasePhase::Releasing { .. }
                                )
                        })
                        .map(|lease| lease.identity.seat)
                        .collect::<Vec<_>>();
                    for lease_seat in seats {
                        request_application_route_lease_release(
                            &mut application_route_leases,
                            &layout.client_routes,
                            route_lease_release_sender,
                            lease_seat,
                            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                        )?;
                    }
                }
                if let Some(wm_session) = wm_session.as_mut() {
                    for surface in &removed_surfaces {
                        require_wm_request_admission(
                            wm_session.enqueue_surface_removed(*surface)?,
                            "surface_removed",
                        )?;
                    }
                }
                let layout_observation = layout.observe_authority_batch(&batch);
                if layout_observation.admission_group_invalid {
                    return Err("pre-admission authority group is not transaction-homogeneous".into());
                }
                if layout_observation.admission_group_overflowed {
                    return Err("pre-admission authority-group capacity exceeded".into());
                }
                if layout_observation.output_reservations_changed
                    && let Some(wm_session) = wm_session.as_mut()
                {
                    require_wm_request_admission(
                        wm_session.update_output_work_areas(&layout, &outputs, output)?,
                        "work_area",
                    )?;
                }
                if let Some(wm_session) = wm_session.as_mut() {
                    for surface in &layout_observation.withdrawn_surfaces {
                        require_wm_request_admission(
                            wm_session.enqueue_surface_removed(*surface)?,
                            "surface_withdrawn",
                        )?;
                    }
                }
                for surface in layout_observation.new_surfaces {
                    if session_launches.observe_surface(surface) {
                        println!(
                            "sophia_session_app schema=2 status=surface_observed source=action transaction={} surface={}",
                            session_launches
                                .admission()
                                .expect("observed launch surface requires an admission")
                                .intent
                                .transaction
                                .raw(),
                            surface.index(),
                        );
                    }
                }
                service_layout_progress!("authority");
                let mut wm_update = pending_wm_update.take();
                if !resize_proof_complete
                    && let Some((transaction, surface, size)) = resize_proof
                    && layout.pending.is_none()
                    && layout.layout_epochs.committed_size(surface) == Some(size)
                {
                    if resize_proof_targets.len() > 1 {
                        println!(
                            "sophia_live_resize schema=2 status=committed transaction={} surface={} width={} height={} configure_delivered=true pixels=true step={} total={}",
                            transaction.raw(),
                            surface.index(),
                            size.width,
                            size.height,
                            resize_proof_completed + 1,
                            resize_proof_targets.len(),
                        );
                    } else {
                        println!(
                            "sophia_live_resize schema=1 status=committed transaction={} surface={} width={} height={} configure_delivered=true pixels=true",
                            transaction.raw(),
                            surface.index(),
                            size.width,
                            size.height,
                        );
                    }
                    // Only exact committed geometry and pixels advance the sequence.
                    resize_proof = None;
                    resize_proof_completed = resize_proof_completed.saturating_add(1);
                    resize_proof_complete = resize_proof_completed == resize_proof_targets.len();
                    if resize_proof_complete && resize_proof_targets.len() > 1 {
                        println!(
                            "sophia_live_resize_storm schema=1 status=complete steps={} surface={} exact_pixels=true",
                            resize_proof_completed,
                            surface.index(),
                        );
                    }
                }
                if wm_update.is_none() {
                    if let Some(result) =
                        layout.expire_pending(&mut session_controls)?
                    {
                        wm_update = Some(apply_wm_commit_result!(
                            result,
                            focus.focused_surface(seat)
                        ));
                    }
                }
                if wm_update.is_none()
                    && layout.pending.is_none()
                    && let Some(surface) = layout.next_unmanaged_surface()
                    && let Some(wm_session) = wm_session.as_mut()
                {
                    require_wm_request_admission(
                        wm_session.enqueue_manage(surface, &layout, output)?,
                        "manage",
                    )?;
                }
                if resize_proof.is_none()
                    && !resize_proof_complete
                    && let Some(size) = resize_proof_targets.get(resize_proof_completed).copied()
                    && startup_ready_reported
                    && wm_session.as_ref().is_none_or(|wm| wm.committed > 0)
                    && layout.layers.len() >= if config.secondary_terminal { 2 } else { 1 }
                    && layout.pending.is_none()
                {
                    let surface = layout
                        .layers
                        .keys()
                        .next()
                        .copied()
                        .ok_or("surface resize proof has no target")?;
                    let transaction = TransactionId::from_raw(
                        2_000_000u64.saturating_add(
                            u64::try_from(resize_proof_completed).unwrap_or(u64::MAX),
                        ),
                    );
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
                        configure_deliveries: 0,
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
                        source: None,
                        effects: None,
                        policy_settlement: None,
                    };
                    if let Some(result) =
                        layout.stage(proposal, &mut session_controls)?
                    {
                        wm_update = Some(apply_wm_commit_result!(
                            result,
                            focus.focused_surface(seat)
                        ));
                    }
                    resize_proof = Some((transaction, surface, size));
                    if resize_proof_targets.len() > 1 {
                        println!(
                            "sophia_live_resize schema=2 status=requested transaction={} surface={} width={} height={} step={} total={}",
                            transaction.raw(),
                            surface.index(),
                            size.width,
                            size.height,
                            resize_proof_completed + 1,
                            resize_proof_targets.len(),
                        );
                    } else {
                        println!(
                            "sophia_live_resize schema=1 status=requested transaction={} surface={} width={} height={}",
                            transaction.raw(),
                            surface.index(),
                            size.width,
                            size.height,
                        );
                    }
                }
                layout.write_pending_cpu_buffer_handles(&mut staged_cpu_buffer_handles);
                let (batch, released_admission_groups) = layout.projected_batch(&batch);
                let production_batch =
                    production_authority_batch(&batch, &released_admission_groups, &layout)?;
                if runtime.is_none() {
                    runtime = Some(
                        LiveProductionVisualRuntime::new(&outputs, native_scanout.as_mut(), None)?
                            .with_m4_proof_controls(
                                config.m4_first_acquire_delay,
                                config.m4_reject_first_present,
                                config.m4_diagnose_first_mixed_export,
                            )
                            .with_surface_chrome_style(
                                wm_session
                                    .as_ref()
                                    .and_then(|wm| wm.surface_chrome_style())
                                    .unwrap_or(config.surface_chrome_style),
                            ),
                    );
                }
                let runtime = runtime
                    .as_mut()
                    .expect("persistent backend runtime was initialized above");
                if let Some(update) = wm_update.as_ref() {
                    match update.commit.outcome {
                        TransactionOutcome::Committed => {
                            let staged = runtime.commit_layout_epoch(update.commit.transaction);
                            if staged != 0 {
                                println!(
                                    "sophia_live_resize_epoch schema=3 status=queue_committed epoch={} staged_presents={staged}",
                                    update.commit.transaction.raw(),
                                );
                            }
                        }
                        TransactionOutcome::TimedOut => {
                            let report = runtime.abort_layout_epoch(update.commit.transaction);
                            println!(
                                "sophia_live_resize_epoch schema=3 status=queue_aborted epoch={} rejected_presents={} recovery_extents={}",
                                update.commit.transaction.raw(),
                                report.rejected.len(),
                                layout.recovery_extent_count(),
                            );
                        }
                        _ => {}
                    }
                }
                // Layout progress can promote staged WM chrome after the
                // earlier WM phase. Synchronize again at the production
                // boundary so the committed clearance and rendered chrome
                // always belong to the same transaction.
                let surface_chrome_style = wm_session
                    .as_ref()
                    .and_then(|wm| wm.surface_chrome_style())
                    .unwrap_or(config.surface_chrome_style);
                synchronize_runtime_surface_chrome_style(runtime, surface_chrome_style);
                if layout.pending.is_none() {
                    runtime.release_layout_deferred_presentations();
                }
                let raised_surface = layout
                    .top_client_positioned_surface()
                    .or_else(|| focus.focused_surface(seat));
                let focused_surface = focus.focused_surface(seat);
                let cursor_presentation = if native_scanout.is_some() {
                    LiveProductionCursorPresentation::HardwarePlane
                } else {
                    LiveProductionCursorPresentation::Software(pointer.position())
                };
                let mut presentation_layout = Vec::with_capacity(layout.layers.len());
                for layer in layout.layers.values() {
                    let visible = if layout.is_client_positioned(layer.surface) {
                        layout.client_positioned_mapped(layer.surface)
                            && match layout.presentation_owner(layer.surface) {
                                Some(owner) if !layout.knows_surface(owner) => false,
                                Some(owner) => match wm_session.as_ref() {
                                    Some(wm) => wm.surface_visible_on_output(owner, output.id)?,
                                    None => layout.mapped_surfaces.contains(&owner),
                                },
                                None => true,
                            }
                    } else {
                        match wm_session.as_ref() {
                        Some(wm) => {
                            wm.surface_visible_on_output(layer.surface, output.id)?
                        }
                        None => true,
                        }
                    };
                    if visible {
                        presentation_layout.push(layer.clone());
                    }
                }
                presentation_layout.sort_by_key(|layer| layer.stack_rank);
                let chrome_surfaces = presentation_layout
                    .iter()
                    .filter(|layer| !layout.is_client_positioned(layer.surface))
                    .map(|layer| layer.surface)
                    .collect::<Vec<_>>();
                let native_owner_policy = production_cycle_native_owner_policy(
                    native_scanout.is_some(),
                    defer_cpu_frame,
                );
                let (_tick, report, committed_surfaces, composed, compose_elapsed) =
                    if !production_batch.has_dma_buf_present_submissions()
                        && !runtime.released_surface_content_requires_gpu()
                    {
                        let (submission, committed_surfaces) =
                            runtime.run_cpu_production_cycle(LiveProductionCycleRequest {
                                batch: &production_batch,
                                scene: &mut scene,
                                raised_surface,
                                focused_surface,
                                cursor_presentation,
                                defer_frame: defer_cpu_frame,
                                output_descriptors: &outputs,
                                native_scanout: native_scanout.as_mut().filter(|_| {
                                    native_owner_policy
                                        == ProductionCycleNativeOwnerPolicy::Available
                                }),
                                wm_update,
                                presentation_layout: &presentation_layout,
                                chrome_surfaces: &chrome_surfaces,
                                staged_cpu_buffer_handles: &staged_cpu_buffer_handles,
                            })?;
                        (
                            submission.tick,
                            submission.composition,
                            committed_surfaces,
                            submission.composed,
                            submission.compose_elapsed,
                        )
                    } else {
                        let (submission, committed_surfaces) =
                            runtime.run_gpu_production_cycle(LiveProductionCycleRequest {
                                batch: &production_batch,
                                scene: &mut scene,
                                raised_surface,
                                focused_surface,
                                cursor_presentation,
                                defer_frame: defer_cpu_frame,
                                output_descriptors: &outputs,
                                native_scanout: native_scanout.as_mut().filter(|_| {
                                    native_owner_policy
                                        == ProductionCycleNativeOwnerPolicy::Available
                                }),
                                wm_update,
                                presentation_layout: &presentation_layout,
                                chrome_surfaces: &chrome_surfaces,
                                staged_cpu_buffer_handles: &staged_cpu_buffer_handles,
                            })?;
                        (
                            submission.tick,
                            submission.composition,
                            committed_surfaces,
                            submission.composed,
                            submission.compose_elapsed,
                        )
                    };
                if let Some(ring) = runtime.take_focus_ring_observation() {
                    println!(
                        "sophia_live_compositor_chrome schema=2 status=focus_ring_composed surface={} generation={} primitives={}",
                        ring.surface.index(),
                        ring.generation,
                        ring.primitives,
                    );
                }
                if let Some(chrome) = runtime.take_chrome_set_observation() {
                    println!(
                        "sophia_live_compositor_chrome_set schema=1 status=composed generation={} eligible_surfaces={} frames={} focused_frames={} unfocused_frames={} focus_rings={} primitives={} clearance={}",
                        chrome.generation,
                        chrome.eligible_surfaces,
                        chrome.frames,
                        chrome.focused_frames,
                        chrome.unfocused_frames,
                        chrome.focus_rings,
                        chrome.primitives,
                        chrome.clearance,
                    );
                }
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
                    key_repeat.cancel_surface(surface);
                    let abandoned = clear_client_pressed_keys_state_only(
                        surface,
                        &mut client_keys,
                        &mut client_key_scratch,
                        &mut modifiers,
                        input_sender,
                        &mut input_delivery.next,
                        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    )?;
                    if abandoned != 0 {
                        eprintln!(
                            "sophia_live_session_keys schema=1 status=abandoned reason=surface_removed surface={} count={abandoned}",
                            surface.index(),
                        );
                    }
                    if applied_client_focus == Some(surface) {
                        applied_client_focus = None;
                    }
                    if input_content_surface == Some(surface) {
                        input_content_surface = None;
                    }
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
                    session_controls: &mut session_controls,
                    next_focus_control_transaction: &mut next_focus_control_transaction,
                })?;
                reconcile_pending_wm_focus!(runtime);
                if let Some(surface) = focus.focused_surface(seat) {
                    let cpu_visual_detail =
                        scene.surface_has_visual_detail(runtime.committed_surfaces(), surface);
                    if !startup_content_ready && cpu_visual_detail {
                        startup_content_ready = true;
                        let _ = reduce_session_startup(
                            &mut startup_readiness,
                            SessionStartupEvent::VisualDetail(surface),
                        );
                        let focused_geometry = runtime
                            .committed_surfaces()
                            .iter()
                            .find(|committed| committed.surface == surface)
                            .map(|committed| committed.geometry);
                        let output_bounds = wm_output_bounds(&outputs);
                        startup_required_submissions = native_scanout.as_ref().map(|native| {
                            native
                                .heads
                                .iter()
                                .map(|head| {
                                    let intersects = output_bounds
                                        .iter()
                                        .find(|(output, _)| *output == head.output.id)
                                        .is_some_and(|(_, bounds)| {
                                            focused_geometry.is_some_and(|geometry| {
                                                rects_intersect(geometry, *bounds)
                                            })
                                        });
                                    startup_submission_requirement(
                                        head.submissions,
                                        head.presented_submissions,
                                        intersects,
                                    )
                                })
                                .collect()
                        });
                        println!(
                            "sophia_live_session_startup schema=1 status=content_ready source=cpu_visual_detail"
                        );
                        std::io::stdout().flush()?;
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                let expired = layout.expire_pending(&mut session_controls)?;
                if let Some(result) = expired {
                    pending_wm_update = Some(apply_wm_commit_result!(
                        result,
                        focus.focused_surface(seat)
                    ));
                }
                if layout.pending.is_none()
                    && let Some(surface) = layout.next_unmanaged_surface()
                    && let Some(wm_session) = wm_session.as_mut()
                {
                    require_wm_request_admission(
                        wm_session.enqueue_manage(surface, &layout, output)?,
                        "manage",
                    )?;
                }
                if let (Some(runtime), Some(native_scanout)) =
                    (runtime.as_mut(), native_scanout.as_mut())
                {
                    if layout.pending.is_none() {
                        runtime.release_layout_deferred_presentations();
                    }
                    let service = runtime.service_native(native_scanout)?;
                    last_native_frame_service = Instant::now();
                    let native_work_remains = native_frame_service_requires_owner_progress(
                        &runtime.native_output_service_request(native_scanout)?,
                    );
                    if native_work_remains {
                        native_frame_service_deadline_armed = true;
                        native_frame_idle_service_cycles = 0;
                    } else if native_frame_service_deadline_armed {
                        native_frame_idle_service_cycles =
                            native_frame_idle_service_cycles.saturating_add(1);
                        if native_frame_idle_service_cycles >= 4 {
                            native_frame_service_deadline_armed = false;
                            native_frame_idle_service_cycles = 0;
                        }
                    }
                    if let Some(retired) = service.retired_present {
                        let NativePresentRetirementObservation {
                            surface,
                            stable,
                            ust_usec: _,
                            msc: _,
                        } =
                            record_native_present_retirement(
                                &mut layout,
                                runtime,
                                native_scanout,
                                retired,
                                &mut retired_present_surfaces,
                                &mut startup_surface_presentations,
                                &mut startup_readiness,
                            );
                        if stable_gpu_frame_proves_post_input_pixels(
                            input_proof_started_at.is_some(),
                            input_surface,
                            surface,
                            stable,
                        ) {
                            input_pixel_change = true;
                        }
                    }
                    for retired in service.retired_software_presents {
                        record_native_software_present_retirement(&mut layout, retired);
                    }
                    correlate_physical_input_page_flip(
                        input_proof_started_at.is_some(),
                        input_pixel_change,
                        input_raw_ingress_msec,
                        input_change_submission_baseline,
                        native_scanout,
                        &mut input_presented_ust_usec,
                        &mut input_submit_to_page_flip,
                    );
                    metrics.backend_ticks = metrics
                        .backend_ticks
                        .saturating_add(service.ticks.len());
                    metrics.runtime_surfaces =
                        u64::try_from(runtime.committed_surfaces().len()).unwrap_or(u64::MAX);
                    reconcile_initial_session_focus(InitialSessionFocusContext {
                        runtime,
                        focus: &mut focus,
                        seat,
                        wm_session_present: wm_session.is_some(),
                        layout: &layout,
                        session_controls: &mut session_controls,
                        next_focus_control_transaction: &mut next_focus_control_transaction,
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

}
