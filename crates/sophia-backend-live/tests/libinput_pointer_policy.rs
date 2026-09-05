//! A pointer preference a device cannot hold must not fail the seat.
//!
//! One composite HID with no acceleration profile used to end every login:
//! `apply_native_pointer_policy` returned false for any error, and a single
//! failure became `DeviceConfigurationFailed`. libinput answers `Unsupported`
//! for a knob a device does not have and `Invalid` for a value it refuses, and
//! only the second is a configuration failure.

#![cfg(feature = "libinput-events")]

use input::DeviceConfigError;
use sophia_backend_live::NativePointerPolicyOutcome;

#[test]
fn a_device_without_a_knob_is_skipped_not_refused() {
    let mut outcome = NativePointerPolicyOutcome::default();
    outcome.record("accel-profile", Err(DeviceConfigError::Unsupported));
    assert_eq!(outcome.unsupported, 1);
    assert_eq!(outcome.applied, 0);
    assert!(outcome.accepted(), "a missing knob must not fail the seat");
}

#[test]
fn a_device_refusing_a_value_is_fatal_and_names_the_setting() {
    let mut outcome = NativePointerPolicyOutcome::default();
    outcome.record("accel-speed", Err(DeviceConfigError::Invalid));
    assert!(!outcome.accepted());
    assert_eq!(
        outcome.refused,
        Some("accel-speed"),
        "a session that will not start has to say what it would not accept"
    );
}

#[test]
fn the_first_refusal_is_the_one_reported() {
    let mut outcome = NativePointerPolicyOutcome::default();
    outcome.record("accel-profile", Err(DeviceConfigError::Invalid));
    outcome.record("left-handed", Err(DeviceConfigError::Invalid));
    assert_eq!(outcome.refused, Some("accel-profile"));
}

#[test]
fn applied_and_skipped_settings_are_counted_apart() {
    let mut outcome = NativePointerPolicyOutcome::default();
    outcome.record("natural-scroll", Ok(()));
    outcome.record("accel-profile", Err(DeviceConfigError::Unsupported));
    outcome.record("left-handed", Ok(()));
    assert_eq!(outcome.applied, 2);
    assert_eq!(outcome.unsupported, 1);
    assert!(outcome.accepted());
}

#[test]
fn an_untouched_policy_settles_as_accepted() {
    let outcome = NativePointerPolicyOutcome::default();
    assert!(outcome.accepted());
    assert_eq!(outcome.applied, 0);
    assert_eq!(outcome.unsupported, 0);
}
