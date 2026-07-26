use std::collections::VecDeque;

use sophia_protocol::{SessionApplicationId, SurfaceId, TransactionId};

pub const SESSION_ACTION_APPLICATION_CAPACITY: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionLaunchIntent {
    pub transaction: TransactionId,
    pub application: SessionApplicationId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionLaunchAdmission {
    pub intent: SessionLaunchIntent,
    pub observed_surface: Option<SurfaceId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionLaunchQueueOutcome {
    Queued { depth: usize },
    RejectedCapacity,
}

#[derive(Debug, Default)]
pub struct SessionLaunchQueue {
    pending: VecDeque<SessionLaunchIntent>,
    admission: Option<SessionLaunchAdmission>,
    peak_depth: usize,
    rejected: usize,
    timed_out: usize,
}

impl SessionLaunchQueue {
    pub fn enqueue(
        &mut self,
        intent: SessionLaunchIntent,
        active_applications: usize,
    ) -> SessionLaunchQueueOutcome {
        if active_applications.saturating_add(self.pending.len())
            >= SESSION_ACTION_APPLICATION_CAPACITY
        {
            self.rejected = self.rejected.saturating_add(1);
            return SessionLaunchQueueOutcome::RejectedCapacity;
        }
        self.pending.push_back(intent);
        self.peak_depth = self.peak_depth.max(self.pending.len());
        SessionLaunchQueueOutcome::Queued {
            depth: self.pending.len(),
        }
    }

    pub fn begin_next(
        &mut self,
        startup_ready: bool,
        admission_pipeline_idle: bool,
    ) -> Option<SessionLaunchIntent> {
        if !startup_ready || !admission_pipeline_idle || self.admission.is_some() {
            return None;
        }
        let intent = self.pending.pop_front()?;
        self.admission = Some(SessionLaunchAdmission {
            intent,
            observed_surface: None,
        });
        Some(intent)
    }

    pub fn observe_surface(&mut self, surface: SurfaceId) -> bool {
        let Some(admission) = self.admission.as_mut() else {
            return false;
        };
        if admission.observed_surface.is_none() {
            admission.observed_surface = Some(surface);
            return true;
        }
        false
    }

    pub fn complete_if_presented(
        &mut self,
        admission_pipeline_idle: bool,
        presented_surface: Option<SurfaceId>,
    ) -> Option<SessionLaunchAdmission> {
        let admission = self.admission?;
        if !admission_pipeline_idle
            || admission.observed_surface.is_none()
            || admission.observed_surface != presented_surface
        {
            return None;
        }
        self.admission.take()
    }

    pub fn fail_current(&mut self) -> Option<SessionLaunchAdmission> {
        self.admission.take()
    }

    pub fn complete_observed_exit(&mut self) -> Option<SessionLaunchAdmission> {
        self.admission
            .is_some_and(|admission| admission.observed_surface.is_some())
            .then(|| self.admission.take())
            .flatten()
    }

    pub fn timeout_current(&mut self) -> Option<SessionLaunchAdmission> {
        let admission = self.admission.take()?;
        self.timed_out = self.timed_out.saturating_add(1);
        Some(admission)
    }

    pub fn cancel_pending(&mut self) -> usize {
        let cancelled = self.pending.len();
        self.pending.clear();
        cancelled
    }

    pub fn admission(&self) -> Option<SessionLaunchAdmission> {
        self.admission
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn peak_depth(&self) -> usize {
        self.peak_depth
    }

    pub fn rejected(&self) -> usize {
        self.rejected
    }

    pub fn timed_out(&self) -> usize {
        self.timed_out
    }
}
