/// Distinct (major, minor, code) triples retained before the tally resets.
///
/// The reset re-emits rather than dropping, so a client churning through opcodes
/// stays visible instead of silently capping.
const SESSION_PROTOCOL_ERROR_TALLY_MAX_ENTRIES: usize = 64;

/// Every X protocol error a session saw, grouped by the request that produced it.
///
/// The session keeps only the first error otherwise, so a run reporting two dozen
/// failures named one opcode and discarded the rest. Each cause then cost its own
/// physical run to find. This carries opcodes and counts only: the resource id
/// stays inside the frontend, because a raw XID may not reach a default-level log.
#[derive(Debug, Default)]
struct SessionProtocolErrorTally {
    counts: BTreeMap<(u8, u16, u8), u64>,
    discarded: u64,
}

impl SessionProtocolErrorTally {
    fn observe(&mut self, error: &sophia_x_authority::XAuthorityProtocolErrorObservation) {
        let key = (error.major_code, error.minor_code, error.code);
        if self.counts.len() >= SESSION_PROTOCOL_ERROR_TALLY_MAX_ENTRIES
            && !self.counts.contains_key(&key)
        {
            // Keep the newest opcodes rather than the oldest, and say how many
            // observations the reset dropped so the total still reconciles.
            self.discarded = self
                .discarded
                .saturating_add(self.counts.values().copied().sum());
            self.counts.clear();
        }
        let count = self.counts.entry(key).or_insert(0);
        *count = count.saturating_add(1);
    }

    fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// One line per opcode, never a merged bucket, so a run that hit two causes
    /// cannot disguise itself as one.
    fn report(&self) {
        let distinct = self.counts.len();
        for ((major, minor, code), count) in &self.counts {
            println!(
                "sophia_live_session_protocol_error_tally schema=1 status=degraded major={major} minor={minor} code={code} count={count} distinct={distinct} discarded={}",
                self.discarded
            );
        }
    }

    /// A compact summary for the failure string, which is all a failed run prints.
    fn summary(&self) -> String {
        self.counts
            .iter()
            .map(|((major, minor, code), count)| {
                format!("{major}/{minor}/{code}x{count}")
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn session_protocol_errors_are_fatal(
    normal_session: bool,
    application_proof: bool,
    protocol_error_count: usize,
) -> bool {
    protocol_error_count != 0 && (normal_session || application_proof)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SessionFatalCleanupEvidence {
    frontend_intake_stopped: bool,
    native_heads_in_flight_before: usize,
    native_cleanup_required: bool,
    native_suspend_attempted: bool,
    native_suspend_reported: bool,
    native_drained: bool,
    abandoned_scanouts: usize,
    renderer_images_cleared: bool,
    presentations_shutdown: bool,
}

impl SessionFatalCleanupEvidence {
    const fn clean(self) -> bool {
        self.frontend_intake_stopped
            && (!self.native_cleanup_required
            || (self.native_suspend_attempted
                && self.native_suspend_reported
                && self.native_drained
                && self.abandoned_scanouts == 0
                && self.renderer_images_cleared))
            && self.presentations_shutdown
    }
}

fn settle_session_fatal_error(
    original: &str,
    evidence: SessionFatalCleanupEvidence,
    cleanup_failures: &[String],
) -> String {
    if evidence.clean() && cleanup_failures.is_empty() {
        return original.to_owned();
    }
    let details = if cleanup_failures.is_empty() {
        "bounded cleanup did not reach a clean terminal state".to_owned()
    } else {
        cleanup_failures.join("; ")
    };
    format!("{original}; bounded session cleanup failed: {details}")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PhysicalInputRoutingMode {
    Suppressed,
    CursorOnly,
    ShortcutsOnly,
    ControlPlaneOnly,
    Full,
}

fn physical_input_routing_mode_label(mode: PhysicalInputRoutingMode) -> &'static str {
    match mode {
        PhysicalInputRoutingMode::Suppressed => "suppressed",
        PhysicalInputRoutingMode::CursorOnly => "cursor_only",
        PhysicalInputRoutingMode::ShortcutsOnly => "shortcuts_only",
        PhysicalInputRoutingMode::ControlPlaneOnly => "control_plane_only",
        PhysicalInputRoutingMode::Full => "full",
    }
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
    // An unchanged focus satisfies the request as fully as a moved one. Treating
    // it as unsatisfied would keep the request pending and re-arm it every turn,
    // which is the loop this idempotency exists to end.
    (!matches!(
        decision,
        InputFocusDecision::Focused | InputFocusDecision::AlreadyFocused
    ))
    .then_some(request)
}

struct InitialSessionFocusContext<'a> {
    runtime: &'a LiveProductionVisualRuntime,
    focus: &'a mut InputFocusState,
    seat: SeatId,
    wm_session_present: bool,
    layout: &'a PersistentLiveLayout,
    session_controls: &'a mut SessionControlQueue,
    next_focus_control_transaction: &'a mut u64,
}

fn initial_session_focus_candidate(
    wm_session_present: bool,
    focused_surface: Option<SurfaceId>,
    committed_surfaces: &[CommittedSurfaceState],
) -> Option<SurfaceId> {
    if wm_session_present || focused_surface.is_some() {
        return None;
    }
    committed_surfaces.first().map(|surface| surface.surface)
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
        session_controls,
        next_focus_control_transaction,
    } = context;
    let Some(surface) = initial_session_focus_candidate(
        wm_session_present,
        focus.focused_surface(seat),
        runtime.committed_surfaces(),
    ) else {
        return Ok(());
    };
    // `AlreadyFocused` is unreachable here: the candidate is only produced when
    // the seat holds no focus at all, so a decision other than `Focused` means
    // the surface could not take it.
    if focus.focus_surface(seat, surface, runtime.committed_surfaces())
        != InputFocusDecision::Focused
    {
        return Ok(());
    }
    let client = layout
        .client_routes
        .client_for_surface(surface)
        .ok_or("initial X11 focus has no client route")?;
    let transaction = TransactionId::from_raw(*next_focus_control_transaction);
    *next_focus_control_transaction = next_focus_control_transaction
        .checked_add(1)
        .ok_or("initial X11 focus transaction exhausted")?;
    session_controls
        .enqueue(XAuthorityClientControlCommand {
            client,
            command: XAuthorityControlCommand::FocusSurface {
                transaction,
                surface,
            },
        }, Instant::now())
        .map_err(|error| format!("failed to queue initial X11 focus: {error:?}"))?;
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
    focused_gpu_presented: bool,
    cpu_baseline_presented: bool,
) -> bool {
    focused_gpu_presented || cpu_baseline_presented
}

fn current_cpu_frame_is_presented(
    scene_frame: Option<(u64, usize)>,
    native_frame: Option<(u64, usize)>,
) -> bool {
    scene_frame.is_some_and(|(checksum, nonzero_pixel_bytes)| {
        nonzero_pixel_bytes > 0
            && native_frame.is_none_or(|(presented_logical_checksum, nonzero_exports)| {
                presented_logical_checksum == checksum && nonzero_exports > 0
            })
    })
}

fn stable_gpu_frame_proves_post_input_pixels(
    input_delivery_complete: bool,
    input_surface: Option<SurfaceId>,
    retired_surface: SurfaceId,
    stable: bool,
) -> bool {
    input_delivery_complete && stable && input_surface == Some(retired_surface)
}

fn physical_input_page_flip_correlates(
    input_delivery_complete: bool,
    input_pixel_change: bool,
    ingress_ust_usec: u64,
    baseline_submission: usize,
    presented_submission: usize,
    submission_ust_usec: u64,
    page_flip_ust_usec: u64,
) -> bool {
    input_delivery_complete
        && input_pixel_change
        && presented_submission > baseline_submission
        && submission_ust_usec >= ingress_ust_usec
        && page_flip_ust_usec >= submission_ust_usec
}

fn software_batch_may_coalesce(batch: &XAuthorityObservedTransactionBatch) -> bool {
    batch.removed_surfaces.is_empty()
        && batch.dma_buf_registrations.is_empty()
        && batch.fence_registrations.is_empty()
        && batch.present_submissions.is_empty()
        && batch.software_present_submissions.is_empty()
        && batch.released_dma_bufs.is_empty()
        && batch.released_fences.is_empty()
        && (!batch.transactions.is_empty() || !batch.cpu_buffer_updates.is_empty())
}

/// Queued batches one authority drain may buffer.
const AUTHORITY_DRAIN_CAPACITY: usize = 64;
/// Batches one production cycle may commit together, counting the head. The
/// bound keeps a single owner turn short enough that native frame service is
/// not starved; the producer channel holds 256, so a burst still drains in a
/// handful of cycles rather than one per frame.
const AUTHORITY_MERGE_RUN_LIMIT: usize = 64;

/// Committed transactions a merged run may add beyond its head.
///
/// Every commit contributes one runtime observation, and the session runtime
/// refuses a tick whose batch exceeds `MAX_SESSION_RUNTIME_OBSERVATION_BATCH`.
/// The real budget is therefore transactions, not batches: one batch can carry
/// several. The reserve leaves room for the fixed per-tick observations — tick
/// start and end, event polling, frame scheduling, scanout state — that share
/// the same batch.
const AUTHORITY_MERGE_OBSERVATION_RESERVE: usize = 16;
const AUTHORITY_MERGE_TRANSACTION_LIMIT: usize =
    sophia_runtime::MAX_SESSION_RUNTIME_OBSERVATION_BATCH - AUTHORITY_MERGE_OBSERVATION_RESERVE;

/// Whether a batch carries only client content: pixels and geometry for
/// surfaces the layout already knows, with no lifecycle, reservation, or
/// resource edge that a later batch in the same cycle could reorder.
fn authority_batch_is_pure_content(batch: &XAuthorityObservedTransactionBatch) -> bool {
    software_batch_may_coalesce(batch)
        && batch.surface_presentations.is_empty()
        && batch.presentation_intents.is_empty()
        && batch.surface_output_reservations.is_empty()
}

/// Whether a batch may follow another inside one merged commit run.
///
/// A raster response is evaluated against the requirement state of its own
/// cycle, so it may open a run but never join one: merging accepts every
/// response before any commit, and a response admitted against a state two
/// batches stale is exactly the confusion this boundary exists to prevent.
fn authority_batch_may_follow_merge(batch: &XAuthorityObservedTransactionBatch) -> bool {
    authority_batch_is_pure_content(batch) && batch.raster_responses.is_empty()
}

/// Whether a batch carries work a production cycle must observe.
fn authority_batch_has_engine_work(batch: &XAuthorityObservedTransactionBatch) -> bool {
    !batch.transactions.is_empty()
        || !batch.removed_surfaces.is_empty()
        || !batch.cpu_buffer_updates.is_empty()
        || !batch.dma_buf_registrations.is_empty()
        || !batch.fence_registrations.is_empty()
        || !batch.present_submissions.is_empty()
        || !batch.software_present_submissions.is_empty()
        || !batch.released_dma_bufs.is_empty()
        || !batch.released_fences.is_empty()
        || !batch.surface_presentations.is_empty()
        || !batch.presentation_intents.is_empty()
        || !batch.surface_output_reservations.is_empty()
}

/// How many queued batches this production cycle commits together, counting
/// `head`.
///
/// One reproduces the historical cadence exactly. A longer run is admitted
/// only while the admission pipeline is quiescent — projection drains the
/// released-group queue, so a release landing between two projections in one
/// cycle would emit a quarantined group twice — and only across batches that
/// carry nothing but client content. A repeated transaction identity also ends
/// the run: production groups are bucketed per projection call, so a repeat
/// would split one atomic group across the concatenation.
fn authority_merge_run_len<'a>(
    head: &XAuthorityObservedTransactionBatch,
    queued: impl IntoIterator<Item = &'a XAuthorityObservedTransactionBatch>,
    admission_quiescent: bool,
    limit: usize,
) -> usize {
    if limit <= 1 || !admission_quiescent || !authority_batch_is_pure_content(head) {
        return 1;
    }
    let mut identities = BTreeSet::new();
    authority_batch_transaction_identities(head, &mut identities);
    let mut len = 1;
    // The head commits regardless of its size, exactly as it did before
    // merging; only the batches joining it are held to the observation budget.
    let mut transactions = head.transactions.len();
    for batch in queued {
        if len >= limit || !authority_batch_may_follow_merge(batch) {
            break;
        }
        let joined = transactions.saturating_add(batch.transactions.len());
        if joined > AUTHORITY_MERGE_TRANSACTION_LIMIT {
            break;
        }
        let mut candidate = identities.clone();
        let before = candidate.len();
        authority_batch_transaction_identities(batch, &mut candidate);
        if candidate.len() != before.saturating_add(batch_transaction_identity_count(batch)) {
            break;
        }
        identities = candidate;
        transactions = joined;
        len = len.saturating_add(1);
    }
    len
}

fn authority_batch_transaction_identities(
    batch: &XAuthorityObservedTransactionBatch,
    identities: &mut BTreeSet<TransactionId>,
) {
    identities.insert(batch.transaction);
    for transaction in &batch.transactions {
        identities.insert(transaction.transaction);
    }
}

fn batch_transaction_identity_count(batch: &XAuthorityObservedTransactionBatch) -> usize {
    let mut identities = BTreeSet::new();
    authority_batch_transaction_identities(batch, &mut identities);
    identities.len()
}

/// Buffers immediately available batches without blocking.
///
/// A `try_recv` returning `Empty` means nothing is ready now, so the drain
/// ends there rather than spinning: the next iteration sees whatever arrives,
/// and spinning would couple owner latency to producer scheduling.
fn drain_queued_authority_batches(
    receiver: &std::sync::mpsc::Receiver<XAuthorityObservedTransactionBatch>,
    queued: &mut VecDeque<XAuthorityObservedTransactionBatch>,
    capacity: usize,
    budget: Duration,
) -> Result<(), &'static str> {
    let started = Instant::now();
    while queued.len() < capacity && started.elapsed() < budget {
        match receiver.try_recv() {
            Ok(batch) => queued.push_back(batch),
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                return Err("persistent X authority transaction channel disconnected");
            }
        }
    }
    Ok(())
}

struct SessionActionExecutionContext<'a> {
    config: &'a PersistentXtermSessionConfig,
    xauthority: &'a std::path::Path,
    children: &'a mut Vec<ManagedSessionChild>,
    launches: &'a mut SessionLaunchQueue,
    launch_admission_started_at: &'a mut Option<Instant>,
    startup_ready: bool,
    admission_pipeline_idle: bool,
    stable_admission_surface: Option<SurfaceId>,
    withdrawn_admissions: &'a [SurfaceId],
    layout: &'a PersistentLiveLayout,
    focus: &'a InputFocusState,
    seat: SeatId,
    session_controls: &'a mut SessionControlQueue,
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
        stable_admission_surface,
        withdrawn_admissions,
        layout,
        focus,
        seat,
        session_controls,
    } = context;
    if let Some(admission) =
        launches.complete_if_stable(admission_pipeline_idle, stable_admission_surface)
    {
        *launch_admission_started_at = None;
        println!(
            "sophia_session_app schema=2 status=admitted source=action transaction={} surface={}",
            admission.intent.transaction.raw(),
            stable_admission_surface
                .expect("settled launch admission requires a stable surface")
                .index(),
        );
    } else if let Some(admission) = launches.withdraw_current(withdrawn_admissions) {
        // The surface this launch was waiting on is gone, so the remaining
        // budget would be spent waiting for something that cannot arrive --
        // and every later press queues behind it in silence.
        *launch_admission_started_at = None;
        eprintln!(
            "sophia_session_app schema=2 status=failed source=action transaction={} reason=surface_withdrawn",
            admission.intent.transaction.raw(),
        );
    } else if launch_admission_started_at
        .is_some_and(|started| {
            started.elapsed() >= Duration::from_millis(SESSION_APP_ADMISSION_TIMEOUT_MSEC)
        })
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
                session_controls.enqueue(XAuthorityClientControlCommand {
                    client,
                    command: XAuthorityControlCommand::CloseSurface {
                        transaction,
                        surface,
                    },
                }, Instant::now()).map_err(|error| {
                    format!("failed to queue polite close control: {error:?}")
                })?;
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
            BROWSER_APPLICATION_ID => config.session_browser.as_deref(),
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
