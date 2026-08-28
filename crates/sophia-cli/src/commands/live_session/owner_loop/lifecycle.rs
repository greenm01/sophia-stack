{
        if let Some(controller) = seat_controller.as_mut() {
            if let Some(event) = controller.dispatch()? {
                seat_state = seat_state.observe(event);
            }
            if seat_state == sophia_backend_live::LiveSeatState::Active
                && let Some((terminal, queued_at)) = pending_virtual_terminal
            {
                InputDeliveryPhase {
                    receiver: input_delivery_receiver,
                    state: &mut input_delivery,
                    client_key_release_barrier: &mut client_key_release_barrier,
                    proof_started_at: &mut input_proof_started_at,
                    post_input_deadline: &mut post_input_deadline,
                }
                .drain()?;
                if !input_delivery.pending.is_empty() {
                    if queued_at.elapsed() >= Duration::from_millis(500) {
                        pending_virtual_terminal = None;
                        modifiers = config.keyboard_mapper();
                        virtual_terminal_chord = VirtualTerminalChordState::default();
                        if let Some(wm) = wm_session.as_mut()
                            && let Some(shortcuts) = wm.shortcuts.as_mut()
                        {
                            let _ = shortcuts.clear_seat(seat);
                        }
                        eprintln!(
                            "sophia_live_session_vt schema=4 status=rejected target={terminal} phase=modifier_release_timeout pending_deliveries={}",
                            input_delivery.pending.len(),
                        );
                    } else {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    continue;
                }
                pending_virtual_terminal = None;
                println!(
                    "sophia_live_session_vt schema=4 status=preparing target={terminal}"
                );
                std::io::stdout().flush()?;
                let revoked_input_leases = advance_application_input_security_epoch(
                    &mut application_route_leases,
                    input_sender,
                    &layout.client_routes,
                    route_lease_release_sender,
                )?;
                revoke_floating_pointer_interaction!("virtual_terminal");
                revoke_chrome_captures!("virtual_terminal");
                keyboard_focus_handoff = KeyboardFocusHandoffState::default();
                deferred_physical_key_timings.clear();
                println!(
                    "sophia_live_input_epoch schema=1 reason=virtual_terminal epoch={} revoked_leases={revoked_input_leases}",
                    application_route_leases.control_epoch(),
                );
                physical_input.take();
                let quiesced = if let (Some(runtime), Some(native)) =
                    (runtime.as_mut(), native_scanout.as_mut())
                {
                    runtime
                        .suspend_native_scanout(native, &outputs, Duration::from_secs(2))
                } else {
                    Ok(Default::default())
                };
                match quiesced {
                    Ok(report) => {
                        suspended_renderer_images = match (runtime.as_ref(), native_scanout.as_mut())
                        {
                            (Some(runtime), Some(native)) => {
                                Some(capture_renderer_image_handoff(runtime, native, output.id)?)
                            }
                            _ => None,
                        };
                        println!(
                            "sophia_live_renderer_handoff schema=1 status=captured images={}",
                            suspended_renderer_images.as_ref().map_or(0, |handoff| handoff.len()),
                        );
                        native_scanout.take();
                        seat_release_prepared = true;
                        println!(
                            "sophia_live_session_vt schema=6 status=quiesced target={terminal} outcome={} drained={} abandoned_scanouts={} skipped_present={}",
                            report.outcome.reduced_name(),
                            report.outcome.drained(),
                            report.abandoned_scanouts,
                            report
                                .skipped_present
                                .map_or_else(|| "none".to_owned(), |transaction| transaction.raw().to_string()),
                        );
                        match controller.switch_session(terminal) {
                            Ok(()) => {
                                requested_virtual_terminal =
                                    Some((terminal, Instant::now()));
                                println!(
                                    "sophia_live_session_vt schema=4 status=requested target={terminal}"
                                );
                                std::io::stdout().flush()?;
                                continue;
                            }
                            Err(error) => {
                                seat_release_prepared = false;
                                let mut resumed = LiveProductionNativeScanout::new_with_seat_mirroring_and_mapping(
                                    &controller.device_opener(),
                                    mirror_grouping,
                                    initial_head_mapping,
                                )?;
                                if resumed.outputs() != outputs {
                                    schedule_output_topology_rebuild!("switch_rejected", true);
                                    drop(resumed);
                                } else {
                                    let restored = resume_native_scanout_from_scene(
                                        runtime.as_mut().ok_or(
                                            "seat switch rejection lost the visual runtime",
                                        )?,
                                        &mut resumed,
                                        &outputs,
                                        &mut scene,
                                        suspended_renderer_images.take(),
                                    )?;
                                    publish_resumed_topology_transport!(resumed);
                                    *native_scanout = Some(resumed);
                                    println!(
                                        "sophia_live_renderer_handoff schema=1 status=restored images={restored} source=switch_rejected"
                                    );
                                }
                                let device_map =
                                    sophia_backend_live::NativeLibinputDeviceMap::new(
                                        SeatId::from_raw(SESSION_SEAT_RAW),
                                    )
                                    .with_keyboard_device(DeviceId::from_raw(
                                        SESSION_KEYBOARD_DEVICE_RAW,
                                    ))
                                    .with_pointer_device(DeviceId::from_raw(
                                        SESSION_POINTER_DEVICE_RAW,
                                    ));
                                *physical_input = open_session_physical_input(
                                    config,
                                    device_map,
                                    Some(controller.device_opener()),
                                )?;
                                modifiers = config.keyboard_mapper();
                                virtual_terminal_chord = VirtualTerminalChordState::default();
                                emergency_chord = EmergencyChordState::armed();
                                cursor_updates =
                                    CursorUpdateState::new(pointer.position().is_some());
                                eprintln!(
                                    "sophia_live_session_vt schema=4 status=rejected target={terminal} phase=request error={error}"
                                );
                            }
                        }
                    }
                    Err(error) => {
                        let device_map = sophia_backend_live::NativeLibinputDeviceMap::new(
                            SeatId::from_raw(SESSION_SEAT_RAW),
                        )
                        .with_keyboard_device(DeviceId::from_raw(SESSION_KEYBOARD_DEVICE_RAW))
                        .with_pointer_device(DeviceId::from_raw(SESSION_POINTER_DEVICE_RAW));
                        *physical_input = open_session_physical_input(
                            config,
                            device_map,
                            Some(controller.device_opener()),
                        )?;
                        modifiers = config.keyboard_mapper();
                        virtual_terminal_chord = VirtualTerminalChordState::default();
                        emergency_chord = EmergencyChordState::armed();
                        eprintln!(
                            "sophia_live_session_vt schema=4 status=rejected target={terminal} phase=quiesce error={error}"
                        );
                    }
                }
                std::io::stdout().flush()?;
            }
            if seat_state == sophia_backend_live::LiveSeatState::Active
                && let Some((terminal, requested_at)) = requested_virtual_terminal
                && requested_at.elapsed() >= Duration::from_secs(2)
            {
                requested_virtual_terminal = None;
                seat_release_prepared = false;
                let mut resumed =
                    LiveProductionNativeScanout::new_with_seat_mirroring_and_mapping(
                    &controller.device_opener(),
                    &mirror_grouping,
                    initial_head_mapping,
                )?;
                if resumed.outputs() != outputs {
                    schedule_output_topology_rebuild!("switch_timeout", true);
                    drop(resumed);
                } else {
                    let restored = resume_native_scanout_from_scene(
                        runtime
                            .as_mut()
                            .ok_or("seat switch timeout lost the visual runtime")?,
                        &mut resumed,
                        &outputs,
                        &mut scene,
                        suspended_renderer_images.take(),
                    )?;
                    publish_resumed_topology_transport!(resumed);
                    *native_scanout = Some(resumed);
                    println!(
                        "sophia_live_renderer_handoff schema=1 status=restored images={restored} source=disable_timeout"
                    );
                }
                let device_map = sophia_backend_live::NativeLibinputDeviceMap::new(
                    SeatId::from_raw(SESSION_SEAT_RAW),
                )
                .with_keyboard_device(DeviceId::from_raw(SESSION_KEYBOARD_DEVICE_RAW))
                .with_pointer_device(DeviceId::from_raw(SESSION_POINTER_DEVICE_RAW));
                *physical_input = open_session_physical_input(
                    config,
                    device_map,
                    Some(controller.device_opener()),
                )?;
                modifiers = config.keyboard_mapper();
                key_repeat.cancel_seat(seat);
                virtual_terminal_chord = VirtualTerminalChordState::default();
                emergency_chord = EmergencyChordState::armed();
                if let Some(wm) = wm_session.as_mut()
                    && let Some(shortcuts) = wm.shortcuts.as_mut()
                {
                    let _ = shortcuts.clear_seat(seat);
                }
                cursor_updates = CursorUpdateState::new(pointer.position().is_some());
                eprintln!(
                    "sophia_live_session_vt schema=4 status=rejected target={terminal} phase=disable_timeout"
                );
                std::io::stdout().flush()?;
            }
            if seat_state == sophia_backend_live::LiveSeatState::Active
                && requested_virtual_terminal.is_some()
            {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            if seat_state == sophia_backend_live::LiveSeatState::ReleasePending {
                println!("sophia_live_seat schema=1 status=release_pending");
                if !seat_release_prepared {
                    let revoked_input_leases = advance_application_input_security_epoch(
                        &mut application_route_leases,
                        input_sender,
                        &layout.client_routes,
                        route_lease_release_sender,
                    )?;
                    revoke_floating_pointer_interaction!("seat_release");
                    revoke_chrome_captures!("seat_release");
                    keyboard_focus_handoff = KeyboardFocusHandoffState::default();
                    deferred_physical_key_timings.clear();
                    println!(
                        "sophia_live_input_epoch schema=1 reason=seat_release epoch={} revoked_leases={revoked_input_leases}",
                        application_route_leases.control_epoch(),
                    );
                }
                if let Some(surface) = applied_client_focus {
                    flush_client_keys!(surface, "seat_release");
                }
                physical_input.take();
                if !seat_release_prepared
                    && let Some(runtime) = runtime.as_mut()
                {
                    let report = runtime.suspend_revoked_native_scanout(&outputs)?;
                    let discarded_renderer_images = runtime.discard_retained_renderer_images();
                    suspended_renderer_images = None;
                    println!(
                        "sophia_live_seat schema=2 status=forced_detach abandoned_scanouts={} skipped_present={}",
                        report.abandoned_scanouts,
                        report
                            .skipped_present
                            .map_or_else(|| "none".to_owned(), |transaction| transaction.raw().to_string()),
                    );
                    println!(
                        "sophia_live_renderer_handoff schema=1 status=discarded images={discarded_renderer_images} source=forced_detach"
                    );
                }
                native_scanout.take();
                controller.acknowledge_disable()?;
                seat_state = seat_state.released();
                seat_release_prepared = false;
                requested_virtual_terminal = None;
                modifiers = config.keyboard_mapper();
                key_repeat.cancel_seat(seat);
                virtual_terminal_chord = VirtualTerminalChordState::default();
                emergency_chord = EmergencyChordState::armed();
                println!("sophia_live_seat schema=1 status=suspended");
                std::io::stdout().flush()?;
            }
            if seat_state == sophia_backend_live::LiveSeatState::AcquirePending {
                println!("sophia_live_seat schema=1 status=acquire_pending");
                let mut resumed =
                    LiveProductionNativeScanout::new_with_seat_mirroring_and_mapping(
                    &controller.device_opener(),
                    &mirror_grouping,
                    initial_head_mapping,
                )?;
                if resumed.outputs() != outputs {
                    schedule_output_topology_rebuild!("seat_resume", true);
                    drop(resumed);
                } else {
                    let frames = scene.frames_for_outputs(&outputs)?;
                    let scene_outputs = frames.len();
                    let nonzero_scene_outputs = frames
                        .iter()
                        .filter(|frame| frame.nonzero_pixel_bytes > 0)
                        .count();
                    let primary_nonzero_pixel_bytes = frames
                        .first()
                        .map_or(0, |frame| frame.nonzero_pixel_bytes);
                    let restored = resume_native_scanout_from_scene(
                        runtime
                            .as_mut()
                            .ok_or("seat resume lost the visual runtime")?,
                        &mut resumed,
                        &outputs,
                        &mut scene,
                        suspended_renderer_images.take(),
                    )?;
                    publish_resumed_topology_transport!(resumed);
                    *native_scanout = Some(resumed);
                    // CPU snapshots live in the Engine scene, outside the imported
                    // renderer-image table. Record both recovery paths separately.
                    println!(
                        "sophia_live_scene_handoff schema=1 status=rehydrated outputs={scene_outputs} nonzero_outputs={nonzero_scene_outputs} primary_nonzero_pixel_bytes={primary_nonzero_pixel_bytes} source=seat_resume"
                    );
                    println!(
                        "sophia_live_renderer_handoff schema=1 status=restored images={restored} source=seat_resume"
                    );
                }
                let device_map = sophia_backend_live::NativeLibinputDeviceMap::new(
                    SeatId::from_raw(SESSION_SEAT_RAW),
                )
                .with_keyboard_device(DeviceId::from_raw(SESSION_KEYBOARD_DEVICE_RAW))
                .with_pointer_device(DeviceId::from_raw(SESSION_POINTER_DEVICE_RAW));
                *physical_input = open_session_physical_input(
                    config,
                    device_map,
                    Some(controller.device_opener()),
                )?;
                cursor_updates = CursorUpdateState::new(pointer.position().is_some());
                seat_state = seat_state.acquired();
                println!("sophia_live_seat schema=1 status=active source=resume");
                std::io::stdout().flush()?;
            }
            if seat_state == sophia_backend_live::LiveSeatState::Suspended {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            if seat_state == sophia_backend_live::LiveSeatState::Failed {
                return Err("invalid libseat lifecycle transition".into());
            }
        }
        let child_reap_started = Instant::now();
        if !primary_child_exited
            && let Some(primary_child) = child.as_deref_mut()
            && let Some(status) = primary_child.try_wait()?
        {
            primary_exit_status = Some(status);
            if !status.success() && !config.normal_session {
                let error =
                    format!("session client exited during live session with status {status}");
                terminal_client_error = Some(("primary", error));
                match frontend_service_sender.send(XServerFrontendServiceCommand::StopAccepting) {
                    Ok(()) => terminal_client_intake_stopped = true,
                    Err(error) => terminal_client_cleanup_failures
                        .push(format!("frontend intake stop failed: {error}")),
                }
                println!(
                    "sophia_live_session_client_fatal schema=1 status=detected source=primary exit_status={status} action=bounded_cleanup"
                );
                break 'session;
            }
            if status.success()
                && config.expect_physical_pointer
                && metrics.physical_pointer_buttons_routed == 0
            {
                return Err(
                    "session client exited before the required physical pointer selection".into(),
                );
            }
            if config.application_proof_requested() {
                client_stdout = client_stdout_capture
                    .ok_or("application stdout capture is missing")?
                    .read_bounded()?;
                if client_stdout.len() > 4_096 {
                    return Err("application stdout exceeded the 4096-byte evidence bound".into());
                }
                if let (Some(text), Some(expected)) = (
                    config.inject_text.as_deref(),
                    config.expect_client_stdout.as_deref(),
                ) && client_stdout == expected.as_bytes()
                {
                    input_text_match = true;
                    println!(
                        "sophia_live_session_input schema=3 status=semantic_complete source=synthetic text_match=true bytes={}",
                        text.len()
                    );
                }
            }
            if config.normal_session {
                if let Some(id) = config.applications.startup.first() {
                    println!(
                        "sophia_session_app schema=1 status=exited id={id} source=startup exit_status={status}",
                    );
                }
                primary_child_exited = true;
                if config.exit_when_startup_exits {
                    break;
                }
            } else {
                if status.success()
                    && successful_primary_exit_ends_session(config.input_proof_requested())
                {
                    break;
                }
                // The proof helper intentionally exits after displaying its
                // received text. Keep the session and secondary terminal alive so
                // the final native frame can retire and pointer evidence can run.
                primary_child_exited = true;
            }
        }
        if config.application_proof_requested()
            && !input_text_match
            && physical_text_proof
                .as_ref()
                .is_some_and(PhysicalTextProof::is_complete)
        {
            input_text_match = true;
            println!(
                "sophia_live_session_input schema=3 status=semantic_complete source=physical text_match=true bytes={}",
                config.expect_physical_text.as_ref().map_or(0, String::len)
            );
        }
        let mut secondary_index = 0;
        while secondary_index < secondary_children.len() {
            if let Some(status) = secondary_children[secondary_index].child.try_wait()? {
                if managed_child_exit_is_nonfatal(
                    config.normal_session,
                    secondary_children[secondary_index].launch_transaction,
                ) {
                    terminate_session_child(&mut secondary_children[secondary_index].child, true)?;
                    let launch_transaction =
                        secondary_children[secondary_index].launch_transaction;
                    let id = secondary_children[secondary_index]
                        .id
                        .as_deref()
                        .unwrap_or("untracked");
                    println!(
                        "sophia_session_app schema=1 status=exited id={id} source=managed exit_status={status}",
                    );
                    let exiting_admission = launch_transaction.is_some_and(|transaction| {
                        session_launches
                            .admission()
                            .is_some_and(|admission| admission.intent.transaction == transaction)
                    });
                    if exiting_admission
                        && status.success()
                        && let Some(admission) = session_launches.complete_observed_exit()
                    {
                        launch_admission_started_at = None;
                        println!(
                            "sophia_session_app schema=2 status=completed id={id} source=action transaction={} reason=normal_exit_after_surface exit_status={status}",
                            admission.intent.transaction.raw(),
                        );
                    } else if exiting_admission
                        && let Some(admission) = session_launches.fail_current()
                    {
                        launch_admission_started_at = None;
                        eprintln!(
                            "sophia_session_app schema=2 status=failed id={id} source=action transaction={} reason=exit_before_admission exit_status={status}",
                            admission.intent.transaction.raw(),
                        );
                    }
                    secondary_children.remove(secondary_index);
                } else {
                    let error = format!(
                        "secondary xterm {} exited during live session with status {status}",
                        secondary_index + 1
                    );
                    terminal_client_error = Some(("secondary", error));
                    match frontend_service_sender
                        .send(XServerFrontendServiceCommand::StopAccepting)
                    {
                        Ok(()) => terminal_client_intake_stopped = true,
                        Err(error) => terminal_client_cleanup_failures
                            .push(format!("frontend intake stop failed: {error}")),
                    }
                    println!(
                        "sophia_live_session_client_fatal schema=1 status=detected source=secondary index={} exit_status={status} action=bounded_cleanup",
                        secondary_index + 1,
                    );
                    break 'session;
                }
            } else {
                secondary_index += 1;
            }
        }
        metrics.max_child_reap = metrics.max_child_reap.max(child_reap_started.elapsed());
        InputDeliveryPhase {
            receiver: input_delivery_receiver,
            state: &mut input_delivery,
            client_key_release_barrier: &mut client_key_release_barrier,
            proof_started_at: &mut input_proof_started_at,
            post_input_deadline: &mut post_input_deadline,
        }
        .drain()?;
        if emergency_exit_requested && input_delivery.pending.is_empty() {
            break;
        }
        if !input_text_match
            && let (Some(expected), Some(result)) = (
                config
                    .inject_text
                    .as_deref()
                    .or(config.expect_physical_text.as_deref()),
                input_proof_result,
            )
            && let Some(received) = result.received()?
        {
            if received != expected.as_bytes() {
                return Err(format!(
                    "persistent live session terminal received incorrect input: expected_bytes={} received_bytes={}",
                    expected.len(),
                    received.len(),
                )
                .into());
            }
            input_text_match = true;
            println!(
                "sophia_live_session_input schema=3 status=semantic_complete source={} text_match=true bytes={}",
                if config.inject_text.is_some() {
                    "synthetic"
                } else {
                    "physical"
                },
                received.len(),
            );
            std::io::stdout().flush()?;
        }
        if let Some(post_input_deadline) = post_input_deadline
            && Instant::now() >= post_input_deadline
            && !input_text_match
        {
            return Err(
                "persistent live session timed out waiting for the terminal to receive exact text and Return"
                    .into(),
            );
        }
        if input_presented_latency.is_none()
            && let Some(post_input_deadline) = post_input_deadline
            && Instant::now() >= post_input_deadline
        {
            if !input_pixel_change {
                return Err(format!(
                    "persistent live session timed out waiting for pixels after flushed X11 input: expected={} flushed={} authority_batches_after_input={} cpu_updates_after_input={} baseline_checksum={injection_checksum:?} final_checksum={:?} baseline_generation={input_surface_generation:?} final_generation={:?} input_surface_pixel_change={input_surface_pixel_change} native_submission_baseline={input_change_submission_baseline:?} native_submissions={} native_callbacks={}",
                    input_delivery.events_expected,
                    input_delivery.events_flushed,
                    metrics.batches.saturating_sub(input_batch_baseline.unwrap_or(metrics.batches)),
                    metrics.cpu_buffer_updates.saturating_sub(input_cpu_update_baseline.unwrap_or(metrics.cpu_buffer_updates)),
                    scene.last_report().map(|report| report.checksum),
                    input_surface.and_then(|surface| {
                        runtime.as_ref().and_then(|runtime| {
                            scene.surface_buffer_generation(runtime.committed_surfaces(), surface)
                        })
                    }),
                    native_scanout.as_ref().map_or(0, |native| native.submissions),
                    native_scanout.as_ref().map_or(0, |native| native.callback_accepted),
                )
                .into());
            }
            return Err("persistent live session input pixels were not presented within the post-flush proof window".into());
        }
        if (post_input_deadline.is_none() || input_presented_latency.is_some())
            && deadline.is_some_and(|deadline| Instant::now() >= deadline)
        {
            if config.input_proof_requested() && injection_checksum.is_none() {
                return Err(
                    "persistent live session startup budget elapsed before a focused terminal frame was ready for input proof"
                        .into(),
                );
            }
            // The global runtime budget bounds startup. Once input has been
            // injected, its delivery and pixel/semantic stages own narrower
            // explicit deadlines. Ending here can strand already-routed keys
            // without giving the frontend a chance to acknowledge them.
            if global_runtime_deadline_ends_session(config.input_proof_requested()) {
                service_runtime_deadline_key_drain!();
            }
        }
        if let (Some(runtime), Some(native_scanout)) = (runtime.as_mut(), native_scanout.as_mut())
            && native_scanout.output_topology_allows_frame_service()
        {
            if layout.pending.is_none() {
                runtime.release_layout_deferred_presentations();
            }
            let service = match runtime.service_native(native_scanout, &scene) {
                Ok(service) => Some(service),
                Err(error) => {
                    let Some(execution) = active_output_topology_preparation.as_mut() else {
                        return Err(error);
                    };
                    let transaction = execution.effect.transaction;
                    let failure = error.to_string();
                    let recovered = begin_output_topology_first_presentation_rollback(
                        &mut execution.phase,
                        transaction,
                        &failure,
                        |reason| native_scanout.request_output_topology_rollback(reason),
                        |transaction| {
                            wm_session
                                .as_mut()
                                .ok_or_else(|| {
                                    Box::<dyn std::error::Error>::from(
                                        "first-presentation rollback lost its WM owner",
                                    )
                                })?
                                .reject_output_topology_effect(
                                    transaction,
                                    sophia_engine::OutputTopologyTransactionFailure::FirstPresentation,
                                )
                        },
                    )?;
                    if !recovered {
                        return Err(error);
                    }
                    tracing::warn!(
                        "sophia_live_output_authority schema=2 status=rollback_started transaction={} reason=first_presentation_service error={error} published=false",
                        transaction.raw(),
                    );
                    None
                }
            };
            if let Some(service) = service {
                for retired in service.retired_software_presents {
                    record_native_software_present_retirement(&mut layout, retired);
                }
                if let Some(retired) = service.retired_present {
                    let NativePresentRetirementObservation {
                        surface,
                        stable,
                        ust_usec: _,
                        msc: _,
                    } = record_native_present_retirement(
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
            }
            correlate_physical_input_page_flip(
                input_proof_started_at.is_some(),
                input_pixel_change,
                input_raw_ingress_msec,
                input_change_submission_baseline,
                input_change_frame_baseline,
                native_scanout,
                &mut input_presented_ust_usec,
                &mut input_submit_to_page_flip,
            );
            if let Some(head) = native_scanout.heads.first() {
                input_latency_samples.observe_page_flip(
                    head.presented_submissions,
                    head.presented_content
                        .map_or(0, |content| content.frame().raw()),
                    head.presented_submission_ust_usec,
                    head.presented_page_flip_ust_usec,
                );
            }
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
            // Admission focus can become eligible on a page-flip retirement.
            // Reconcile it here so an idle client does not need to emit another
            // authority batch before it can receive focus.
            reconcile_pending_wm_focus!(runtime);
        }
        let mut input_routing_mode = physical_input_routing_mode(
            primary_child_exited,
            focus.focused_surface(seat),
            input_surface,
            wm_session.as_ref().is_some_and(|wm| wm.shortcuts.is_some()),
        );
        if config.expect_physical_text.is_some() && physical_input_ready_at.is_none() {
            input_routing_mode = PhysicalInputRoutingMode::CursorOnly;
        }
        if input_routing_mode != PhysicalInputRoutingMode::Suppressed
            && focus.focused_surface(seat) != applied_client_focus
        {
            input_routing_mode = PhysicalInputRoutingMode::ControlPlaneOnly;
        }
        if output_topology_owner.input_quarantined() {
            input_routing_mode = PhysicalInputRoutingMode::ShortcutsOnly;
        }
        if runtime_deadline_key_drain.is_draining() {
            input_routing_mode = PhysicalInputRoutingMode::Suppressed;
        }
        let empty_explicit_projections = [];
        let explicit_projections = runtime.as_ref().map_or(
            &empty_explicit_projections[..],
            |runtime| runtime.input_projections(),
        );
        let explicit_controls = drain_explicit_pointer_grab_controls(
            explicit_pointer_grabs,
            &mut application_route_leases,
            &layout.client_routes,
            &focus,
            explicit_projections,
            seat,
            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        )?;
        if explicit_controls != ExplicitPointerGrabControlReport::default() {
            println!(
                "sophia_live_explicit_pointer_grab schema=1 prepared={} activated={} released={} aborted={} rejected={}",
                explicit_controls.prepared,
                explicit_controls.activated,
                explicit_controls.released,
                explicit_controls.aborted,
                explicit_controls.rejected,
            );
        }
        let input_phase_started = Instant::now();
        let input_requested_exit = input_routing_mode != PhysicalInputRoutingMode::Suppressed
            && drain_physical_input!(input_routing_mode);
        metrics.max_input_phase = metrics.max_input_phase.max(input_phase_started.elapsed());
        if input_requested_exit {
            break;
        }
        if cursor_updates.dirty
            && let (Some(native_scanout), Some(runtime), Some(position)) =
                (native_scanout.as_mut(), runtime.as_ref(), pointer.position())
        {
            let logical_viewports = runtime.logical_viewports();
            match native_scanout
                .update_classic_hardware_cursor(position, &logical_viewports)
            {
                Ok(ClassicHardwareCursorUpdate::Visible) => {
                    pointer_pixel_change |= metrics.physical_pointer_routed > 0;
                    if let Some(started) = cursor_updates.dirty_since.take() {
                        metrics.cursor_max_motion_to_submit =
                            metrics.cursor_max_motion_to_submit.max(started.elapsed());
                    }
                    cursor_updates.dirty = false;
                    if !cursor_visible_reported {
                        println!(
                            "sophia_live_session_pointer schema=2 status=visible source=hardware_cursor"
                        );
                        cursor_visible_reported = true;
                    }
                    if config.expect_physical_pointer
                        && physical_input_completion_reported
                        && input_pixel_change
                        && pointer_phase_started_at.is_none()
                    {
                        pointer_checksum = Some(0);
                        pointer_phase_started_at = Some(Instant::now());
                        println!(
                            "sophia_live_session_pointer schema=1 status=visible source=physical position=center"
                        );
                        println!(
                            "sophia_live_session_pointer schema=1 status=ready source=physical action=select"
                        );
                        std::io::stdout().flush()?;
                    }
                }
                Ok(ClassicHardwareCursorUpdate::Hidden) => {
                    cursor_updates.dirty = false;
                }
                Ok(ClassicHardwareCursorUpdate::Deferred) => {}
                Err(error) => {
                    eprintln!(
                        "sophia_live_session_pointer schema=2 status=unavailable source=hardware_cursor error={error}"
                    );
                    return Err(format!(
                        "native session cannot provide an owned atomic cursor: {error}"
                    )
                    .into());
                }
            }
        }
        if let Some(candidate) = pointer_cursor_checksum
            && native_scanout.as_ref().is_none_or(|native| {
                native.heads.first().is_some_and(|head| {
                    head.presented_logical_checksum == candidate && head.nonzero_exports > 0
                })
            })
        {
            pointer_checksum = Some(candidate);
            pointer_cursor_checksum = None;
            pointer_phase_started_at = Some(Instant::now());
            println!(
                "sophia_live_session_pointer schema=1 status=visible source=physical position=center"
            );
            println!(
                "sophia_live_session_pointer schema=1 status=ready source=physical action=select"
            );
            std::io::stdout().flush()?;
        }

        if let Some(surface) = focus.focused_surface(seat) {
            let cpu_visual_detail = runtime.as_ref().and_then(|runtime| {
                runtime
                    .committed_surfaces()
                    .iter()
                    .any(|committed| committed.surface == surface)
                    .then(|| {
                        scene.surface_has_visual_detail(runtime.committed_surfaces(), surface)
                    })
            });
            let stable_gpu_pixels = startup_surface_presentations.nonzero_rgb_pixels(surface);
            let stable_gpu_detail = startup_surface_presentations.visual_detail(surface);
            let visual_detail =
                startup_surface_visual_detail(cpu_visual_detail, stable_gpu_pixels);
            if visual_detail {
                let _ = reduce_session_startup(
                    &mut startup_readiness,
                    SessionStartupEvent::VisualDetail(surface),
                );
            }
            if stable_gpu_detail {
                let _ = reduce_session_startup(
                    &mut startup_readiness,
                    SessionStartupEvent::StablePresented(surface),
                );
            }
            if input_content_surface != Some(surface) && visual_detail {
                input_content_surface = Some(surface);
                println!(
                    "sophia_live_session_input_pipeline schema=2 status=content_ready source={}",
                    if stable_gpu_detail {
                        "stable_present_scanout"
                    } else {
                        "cpu_visual_detail"
                    }
                );
                std::io::stdout().flush()?;
            }
            if !startup_content_ready && visual_detail {
                startup_content_ready = true;
                println!(
                    "sophia_live_session_startup schema=2 status=content_ready source={} nonzero_rgb_pixels={stable_gpu_pixels}",
                    if stable_gpu_detail {
                        "stable_present_scanout"
                    } else {
                        "cpu_visual_detail"
                    }
                );
                std::io::stdout().flush()?;
            }
        }
        let focused_surface = focus.focused_surface(seat);
        let focused_client_ready =
            focused_surface.is_some() && applied_client_focus == focused_surface;
        let missing_output_callback = native_scanout.as_ref().is_some_and(|native| {
            native
                .heads
                .iter()
                .any(|head| {
                    head.callback_accepted == 0 && head.initial_modeset_submission.is_none()
                })
        });
        if !startup_outputs_ready_reported
            && let Some(native) = native_scanout.as_ref()
            && startup_output_evidence(native, None)
                .is_some_and(|outputs| all_startup_outputs_presented(&outputs))
        {
            startup_outputs_ready_reported = true;
            for record in logical_synchronous_modeset_records(native.heads.iter().map(|head| {
                (head.output.id, head.initial_modeset_submission)
            })) {
                println!("{record}");
            }
            let _ = reduce_session_startup(
                &mut startup_readiness,
                SessionStartupEvent::OutputsPresented,
            );
            let (ready_outputs, output_count) = logical_startup_output_progress(
                native.heads.iter().map(|head| {
                    (
                        head.output.id,
                        head.callback_accepted > 0 || head.initial_modeset_submission.is_some(),
                    )
                }),
            );
            println!(
                "sophia_live_session_startup schema=2 status=output_baseline_ready outputs={}/{}",
                ready_outputs,
                output_count,
            );
            std::io::stdout().flush()?;
        }
        // Pixel content is application-readiness evidence, not transport
        // liveness. A valid black Present may have more client work queued;
        // rebuilding its renderer here would invalidate retained snapshots.
        let recovery_reason =
            startup_native_recovery_reason(missing_output_callback, started.elapsed());
        if !startup_ready_reported
            && !startup_native_recovery_attempted
            && recovery_reason.is_some()
            && runtime.is_some()
            && native_scanout.is_some()
            && seat_controller.is_some()
        {
            include!("startup_native_recovery.rs");
        }
        let startup_frame_presented = native_scanout.as_ref().map_or(
            !output_topology_owner.input_quarantined(),
            |native| {
            let all_outputs_presented = startup_required_submissions
                .as_ref()
                .and_then(|required| startup_output_evidence(native, Some(required)))
                .is_some_and(|outputs| all_startup_outputs_presented(&outputs));
            let focused_mixed_presented = startup_readiness.surface.is_some_and(|surface| {
                startup_surface_presentations.stable_presented(surface)
            });
            let every_output_has_retired = startup_output_evidence(native, None)
                .is_some_and(|outputs| all_startup_outputs_presented(&outputs));
                (focused_mixed_presented && every_output_has_retired) || all_outputs_presented
            },
        );
        if !startup_ready_reported
            && startup_readiness.surface.is_some()
            && startup_readiness.client_focus_applied
            && startup_readiness.visual_detail
            && startup_frame_presented
        {
            if startup_frame_presented
                && let Some(surface) = startup_readiness.surface
            {
                let _ = reduce_session_startup(
                    &mut startup_readiness,
                    SessionStartupEvent::StablePresented(surface),
                );
            }
            if startup_outputs_ready_reported || native_scanout.is_none() {
                let _ = reduce_session_startup(
                    &mut startup_readiness,
                    SessionStartupEvent::OutputsPresented,
                );
            }
        }
        if !startup_ready_reported && startup_readiness.ready {
            startup_ready_reported = true;
            startup_ready_msec.get_or_insert_with(|| started.elapsed().as_millis());
            let logical_output_progress = native_scanout.as_ref().map(|native| {
                logical_startup_output_progress(native.heads.iter().map(|head| {
                    (
                        head.output.id,
                        head.callback_accepted > 0 || head.initial_modeset_submission.is_some(),
                    )
                }))
            });
            println!(
                "sophia_live_session_startup schema=2 status=ready elapsed_msec={} surface=true visual_detail=true presented=true outputs_ready={}/{} recovery_attempts={}",
                started.elapsed().as_millis(),
                logical_output_progress.map_or(1, |progress| progress.0),
                logical_output_progress.map_or(1, |progress| progress.1),
                usize::from(startup_native_recovery_attempted),
            );
            std::io::stdout().flush()?;
        }
        include!("startup_watchdog.rs");

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
        if require_startup_focus
            && focus.focused_surface(seat).is_none()
            && focus_deadline_started_at
                .is_some_and(|started: Instant| started.elapsed() >= Duration::from_secs(5))
        {
            return Err(
                "live-session input focus was not ready within five seconds of the first presented frame"
                    .into(),
            );
        }
        let physical_sequence_complete = physical_text_proof
            .as_ref()
            .is_none_or(PhysicalTextProof::is_complete);
        let waiting_for_keyboard_sequence =
            physical_input_ready_at.is_some() && !physical_sequence_complete;
        let waiting_for_pointer_selection = sophia_cli::input_proof::pointer_selection_waiting(
            config.expect_physical_pointer,
            physical_sequence_complete,
            input_pixel_change,
            pointer_checksum.is_some(),
            metrics.physical_pointer_buttons_routed,
            pointer_pixel_change,
        );
        if waiting_for_keyboard_sequence {
            let ready_at = physical_input_ready_at.expect("checked above");
            if ready_at.elapsed()
                >= Duration::from_millis(config.physical_sequence_timeout_msec)
            {
                let proof = physical_text_proof.as_ref().expect("checked above");
                return Err(format!(
                    "persistent live session timed out waiting for exact physical input sequence: matched_events={} expected_events={} keyboard_routed={physical_keys_routed}",
                    proof.matched_events(),
                    proof.expected_events(),
                    physical_keys_routed = metrics.physical_keys_routed,
                )
                .into());
            }
        } else if waiting_for_pointer_selection {
            let started_at = pointer_phase_started_at.expect("set above");
            if started_at.elapsed()
                >= Duration::from_millis(config.physical_sequence_timeout_msec)
            {
                return Err(format!(
                    "persistent live session timed out waiting for a routed physical pointer button: pointer_observed={physical_pointer_events} pointer_routed={physical_pointer_routed} pointer_buttons={physical_pointer_buttons_routed} pointer_pixels={pointer_pixel_change}",
                    physical_pointer_events = metrics.physical_pointer_events,
                    physical_pointer_routed = metrics.physical_pointer_routed,
                    physical_pointer_buttons_routed =
                        metrics.physical_pointer_buttons_routed,
                )
                .into());
            }
        } else if input_delivery.wait_started_at.is_none()
            && (input_proof_started_at.is_none() || input_presented_latency.is_some())
        {
            if config
                .max_ticks
                .is_some_and(|max_ticks| metrics.session_ticks >= max_ticks)
            {
                break;
            }
            metrics.session_ticks = metrics.session_ticks.saturating_add(1);
        }

        ()
}
