#![cfg(test)]
use super::CpuVisualProgress;
use std::time::{Duration, Instant};

#[test]
fn startup_only_activity_cannot_claim_continuous_progress() {
    let started = Instant::now();
    let mut progress = CpuVisualProgress::default();

    progress.observe_updates(12, started + Duration::from_millis(10));
    progress.observe_composition(41, started + Duration::from_millis(20));
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
    progress.observe_ready(ready, 2, Some(100), 60_000);

    progress.observe_updates(3, ready + Duration::from_millis(10));
    assert_eq!(progress.pending_updates(), 1);
    assert!(!progress.is_settled());
    progress.observe_composition(101, ready + Duration::from_millis(12));
    progress.observe_updates(2, ready + Duration::from_millis(20));
    progress.observe_composition(102, ready + Duration::from_millis(22));
    progress.observe_primary_state(3, Some(102), 60_000, ready + Duration::from_millis(30));

    assert_eq!(progress.pending_updates(), 0);
    assert!(progress.is_settled());
    let record = progress.record(ready + Duration::from_millis(40), 125);
    assert!(record.contains("post_startup_updates=5"));
    assert!(record.contains("accounted_updates=5"));
    assert!(record.contains("presented_updates=1"));
    assert!(record.contains("superseded_updates=4"));
    assert!(record.contains("pending_updates=0"));
    assert!(record.contains("max_update_to_retirement_usec=10000"));
}

#[test]
fn unchanged_composition_settles_an_update_as_superseded() {
    let ready = Instant::now();
    let mut progress = CpuVisualProgress::default();
    progress.observe_ready(ready, 1, Some(7), 59_940);
    progress.observe_updates(1, ready + Duration::from_millis(5));
    progress.observe_composition(7, ready + Duration::from_millis(6));

    let record = progress.record(ready + Duration::from_millis(10), 80);
    assert!(record.contains("post_startup_updates=1"));
    assert!(record.contains("accounted_updates=1"));
    assert!(record.contains("superseded_updates=1"));
    assert!(record.contains("pending_updates=0"));
}

#[test]
fn cadence_fields_measure_post_ready_source_and_display_gaps() {
    let ready = Instant::now();
    let mut progress = CpuVisualProgress::default();
    progress.observe_ready(ready, 9, Some(90), 60_000);

    for (index, elapsed) in [100, 350, 800].into_iter().enumerate() {
        let checksum = 91 + u64::try_from(index).unwrap();
        let now = ready + Duration::from_millis(elapsed);
        progress.observe_updates(1, now);
        progress.observe_composition(checksum, now);
        progress.observe_primary_state(10 + index, Some(checksum), 60_000, now);
    }

    let record = progress.record(ready + Duration::from_millis(900), 50);
    assert!(record.contains("changed_primary_retirements=3"));
    assert!(record.contains("source_max_gap_msec=450"));
    assert!(record.contains("display_max_gap_msec=450"));
    assert!(record.contains("last_source_to_completion_msec=100"));
    assert!(record.contains("pending_updates=0"));
}
