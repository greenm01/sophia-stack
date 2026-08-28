use sophia_cli::input_latency_samples::{
    INPUT_LATENCY_SAMPLE_CAPACITY, InputLatencySamples, PendingInputLatencySample, percentile_usec,
};

fn press(
    serial: u64,
    ingress_ust_usec: u64,
    baseline_submission: usize,
) -> PendingInputLatencySample {
    PendingInputLatencySample {
        serial,
        ingress_ust_usec,
        baseline_submission,
        queue_dwell_usec: 500,
    }
}

#[test]
fn a_session_without_a_flip_reports_no_summary() {
    let mut samples = InputLatencySamples::new();
    samples.observe_press(press(1, 1_000, 10));

    assert!(
        samples.summary().is_none(),
        "a press that never reached a flip is not a measurement"
    );
}

#[test]
fn one_flip_settles_every_press_it_showed() {
    // Three presses inside one frame reach the screen together, and each
    // waited a different length for the same photon.
    let mut samples = InputLatencySamples::new();
    samples.observe_press(press(1, 1_000, 10));
    samples.observe_press(press(2, 3_000, 10));
    samples.observe_press(press(3, 5_000, 10));

    samples.observe_page_flip(11, 6_000, 9_000);

    let summary = samples.summary().expect("three settled samples");
    assert_eq!(summary.samples, 3);
    assert_eq!(summary.pending, 0);
    assert_eq!(summary.min_usec, 4_000);
    assert_eq!(summary.max_usec, 8_000);
    assert_eq!(summary.max_submit_to_page_flip_usec, 3_000);
}

#[test]
fn a_flip_that_predates_a_press_settles_nothing() {
    let mut samples = InputLatencySamples::new();
    samples.observe_press(press(1, 5_000, 20));

    // The flip's submission did not advance past the press's baseline.
    samples.observe_page_flip(20, 6_000, 9_000);

    assert!(samples.summary().is_none());
}

#[test]
fn clocks_that_disagree_about_order_produce_no_sample() {
    let mut samples = InputLatencySamples::new();
    samples.observe_press(press(1, 9_000, 10));

    // Submission timestamped before the press it supposedly carries.
    samples.observe_page_flip(11, 5_000, 12_000);

    assert!(
        samples.summary().is_none(),
        "a negative interval is a clock disagreement, not a fast frame"
    );
}

#[test]
fn presses_that_outran_presentation_are_counted_not_lost() {
    let mut samples = InputLatencySamples::new();
    for serial in 0..(INPUT_LATENCY_SAMPLE_CAPACITY as u64 + 5) {
        samples.observe_press(press(serial, 1_000 + serial, 10));
    }
    samples.observe_page_flip(11, 100_000, 200_000);

    let summary = samples.summary().expect("settled samples");
    assert_eq!(
        summary.abandoned, 5,
        "the oldest unshown presses are counted"
    );
    assert_eq!(summary.samples, INPUT_LATENCY_SAMPLE_CAPACITY);
}

#[test]
fn the_retained_window_reports_what_it_dropped() {
    let mut samples = InputLatencySamples::new();
    // Settle more samples than the ring retains, one flip at a time.
    for index in 0..(INPUT_LATENCY_SAMPLE_CAPACITY as u64 + 10) {
        let baseline = index as usize;
        samples.observe_press(press(index, index * 10, baseline));
        samples.observe_page_flip(baseline + 1, index * 10 + 1, index * 10 + 2_000);
    }

    let summary = samples.summary().expect("settled samples");
    assert_eq!(summary.samples, INPUT_LATENCY_SAMPLE_CAPACITY);
    assert_eq!(summary.evicted, 10);
}

#[test]
fn percentiles_use_nearest_rank_over_the_settled_population() {
    let ascending: Vec<u64> = (1..=100).collect();

    assert_eq!(percentile_usec(&ascending, 50), 50);
    assert_eq!(percentile_usec(&ascending, 95), 95);
    assert_eq!(percentile_usec(&ascending, 99), 99);
    // The rank never runs off either end.
    assert_eq!(percentile_usec(&ascending, 100), 100);
    assert_eq!(percentile_usec(&[7], 99), 7);
    assert_eq!(percentile_usec(&[], 99), 0);
}

#[test]
fn a_summary_orders_its_percentiles() {
    let mut samples = InputLatencySamples::new();
    // A long tail: most frames fast, a few slow.
    for index in 0..100_u64 {
        let baseline = index as usize;
        let latency = if index >= 97 { 40_000 } else { 4_000 };
        samples.observe_press(press(index, index * 100_000, baseline));
        samples.observe_page_flip(baseline + 1, index * 100_000 + 1, index * 100_000 + latency);
    }

    let summary = samples.summary().expect("settled samples");
    assert_eq!(summary.samples, 100);
    assert!(summary.p50_usec <= summary.p95_usec);
    assert!(summary.p95_usec <= summary.p99_usec);
    assert!(summary.p99_usec <= summary.max_usec);
    // The tail is what a p99 exists to see: a p95 would miss these three.
    assert_eq!(summary.p95_usec, 4_000);
    assert_eq!(summary.p99_usec, 40_000);
}
