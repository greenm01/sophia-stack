fn session_protocol_errors_are_fatal(
    normal_session: bool,
    application_proof: bool,
    protocol_error_count: usize,
) -> bool {
    protocol_error_count != 0 && (normal_session || application_proof)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PhysicalInputRoutingMode {
    Suppressed,
    CursorOnly,
    ShortcutsOnly,
    Full,
}

fn physical_input_routing_mode(
    primary_child_exited: bool,
    focused_surface: Option<SurfaceId>,
    proof_surface: Option<SurfaceId>,
    global_shortcuts_available: bool,
) -> PhysicalInputRoutingMode {
    if !primary_child_exited || focused_surface != proof_surface {
        PhysicalInputRoutingMode::Full
    } else if global_shortcuts_available {
        PhysicalInputRoutingMode::ShortcutsOnly
    } else {
        PhysicalInputRoutingMode::Suppressed
    }
}

struct InitialSessionFocusContext<'a> {
    runtime: &'a LiveProductionVisualRuntime,
    focus: &'a mut InputFocusState,
    seat: SeatId,
    wm_session_present: bool,
    layout: &'a PersistentLiveLayout,
    control_sender: &'a SyncSender<XAuthorityClientControlCommand>,
    next_focus_control_transaction: &'a mut u64,
    focused_client_control: &'a mut Option<(TransactionId, SurfaceId)>,
}

fn reconcile_initial_session_focus(
    context: InitialSessionFocusContext<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let InitialSessionFocusContext {
        runtime,
        focus,
        seat,
        wm_session_present,
        layout,
        control_sender,
        next_focus_control_transaction,
        focused_client_control,
    } = context;
    if focus.focused_surface(seat).is_some() {
        return Ok(());
    }
    let Some(surface) = runtime.committed_surfaces().first() else {
        return Ok(());
    };
    if focus.focus_surface(seat, surface.surface, runtime.committed_surfaces())
        != InputFocusDecision::Focused
        || wm_session_present
    {
        return Ok(());
    }
    let client = layout
        .client_routes
        .client_for_surface(surface.surface)
        .ok_or("initial X11 focus has no client route")?;
    let transaction = TransactionId::from_raw(*next_focus_control_transaction);
    *next_focus_control_transaction = next_focus_control_transaction
        .checked_add(1)
        .ok_or("initial X11 focus transaction exhausted")?;
    control_sender
        .try_send(XAuthorityClientControlCommand {
            client,
            command: XAuthorityControlCommand::FocusSurface {
                transaction,
                surface: surface.surface,
            },
        })
        .map_err(|error| match error {
            TrySendError::Full(_) => "initial X11 focus control queue is full",
            TrySendError::Disconnected(_) => "initial X11 focus control queue is disconnected",
        })?;
    *focused_client_control = Some((transaction, surface.surface));
    Ok(())
}

fn authority_transaction_count(transactions: &[SurfaceTransaction]) -> usize {
    transactions.len()
}

fn take_settled_input_delivery_wait(
    wait_started: &mut Option<Instant>,
    pending_deliveries_empty: bool,
) -> Option<Instant> {
    if pending_deliveries_empty {
        wait_started.take()
    } else {
        None
    }
}

fn record_runtime_commits(committed: u64, accepted_transactions: usize) -> u64 {
    committed.saturating_add(u64::try_from(accepted_transactions).unwrap_or(u64::MAX))
}

fn physical_input_pixels_already_changed(
    baseline_checksum: Option<u64>,
    current_checksum: Option<u64>,
    input_surface_changed: bool,
) -> bool {
    input_surface_changed
        && baseline_checksum
            .zip(current_checksum)
            .is_some_and(|(baseline, current)| baseline != current)
}

fn input_baseline_is_presented(
    focused_content_ready: bool,
    cpu_baseline_presented: bool,
) -> bool {
    focused_content_ready || cpu_baseline_presented
}

fn stable_gpu_frame_proves_post_input_pixels(
    input_delivery_complete: bool,
    input_surface: Option<SurfaceId>,
    retired_surface: SurfaceId,
    stable: bool,
) -> bool {
    input_delivery_complete && stable && input_surface == Some(retired_surface)
}

fn software_batch_may_coalesce(batch: &XAuthorityObservedTransactionBatch) -> bool {
    batch.removed_surfaces.is_empty()
        && batch.dma_buf_registrations.is_empty()
        && batch.fence_registrations.is_empty()
        && batch.present_submissions.is_empty()
        && batch.released_dma_bufs.is_empty()
        && batch.released_fences.is_empty()
        && (!batch.transactions.is_empty() || !batch.cpu_buffer_updates.is_empty())
}

struct SessionActionExecutionContext<'a> {
    config: &'a PersistentXtermSessionConfig,
    xauthority: &'a std::path::Path,
    children: &'a mut Vec<ManagedSessionChild>,
    layout: &'a PersistentLiveLayout,
    focus: &'a InputFocusState,
    seat: SeatId,
    control_sender: &'a SyncSender<XAuthorityClientControlCommand>,
    control_ack_receiver: &'a Receiver<XAuthorityClientControlAck>,
}

fn execute_committed_session_actions(
    context: SessionActionExecutionContext<'_>,
    actions: &mut VecDeque<(TransactionId, WmSessionAction, Option<SurfaceId>)>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let SessionActionExecutionContext {
        config,
        xauthority,
        children,
        layout,
        focus,
        seat,
        control_sender,
        control_ack_receiver,
    } = context;
    let mut retained = Vec::with_capacity(children.len());
    for mut child in children.drain(..) {
        let status = child.child.try_wait()?;
        match status {
            None => retained.push(child),
            Some(status) => {
                if let Some(id) = child.id.as_deref() {
                    terminate_session_child(&mut child.child, true)?;
                    if !status.success() {
                        return Err(format!(
                            "managed session application {id:?} exited abnormally: {status}"
                        )
                        .into());
                    }
                    println!(
                        "sophia_session_app schema=1 status=exited id={id} source=managed exit_status={status}"
                    );
                }
            }
        }
    }
    *children = retained;
    let mut logout = false;
    while let Some((transaction, action, target)) = actions.pop_front() {
        match action {
            WmSessionAction::LaunchApplication { application }
                if application == TERMINAL_APPLICATION_ID =>
            {
                if children.len() >= 16 {
                    return Err("approved session child limit reached".into());
                }
                if config.normal_session {
                    let app = config
                        .application_for_action(action)
                        .ok_or("WM requested an unadvertised session application")?;
                    children.push(ManagedSessionChild::new(
                        Some(app.id.clone()),
                        PersistentXtermSessionConfig::spawn_session_application(
                            app,
                            &config.display,
                            xauthority,
                        )?,
                    ));
                    println!(
                        "sophia_session_app schema=1 status=started id={} source=action",
                        app.id
                    );
                } else {
                    children.push(ManagedSessionChild::new(
                        None,
                        spawn_secondary_xterm(
                            std::path::Path::new(&config.terminal),
                            &config.display,
                            xauthority,
                            None,
                        )?,
                    ));
                }
            }
            WmSessionAction::LaunchApplication { application } => {
                if children.len() >= 16 {
                    return Err("approved session child limit reached".into());
                }
                if config.normal_session {
                    let app = config
                        .application_for_action(action)
                        .ok_or("WM requested an unadvertised session application")?;
                    children.push(ManagedSessionChild::new(
                        Some(app.id.clone()),
                        PersistentXtermSessionConfig::spawn_session_application(
                            app,
                            &config.display,
                            xauthority,
                        )?,
                    ));
                    println!(
                        "sophia_session_app schema=1 status=started id={} source=action",
                        app.id
                    );
                } else {
                    let program = match application {
                        LAUNCHER_APPLICATION_ID => config.session_launcher.as_deref(),
                        BROWSER_APPLICATION_ID => config.session_firefox.as_deref(),
                        _ => None,
                    }
                    .ok_or("WM requested an unadvertised session executable")?;
                    children.push(ManagedSessionChild::new(
                        None,
                        spawn_approved_application(program, &config.display, xauthority)?,
                    ));
                }
            }
            WmSessionAction::CloseFocused => {
                let surface = target
                    .or_else(|| focus.focused_surface(seat))
                    .ok_or("WM close action has no focused surface")?;
                let client = layout
                    .client_routes
                    .client_for_surface(surface)
                    .ok_or("WM close action has no X11 client route")?;
                println!(
                    "sophia_live_wm schema=1 status=close_routed transaction={} target=surface surface={surface:?} client={client:?}",
                    transaction.raw()
                );
                control_sender.try_send(XAuthorityClientControlCommand {
                    client,
                    command: XAuthorityControlCommand::CloseSurface {
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
                    return Err(format!(
                        "X Authority rejected polite close: {:?}",
                        acknowledgement.acknowledgement.outcome
                    )
                    .into());
                }
            }
            WmSessionAction::Logout => logout = true,
        }
        println!(
            "sophia_live_wm schema=1 status=session_action_committed transaction={} action={}",
            transaction.raw(),
            session_action_evidence_name(action)
        );
    }
    Ok(logout)
}
