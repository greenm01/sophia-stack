use sophia_cli::session_keyboard::{
    SESSION_CLIENT_PRESSED_KEY_CAPACITY, SessionClientKeyState, SessionClientPressedKey,
    VirtualTerminalChordAction, VirtualTerminalChordState,
};
use sophia_protocol::{DeviceId, SeatId, SurfaceId};

fn pressed_key(surface: u32, keycode: u32) -> SessionClientPressedKey {
    SessionClientPressedKey {
        surface: SurfaceId::new(surface, 1),
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(1),
        keycode,
    }
}

#[test]
fn control_alt_function_keys_activate_each_linux_vt_once_per_press() {
    let mut chord = VirtualTerminalChordState::default();
    assert_eq!(chord.observe(29, true), VirtualTerminalChordAction::Pass);
    assert_eq!(chord.observe(56, true), VirtualTerminalChordAction::Pass);
    for (keycode, terminal) in (59..=68).zip(1..=10).chain([(87, 11), (88, 12)]) {
        assert_eq!(
            chord.observe(keycode, true),
            VirtualTerminalChordAction::Activate(terminal)
        );
        assert_eq!(
            chord.observe(keycode, true),
            VirtualTerminalChordAction::Consume
        );
        assert_eq!(
            chord.observe(keycode, false),
            VirtualTerminalChordAction::Consume
        );
    }
    assert_eq!(chord.observe(56, false), VirtualTerminalChordAction::Pass);
    assert_eq!(chord.observe(59, true), VirtualTerminalChordAction::Pass);
}

#[test]
fn either_control_and_alt_side_is_accepted_but_plain_function_keys_pass() {
    let mut chord = VirtualTerminalChordState::default();
    assert_eq!(chord.observe(59, true), VirtualTerminalChordAction::Pass);
    assert_eq!(chord.observe(59, false), VirtualTerminalChordAction::Pass);
    assert_eq!(chord.observe(97, true), VirtualTerminalChordAction::Pass);
    assert_eq!(chord.observe(100, true), VirtualTerminalChordAction::Pass);
    assert_eq!(
        chord.observe(88, true),
        VirtualTerminalChordAction::Activate(12)
    );
}

#[test]
fn active_vt_chord_exposes_modifier_keys_that_need_synthetic_release() {
    let mut chord = VirtualTerminalChordState::default();
    let _ = chord.observe(29, true);
    let _ = chord.observe(100, true);
    assert_eq!(
        chord.pressed_modifier_keycodes(),
        [Some(29), None, None, Some(100)]
    );
}

#[test]
fn client_key_state_drains_one_surface_without_touching_another() {
    let mut state = SessionClientKeyState::default();
    let old_super = pressed_key(1, 125);
    let old_letter = pressed_key(1, 30);
    let new_letter = pressed_key(2, 31);
    state.record_routed(old_super, true).unwrap();
    state.record_routed(old_letter, true).unwrap();
    state.record_routed(new_letter, true).unwrap();

    let mut releases = Vec::new();
    state.copy_surface_keys(old_super.surface, &mut releases);
    assert_eq!(releases.len(), 2);
    for release in releases {
        state.record_synthetic_release(release);
    }

    assert_eq!(state.pending_len(), 1);
    assert!(state.release_is_routable(new_letter));
    assert_eq!(state.metrics().synthetic_releases, 2);
}

#[test]
fn client_key_state_suppresses_orphan_release_and_is_bounded() {
    let mut state = SessionClientKeyState::default();
    let orphan = pressed_key(1, 30);
    assert!(!state.release_is_routable(orphan));
    state.record_routed(orphan, false).unwrap();
    assert_eq!(state.metrics().orphan_releases_suppressed, 1);

    for keycode in 1..=SESSION_CLIENT_PRESSED_KEY_CAPACITY {
        state
            .record_routed(pressed_key(1, keycode as u32), true)
            .unwrap();
    }
    assert!(state.record_routed(pressed_key(2, 999), true).is_err());
}

#[test]
fn state_only_release_retires_key_from_a_removed_surface() {
    let mut state = SessionClientKeyState::default();
    let key = pressed_key(4, 28);
    state.record_routed(key, true).unwrap();
    state.record_state_only_release(key);
    assert_eq!(state.pending_len(), 0);
    assert_eq!(state.metrics().state_only_releases, 1);
    assert_eq!(state.metrics().removed_surface_keys, 0);
}
