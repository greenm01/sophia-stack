use sophia_cli::session_keyboard::{VirtualTerminalChordAction, VirtualTerminalChordState};

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
