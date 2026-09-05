use sophia_backend_live::{
    LiveProductionCpuProgress, LiveProductionCpuTarget, LiveProductionCpuUpdateIdentity,
    LiveProductionNativeFrameId, LiveProductionNativeScanout, LiveProductionScanoutContent,
};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
struct PendingCpuUpdate {
    accepted_at: Instant,
    identity: LiveProductionCpuUpdateIdentity,
}

#[derive(Clone, Copy, Debug)]
struct QueuedCpuUpdate {
    accepted_at: Instant,
    identity: LiveProductionCpuUpdateIdentity,
    target: LiveProductionCpuTarget,
}

/// Bounded evidence that post-readiness CPU content keeps reaching glass.
///
/// CPU updates are latest-wins only until native queueing transfers ownership.
/// Queued updates remain identified by frame until that frame retires or leaves
/// every native owner. A clean terminal proof requires no unbound or queued
/// update after native drain.
#[derive(Debug, Default)]
pub(super) struct CpuVisualProgress {
    ready_at: Option<Instant>,
    accepted_updates: u64,
    presented_updates: u64,
    superseded_updates: u64,
    compositions: u64,
    lifecycle_superseded_updates: u64,
    native_target_bindings: u64,
    primary_retirements: u64,
    changed_primary_retirements: u64,
    pending: Option<PendingCpuUpdate>,
    queued: Vec<QueuedCpuUpdate>,
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

pub(super) fn presented_logical_checksum(
    content: Option<LiveProductionScanoutContent>,
) -> Option<u64> {
    content.and_then(LiveProductionScanoutContent::logical_checksum)
}
const MAX_QUEUED_CPU_UPDATES: usize = 16;

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

    pub(super) fn observe_production(
        &mut self,
        progress: &LiveProductionCpuProgress,
        now: Instant,
    ) -> Result<(), &'static str> {
        let Some(ready_at) = self.ready_at else {
            return Ok(());
        };
        let count = u64::try_from(progress.accepted_updates).unwrap_or(u64::MAX);
        if count != 0 {
            let identity = progress
                .latest_update
                .ok_or("CPU progress accepted updates without a latest identity")?;
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
                identity,
            });
            self.first_update_after_ready.get_or_insert(elapsed);
            if let Some(previous) = self.previous_update_after_ready {
                self.source_max_gap = self.source_max_gap.max(elapsed.saturating_sub(previous));
            }
            self.previous_update_after_ready = Some(elapsed);
            self.last_update_after_ready = Some(elapsed);
        }

        let mut lifecycle_superseded = 0usize;
        if self.pending.is_some_and(|pending| {
            progress
                .removed_surfaces
                .contains(&pending.identity.surface)
        }) {
            self.pending = None;
            lifecycle_superseded = lifecycle_superseded.saturating_add(1);
        }
        if !progress.removed_surfaces.is_empty() {
            let before = self.queued.len();
            self.queued
                .retain(|queued| !progress.removed_surfaces.contains(&queued.identity.surface));
            lifecycle_superseded =
                lifecycle_superseded.saturating_add(before.saturating_sub(self.queued.len()));
        }
        let lifecycle_superseded = u64::try_from(lifecycle_superseded).unwrap_or(u64::MAX);
        self.superseded_updates = self.superseded_updates.saturating_add(lifecycle_superseded);
        self.lifecycle_superseded_updates = self
            .lifecycle_superseded_updates
            .saturating_add(lifecycle_superseded);

        let Some(target) = progress.primary_logical_target else {
            return Ok(());
        };
        self.compositions = self.compositions.saturating_add(1);
        if self.pending.is_none() {
            return Ok(());
        }
        if self.last_presented_checksum == Some(target.logical_checksum)
            || self
                .queued
                .iter()
                .any(|queued| queued.target.frame == target.frame)
        {
            self.pending = None;
            self.superseded_updates = self.superseded_updates.saturating_add(1);
            return Ok(());
        }
        if self.queued.len() >= MAX_QUEUED_CPU_UPDATES {
            return Err("CPU visual target owner capacity exceeded");
        }
        let pending = self
            .pending
            .take()
            .expect("checked pending CPU update exists");
        self.queued.push(QueuedCpuUpdate {
            accepted_at: pending.accepted_at.min(now),
            identity: pending.identity,
            target,
        });
        self.native_target_bindings = self.native_target_bindings.saturating_add(1);
        Ok(())
    }

    pub(super) fn close_native_owner(&mut self) {
        let released = u64::try_from(self.queued.len()).unwrap_or(u64::MAX);
        self.queued.clear();
        self.superseded_updates = self.superseded_updates.saturating_add(released);
        self.lifecycle_superseded_updates =
            self.lifecycle_superseded_updates.saturating_add(released);
        self.last_seen_submissions = 0;
        self.last_presented_checksum = None;
        // A seat pause is not an inter-frame sample.
        self.previous_retirement_after_ready = None;
    }

    pub(super) fn observe_native_scanout(
        &mut self,
        native_scanout: &LiveProductionNativeScanout,
        now: Instant,
    ) {
        let Some(head) = native_scanout.heads.first() else {
            return;
        };
        let output = head.output.id;
        self.observe_primary_state(
            head.presented_submissions,
            head.presented_content,
            head.refresh_millihz,
            now,
            |frame| native_scanout.output_owns_frame(output, frame),
        );
    }

    pub(super) fn observe_primary_state(
        &mut self,
        presented_submissions: usize,
        presented_content: Option<LiveProductionScanoutContent>,
        refresh_millihz: u32,
        now: Instant,
        mut frame_is_owned: impl FnMut(LiveProductionNativeFrameId) -> bool,
    ) {
        let Some(ready_at) = self.ready_at else {
            return;
        };
        self.refresh_millihz = refresh_millihz;
        if presented_submissions > self.last_seen_submissions {
            let retired = presented_submissions.saturating_sub(self.last_seen_submissions);
            self.last_seen_submissions = presented_submissions;
            self.primary_retirements = self
                .primary_retirements
                .saturating_add(u64::try_from(retired).unwrap_or(u64::MAX));

            let presented_checksum = presented_logical_checksum(presented_content);
            let changed =
                presented_checksum.is_some() && presented_checksum != self.last_presented_checksum;
            self.last_presented_checksum = presented_checksum;

            if let Some(content) = presented_content
                && let Some(index) = self.queued.iter().position(|queued| {
                    queued.target.frame == content.frame()
                        && Some(queued.target.logical_checksum) == content.logical_checksum()
                })
            {
                let queued = self.queued.remove(index);
                self.presented_updates = self.presented_updates.saturating_add(1);
                self.max_update_to_retirement = self
                    .max_update_to_retirement
                    .max(now.saturating_duration_since(queued.accepted_at));
            }

            if changed {
                let elapsed = now.saturating_duration_since(ready_at);
                self.changed_primary_retirements =
                    self.changed_primary_retirements.saturating_add(1);
                self.first_retirement_after_ready.get_or_insert(elapsed);
                if let Some(previous) = self.previous_retirement_after_ready {
                    self.display_max_gap =
                        self.display_max_gap.max(elapsed.saturating_sub(previous));
                }
                self.previous_retirement_after_ready = Some(elapsed);
                self.last_retirement_after_ready = Some(elapsed);
            }
        }

        let before = self.queued.len();
        self.queued
            .retain(|queued| frame_is_owned(queued.target.frame));
        self.superseded_updates = self.superseded_updates.saturating_add(
            u64::try_from(before.saturating_sub(self.queued.len())).unwrap_or(u64::MAX),
        );
    }

    pub(super) fn pending_updates(&self) -> usize {
        usize::from(self.pending.is_some()).saturating_add(self.queued.len())
    }

    pub(super) fn is_settled(&self) -> bool {
        self.pending.is_none() && self.queued.is_empty()
    }

    pub(super) fn pending_identity(&self) -> Option<LiveProductionCpuUpdateIdentity> {
        self.pending
            .map(|pending| pending.identity)
            .or_else(|| self.queued.first().map(|queued| queued.identity))
    }

    pub(super) fn pending_target_checksum(&self) -> Option<u64> {
        self.queued
            .first()
            .map(|queued| queued.target.logical_checksum)
    }

    pub(super) fn record(&self, completed_at: Instant, startup_ready_msec: u128) -> String {
        let observed_msec = self.ready_at.map_or(0, |ready| {
            completed_at.saturating_duration_since(ready).as_millis()
        });
        let pending_updates = u64::try_from(self.pending_updates()).unwrap_or(u64::MAX);
        let accounted_updates = self
            .presented_updates
            .saturating_add(self.superseded_updates)
            .saturating_add(pending_updates);
        let last_source_to_completion_msec = self
            .last_update_after_ready
            .map(|last| observed_msec.saturating_sub(last.as_millis()));
        format!(
            "sophia_live_cpu_visual_progress schema=3 status=complete post_startup_updates={} compositions={} native_target_bindings={} primary_retirements={} changed_primary_retirements={} presented_updates={} superseded_updates={} lifecycle_superseded_updates={} pending_updates={} discarded_updates=0 accounted_updates={} startup_ready_msec={} observed_msec={} first_update_after_ready_msec={} last_update_after_ready_msec={} last_source_to_completion_msec={} source_max_gap_msec={} source_max_gap_usec={} first_retirement_after_ready_msec={} last_retirement_after_ready_msec={} display_max_gap_msec={} display_max_gap_usec={} max_update_to_retirement_usec={} refresh_millihz={}",
            self.accepted_updates,
            self.compositions,
            self.native_target_bindings,
            self.primary_retirements,
            self.changed_primary_retirements,
            self.presented_updates,
            self.superseded_updates,
            self.lifecycle_superseded_updates,
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
