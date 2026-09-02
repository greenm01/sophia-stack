#![cfg(feature = "native-session")]

use sophia_session::{LIVE_RESOURCE_SAMPLE_CAPACITY, LIVE_RESOURCE_SAMPLE_INTERVAL};
use std::time::Duration;

#[test]
fn capacity_covers_two_hours_plus_teardown_margin() {
    assert_eq!(LIVE_RESOURCE_SAMPLE_INTERVAL, Duration::from_secs(5));
    assert_eq!(LIVE_RESOURCE_SAMPLE_CAPACITY, 1_560);
    assert!(
        LIVE_RESOURCE_SAMPLE_INTERVAL.saturating_mul(LIVE_RESOURCE_SAMPLE_CAPACITY as u32)
            >= Duration::from_secs(2 * 60 * 60 + 10 * 60)
    );
}
