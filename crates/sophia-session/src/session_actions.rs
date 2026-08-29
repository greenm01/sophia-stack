use std::collections::VecDeque;

use sophia_protocol::{SessionApplicationId, SurfaceId, TransactionId};

pub const SESSION_ACTION_APPLICATION_CAPACITY: usize = 16;
pub const SESSION_ACTION_SURFACE_CAPACITY: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionLaunchIntent {
    pub transaction: TransactionId,
    pub application: SessionApplicationId,
    pub placement_classification: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionLaunchSurfaceObservation {
    pub intent: SessionLaunchIntent,
    pub surface: SurfaceId,
    /// Present only for the first surface observed from a classified launch.
    pub placement_classification: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionLaunchAdmission {
    pub intent: SessionLaunchIntent,
    observed_surfaces: [Option<SurfaceId>; SESSION_ACTION_SURFACE_CAPACITY],
    observed_surface_count: usize,
    placement_classification_consumed: bool,
}

impl SessionLaunchAdmission {
    pub fn observed_surfaces(&self) -> impl Iterator<Item = SurfaceId> + '_ {
        self.observed_surfaces.iter().flatten().copied()
    }

    pub const fn has_observed_surface(&self) -> bool {
        self.observed_surface_count != 0
    }
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
    withdrawn: usize,
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
            observed_surfaces: [None; SESSION_ACTION_SURFACE_CAPACITY],
            observed_surface_count: 0,
            placement_classification_consumed: false,
        });
        Some(intent)
    }

    pub fn observe_surface(
        &mut self,
        surface: SurfaceId,
    ) -> Option<SessionLaunchSurfaceObservation> {
        let Some(admission) = self.admission.as_mut() else {
            return None;
        };
        if admission
            .observed_surfaces()
            .any(|candidate| candidate == surface)
            || admission.observed_surface_count >= SESSION_ACTION_SURFACE_CAPACITY
        {
            return None;
        }
        admission.observed_surfaces[admission.observed_surface_count] = Some(surface);
        admission.observed_surface_count += 1;
        let placement_classification = (!admission.placement_classification_consumed)
            .then_some(admission.intent.placement_classification)
            .flatten();
        admission.placement_classification_consumed |= placement_classification.is_some();
        Some(SessionLaunchSurfaceObservation {
            intent: admission.intent,
            surface,
            placement_classification,
        })
    }

    pub fn complete_if_stable(
        &mut self,
        admission_pipeline_idle: bool,
        stable_surface: Option<SurfaceId>,
    ) -> Option<SessionLaunchAdmission> {
        let admission = self.admission?;
        if !admission_pipeline_idle
            || !stable_surface
                .is_some_and(|surface| admission.observed_surfaces().any(|seen| seen == surface))
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
            .is_some_and(|admission| admission.has_observed_surface())
            .then(|| self.admission.take())
            .flatten()
    }

    /// Abandons the outstanding launch when the coordinator gave up on a
    /// surface it was waiting for.
    ///
    /// A withdrawn surface is not a slow one: the admission it would have
    /// settled no longer exists, so waiting out the remaining budget only
    /// holds the queue shut behind a launch that can never complete.
    /// Counted apart from a timeout because it is a different outcome --
    /// the deadline was never reached.
    pub fn withdraw_current(&mut self, surfaces: &[SurfaceId]) -> Option<SessionLaunchAdmission> {
        let admission = self.admission?;
        if !admission
            .observed_surfaces()
            .any(|seen| surfaces.contains(&seen))
        {
            return None;
        }
        self.withdrawn = self.withdrawn.saturating_add(1);
        self.admission.take()
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

    pub fn withdrawn(&self) -> usize {
        self.withdrawn
    }
}
