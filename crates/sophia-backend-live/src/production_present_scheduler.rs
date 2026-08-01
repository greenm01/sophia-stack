use crate::{
    LivePresentationResourceSession, LivePresentationSubmission, LiveProductionAuthorityGroup,
    LiveProductionPresentDisposition,
};
use sophia_engine::PreparedSurfaceCommit;
use sophia_protocol::{
    BufferSource, LayerSnapshot, Rect, SurfaceId, SurfaceTransaction, TransactionId,
};
use sophia_renderer_live::LiveCpuPresentationLayer;
use std::collections::VecDeque;
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct LiveProductionQueuedPresent {
    pub submission: LivePresentationSubmission,
    pub surface: sophia_protocol::SurfaceId,
    pub candidate: SurfaceTransaction,
    pub cpu_layers: Arc<[LiveCpuPresentationLayer]>,
    pub target: Rect,
    pub surface_clip: Rect,
    deferred_by_layout: bool,
    x_offset: i32,
    y_offset: i32,
    deadline: Instant,
    not_before: Instant,
}

#[derive(Debug)]
pub struct LiveProductionSubmittedPresent {
    pub transaction: TransactionId,
    pub surface: sophia_protocol::SurfaceId,
    pub prepared: PreparedSurfaceCommit,
    pub displayed_layer: crate::LiveRetainedDmaBufLayer,
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

#[derive(Debug, Default)]
pub struct LiveProductionPresentScheduler {
    queued: VecDeque<LiveProductionQueuedPresent>,
    in_flight: Option<LiveProductionInFlightPresent>,
    first_acquire_delay: Option<Duration>,
    first_acquire_delay_applied: bool,
    reject_first_present: bool,
    acquire_waits: usize,
    controlled_rejections: usize,
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
            let deferred_by_layout = matches!(
                submission.layout_disposition,
                LiveProductionPresentDisposition::StageLayout { .. }
            );
            let reject_for_layout = matches!(
                submission.layout_disposition,
                LiveProductionPresentDisposition::RejectLayoutMismatch
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
                    && transaction.target_buffer
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
            if deferred_by_layout {
                let mut retained = VecDeque::with_capacity(self.queued.len());
                while let Some(queued) = self.queued.pop_front() {
                    if queued.deferred_by_layout && queued.surface == surface {
                        superseded.push(queued.submission.transaction);
                    } else {
                        retained.push_back(queued);
                    }
                }
                self.queued = retained;
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
                deferred_by_layout,
                x_offset,
                y_offset,
                deadline: not_before + Duration::from_millis(u64::from(timeout_msec)),
                not_before,
            });
        }
        Ok(superseded)
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
            .position(|queued| !queued.deferred_by_layout)
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
        self.queued.iter().any(|queued| !queued.deferred_by_layout)
    }

    pub fn has_layout_deferred(&self) -> bool {
        self.queued.iter().any(|queued| queued.deferred_by_layout)
    }

    pub fn release_layout_deferred(&mut self) {
        for queued in &mut self.queued {
            queued.deferred_by_layout = false;
        }
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

    pub fn drain_layout_deferred_transactions(&mut self) -> Vec<TransactionId> {
        let mut retained = VecDeque::with_capacity(self.queued.len());
        let mut drained = Vec::new();
        while let Some(queued) = self.queued.pop_front() {
            if queued.deferred_by_layout {
                drained.push(queued.submission.transaction);
            } else {
                retained.push_back(queued);
            }
        }
        self.queued = retained;
        drained
    }

    pub const fn acquire_waits(&self) -> usize {
        self.acquire_waits
    }

    pub const fn controlled_rejections(&self) -> usize {
        self.controlled_rejections
    }
}
