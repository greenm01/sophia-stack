#![cfg(test)]
use super::{CpuVisualProgress, presented_logical_checksum};
use sophia_backend_live::{
    LiveProductionCpuProgress, LiveProductionCpuUpdateIdentity, LiveProductionNativeFrameId,
    LiveProductionScanoutContent,
};
use sophia_protocol::{SurfaceId, TransactionId};
use std::time::{Duration, Instant};

fn identity(serial: u64, surface: SurfaceId) -> LiveProductionCpuUpdateIdentity {
    LiveProductionCpuUpdateIdentity {
        transaction: TransactionId::from_raw(serial),
        surface,
        handle: serial.saturating_add(100),
        generation: serial,
    }
}

fn production(
    accepted_updates: usize,
    latest_update: Option<LiveProductionCpuUpdateIdentity>,
    removed_surfaces: Vec<SurfaceId>,
    primary_logical_target_checksum: Option<u64>,
) -> LiveProductionCpuProgress {
    LiveProductionCpuProgress {
        accepted_updates,
        latest_update,
        removed_surfaces,
        primary_logical_target_checksum,
    }
}

#[test]
fn mixed_retirement_cannot_inherit_a_stale_logical_checksum() {
    let frame = LiveProductionNativeFrameId::from_raw(1);
    assert_eq!(
        presented_logical_checksum(Some(LiveProductionScanoutContent::Cpu {
            frame,
            checksum: 41,
        })),
        Some(41),
    );
    assert_eq!(
        presented_logical_checksum(Some(LiveProductionScanoutContent::RetainedMixed {
            frame,
            nonzero_rgb_pixels: 1,
        })),
        None,
    );
    assert_eq!(
        presented_logical_checksum(Some(LiveProductionScanoutContent::MixedPresent {
            frame,
            transaction: TransactionId::from_raw(1),
            nonzero_rgb_pixels: 1,
        })),
        None,
    );
}

#[test]
fn startup_only_activity_cannot_claim_continuous_progress() {
    let started = Instant::now();
    let mut progress = CpuVisualProgress::default();
    let surface = SurfaceId::new(1, 1);

    progress.observe_production(
        &production(12, Some(identity(1, surface)), Vec::new(), Some(41)),
        started + Duration::from_millis(10),
    );
    progress.observe_primary_state(3, Some(41), 60_000, started + Duration::from_millis(30));
    progress.observe_ready(started + Duration::from_millis(40), 3, Some(41), 60_000);

    let record = progress.record(started + Duration::from_secs(2), 40);
    assert!(record.contains("post_startup_updates=0"));
    assert!(record.contains("changed_primary_retirements=0"));
    assert!(record.contains("first_update_after_ready_msec=none"));
}

#[test]
fn latest_wins_updates_are_all_accounted_after_retirement() {
    let ready = Instant::now();
    let mut progress = CpuVisualProgress::default();
    let surface = SurfaceId::new(1, 1);
    progress.observe_ready(ready, 2, Some(100), 60_000);

    progress.observe_production(
        &production(3, Some(identity(1, surface)), Vec::new(), Some(101)),
        ready + Duration::from_millis(10),
    );
    assert_eq!(progress.pending_updates(), 1);
    assert!(!progress.is_settled());
    progress.observe_production(
        &production(2, Some(identity(2, surface)), Vec::new(), Some(102)),
        ready + Duration::from_millis(20),
    );
    progress.observe_primary_state(3, Some(102), 60_000, ready + Duration::from_millis(30));

    assert_eq!(progress.pending_updates(), 0);
    assert!(progress.is_settled());
    let record = progress.record(ready + Duration::from_millis(40), 125);
    assert!(record.contains("post_startup_updates=5"));
    assert!(record.contains("accounted_updates=5"));
    assert!(record.contains("presented_updates=1"));
    assert!(record.contains("superseded_updates=4"));
    assert!(record.contains("pending_updates=0"));
    assert!(record.contains("native_target_bindings=2"));
    assert!(record.contains("max_update_to_retirement_usec=10000"));
}

#[test]
fn unchanged_native_target_settles_an_update_as_superseded() {
    let ready = Instant::now();
    let mut progress = CpuVisualProgress::default();
    let surface = SurfaceId::new(1, 1);
    progress.observe_ready(ready, 1, Some(7), 59_940);
    progress.observe_production(
        &production(1, Some(identity(1, surface)), Vec::new(), Some(7)),
        ready + Duration::from_millis(5),
    );

    let record = progress.record(ready + Duration::from_millis(10), 80);
    assert!(record.contains("post_startup_updates=1"));
    assert!(record.contains("accounted_updates=1"));
    assert!(record.contains("superseded_updates=1"));
    assert!(record.contains("pending_updates=0"));
}

#[test]
fn lifecycle_removal_settles_only_its_pending_surface() {
    let ready = Instant::now();
    let surface = SurfaceId::new(1, 1);
    let unrelated = SurfaceId::new(2, 1);
    let mut progress = CpuVisualProgress::default();
    progress.observe_ready(ready, 0, None, 60_000);
    progress.observe_production(
        &production(1, Some(identity(1, surface)), Vec::new(), None),
        ready + Duration::from_millis(1),
    );
    progress.observe_production(
        &production(0, None, vec![unrelated], None),
        ready + Duration::from_millis(2),
    );
    assert_eq!(progress.pending_identity(), Some(identity(1, surface)));

    progress.observe_production(
        &production(0, None, vec![surface], None),
        ready + Duration::from_millis(3),
    );
    assert!(progress.is_settled());
    let record = progress.record(ready + Duration::from_millis(4), 0);
    assert!(record.contains("presented_updates=0"));
    assert!(record.contains("superseded_updates=1"));
    assert!(record.contains("lifecycle_superseded_updates=1"));
}

#[test]
fn missing_logical_target_never_fabricates_retirement() {
    let ready = Instant::now();
    let surface = SurfaceId::new(1, 1);
    let mut progress = CpuVisualProgress::default();
    progress.observe_ready(ready, 0, None, 60_000);
    progress.observe_production(
        &production(1, Some(identity(1, surface)), Vec::new(), None),
        ready + Duration::from_millis(1),
    );

    assert_eq!(progress.pending_target_checksum(), None);
    assert!(!progress.is_settled());
    let record = progress.record(ready + Duration::from_millis(2), 0);
    assert!(record.contains("native_target_bindings=0"));
    assert!(record.contains("pending_updates=1"));
}

#[test]
fn same_cycle_update_and_surface_removal_is_lifecycle_settled() {
    let ready = Instant::now();
    let surface = SurfaceId::new(1, 1);
    let mut progress = CpuVisualProgress::default();
    progress.observe_ready(ready, 0, None, 60_000);
    progress.observe_production(
        &production(1, Some(identity(1, surface)), vec![surface], Some(99)),
        ready + Duration::from_millis(1),
    );

    assert!(progress.is_settled());
    let record = progress.record(ready + Duration::from_millis(2), 0);
    assert!(record.contains("lifecycle_superseded_updates=1"));
    assert!(record.contains("native_target_bindings=0"));
}

#[test]
fn removal_after_exact_retirement_does_not_double_settle() {
    let ready = Instant::now();
    let surface = SurfaceId::new(1, 1);
    let mut progress = CpuVisualProgress::default();
    progress.observe_ready(ready, 0, None, 60_000);
    progress.observe_production(
        &production(1, Some(identity(1, surface)), Vec::new(), Some(99)),
        ready + Duration::from_millis(1),
    );
    progress.observe_primary_state(1, Some(99), 60_000, ready + Duration::from_millis(2));
    progress.observe_production(
        &production(0, None, vec![surface], None),
        ready + Duration::from_millis(3),
    );

    let record = progress.record(ready + Duration::from_millis(4), 0);
    assert!(record.contains("presented_updates=1"));
    assert!(record.contains("superseded_updates=0"));
    assert!(record.contains("lifecycle_superseded_updates=0"));
}

#[test]
fn cadence_fields_measure_post_ready_source_and_display_gaps() {
    let ready = Instant::now();
    let surface = SurfaceId::new(1, 1);
    let mut progress = CpuVisualProgress::default();
    progress.observe_ready(ready, 9, Some(90), 60_000);

    for (index, elapsed) in [100, 350, 800].into_iter().enumerate() {
        let checksum = 91 + u64::try_from(index).unwrap();
        let now = ready + Duration::from_millis(elapsed);
        progress.observe_production(
            &production(
                1,
                Some(identity(checksum, surface)),
                Vec::new(),
                Some(checksum),
            ),
            now,
        );
        progress.observe_primary_state(10 + index, Some(checksum), 60_000, now);
    }

    let record = progress.record(ready + Duration::from_millis(900), 50);
    assert!(record.contains("schema=3"));
    assert!(record.contains("changed_primary_retirements=3"));
    assert!(record.contains("source_max_gap_msec=450"));
    assert!(record.contains("source_max_gap_usec=450000"));
    assert!(record.contains("display_max_gap_msec=450"));
    assert!(record.contains("display_max_gap_usec=450000"));
    assert!(record.contains("last_source_to_completion_msec=100"));
    assert!(record.contains("pending_updates=0"));
}
