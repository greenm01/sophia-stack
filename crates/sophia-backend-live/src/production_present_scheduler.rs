use crate::{
    LivePresentationResourceSession, LivePresentationSubmission, LiveProductionAuthorityGroup,
    LiveProductionNativeFrameId, LiveProductionPresentDisposition,
};
use sophia_engine::{PreparedSurfaceCommit, SURFACE_CONTENT_STREAM_CAPACITY};
use sophia_protocol::{
    BufferSource, CommittedSurfaceState, LayerSnapshot, Rect, SurfaceId, SurfaceTransaction,
    SurfaceTransactionKey, TransactionId,
};
use sophia_renderer_live::LiveCpuPresentationLayer;
use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};

const LAYOUT_EPOCH_HISTORY_CAPACITY: usize = SURFACE_CONTENT_STREAM_CAPACITY;

#[derive(Clone, Debug)]
pub struct LiveProductionQueuedPresent {
    pub submission: LivePresentationSubmission,
    pub surface: sophia_protocol::SurfaceId,
    pub candidate: SurfaceTransaction,
    pub cpu_layers: Arc<[LiveCpuPresentationLayer]>,
    pub target: Rect,
    pub surface_clip: Rect,
    layout_state: LiveProductionPresentLayoutState,
    x_offset: i32,
    y_offset: i32,
    deadline: Instant,
    not_before: Instant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveProductionPresentLayoutState {
    Runnable,
    Staged { epoch: TransactionId },
    AwaitingVisibility { epoch: TransactionId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveProductionLayoutEpochOutcome {
    Committed,
    Aborted,
}

impl LiveProductionPresentLayoutState {
    const fn runnable(self) -> bool {
        matches!(self, Self::Runnable)
    }
}

impl LiveProductionQueuedPresent {
    const fn runnable(&self) -> bool {
        self.layout_state.runnable()
    }
}

#[derive(Debug)]
pub struct LiveProductionSubmittedPresent {
    pub frame: LiveProductionNativeFrameId,
    pub candidate: SurfaceTransactionKey,
    pub transaction: TransactionId,
    pub surface: sophia_protocol::SurfaceId,
    pub prepared: PreparedSurfaceCommit,
    pub displayed_layer: crate::LiveRetainedRendererImageLayer,
}

#[derive(Debug)]
enum LiveProductionInFlightPresent {
    Rendering(LiveProductionSubmittedPresent),
    Submitted(LiveProductionSubmittedPresent),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionPresentGate {
    Idle,
    SubmittedInFlight,
    WaitingAcquire,
    Reject(TransactionId),
    Ready(TransactionId),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveProductionLayoutRollbackReport {
    pub rejected: Vec<TransactionId>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveProductionLayoutReleaseReport {
    pub released: usize,
    pub superseded: Vec<TransactionId>,
}

#[derive(Debug, Default)]
pub struct LiveProductionPresentScheduler {
    queued: VecDeque<LiveProductionQueuedPresent>,
    in_flight: Option<LiveProductionInFlightPresent>,
    resolved_layout_epochs: VecDeque<(TransactionId, LiveProductionLayoutEpochOutcome)>,
    highest_resolved_layout_epoch: Option<TransactionId>,
    first_acquire_delay: Option<Duration>,
    first_acquire_delay_applied: bool,
    reject_first_present: bool,
    acquire_waits: usize,
    controlled_rejections: usize,
    pending_supersessions: usize,
    max_pending_queued: usize,
    max_total_queued: usize,
    diagnose_first_mixed_export: bool,
}

impl LiveProductionPresentScheduler {
    pub fn with_controls(
        mut self,
        first_acquire_delay: Option<Duration>,
        reject_first_present: bool,
        diagnose_first_mixed_export: bool,
    ) -> Self {
        self.first_acquire_delay = first_acquire_delay;
        self.reject_first_present = reject_first_present;
        self.diagnose_first_mixed_export = diagnose_first_mixed_export;
        self
    }

    pub fn enqueue_group(
        &mut self,
        group: &LiveProductionAuthorityGroup,
        presentation_layout: &[LayerSnapshot],
        cpu_layers: Vec<LiveCpuPresentationLayer>,
        resources: &mut LivePresentationResourceSession,
        now: Instant,
    ) -> Result<Vec<TransactionId>, Box<dyn Error>> {
        let mut superseded = Vec::new();
        let cpu_layers: Arc<[LiveCpuPresentationLayer]> = cpu_layers.into();
        group.validate()?;
        for submission in &group.present_submissions {
            let resolved_layout = match submission.layout_disposition {
                LiveProductionPresentDisposition::StageLayout { epoch } => {
                    self.resolved_layout_epoch(epoch)
                }
                _ => None,
            };
            let visible = presentation_layout
                .iter()
                .any(|layer| layer.surface == submission.surface);
            let layout_state = match (submission.layout_disposition, resolved_layout) {
                (LiveProductionPresentDisposition::Immediate, _) => {
                    LiveProductionPresentLayoutState::Runnable
                }
                (
                    LiveProductionPresentDisposition::StageLayout { .. },
                    Some(LiveProductionLayoutEpochOutcome::Committed),
                ) if visible => LiveProductionPresentLayoutState::Runnable,
                (
                    LiveProductionPresentDisposition::StageLayout { epoch },
                    Some(LiveProductionLayoutEpochOutcome::Committed),
                ) => LiveProductionPresentLayoutState::AwaitingVisibility { epoch },
                (
                    LiveProductionPresentDisposition::StageLayout { .. },
                    Some(LiveProductionLayoutEpochOutcome::Aborted),
                ) => LiveProductionPresentLayoutState::Runnable,
                (LiveProductionPresentDisposition::StageLayout { epoch }, None) => {
                    LiveProductionPresentLayoutState::Staged { epoch }
                }
                (LiveProductionPresentDisposition::RejectSuperseded, _) => {
                    LiveProductionPresentLayoutState::Runnable
                }
            };
            let reject_for_layout = matches!(
                (submission.layout_disposition, resolved_layout),
                (LiveProductionPresentDisposition::RejectSuperseded, _)
                    | (
                        LiveProductionPresentDisposition::StageLayout { .. },
                        Some(LiveProductionLayoutEpochOutcome::Aborted)
                    )
            );
            let surface = submission.surface;
            let x_offset = submission.x_offset;
            let y_offset = submission.y_offset;
            let submission = LivePresentationSubmission {
                transaction: submission.transaction,
                buffer: submission.buffer,
                acquire_fence: submission.acquire_fence,
                idle_fence: submission.idle_fence,
            };
            resources.begin(submission)?;
            if reject_for_layout {
                self.controlled_rejections = self.controlled_rejections.saturating_add(1);
                superseded.push(submission.transaction);
                continue;
            }
            let mut candidates = group.transactions.iter().filter(|transaction| {
                transaction.surface == surface
                    && transaction.transaction == submission.transaction
                    && transaction.target_buffer()
                        == (BufferSource::DmaBuf {
                            handle: submission.buffer.raw(),
                        })
            });
            let Some(candidate) = candidates.next() else {
                self.controlled_rejections = self.controlled_rejections.saturating_add(1);
                superseded.push(submission.transaction);
                continue;
            };
            if candidates.next().is_some() {
                self.controlled_rejections = self.controlled_rejections.saturating_add(1);
                superseded.push(submission.transaction);
                continue;
            }
            let mut candidate = candidate.clone();
            let geometry = presentation_layout
                .iter()
                .find(|layer| layer.surface == surface)
                .map_or(candidate.target_geometry, |layer| layer.geometry);
            candidate.target_geometry = geometry;
            let acquire_delay =
                if !self.first_acquire_delay_applied && self.first_acquire_delay.is_some() {
                    self.first_acquire_delay_applied = true;
                    self.first_acquire_delay.unwrap_or(Duration::ZERO)
                } else {
                    Duration::ZERO
                };
            let not_before = now + acquire_delay;
            if layout_state.runnable() {
                // A newer Present replaces queued work only for the same
                // surface. Unrelated surfaces carry independent visual debt
                // and must each reach retirement or explicit rejection.
                self.supersede_queued_where(
                    |queued| queued.runnable() && queued.surface == surface,
                    &mut superseded,
                );
            } else {
                self.supersede_queued_where(
                    |queued| !queued.runnable() && queued.surface == surface,
                    &mut superseded,
                );
            }
            let timeout_msec = candidate.timeout_msec.clamp(100, 2_000);
            self.queued.push_back(LiveProductionQueuedPresent {
                submission,
                surface,
                candidate,
                cpu_layers: Arc::clone(&cpu_layers),
                target: Rect {
                    x: geometry.x.saturating_add(x_offset),
                    y: geometry.y.saturating_add(y_offset),
                    ..geometry
                },
                surface_clip: geometry,
                layout_state,
                x_offset,
                y_offset,
                deadline: not_before + Duration::from_millis(u64::from(timeout_msec)),
                not_before,
            });
            self.observe_queue_depth();
        }
        Ok(superseded)
    }

    fn supersede_queued_where(
        &mut self,
        mut predicate: impl FnMut(&LiveProductionQueuedPresent) -> bool,
        superseded: &mut Vec<TransactionId>,
    ) {
        let mut retained = VecDeque::with_capacity(self.queued.len());
        while let Some(queued) = self.queued.pop_front() {
            if predicate(&queued) {
                superseded.push(queued.submission.transaction);
                self.pending_supersessions = self.pending_supersessions.saturating_add(1);
            } else {
                retained.push_back(queued);
            }
        }
        self.queued = retained;
    }

    fn retain_newest_runnable_per_surface(&mut self) -> Vec<TransactionId> {
        let mut newest = BTreeMap::new();
        for (index, queued) in self
            .queued
            .iter()
            .enumerate()
            .filter(|(_, queued)| queued.runnable())
        {
            newest.insert(queued.surface, index);
        }
        let mut retained = VecDeque::with_capacity(self.queued.len());
        let mut superseded = Vec::new();
        for (index, queued) in self.queued.drain(..).enumerate() {
            if queued.runnable() && newest.get(&queued.surface) != Some(&index) {
                superseded.push(queued.submission.transaction);
                self.pending_supersessions = self.pending_supersessions.saturating_add(1);
            } else {
                retained.push_back(queued);
            }
        }
        self.queued = retained;
        superseded
    }

    fn observe_queue_depth(&mut self) {
        self.max_pending_queued = self.max_pending_queued.max(
            self.queued
                .iter()
                .filter(|queued| queued.runnable())
                .count(),
        );
        self.max_total_queued = self.max_total_queued.max(self.queued.len());
    }

    pub fn reproject_surface(&mut self, surface: SurfaceId, geometry: Rect) {
        for queued in &mut self.queued {
            if queued.surface != surface {
                continue;
            }
            queued.target = Rect {
                x: geometry.x.saturating_add(queued.x_offset),
                y: geometry.y.saturating_add(queued.y_offset),
                ..geometry
            };
            queued.surface_clip = geometry;
            queued.candidate.target_geometry = geometry;
        }
    }

    pub fn poll_gate(
        &mut self,
        resources: &mut LivePresentationResourceSession,
        now: Instant,
    ) -> Result<LiveProductionPresentGate, Box<dyn Error>> {
        if self.in_flight.is_some() {
            return Ok(LiveProductionPresentGate::SubmittedInFlight);
        }
        let Some(eligible) = self
            .queued
            .iter()
            .position(|queued| queued.layout_state.runnable())
        else {
            return Ok(LiveProductionPresentGate::Idle);
        };
        if eligible != 0 {
            let queued = self
                .queued
                .remove(eligible)
                .expect("eligible queued Present index exists");
            self.queued.push_front(queued);
        }
        let queued = self
            .queued
            .front()
            .expect("eligible Present was moved to the queue front");
        let transaction = queued.submission.transaction;
        if now < queued.not_before {
            self.acquire_waits = self.acquire_waits.saturating_add(1);
            return Ok(LiveProductionPresentGate::WaitingAcquire);
        }
        if !resources.poll_acquire_fence(transaction)? {
            self.acquire_waits = self.acquire_waits.saturating_add(1);
            if now >= queued.deadline {
                self.queued.pop_front();
                return Ok(LiveProductionPresentGate::Reject(transaction));
            }
            return Ok(LiveProductionPresentGate::WaitingAcquire);
        }
        if self.reject_first_present {
            self.reject_first_present = false;
            self.controlled_rejections = self.controlled_rejections.saturating_add(1);
            self.queued.pop_front();
            return Ok(LiveProductionPresentGate::Reject(transaction));
        }
        Ok(LiveProductionPresentGate::Ready(transaction))
    }

    pub fn front(&self) -> Option<&LiveProductionQueuedPresent> {
        self.queued.front()
    }

    pub fn pop_front(&mut self) -> Option<LiveProductionQueuedPresent> {
        self.queued.pop_front()
    }

    pub fn mark_submitted(&mut self, submitted: LiveProductionSubmittedPresent) {
        self.in_flight = Some(LiveProductionInFlightPresent::Submitted(submitted));
    }

    pub fn mark_rendering(&mut self, rendering: LiveProductionSubmittedPresent) {
        self.in_flight = Some(LiveProductionInFlightPresent::Rendering(rendering));
    }

    pub fn promote_rendering_to_submitted(&mut self) -> Option<TransactionId> {
        match self.in_flight.take()? {
            LiveProductionInFlightPresent::Rendering(rendering) => {
                let transaction = rendering.transaction;
                self.in_flight = Some(LiveProductionInFlightPresent::Submitted(rendering));
                Some(transaction)
            }
            submitted @ LiveProductionInFlightPresent::Submitted(_) => {
                self.in_flight = Some(submitted);
                None
            }
        }
    }

    pub fn take_rendering(&mut self) -> Option<LiveProductionSubmittedPresent> {
        match self.in_flight.take()? {
            LiveProductionInFlightPresent::Rendering(rendering) => Some(rendering),
            submitted @ LiveProductionInFlightPresent::Submitted(_) => {
                self.in_flight = Some(submitted);
                None
            }
        }
    }

    pub fn take_submitted(&mut self) -> Option<LiveProductionSubmittedPresent> {
        match self.in_flight.take()? {
            LiveProductionInFlightPresent::Submitted(submitted) => Some(submitted),
            rendering @ LiveProductionInFlightPresent::Rendering(_) => {
                self.in_flight = Some(rendering);
                None
            }
        }
    }

    pub fn has_queued(&self) -> bool {
        !self.queued.is_empty()
    }

    pub fn has_runnable_queued(&self) -> bool {
        self.in_flight.is_none() && self.has_eligible()
    }

    pub fn has_submitted(&self) -> bool {
        matches!(
            self.in_flight,
            Some(LiveProductionInFlightPresent::Submitted(_))
        )
    }

    pub fn submitted_frame(&self) -> Option<LiveProductionNativeFrameId> {
        match self.in_flight.as_ref()? {
            LiveProductionInFlightPresent::Submitted(submitted) => Some(submitted.frame),
            LiveProductionInFlightPresent::Rendering(_) => None,
        }
    }

    pub fn in_flight_displayed_layer(
        &self,
    ) -> Option<(SurfaceId, &crate::LiveRetainedRendererImageLayer)> {
        match self.in_flight.as_ref()? {
            LiveProductionInFlightPresent::Rendering(present)
            | LiveProductionInFlightPresent::Submitted(present) => {
                Some((present.surface, &present.displayed_layer))
            }
        }
    }

    pub fn in_flight_candidate(&self) -> Option<&[CommittedSurfaceState]> {
        match self.in_flight.as_ref()? {
            LiveProductionInFlightPresent::Rendering(present)
            | LiveProductionInFlightPresent::Submitted(present) => {
                Some(present.prepared.candidate())
            }
        }
    }

    pub fn has_rendering(&self) -> bool {
        matches!(
            self.in_flight,
            Some(LiveProductionInFlightPresent::Rendering(_))
        )
    }

    pub fn has_in_flight(&self) -> bool {
        self.in_flight.is_some()
    }

    pub fn has_eligible(&self) -> bool {
        self.queued
            .iter()
            .any(|queued| queued.layout_state.runnable())
    }

    pub fn has_layout_deferred(&self) -> bool {
        self.queued
            .iter()
            .any(|queued| !queued.layout_state.runnable())
    }

    /// Records that one layout epoch committed. Its Presents remain fenced
    /// until the committed projection contains their surfaces.
    pub fn commit_layout_epoch(&mut self, epoch: TransactionId) -> usize {
        self.record_layout_epoch(epoch, LiveProductionLayoutEpochOutcome::Committed);
        let mut committed = 0usize;
        for queued in &mut self.queued {
            if queued.layout_state == (LiveProductionPresentLayoutState::Staged { epoch }) {
                queued.layout_state =
                    LiveProductionPresentLayoutState::AwaitingVisibility { epoch };
                committed = committed.saturating_add(1);
            }
        }
        committed
    }

    /// Releases staged Presents only after their surfaces enter the committed
    /// presentation projection. A rollback-preserved admission Present can be
    /// pixel-coherent before it is visible; releasing it earlier would make
    /// `drive_gpu_presentation` reject it as an absent surface.
    pub fn release_layout_deferred_for_surfaces(
        &mut self,
        visible: &[SurfaceId],
        committed: &[CommittedSurfaceState],
    ) -> LiveProductionLayoutReleaseReport {
        let mut released = 0usize;
        for queued in &mut self.queued {
            if matches!(
                queued.layout_state,
                LiveProductionPresentLayoutState::AwaitingVisibility { .. }
            ) && visible.contains(&queued.surface)
            {
                // Recovery admission may first commit a CPU snapshot for this
                // surface. The preserved DMA-BUF is the successor to that
                // snapshot, not a competing candidate for the old generation.
                queued.candidate.previous_committed_generation = committed
                    .iter()
                    .find(|state| state.surface == queued.surface)
                    .map_or(0, |state| state.committed_generation);
                queued.layout_state = LiveProductionPresentLayoutState::Runnable;
                released = released.saturating_add(1);
            }
        }
        let superseded = self.retain_newest_runnable_per_surface();
        self.observe_queue_depth();
        LiveProductionLayoutReleaseReport {
            released,
            superseded,
        }
    }

    /// Rejects only Presents staged by the epoch that actually aborted.
    pub fn abort_layout_epoch(
        &mut self,
        epoch: TransactionId,
    ) -> LiveProductionLayoutRollbackReport {
        self.record_layout_epoch(epoch, LiveProductionLayoutEpochOutcome::Aborted);
        let mut retained = VecDeque::with_capacity(self.queued.len());
        let mut report = LiveProductionLayoutRollbackReport::default();
        while let Some(queued) = self.queued.pop_front() {
            if queued.layout_state == (LiveProductionPresentLayoutState::Staged { epoch }) {
                report.rejected.push(queued.submission.transaction);
            } else {
                retained.push_back(queued);
            }
        }
        self.queued = retained;
        report
    }

    fn record_layout_epoch(
        &mut self,
        epoch: TransactionId,
        outcome: LiveProductionLayoutEpochOutcome,
    ) {
        if let Some((_, current)) = self
            .resolved_layout_epochs
            .iter_mut()
            .find(|(candidate, _)| *candidate == epoch)
        {
            *current = outcome;
            return;
        }
        if self.resolved_layout_epochs.len() == LAYOUT_EPOCH_HISTORY_CAPACITY {
            self.resolved_layout_epochs.pop_front();
        }
        self.resolved_layout_epochs.push_back((epoch, outcome));
        if self
            .highest_resolved_layout_epoch
            .is_none_or(|highest| epoch.raw() > highest.raw())
        {
            self.highest_resolved_layout_epoch = Some(epoch);
        }
    }

    fn resolved_layout_epoch(
        &self,
        epoch: TransactionId,
    ) -> Option<LiveProductionLayoutEpochOutcome> {
        self.resolved_layout_epochs
            .iter()
            .rev()
            .find_map(|(candidate, outcome)| (*candidate == epoch).then_some(*outcome))
            .or_else(|| {
                // A content-stream delay can outlive the bounded exact
                // history. Resolved WM epochs are monotonic; fail closed
                // instead of recreating a stage that no future event can end.
                self.highest_resolved_layout_epoch
                    .is_some_and(|highest| epoch.raw() <= highest.raw())
                    .then_some(LiveProductionLayoutEpochOutcome::Aborted)
            })
    }

    pub fn take_diagnose_first_mixed_export(&mut self) -> bool {
        std::mem::take(&mut self.diagnose_first_mixed_export)
    }

    pub fn drain_transactions(&mut self) -> Vec<TransactionId> {
        self.queued
            .drain(..)
            .map(|queued| queued.submission.transaction)
            .collect()
    }

    pub const fn acquire_waits(&self) -> usize {
        self.acquire_waits
    }

    pub const fn controlled_rejections(&self) -> usize {
        self.controlled_rejections
    }

    pub const fn pending_supersessions(&self) -> usize {
        self.pending_supersessions
    }

    pub const fn max_pending_queued(&self) -> usize {
        self.max_pending_queued
    }

    pub const fn max_total_queued(&self) -> usize {
        self.max_total_queued
    }
}
