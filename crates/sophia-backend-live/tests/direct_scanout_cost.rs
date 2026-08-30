//! What the cost aggregation promises about the numbers it reports.

use std::time::Duration;

use sophia_backend_live::DirectScanoutCost;

fn micros(values: &[u64]) -> Vec<Duration> {
    values.iter().copied().map(Duration::from_micros).collect()
}

/// The two populations are kept apart. Mixing them would answer the question
/// this instrumentation exists for -- does a direct frame cost less than a
/// composed one -- with a single number that cannot distinguish them.
#[test]
fn direct_and_composed_samples_never_mix() {
    let mut cost = DirectScanoutCost::default();
    for elapsed in micros(&[100, 200, 300]) {
        cost.record_offer_to_submit(true, elapsed);
    }
    for elapsed in micros(&[4_000, 5_000]) {
        cost.record_offer_to_submit(false, elapsed);
    }

    let direct = cost
        .direct
        .offer_to_submit
        .summary()
        .expect("direct frames");
    let composed = cost
        .composed
        .offer_to_submit
        .summary()
        .expect("composed frames");
    assert_eq!(direct.frames, 3);
    assert_eq!(composed.frames, 2);
    assert_eq!(direct.max, 300);
    assert_eq!(composed.min, 4_000);
}

/// An absent population is absent, not zero.
///
/// A session that never opened an overlay has no composed frames to compare
/// against. Reporting zeros for it would read as "composition is free",
/// which is the opposite of what the evidence says.
#[test]
fn a_population_that_never_happened_has_no_summary() {
    let mut cost = DirectScanoutCost::default();
    cost.record_offer_to_submit(true, Duration::from_micros(120));

    assert!(cost.direct.offer_to_submit.summary().is_some());
    assert!(
        cost.composed.offer_to_submit.summary().is_none(),
        "a population with no frames must not report a distribution"
    );
    assert!(
        cost.direct.submit_to_flip.summary().is_none(),
        "and neither must a measurement that was never taken"
    );
}

/// Percentiles are measurements, by nearest rank, not interpolations between
/// two samples that never happened.
#[test]
fn percentiles_report_samples_that_were_actually_measured() {
    let mut cost = DirectScanoutCost::default();
    for elapsed in micros(&[10, 20, 30, 40, 50, 60, 70, 80, 90, 1_000]) {
        cost.record_submit_to_flip(true, elapsed);
    }
    let summary = cost.direct.submit_to_flip.summary().expect("samples");
    assert_eq!(summary.frames, 10);
    assert_eq!(summary.min, 10);
    assert_eq!(summary.p50, 50);
    assert_eq!(summary.p99, 1_000, "the tail is a real sample");
    assert_eq!(summary.max, 1_000);
    assert!(!summary.saturated);
}

/// Storage is bounded, and says so when it stops accepting.
///
/// Milestone 14 asks for no steady-state allocation growth. Instrumentation
/// that grew without limit would answer a question about efficiency by
/// becoming the leak -- and a summary silently describing a prefix as though
/// it were the run would be worse than no summary.
#[test]
fn a_full_reservoir_reports_that_it_is_full() {
    let mut cost = DirectScanoutCost::default();
    for index in 0..5_000u64 {
        cost.record_offer_to_submit(true, Duration::from_micros(index));
    }
    let summary = cost.direct.offer_to_submit.summary().expect("samples");
    assert_eq!(summary.frames, 4_096, "the reservoir is capped");
    assert!(summary.saturated, "and the cap is reported, not hidden");
}

/// Merging exporters into one session summary preserves the split and the
/// saturation flag, since the offer half lives per exporter and the flip
/// half lives per head.
#[test]
fn merging_preserves_populations_and_saturation() {
    let mut first = DirectScanoutCost::default();
    first.record_offer_to_submit(true, Duration::from_micros(10));
    let mut second = DirectScanoutCost::default();
    second.record_offer_to_submit(true, Duration::from_micros(20));
    second.record_offer_to_submit(false, Duration::from_micros(900));

    first.merge(&second);
    assert_eq!(first.direct.offer_to_submit.summary().unwrap().frames, 2);
    assert_eq!(first.composed.offer_to_submit.summary().unwrap().frames, 1);

    let mut full = DirectScanoutCost::default();
    for index in 0..5_000u64 {
        full.record_offer_to_submit(true, Duration::from_micros(index));
    }
    let mut host = DirectScanoutCost::default();
    host.merge(&full);
    assert!(
        host.direct.offer_to_submit.summary().unwrap().saturated,
        "saturation survives the merge that reports it"
    );
}
