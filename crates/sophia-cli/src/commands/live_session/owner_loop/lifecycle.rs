{
        if let Some(controller) = seat_controller.as_mut() {
            if let Some(event) = controller.dispatch()? {
                seat_state = seat_state.observe(event);
            }
            if seat_state == sophia_backend_live::LiveSeatState::ReleasePending {
                println!("sophia_live_seat schema=1 status=release_pending");
                physical_input.take();
                if let (Some(runtime), Some(native)) =
                    (runtime.as_mut(), native_scanout.as_mut())
                {
                    runtime.suspend_native_scanout(
                        native,
                        &outputs,
                        Duration::from_millis(500),
                    )?;
                }
                native_scanout.take();
                controller.acknowledge_disable()?;
                seat_state = seat_state.released();
                modifiers = XCoreKeyboardMapper::new();
                virtual_terminal_chord = VirtualTerminalChordState::default();
                emergency_chord = EmergencyChordState::armed();
                println!("sophia_live_seat schema=1 status=suspended");
                std::io::stdout().flush()?;
            }
            if seat_state == sophia_backend_live::LiveSeatState::AcquirePending {
                println!("sophia_live_seat schema=1 status=acquire_pending");
                let mut resumed = LiveProductionNativeScanout::new()?;
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
                *physical_input = open_session_physical_input(config, device_map)?;
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
                    let id = secondary_children[secondary_index]
                        .id
                        .as_deref()
                        .unwrap_or("untracked");
                    println!(
                        "sophia_session_app schema=1 status=exited id={id} source=managed exit_status={status}",
                    );
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
            && drain_physical_input!(input_routing_mode)
        {
            break;
        }
        if let (Some(runtime), Some(native_scanout)) = (runtime.as_mut(), native_scanout.as_mut()) {
            let service = runtime.service_native(native_scanout)?;
            if let Some(retired) = service.retired_present {
                let stable = runtime.stable_present(native_scanout, retired.transaction);
                retired_present_surfaces.insert(retired.surface, retired.transaction);
                if stable_gpu_frame_proves_post_input_pixels(
                    input_proof_started_at.is_some(),
                    input_surface,
                    retired.surface,
                    stable,
                ) {
                    input_pixel_change = true;
                }
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
                    "sophia_live_session_startup schema=1 status=content_ready source={}",
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
        let startup_frame_presented = native_scanout.as_ref().is_none_or(|native| {
            focused_surface.is_some_and(|surface| {
                retired_present_surfaces
                    .get(&surface)
                    .is_some_and(|transaction| {
                        runtime
                            .as_ref()
                            .is_some_and(|runtime| runtime.stable_present(native, *transaction))
                    })
            }) || startup_required_submissions
                .as_ref()
                .is_some_and(|required| {
                    native
                        .heads
                        .iter()
                        .zip(required)
                        .next()
                        .is_some_and(|(head, required)| head.presented_submissions >= *required)
                })
        });
        if !startup_ready_reported
            && config.startup_ready_timeout.is_some()
            && focus.focused_surface(seat).is_some()
            && focused_client_ready
            && startup_content_ready
            && startup_frame_presented
        {
            startup_ready_reported = true;
            startup_ready_msec.get_or_insert_with(|| started.elapsed().as_millis());
            println!(
                "sophia_live_session_startup schema=1 status=ready elapsed_msec={} surface=true visual_detail=true presented=true",
                started.elapsed().as_millis()
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
            } else if !focused_client_ready {
                "focus_control_pending"
            } else if !startup_content_ready {
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
