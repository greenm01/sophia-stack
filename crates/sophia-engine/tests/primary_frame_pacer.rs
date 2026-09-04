use std::time::{Duration, Instant};

use sophia_engine::PrimaryFramePacer;

#[test]
fn busy_content_is_latest_wins_at_the_refresh_cadence() {
    let start = Instant::now();
    let mut pacer = PrimaryFramePacer::new(Duration::from_millis(16));

    assert!(!pacer.defer_production(start, false));
    pacer.observe_production(start, true);
    for offset in [1, 4, 8, 12, 15] {
        let now = start + Duration::from_millis(offset);
        assert!(pacer.defer_production(now, false));
        pacer.observe_production(now, false);
    }
    assert!(!pacer.repaint_due(start + Duration::from_millis(15)));
    assert!(pacer.repaint_due(start + Duration::from_millis(16)));

    pacer.observe_repaint(start + Duration::from_millis(16));
    assert!(!pacer.repaint_pending());
    assert!(!pacer.repaint_due(start + Duration::from_millis(31)));
}

#[test]
fn unrelated_input_wakeups_do_not_move_the_deadline() {
    let start = Instant::now();
    let mut still = PrimaryFramePacer::new(Duration::from_millis(16));
    let mut moving = still;

    assert!(!still.defer_production(start, false));
    still.observe_production(start, true);
    assert!(!moving.defer_production(start, false));
    moving.observe_production(start, true);

    let content = start + Duration::from_millis(4);
    assert!(still.defer_production(content, false));
    still.observe_production(content, false);
    assert!(moving.defer_production(content, false));
    moving.observe_production(content, false);

    // Simulated input wakes only query the wait; they do not request frames.
    for offset in 5..16 {
        assert_eq!(
            moving.cap_wait(
                start + Duration::from_millis(offset),
                Duration::from_millis(25)
            ),
            Duration::from_millis(16 - offset),
        );
    }
    assert_eq!(
        still.repaint_due(start + Duration::from_millis(16)),
        moving.repaint_due(start + Duration::from_millis(16)),
    );
}

#[test]
fn backpressure_also_has_a_bounded_repaint() {
    let start = Instant::now();
    let mut pacer = PrimaryFramePacer::new(Duration::from_millis(16));

    assert!(pacer.defer_production(start, true));
    pacer.observe_production(start, false);
    assert_eq!(
        pacer.cap_wait(start, Duration::from_millis(25)),
        Duration::from_millis(16),
    );
    assert!(pacer.repaint_due(start + Duration::from_millis(16)));
}

#[test]
fn refresh_change_rephases_a_pending_repaint() {
    let start = Instant::now();
    let mut pacer = PrimaryFramePacer::new(Duration::from_millis(16));
    assert!(pacer.defer_production(start, true));

    pacer.set_interval(start + Duration::from_millis(2), Duration::from_millis(8));

    assert_eq!(pacer.interval(), Duration::from_millis(8));
    assert!(!pacer.repaint_due(start + Duration::from_millis(9)));
    assert!(pacer.repaint_due(start + Duration::from_millis(10)));
}
