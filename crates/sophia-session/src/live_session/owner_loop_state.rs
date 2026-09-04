#[derive(Clone, Copy, Debug, Default)]
struct SessionLoopMetrics {
    batches: usize,
    transactions: usize,
    cpu_buffer_updates: usize,
    cpu_buffer_replacements: usize,
    cpu_buffer_patch_updates: usize,
    cpu_buffer_patch_rects: usize,
    cpu_buffer_payload_bytes: usize,
    dma_buf_registrations_observed: usize,
    fence_registrations_observed: usize,
    present_submissions_observed: usize,
    software_present_submissions_observed: usize,
    cpu_compositions: usize,
    coalesced_batches: usize,
    cadence_deferred_batches: usize,
    cadence_repaints: usize,
    /// Batches committed alongside another in one production cycle, so a burst
    /// of client draws costs one cycle rather than one frame each.
    merged_batches: usize,
    max_merge_run: usize,
    backend_ticks: usize,
    runtime_committed: u64,
    runtime_surfaces: u64,
    physical_events: usize,
    physical_keys_routed: usize,
    key_repeats_routed: usize,
    physical_pointer_events: usize,
    physical_pointer_routed: usize,
    physical_pointer_buttons_routed: usize,
    session_ticks: usize,
    max_compose: Duration,
    max_child_reap: Duration,
    max_input_phase: Duration,
    protocol_error_count: usize,
    expected_protocol_error_count: usize,
    cursor_moves_coalesced: u64,
    cursor_max_motion_to_submit: Duration,
}

impl SessionLoopMetrics {
    fn new(initialize_empty_runtime: bool) -> Self {
        Self {
            cpu_compositions: usize::from(initialize_empty_runtime),
            ..Self::default()
        }
    }
}

struct InputDeliveryState {
    next: u64,
    pending: BTreeSet<XAuthorityInputDeliveryId>,
    events_expected: usize,
    events_flushed: usize,
    wait_started_at: Option<Instant>,
    source: Option<&'static str>,
    flush_latency: Option<Duration>,
}

impl Default for InputDeliveryState {
    fn default() -> Self {
        Self {
            next: 1,
            pending: BTreeSet::new(),
            events_expected: 0,
            events_flushed: 0,
            wait_started_at: None,
            source: None,
            flush_latency: None,
        }
    }
}

#[derive(Default)]
struct InputObservationState {
    key_observed: bool,
    key_routed: bool,
    key_suppressed_no_focus: bool,
    pointer_motion_observed: bool,
    pointer_motion_routed: bool,
    pointer_button_observed: bool,
    pointer_buttons_suppressed_no_target: usize,
    pointer_button_routed: bool,
    pointer_focus_target: Option<SurfaceId>,
    pointer_focus_key_routed: bool,
    client_positioned_pointer_button_routed: bool,
    pointer_axis_observed: bool,
    pointer_axis_routed: bool,
    client_positioned_pointer_axis_routed: bool,
    return_suppressed: bool,
}

struct CursorUpdateState {
    dirty: bool,
    dirty_since: Option<Instant>,
}

impl CursorUpdateState {
    fn new(dirty: bool) -> Self {
        Self {
            dirty,
            dirty_since: dirty.then(Instant::now),
        }
    }
}

struct InputDeliveryPhase<'a> {
    receiver: &'a Receiver<XAuthorityClientInputDelivery>,
    state: &'a mut InputDeliveryState,
    client_key_release_barrier: &'a mut BTreeSet<XAuthorityInputDeliveryId>,
    proof_started_at: &'a mut Option<Instant>,
    post_input_deadline: &'a mut Option<Instant>,
}

impl InputDeliveryPhase<'_> {
    fn drain(self) -> Result<(), Box<dyn std::error::Error>> {
        while let Ok(delivery) = self.receiver.try_recv() {
            if !self.state.pending.remove(&delivery.delivery) {
                continue;
            }
            self.client_key_release_barrier.remove(&delivery.delivery);
            match delivery.outcome {
                XAuthorityInputDeliveryOutcome::Flushed => {
                    self.state.events_flushed = self.state.events_flushed.saturating_add(1);
                }
                XAuthorityInputDeliveryOutcome::TargetGone => {
                    self.state.events_expected = self.state.events_expected.saturating_sub(1);
                    crate::session_println!(
                        "sophia_live_session_input_delivery schema=1 status=retired reason=target_gone client={}",
                        delivery.client.raw(),
                    );
                }
                // The session revoked this event itself when it closed the
                // input epoch for a topology, policy, or seat boundary. Ending
                // the session over it would make every pointer motion that
                // overlaps an output change fatal, which is what it did.
                XAuthorityInputDeliveryOutcome::EpochRevoked => {
                    self.state.events_expected = self.state.events_expected.saturating_sub(1);
                    crate::session_println!(
                        "sophia_live_session_input_delivery schema=1 status=retired reason=epoch_revoked client={}",
                        delivery.client.raw(),
                    );
                }
                XAuthorityInputDeliveryOutcome::RouteRejected
                | XAuthorityInputDeliveryOutcome::WriteFailed => {
                    return Err(format!(
                        "persistent live session X11 input delivery failed: outcome={:?} client={}",
                        delivery.outcome,
                        delivery.client.raw(),
                    )
                    .into());
                }
            }
        }
        if let Some(wait_started) = self.state.wait_started_at
            && !self.state.pending.is_empty()
            && wait_started.elapsed()
                >= Duration::from_millis(SESSION_INPUT_DELIVERY_TIMEOUT_MSEC)
        {
            return Err(format!(
                "persistent live session timed out waiting for X11 input delivery: expected={} flushed={} pending={}",
                self.state.events_expected,
                self.state.events_flushed,
                self.state.pending.len(),
            )
            .into());
        }
        if let Some(wait_started) = take_settled_input_delivery_wait(
            &mut self.state.wait_started_at,
            self.state.pending.is_empty(),
        )
            && self.proof_started_at.is_none()
        {
            let flushed_at = Instant::now();
            self.state.flush_latency = Some(flushed_at.saturating_duration_since(wait_started));
            *self.proof_started_at = Some(flushed_at);
            *self.post_input_deadline = Some(
                flushed_at + Duration::from_millis(SESSION_PHYSICAL_PIXEL_TIMEOUT_MSEC),
            );
            crate::session_println!(
                "sophia_live_session_input_pipeline schema=2 status=key_flushed source={} expected={} flushed={}",
                self.state.source.unwrap_or("unknown"),
                self.state.events_expected,
                self.state.events_flushed,
            );
            std::io::stdout().flush()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct SessionQuiescence {
    reason: &'static str,
    started_at: Instant,
    deadline: Instant,
    frontend_authority_drained: bool,
}

impl SessionQuiescence {
    fn new(reason: &'static str, now: Instant, timeout: Duration) -> Self {
        Self {
            reason,
            started_at: now,
            deadline: now + timeout,
            frontend_authority_drained: false,
        }
    }

    fn mark_frontend_authority_drained(&mut self) {
        self.frontend_authority_drained = true;
    }

    fn decision(
        &self,
        now: Instant,
        snapshot: SessionQuiescenceSnapshot,
    ) -> SessionQuiescenceDecision {
        if self.frontend_authority_drained
            && snapshot.pending_authority_batches == 0
            && snapshot.pending_coordinator_work == 0
            && !snapshot.cpu_update_pending
            && !snapshot.native_work_pending
        {
            SessionQuiescenceDecision::Complete
        } else if now >= self.deadline {
            SessionQuiescenceDecision::TimedOut
        } else {
            SessionQuiescenceDecision::Pending
        }
    }

    fn elapsed(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.started_at)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SessionQuiescenceSnapshot {
    pending_authority_batches: usize,
    pending_coordinator_work: usize,
    cpu_update_pending: bool,
    native_work_pending: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SessionQuiescenceDecision {
    Pending,
    Complete,
    TimedOut,
}
