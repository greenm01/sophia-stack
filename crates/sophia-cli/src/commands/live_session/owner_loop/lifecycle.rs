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
                    proof_started_at: &mut input_proof_started_at,
                    post_input_deadline: &mut post_input_deadline,
                }
                .drain()?;
                if !input_delivery.pending.is_empty() {
                    if queued_at.elapsed() >= Duration::from_millis(500) {
                        pending_virtual_terminal = None;
                        modifiers = XCoreKeyboardMapper::new();
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
                                let mut resumed = LiveProductionNativeScanout::new_with_seat(
                                    &controller.device_opener(),
                                )?;
                                if resumed.outputs() != outputs {
                                    return Err(
                                        "seat switch rejection changed the physical output topology"
                                            .into(),
                                    );
                                }
                                if let Some(runtime) = runtime.as_mut() {
                                    let frames = scene.frames_for_outputs(&outputs)?;
                                    runtime.resume_native_scanout(
                                        &mut resumed,
                                        &outputs,
                                        frames,
                                    )?;
                                }
                                *native_scanout = Some(resumed);
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
                                modifiers = XCoreKeyboardMapper::new();
                                virtual_terminal_chord = VirtualTerminalChordState::default();
                                emergency_chord = EmergencyChordState::armed();
                                cursor_updates =
                                    CursorUpdateState::new(pointer.position.is_some());
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
                        modifiers = XCoreKeyboardMapper::new();
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
                    LiveProductionNativeScanout::new_with_seat(&controller.device_opener())?;
                if resumed.outputs() != outputs {
                    return Err("seat switch timeout changed the physical output topology".into());
                }
                if let Some(runtime) = runtime.as_mut() {
                    let frames = scene.frames_for_outputs(&outputs)?;
                    runtime.resume_native_scanout(&mut resumed, &outputs, frames)?;
                }
                *native_scanout = Some(resumed);
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
                modifiers = XCoreKeyboardMapper::new();
                virtual_terminal_chord = VirtualTerminalChordState::default();
                emergency_chord = EmergencyChordState::armed();
                if let Some(wm) = wm_session.as_mut()
                    && let Some(shortcuts) = wm.shortcuts.as_mut()
                {
                    let _ = shortcuts.clear_seat(seat);
                }
                cursor_updates = CursorUpdateState::new(pointer.position.is_some());
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
                if let Some(surface) = applied_client_focus {
                    flush_client_keys!(surface, "seat_release");
                }
                physical_input.take();
                if !seat_release_prepared
                    && let Some(runtime) = runtime.as_mut()
                {
                    let report = runtime.suspend_revoked_native_scanout(&outputs)?;
                    println!(
                        "sophia_live_seat schema=2 status=forced_detach abandoned_scanouts={} skipped_present={}",
                        report.abandoned_scanouts,
                        report
                            .skipped_present
                            .map_or_else(|| "none".to_owned(), |transaction| transaction.raw().to_string()),
                    );
                }
                native_scanout.take();
                controller.acknowledge_disable()?;
                seat_state = seat_state.released();
                seat_release_prepared = false;
                requested_virtual_terminal = None;
                modifiers = XCoreKeyboardMapper::new();
                virtual_terminal_chord = VirtualTerminalChordState::default();
                emergency_chord = EmergencyChordState::armed();
                println!("sophia_live_seat schema=1 status=suspended");
                std::io::stdout().flush()?;
            }
            if seat_state == sophia_backend_live::LiveSeatState::AcquirePending {
                println!("sophia_live_seat schema=1 status=acquire_pending");
                let mut resumed =
                    LiveProductionNativeScanout::new_with_seat(&controller.device_opener())?;
                if resumed.outputs() != outputs {
                    return Err("seat resume changed the physical output topology".into());
                }
                if let Some(runtime) = runtime.as_mut() {
                    let frames = scene.frames_for_outputs(&outputs)?;
                    runtime.resume_native_scanout(&mut resumed, &outputs, frames)?;
                }
                *native_scanout = Some(resumed);
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
                cursor_updates = CursorUpdateState::new(pointer.position.is_some());
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
        if !primary_child_exited
            && let Some(primary_child) = child.as_deref_mut()
            && let Some(status) = primary_child.try_wait()?
        {
            primary_exit_status = Some(status);
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
                if !status.success() {
                    return Err(format!(
                        "session client exited during live session with status {status}"
                    )
                    .into());
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
                if config.normal_session {
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
                    if launch_transaction.is_some_and(|transaction| {
                        session_launches
                            .admission()
                            .is_some_and(|admission| admission.intent.transaction == transaction)
                    }) && let Some(admission) = session_launches.fail_current()
                    {
                        launch_admission_started_at = None;
                        eprintln!(
                            "sophia_session_app schema=2 status=failed id={id} source=action transaction={} reason=exit_before_admission exit_status={status}",
                            admission.intent.transaction.raw(),
                        );
                    }
                    secondary_children.remove(secondary_index);
                } else {
                    return Err(format!(
                        "secondary xterm {} exited during live session with status {status}",
                        secondary_index + 1
                    )
                    .into());
                }
            } else {
                secondary_index += 1;
            }
        }
        InputDeliveryPhase {
            receiver: input_delivery_receiver,
            state: &mut input_delivery,
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
                break;
            }
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
        if input_routing_mode != PhysicalInputRoutingMode::Suppressed
            && drain_physical_input!(input_routing_mode)
        {
            break;
        }
        if let (Some(runtime), Some(native_scanout)) = (runtime.as_mut(), native_scanout.as_mut()) {
            runtime.set_present_scheduling_blocked(layout.pending.is_some());
            let service = runtime.service_native(native_scanout)?;
            if let Some(retired) = service.retired_present {
                let stable = runtime.stable_present(native_scanout, retired.transaction);
                retired_present_surfaces.insert(retired.surface, retired.transaction);
                if stable {
                    let _ = reduce_session_startup(
                        &mut startup_readiness,
                        SessionStartupEvent::StablePresented(retired.surface),
                    );
                }
                if stable_gpu_frame_proves_post_input_pixels(
                    input_proof_started_at.is_some(),
                    input_surface,
                    retired.surface,
                    stable,
                ) {
                    input_pixel_change = true;
                }
                let clip = retired.clip.map_or_else(
                    || "none".to_owned(),
                    |clip| format!("{}x{}_{}_{}", clip.width, clip.height, clip.x, clip.y),
                );
                println!(
                    "sophia_live_session_present schema=2 status=retired transaction={} surface={} source={}x{} target={}x{}_{}_{} clip={} unit_scale={}",
                    retired.transaction.raw(),
                    retired.surface.index(),
                    retired.source_size.width,
                    retired.source_size.height,
                    retired.target.width,
                    retired.target.height,
                    retired.target.x,
                    retired.target.y,
                    clip,
                    retired.source_size.width == retired.clip.unwrap_or(retired.target).width
                        && retired.source_size.height
                            == retired.clip.unwrap_or(retired.target).height,
                );
                println!(
                    "sophia_live_session_scanout schema=1 status={} kind=mixed transaction={} pending_primary={}",
                    if stable { "stable" } else { "superseded" },
                    retired.transaction.raw(),
                    !stable,
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
        }
        if cursor_updates.dirty
            && let (Some(native_scanout), Some(position)) =
                (native_scanout.as_mut(), pointer.position)
        {
            match native_scanout.update_classic_hardware_cursor(position) {
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
                    head.presented_checksum == candidate && head.nonzero_exports > 0
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

        if let (Some(runtime), Some(surface)) = (runtime.as_ref(), focus.focused_surface(seat))
            && let Some(committed) = runtime
                .committed_surfaces()
                .iter()
                .find(|committed| committed.surface == surface)
        {
            let cpu_visual_detail =
                scene.surface_has_visual_detail(runtime.committed_surfaces(), surface);
            let retired_gpu_detail =
                retired_present_surfaces
                    .get(&surface)
                    .is_some_and(|transaction| {
                        matches!(committed.buffer, BufferSource::DmaBuf { .. })
                            && native_scanout
                                .as_ref()
                                .is_some_and(|native| runtime.stable_present(native, *transaction))
                    });
            let retired_gpu_pixels = retired_present_surfaces
                .get(&surface)
                .and_then(|transaction| {
                    native_scanout.as_ref().map(|native| {
                        native.presented_mixed_nonzero_rgb_pixels(*transaction)
                    })
                })
                .unwrap_or(0);
            if cpu_visual_detail || retired_gpu_detail {
                let _ = reduce_session_startup(
                    &mut startup_readiness,
                    SessionStartupEvent::VisualDetail(surface),
                );
            }
            if retired_gpu_detail {
                let _ = reduce_session_startup(
                    &mut startup_readiness,
                    SessionStartupEvent::StablePresented(surface),
                );
            }
            if input_content_surface != Some(surface)
                && (cpu_visual_detail || retired_gpu_detail)
            {
                input_content_surface = Some(surface);
                println!(
                    "sophia_live_session_input_pipeline schema=2 status=content_ready source={}",
                    if retired_gpu_detail {
                        "stable_present_scanout"
                    } else {
                        "cpu_visual_detail"
                    }
                );
                std::io::stdout().flush()?;
            }
            if !startup_content_ready && (cpu_visual_detail || retired_gpu_detail) {
                startup_content_ready = true;
                println!(
                    "sophia_live_session_startup schema=2 status=content_ready source={} nonzero_rgb_pixels={retired_gpu_pixels}",
                    if retired_gpu_detail {
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
            for head in &native.heads {
                if let Some(record) = synchronous_modeset_record(
                    head.output.id.raw(),
                    head.initial_modeset_submission,
                ) {
                    println!("{record}");
                }
            }
            let _ = reduce_session_startup(
                &mut startup_readiness,
                SessionStartupEvent::OutputsPresented,
            );
            println!(
                "sophia_live_session_startup schema=2 status=output_baseline_ready outputs={}/{}",
                native.heads.len(),
                native.heads.len(),
            );
            std::io::stdout().flush()?;
        }
        let mixed_without_visible_pixels = !startup_content_ready
            && !retired_present_surfaces.is_empty()
            && focused_surface.is_some();
        let recovery_due = (missing_output_callback
            && started.elapsed() >= Duration::from_millis(750))
            || (mixed_without_visible_pixels
                && started.elapsed() >= Duration::from_millis(1_500));
        if !startup_ready_reported
            && !startup_native_recovery_attempted
            && recovery_due
            && runtime.is_some()
            && native_scanout.is_some()
            && seat_controller.is_some()
        {
            startup_native_recovery_attempted = true;
            let mut current = native_scanout
                .take()
                .ok_or("startup native recovery lost the active scanout")?;
            let suspended = runtime
                .as_mut()
                .ok_or("startup native recovery lost the visual runtime")?
                .suspend_native_scanout(
                    &mut current,
                    &outputs,
                    Duration::from_millis(100),
                )?;
            drop(current);
            let mut replacement = LiveProductionNativeScanout::new_with_seat(
                &seat_controller
                    .as_ref()
                    .ok_or("startup native recovery lost the seat controller")?
                    .device_opener(),
            )?;
            if replacement.outputs() != outputs {
                return Err("startup native recovery changed the owned output topology".into());
            }
            let frames = scene.frames_for_outputs(&outputs)?;
            let runtime = runtime
                .as_mut()
                .ok_or("startup native recovery lost the visual runtime")?;
            runtime.resume_native_scanout(&mut replacement, &outputs, frames)?;
            let _ = runtime.run_cpu_repaint(
                &mut scene,
                focused_surface,
                None,
                &outputs,
                &mut replacement,
            )?;
            *native_scanout = Some(replacement);
            retired_present_surfaces.clear();
            startup_content_ready = false;
            startup_required_submissions = None;
            input_content_surface = None;
            startup_outputs_ready_reported = false;
            let _ = reduce_session_startup(
                &mut startup_readiness,
                SessionStartupEvent::NativeRecovered,
            );
            println!(
                "sophia_live_session_startup schema=3 status=recovered attempt=1 reason={} outcome={} drained={} abandoned_scanouts={}",
                if missing_output_callback {
                    "missing_output_callback"
                } else {
                    "no_visible_mixed_pixels"
                },
                suspended.outcome.reduced_name(),
                suspended.outcome.drained(),
                suspended.abandoned_scanouts,
            );
            std::io::stdout().flush()?;
        }
        let startup_frame_presented = native_scanout.as_ref().is_none_or(|native| {
            let all_outputs_presented = startup_required_submissions
                .as_ref()
                .and_then(|required| startup_output_evidence(native, Some(required)))
                .is_some_and(|outputs| all_startup_outputs_presented(&outputs));
            let focused_mixed_presented = startup_readiness.surface.is_some_and(|surface| {
                retired_present_surfaces
                    .get(&surface)
                    .is_some_and(|transaction| {
                        runtime
                            .as_ref()
                            .is_some_and(|runtime| runtime.stable_present(native, *transaction))
                    })
            });
            let every_output_has_retired = startup_output_evidence(native, None)
                .is_some_and(|outputs| all_startup_outputs_presented(&outputs));
            (focused_mixed_presented && every_output_has_retired) || all_outputs_presented
        });
        if !startup_ready_reported
            && config.startup_ready_timeout.is_some()
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
            if startup_outputs_ready_reported {
                let _ = reduce_session_startup(
                    &mut startup_readiness,
                    SessionStartupEvent::OutputsPresented,
                );
            }
        }
        if !startup_ready_reported
            && config.startup_ready_timeout.is_some()
            && startup_readiness.ready
        {
            startup_ready_reported = true;
            startup_ready_msec.get_or_insert_with(|| started.elapsed().as_millis());
            println!(
                "sophia_live_session_startup schema=2 status=ready elapsed_msec={} surface=true visual_detail=true presented=true outputs_ready={}/{} recovery_attempts={}",
                started.elapsed().as_millis(),
                native_scanout.as_ref().map_or(1, |native| {
                    native
                        .heads
                        .iter()
                        .filter(|head| {
                            head.callback_accepted > 0
                                || head.initial_modeset_submission.is_some()
                        })
                        .count()
                }),
                native_scanout.as_ref().map_or(1, |native| native.heads.len()),
                usize::from(startup_native_recovery_attempted),
            );
            std::io::stdout().flush()?;
        }
        if !startup_ready_reported
            && startup_ready_deadline.is_some_and(|deadline| Instant::now() >= deadline)
        {
            let stage = if layout.layers.is_empty() {
                "no_surface"
            } else if runtime
                .as_ref()
                .is_none_or(|runtime| runtime.committed_surfaces().is_empty())
            {
                "not_committed"
            } else if focus.focused_surface(seat).is_none() {
                "not_focused"
            } else if !startup_readiness.client_focus_applied {
                "focus_control_pending"
            } else if !startup_readiness.visual_detail {
                "no_visual_detail"
            } else {
                "not_presented"
            };
            if let Some(native) = native_scanout.as_ref() {
                for head in &native.heads {
                    if let Some(report) = head.last_submit_report {
                        eprintln!("{}", report.reduced_log_line());
                    }
                }
            }
            eprintln!(
                "sophia_live_session_startup schema=3 status=failed stage={stage} elapsed_msec={} authority_batches={batches} transactions={transactions} layout_surfaces={} runtime_surfaces={runtime_surfaces} focus={} focus_control_ready={focused_client_ready} retired_present_surfaces={} dma_buf_registrations={dma_buf_registrations_observed} fence_registrations={fence_registrations_observed} present_submissions={present_submissions_observed} native_submissions={} native_submit_failures={} native_retirements={} native_callbacks={} native_state={} protocol_errors={protocol_error_count}",
                started.elapsed().as_millis(),
                layout.layers.len(),
                focus.focused_surface(seat).is_some(),
                retired_present_surfaces.len(),
                native_scanout
                    .as_ref()
                    .map_or(0, |native| native.submissions),
                native_scanout
                    .as_ref()
                    .map_or(0, |native| native.submit_failures),
                native_scanout
                    .as_ref()
                    .map_or(0, |native| native.retirements),
                native_scanout
                    .as_ref()
                    .map_or(0, |native| native.callback_accepted),
                runtime.as_ref().map_or_else(
                    || "none".to_owned(),
                    LiveProductionVisualRuntime::native_diagnostic
                ),
                batches = metrics.batches,
                transactions = metrics.transactions,
                runtime_surfaces = metrics.runtime_surfaces,
                dma_buf_registrations_observed = metrics.dma_buf_registrations_observed,
                fence_registrations_observed = metrics.fence_registrations_observed,
                present_submissions_observed = metrics.present_submissions_observed,
                protocol_error_count = metrics.protocol_error_count,
            );
            return Err(format!(
                "startup application was not visibly presented within {} milliseconds: stage={stage}",
                config
                    .startup_ready_timeout
                    .expect("startup deadline requires a timeout")
                    .as_millis()
            )
            .into());
        }

        let input_baseline_presented_before_wait = scene.last_report().is_some_and(|report| {
            report.nonzero_pixel_bytes > 0
                && native_scanout.as_ref().is_none_or(|native| {
                    native.heads.first().is_some_and(|head| {
                        head.presented_checksum == report.checksum && head.nonzero_exports > 0
                    })
                })
        });
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
            if ready_at.elapsed() >= Duration::from_millis(SESSION_PHYSICAL_SEQUENCE_TIMEOUT_MSEC) {
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
            if started_at.elapsed() >= Duration::from_millis(SESSION_PHYSICAL_SEQUENCE_TIMEOUT_MSEC)
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

        input_baseline_presented_before_wait
}
