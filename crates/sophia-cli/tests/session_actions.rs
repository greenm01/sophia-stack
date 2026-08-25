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
    assert_eq!(queue.complete_if_stable(false, Some(surface)), None);
    assert_eq!(
        queue
            .complete_if_stable(true, Some(surface))
            .map(|admission| admission.intent),
        Some(intent(1))
    );
    assert_eq!(queue.begin_next(true, true), Some(intent(2)));
}

#[test]
fn an_observed_application_can_exit_before_the_admission_poll_settles() {
    let mut queue = SessionLaunchQueue::default();
    queue.enqueue(intent(1), 0);
    assert_eq!(queue.begin_next(true, true), Some(intent(1)));
    assert!(queue.complete_observed_exit().is_none());

    assert!(queue.observe_surface(SurfaceId::new(9, 1)));
    assert_eq!(
        queue
            .complete_observed_exit()
            .map(|admission| admission.intent),
        Some(intent(1))
    );
    assert!(queue.admission().is_none());
}

#[test]
fn a_multi_toplevel_launch_settles_on_any_presented_observed_surface() {
    let mut queue = SessionLaunchQueue::default();
    queue.enqueue(intent(1), 0);
    assert_eq!(queue.begin_next(true, true), Some(intent(1)));

    let transient = SurfaceId::new(10, 1);
    let stable = SurfaceId::new(11, 1);
    assert!(queue.observe_surface(transient));
    assert!(queue.observe_surface(stable));
    assert!(!queue.observe_surface(stable));
    assert_eq!(queue.complete_if_stable(true, None), None);
    assert_eq!(
        queue
            .complete_if_stable(true, Some(stable))
            .map(|admission| admission.intent),
        Some(intent(1))
    );
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

#[test]
fn a_withdrawn_surface_releases_the_launch_that_was_waiting_for_it() {
    // When the layout coordinator gives up on a surface, the launch that owned
    // it cannot ever settle. Waiting out the remaining admission budget would
    // hold the queue shut behind it, so every later press queues in silence.
    let mut queue = SessionLaunchQueue::default();
    queue.enqueue(intent(1), 0);
    queue.enqueue(intent(2), 0);
    assert_eq!(queue.begin_next(true, true), Some(intent(1)));

    let surface = SurfaceId::new(7, 1);
    let untouched = SurfaceId::new(8, 1);
    assert!(queue.observe_surface(surface));

    // A withdrawal naming some other surface is not this launch's business.
    assert!(queue.withdraw_current(&[untouched]).is_none());
    assert_eq!(queue.withdrawn(), 0);

    assert_eq!(
        queue
            .withdraw_current(&[untouched, surface])
            .map(|admission| admission.intent),
        Some(intent(1))
    );
    // Counted apart from a timeout: the deadline was never reached.
    assert_eq!(queue.withdrawn(), 1);
    assert_eq!(queue.timed_out(), 0);

    // The queue moves on rather than staying shut.
    assert_eq!(queue.begin_next(true, true), Some(intent(2)));
}

#[test]
fn a_withdrawal_without_an_outstanding_launch_is_not_an_outcome() {
    let mut queue = SessionLaunchQueue::default();
    assert!(queue.withdraw_current(&[SurfaceId::new(7, 1)]).is_none());
    assert_eq!(queue.withdrawn(), 0);
}
