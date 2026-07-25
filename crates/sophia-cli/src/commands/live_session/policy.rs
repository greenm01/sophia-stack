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
    if focused_surface.is_none()
        || !primary_child_exited
        || focused_surface != proof_surface
    {
        PhysicalInputRoutingMode::Full
    } else if global_shortcuts_available {
        PhysicalInputRoutingMode::ShortcutsOnly
    } else {
        PhysicalInputRoutingMode::Suppressed
    }
}

fn pending_wm_focus_after_engine_decision(
    request: (TransactionId, SurfaceId),
    decision: InputFocusDecision,
) -> Option<(TransactionId, SurfaceId)> {
    (decision != InputFocusDecision::Focused).then_some(request)
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
    launches: &'a mut SessionLaunchQueue,
    launch_admission_started_at: &'a mut Option<Instant>,
    startup_ready: bool,
    admission_pipeline_idle: bool,
    presented_admission_surface: Option<SurfaceId>,
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
        launches,
        launch_admission_started_at,
        startup_ready,
        admission_pipeline_idle,
        presented_admission_surface,
        layout,
        focus,
        seat,
        control_sender,
        control_ack_receiver,
    } = context;
    if let Some(admission) =
        launches.complete_if_presented(admission_pipeline_idle, presented_admission_surface)
    {
        *launch_admission_started_at = None;
        println!(
            "sophia_session_app schema=2 status=admitted source=action transaction={} surface={}",
            admission.intent.transaction.raw(),
            admission
                .observed_surface
                .expect("settled launch admission requires a surface")
                .index(),
        );
    } else if launch_admission_started_at
        .is_some_and(|started| started.elapsed() >= Duration::from_millis(SESSION_COMPLETION_TIMEOUT_MSEC))
        && let Some(admission) = launches.timeout_current()
    {
        *launch_admission_started_at = None;
        eprintln!(
            "sophia_session_app schema=2 status=failed source=action transaction={} reason=admission_timeout",
            admission.intent.transaction.raw(),
        );
    }

    let mut logout = false;
    while let Some((transaction, action, target)) = actions.pop_front() {
        if let WmSessionAction::LaunchApplication { application } = action {
            match launches.enqueue(
                SessionLaunchIntent {
                    transaction,
                    application,
                },
                children.len(),
            ) {
                SessionLaunchQueueOutcome::Queued { depth } => println!(
                    "sophia_session_app schema=2 status=queued source=action transaction={} depth={depth}",
                    transaction.raw(),
                ),
                SessionLaunchQueueOutcome::RejectedCapacity => eprintln!(
                    "sophia_session_app schema=2 status=rejected source=action transaction={} reason=capacity",
                    transaction.raw(),
                ),
            }
            println!(
                "sophia_live_wm schema=1 status=session_action_committed transaction={} action={}",
                transaction.raw(),
                session_action_evidence_name(action)
            );
            continue;
        }
        match action {
            WmSessionAction::LaunchApplication { .. } => unreachable!(),
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
            WmSessionAction::Logout => {
                logout = true;
                let cancelled = launches.cancel_pending();
                let admission_cancelled = usize::from(launches.fail_current().is_some());
                *launch_admission_started_at = None;
                if cancelled != 0 {
                    println!(
                        "sophia_session_app schema=2 status=cancelled source=logout pending={cancelled}"
                    );
                }
                if admission_cancelled != 0 {
                    println!(
                        "sophia_session_app schema=2 status=cancelled source=logout admission={admission_cancelled}"
                    );
                }
            }
        }
        println!(
            "sophia_live_wm schema=1 status=session_action_committed transaction={} action={}",
            transaction.raw(),
            session_action_evidence_name(action)
        );
    }

    if logout {
        return Ok(true);
    }
    let Some(intent) = launches.begin_next(startup_ready, admission_pipeline_idle) else {
        return Ok(false);
    };
    if children.len() >= sophia_cli::session_actions::SESSION_ACTION_APPLICATION_CAPACITY {
        let _ = launches.fail_current();
        eprintln!(
            "sophia_session_app schema=2 status=rejected source=action transaction={} reason=capacity",
            intent.transaction.raw(),
        );
        return Ok(false);
    }
    let action = WmSessionAction::LaunchApplication {
        application: intent.application,
    };
    let spawned = if config.normal_session {
        let app = config
            .application_for_action(action)
            .ok_or("WM requested an unadvertised session application")?;
        PersistentXtermSessionConfig::spawn_session_application(app, &config.display, xauthority)
            .map(|child| (Some(app.id.clone()), child))
    } else {
        let program = match intent.application {
            TERMINAL_APPLICATION_ID => Some(config.terminal.as_str()),
            LAUNCHER_APPLICATION_ID => config.session_launcher.as_deref(),
            BROWSER_APPLICATION_ID => config.session_firefox.as_deref(),
            _ => None,
        }
        .ok_or("WM requested an unadvertised session executable")?;
        spawn_approved_application(program, &config.display, xauthority)
            .map(|child| (None, child))
    };
    match spawned {
        Ok((id, child)) => {
            let evidence_id = id.as_deref().unwrap_or("untracked");
            println!(
                "sophia_session_app schema=2 status=started id={evidence_id} source=action transaction={}",
                intent.transaction.raw(),
            );
            // Retain the schema-1 compatibility record consumed by existing
            // physical-session verifiers.
            println!(
                "sophia_session_app schema=1 status=started id={evidence_id} source=action"
            );
            children.push(ManagedSessionChild::for_launch(
                id,
                intent.transaction,
                child,
            ));
            *launch_admission_started_at = Some(Instant::now());
        }
        Err(error) => {
            let _ = launches.fail_current();
            eprintln!(
                "sophia_session_app schema=2 status=failed source=action transaction={} reason=spawn error={error}",
                intent.transaction.raw(),
            );
        }
    }
    Ok(logout)
}
