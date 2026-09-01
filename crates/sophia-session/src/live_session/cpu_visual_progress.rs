use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
struct PendingCpuUpdate {
    accepted_at: Instant,
    target_checksum: Option<u64>,
}

/// Bounded evidence that post-readiness CPU content keeps reaching glass.
///
/// CPU updates are latest-wins. Every accepted update is therefore settled as
/// presented, superseded by a newer update, or still pending. A clean terminal
/// performance proof requires the last category to be empty after native drain.
#[derive(Debug, Default)]
pub(super) struct CpuVisualProgress {
    ready_at: Option<Instant>,
    accepted_updates: u64,
    presented_updates: u64,
    superseded_updates: u64,
    compositions: u64,
    primary_retirements: u64,
    changed_primary_retirements: u64,
    pending: Option<PendingCpuUpdate>,
    last_seen_submissions: usize,
    last_presented_checksum: Option<u64>,
    refresh_millihz: u32,
    first_update_after_ready: Option<Duration>,
    last_update_after_ready: Option<Duration>,
    previous_update_after_ready: Option<Duration>,
    source_max_gap: Duration,
    first_retirement_after_ready: Option<Duration>,
    last_retirement_after_ready: Option<Duration>,
    previous_retirement_after_ready: Option<Duration>,
    display_max_gap: Duration,
    max_update_to_retirement: Duration,
}

impl CpuVisualProgress {
    pub(super) fn observe_ready(
        &mut self,
        now: Instant,
        presented_submissions: usize,
        presented_checksum: Option<u64>,
        refresh_millihz: u32,
    ) {
        if self.ready_at.is_some() {
            return;
        }
        self.ready_at = Some(now);
        self.last_seen_submissions = presented_submissions;
        self.last_presented_checksum = presented_checksum;
        self.refresh_millihz = refresh_millihz;
    }

    pub(super) fn observe_updates(&mut self, count: usize, now: Instant) {
        let Some(ready_at) = self.ready_at else {
            return;
        };
        let Ok(count) = u64::try_from(count) else {
            return;
        };
        if count == 0 {
            return;
        }

        let elapsed = now.saturating_duration_since(ready_at);
        self.accepted_updates = self.accepted_updates.saturating_add(count);
        if self.pending.take().is_some() {
            self.superseded_updates = self.superseded_updates.saturating_add(1);
        }
        self.superseded_updates = self
            .superseded_updates
            .saturating_add(count.saturating_sub(1));
        self.pending = Some(PendingCpuUpdate {
            accepted_at: now,
            target_checksum: None,
        });
        self.first_update_after_ready.get_or_insert(elapsed);
        if let Some(previous) = self.previous_update_after_ready {
            self.source_max_gap = self.source_max_gap.max(elapsed.saturating_sub(previous));
        }
        self.previous_update_after_ready = Some(elapsed);
        self.last_update_after_ready = Some(elapsed);
    }

    pub(super) fn observe_composition(&mut self, checksum: u64, now: Instant) {
        if self.ready_at.is_none() {
            return;
        }
        self.compositions = self.compositions.saturating_add(1);
        let Some(mut pending) = self.pending.take() else {
            return;
        };
        if self.last_presented_checksum == Some(checksum) {
            self.superseded_updates = self.superseded_updates.saturating_add(1);
            return;
        }
        pending.target_checksum = Some(checksum);
        pending.accepted_at = pending.accepted_at.min(now);
        self.pending = Some(pending);
    }

    pub(super) fn observe_primary_state(
        &mut self,
        presented_submissions: usize,
        presented_checksum: Option<u64>,
        refresh_millihz: u32,
        now: Instant,
    ) {
        let Some(ready_at) = self.ready_at else {
            return;
        };
        self.refresh_millihz = refresh_millihz;
        if presented_submissions <= self.last_seen_submissions {
            return;
        }
        let retired = presented_submissions.saturating_sub(self.last_seen_submissions);
        self.last_seen_submissions = presented_submissions;
        self.primary_retirements = self
            .primary_retirements
            .saturating_add(u64::try_from(retired).unwrap_or(u64::MAX));

        let changed =
            presented_checksum.is_some() && presented_checksum != self.last_presented_checksum;
        self.last_presented_checksum = presented_checksum;
        if !changed {
            return;
        }

        let elapsed = now.saturating_duration_since(ready_at);
        self.changed_primary_retirements = self.changed_primary_retirements.saturating_add(1);
        self.first_retirement_after_ready.get_or_insert(elapsed);
        if let Some(previous) = self.previous_retirement_after_ready {
            self.display_max_gap = self.display_max_gap.max(elapsed.saturating_sub(previous));
        }
        self.previous_retirement_after_ready = Some(elapsed);
        self.last_retirement_after_ready = Some(elapsed);

        if self
            .pending
            .is_some_and(|pending| pending.target_checksum == presented_checksum)
        {
            let pending = self.pending.take().expect("matching pending update exists");
            self.presented_updates = self.presented_updates.saturating_add(1);
            self.max_update_to_retirement = self
                .max_update_to_retirement
                .max(now.saturating_duration_since(pending.accepted_at));
        }
    }

    pub(super) fn pending_updates(&self) -> usize {
        usize::from(self.pending.is_some())
    }

    pub(super) fn is_settled(&self) -> bool {
        self.pending.is_none()
    }

    pub(super) fn record(&self, completed_at: Instant, startup_ready_msec: u128) -> String {
        let observed_msec = self.ready_at.map_or(0, |ready| {
            completed_at.saturating_duration_since(ready).as_millis()
        });
        let pending_updates = u64::from(self.pending.is_some());
        let accounted_updates = self
            .presented_updates
            .saturating_add(self.superseded_updates)
            .saturating_add(pending_updates);
        let last_source_to_completion_msec = self
            .last_update_after_ready
            .map(|last| observed_msec.saturating_sub(last.as_millis()));
        format!(
            "sophia_live_cpu_visual_progress schema=2 status=complete post_startup_updates={} compositions={} primary_retirements={} changed_primary_retirements={} presented_updates={} superseded_updates={} pending_updates={} discarded_updates=0 accounted_updates={} startup_ready_msec={} observed_msec={} first_update_after_ready_msec={} last_update_after_ready_msec={} last_source_to_completion_msec={} source_max_gap_msec={} source_max_gap_usec={} first_retirement_after_ready_msec={} last_retirement_after_ready_msec={} display_max_gap_msec={} display_max_gap_usec={} max_update_to_retirement_usec={} refresh_millihz={}",
            self.accepted_updates,
            self.compositions,
            self.primary_retirements,
            self.changed_primary_retirements,
            self.presented_updates,
            self.superseded_updates,
            pending_updates,
            accounted_updates,
            startup_ready_msec,
            observed_msec,
            optional_millis(self.first_update_after_ready),
            optional_millis(self.last_update_after_ready),
            optional_u128(last_source_to_completion_msec),
            self.source_max_gap.as_millis(),
            self.source_max_gap.as_micros(),
            optional_millis(self.first_retirement_after_ready),
            optional_millis(self.last_retirement_after_ready),
            self.display_max_gap.as_millis(),
            self.display_max_gap.as_micros(),
            self.max_update_to_retirement.as_micros(),
            self.refresh_millihz,
        )
    }
}

fn optional_millis(value: Option<Duration>) -> String {
    value.map_or_else(|| "none".to_owned(), |value| value.as_millis().to_string())
}

fn optional_u128(value: Option<u128>) -> String {
    value.map_or_else(|| "none".to_owned(), |value| value.to_string())
}

#[path = "../../tests/support/cpu_visual_progress.rs"]
mod tests;
