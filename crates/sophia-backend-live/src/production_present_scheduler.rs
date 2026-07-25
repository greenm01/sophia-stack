use crate::{
    LiveCpuComposedFrame, LivePresentationResourceSession, LivePresentationSubmission,
    LiveProductionAuthorityBatch,
};
use sophia_engine::PreparedSurfaceCommit;
use sophia_protocol::{Rect, SurfaceId, SurfaceTransaction, TransactionId};
use std::collections::VecDeque;
use std::error::Error;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct LiveProductionQueuedPresent {
    pub submission: LivePresentationSubmission,
    pub surface: sophia_protocol::SurfaceId,
    pub transactions: Vec<SurfaceTransaction>,
    pub cpu_background: Option<LiveCpuComposedFrame>,
    pub target: Rect,
    pub surface_clip: Rect,
    deferred_by_layout: bool,
    x_offset: i16,
    y_offset: i16,
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
    submitted: Option<LiveProductionSubmittedPresent>,
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

    pub fn enqueue_batch(
        &mut self,
        batch: &LiveProductionAuthorityBatch,
        cpu_background: Option<LiveCpuComposedFrame>,
        deferred_by_layout: bool,
        reject_for_layout: bool,
        resources: &mut LivePresentationResourceSession,
        now: Instant,
    ) -> Result<Vec<TransactionId>, Box<dyn Error>> {
        let mut superseded = Vec::new();
        for submission in &batch.present_submissions {
            let surface = submission.surface;
            let x_offset = submission.x_offset;
            let y_offset = submission.y_offset;
            let transaction = batch
                .transactions
                .iter()
                .find(|transaction| transaction.surface == surface)
                .ok_or("Present submission has no matching Engine transaction")?;
            let submission = LivePresentationSubmission {
                transaction: submission.transaction,
                buffer: submission.buffer,
                acquire_fence: submission.acquire_fence,
                idle_fence: submission.idle_fence,
            };
            resources.begin(submission)?;
            if reject_for_layout {
                superseded.push(submission.transaction);
                continue;
            }
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
            self.queued.push_back(LiveProductionQueuedPresent {
                submission,
                surface,
                transactions: batch.transactions.clone(),
                cpu_background: cpu_background.clone(),
                target: Rect {
                    x: transaction
                        .target_geometry
                        .x
                        .saturating_add(i32::from(x_offset)),
                    y: transaction
                        .target_geometry
                        .y
                        .saturating_add(i32::from(y_offset)),
                    ..transaction.target_geometry
                },
                surface_clip: transaction.target_geometry,
                deferred_by_layout,
                x_offset,
                y_offset,
                deadline: not_before
                    + Duration::from_millis(u64::from(transaction.timeout_msec.clamp(100, 2_000))),
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
                x: geometry.x.saturating_add(i32::from(queued.x_offset)),
                y: geometry.y.saturating_add(i32::from(queued.y_offset)),
                ..geometry
            };
            queued.surface_clip = geometry;
            for transaction in &mut queued.transactions {
                if transaction.surface == surface {
                    transaction.target_geometry = geometry;
                }
            }
        }
    }

    pub fn poll_gate(
        &mut self,
        resources: &mut LivePresentationResourceSession,
        now: Instant,
    ) -> Result<LiveProductionPresentGate, Box<dyn Error>> {
        if self.submitted.is_some() {
            return Ok(LiveProductionPresentGate::SubmittedInFlight);
        }
        let Some(queued) = self.queued.front() else {
            return Ok(LiveProductionPresentGate::Idle);
        };
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
        self.submitted = Some(submitted);
    }

    pub fn take_submitted(&mut self) -> Option<LiveProductionSubmittedPresent> {
        self.submitted.take()
    }

    pub fn has_queued(&self) -> bool {
        !self.queued.is_empty()
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
