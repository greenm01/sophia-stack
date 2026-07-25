use sophia_cli::session_actions::{
    SESSION_ACTION_APPLICATION_CAPACITY, SessionLaunchIntent, SessionLaunchQueue,
    SessionLaunchQueueOutcome,
};
use sophia_protocol::{SessionApplicationId, SurfaceId, TransactionId};

fn intent(raw: u64) -> SessionLaunchIntent {
    SessionLaunchIntent {
        transaction: TransactionId::from_raw(raw),
        application: SessionApplicationId::from_raw(1),
    }
}

#[test]
fn launch_burst_is_bounded_without_becoming_an_error() {
    let mut queue = SessionLaunchQueue::default();
    for raw in 1..=32 {
        let outcome = queue.enqueue(intent(raw), 0);
        if raw <= SESSION_ACTION_APPLICATION_CAPACITY as u64 {
            assert!(matches!(outcome, SessionLaunchQueueOutcome::Queued { .. }));
        } else {
            assert_eq!(outcome, SessionLaunchQueueOutcome::RejectedCapacity);
        }
    }

    assert_eq!(queue.pending_len(), SESSION_ACTION_APPLICATION_CAPACITY);
    assert_eq!(queue.rejected(), 16);
}

#[test]
fn launches_advance_only_after_the_observed_surface_is_stable() {
    let mut queue = SessionLaunchQueue::default();
    queue.enqueue(intent(1), 0);
    queue.enqueue(intent(2), 0);

    assert_eq!(queue.begin_next(false, true), None);
    assert_eq!(queue.begin_next(true, true), Some(intent(1)));
    assert_eq!(queue.begin_next(true, true), None);

    let surface = SurfaceId::new(7, 1);
    assert!(queue.observe_surface(surface));
    assert_eq!(queue.complete_if_presented(false, Some(surface)), None);
    assert_eq!(
        queue
            .complete_if_presented(true, Some(surface))
            .map(|admission| admission.intent),
        Some(intent(1))
    );
    assert_eq!(queue.begin_next(true, true), Some(intent(2)));
}

#[test]
fn timeout_and_logout_release_bounded_work() {
    let mut queue = SessionLaunchQueue::default();
    queue.enqueue(intent(1), 0);
    queue.enqueue(intent(2), 0);
    queue.enqueue(intent(3), 0);
    queue.begin_next(true, true);

    assert_eq!(
        queue.timeout_current().map(|admission| admission.intent),
        Some(intent(1))
    );
    assert_eq!(queue.timed_out(), 1);
    assert_eq!(queue.cancel_pending(), 2);
    assert_eq!(queue.pending_len(), 0);
}
