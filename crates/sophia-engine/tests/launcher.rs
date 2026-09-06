use sophia_engine::*;
use sophia_protocol::*;
#[path = "../../sophia-protocol/tests/support/launcher_fixture.rs"]
mod fixture;
fn key(keycode: u32, pressed: bool) -> InputEventPacket {
    InputEventPacket {
        serial: 1,
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        time_msec: 1,
        kind: InputEventKind::Key { keycode, pressed },
        global_position: None,
        target_surface: None,
        local_position: None,
    }
}
#[test]
fn projection_uses_catalog_labels_and_only_available_visible_targets() {
    let candidate = fixture::candidate();
    let catalog = fixture::catalog(3);
    let projection = launcher_projection(
        &candidate,
        &catalog,
        "app",
        1,
        Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
        |s, f| (s.len() as i32 * 8, i32::from(f)),
    )
    .unwrap();
    assert!(projection.overlay.targets.is_empty());
    assert_eq!(
        projection
            .targets
            .iter()
            .map(|(slot, _)| *slot)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    let mut invalid = candidate;
    invalid.entries.push(4);
    assert!(
        launcher_projection(
            &invalid,
            &catalog,
            "",
            1,
            projection.overlay.geometry,
            |_, _| (10, 14)
        )
        .is_err()
    );
}
#[test]
fn query_edits_disarm_activation_until_new_presentation_and_releases_stay_consumed() {
    let mut capture = LauncherCapture::default();
    let output = OutputId::from_raw(1);
    let targets = [(
        1,
        Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 30,
        },
    )];
    assert!(!capture.route(&key(28, true), None, None, false, false).0);
    capture.present(Some((output, 3)), 1, &targets, false);
    assert_eq!(
        capture
            .route(&key(30, true), Some("a"), None, false, false)
            .1
            .unwrap()
            .input,
        LauncherInput::Text("a".into())
    );
    assert!(
        capture
            .route(&key(28, true), None, None, false, false)
            .1
            .is_none()
    );
    capture.route(&key(28, false), None, None, false, false);
    capture.present(Some((output, 3)), 1, &targets, false);
    assert!(
        capture
            .route(&key(28, true), None, None, false, false)
            .1
            .is_none()
    );
    capture.route(&key(28, false), None, None, false, false);
    capture.present(Some((output, 4)), 1, &targets, false);
    assert_eq!(
        capture
            .route(&key(28, true), None, None, false, false)
            .1
            .unwrap()
            .input,
        LauncherInput::Activate(1)
    );
    capture.present(None, 0, &[], true);
    assert!(capture.route(&key(28, false), None, None, false, false).0);
    assert!(capture.route(&key(30, false), None, None, false, false).0);
}
#[test]
fn click_must_complete_on_same_presented_target() {
    let mut capture = LauncherCapture::default();
    let output = OutputId::from_raw(1);
    let targets = [(
        1,
        Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 30,
        },
    )];
    let mut event = key(0, true);
    event.kind = InputEventKind::PointerButton {
        button: 272,
        pressed: true,
    };
    capture.present(Some((output, 3)), 1, &targets, false);
    capture.route(&event, None, Some(Point { x: 4.5, y: 5.0 }), false, false);
    capture.present(Some((output, 4)), 1, &targets, false);
    event.kind = InputEventKind::PointerButton {
        button: 272,
        pressed: false,
    };
    assert!(
        capture
            .route(&event, None, Some(Point { x: 4.5, y: 5.0 }), false, false)
            .1
            .is_none()
    );
}
#[test]
fn engine_text_is_focus_scoped_and_shift_aware() {
    let mut keyboard = LauncherKeyboard::new(
        "evdev",
        "pc105",
        "us",
        "",
        "",
        std::ffi::OsStr::new("C.UTF-8"),
    )
    .unwrap();
    assert_eq!(keyboard.observe(30, true, false), (None, false));
    keyboard.observe(30, false, false);
    keyboard.observe(42, true, true);
    assert_eq!(keyboard.observe(30, true, true).0, Some("A".into()));
    keyboard.observe(30, false, true);
    keyboard.observe(42, false, true);
    keyboard.observe(29, true, true);
    assert_eq!(keyboard.observe(22, true, true), (None, true));
}
