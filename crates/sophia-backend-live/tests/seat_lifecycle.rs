#![cfg(feature = "seat-control")]

use sophia_backend_live::{LiveSeatEvent, LiveSeatState};

#[test]
fn seat_release_and_acquire_form_one_bounded_cycle() {
    let state = LiveSeatState::Active.observe(LiveSeatEvent::Disable);
    assert_eq!(state, LiveSeatState::ReleasePending);
    let state = state.released();
    assert_eq!(state, LiveSeatState::Suspended);
    let state = state.observe(LiveSeatEvent::Enable);
    assert_eq!(state, LiveSeatState::AcquirePending);
    assert_eq!(state.acquired(), LiveSeatState::Active);
}

#[test]
fn duplicate_pending_events_are_idempotent() {
    assert_eq!(
        LiveSeatState::ReleasePending.observe(LiveSeatEvent::Disable),
        LiveSeatState::ReleasePending
    );
    assert_eq!(
        LiveSeatState::AcquirePending.observe(LiveSeatEvent::Enable),
        LiveSeatState::AcquirePending
    );
}

#[test]
fn out_of_order_seat_events_fail_closed() {
    assert_eq!(
        LiveSeatState::ReleasePending.observe(LiveSeatEvent::Enable),
        LiveSeatState::Failed
    );
    assert_eq!(
        LiveSeatState::AcquirePending.observe(LiveSeatEvent::Disable),
        LiveSeatState::Failed
    );
    assert_eq!(LiveSeatState::Active.released(), LiveSeatState::Failed);
    assert_eq!(LiveSeatState::Suspended.acquired(), LiveSeatState::Failed);
}
