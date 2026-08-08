use crate::commands::live_session::{LiveWmOwnerQueue, report_wm_rejection_diagnostic};

#[test]
fn owner_queue_retains_identical_activations_in_fifo_order() {
    let mut queue = LiveWmOwnerQueue::with_capacity(3);

    assert_eq!(queue.try_push_back(41_u64, false), Ok(()));
    assert_eq!(queue.try_push_back(41_u64, false), Ok(()));
    assert_eq!(queue.try_push_back(42_u64, false), Ok(()));
    assert_eq!(queue.pop_front(), Some(41));
    assert_eq!(queue.pop_front(), Some(41));
    assert_eq!(queue.pop_front(), Some(42));
}

#[test]
fn owner_queue_capacity_includes_the_in_flight_request() {
    let mut queue = LiveWmOwnerQueue::with_capacity(2);

    assert_eq!(queue.try_push_back(7_u64, false), Ok(()));
    assert_eq!(queue.try_push_back(8_u64, true), Err(8));
    assert_eq!(queue.pop_front(), Some(7));
}

#[test]
fn owner_queue_capacity_diagnostics_have_a_fixed_record_bound() {
    assert!(report_wm_rejection_diagnostic(1));
    assert!(report_wm_rejection_diagnostic(16));
    assert!(!report_wm_rejection_diagnostic(17));
    assert!(!report_wm_rejection_diagnostic(usize::MAX));
}
