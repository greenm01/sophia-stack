use sophia_engine::{
    CHROME_PRIMARY_BUTTON, ChromeCaptureState, ChromePointerDisposition, IndicatorChromeHitTarget,
    resolve_chrome_pointer_event,
};
use sophia_protocol::{DeviceId, InputEventKind, OutputId, Point, Rect, SeatId, WmActionId};

const PRESENTATION_EPOCH: u64 = 12;

fn target(action: Option<WmActionId>) -> IndicatorChromeHitTarget {
    IndicatorChromeHitTarget {
        publication_generation: 3,
        connection_epoch: 4,
        output: OutputId::from_raw(1),
        indicator: 6,
        action,
        geometry: Rect {
            x: 10,
            y: 0,
            width: 40,
            height: 14,
        },
    }
}

fn button(pressed: bool) -> InputEventKind {
    InputEventKind::PointerButton {
        button: CHROME_PRIMARY_BUTTON,
        pressed,
    }
}

fn route(
    state: &mut ChromeCaptureState,
    device: DeviceId,
    kind: InputEventKind,
    epoch: u64,
    targets: &[IndicatorChromeHitTarget],
    application_owned: bool,
) -> ChromePointerDisposition {
    resolve_chrome_pointer_event(
        state,
        SeatId::from_raw(1),
        device,
        kind,
        Some(Point { x: 20.0, y: 7.0 }),
        Some(OutputId::from_raw(1)),
        epoch,
        targets,
        Some(Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 14,
        }),
        application_owned,
    )
    .unwrap()
}

#[test]
fn primary_press_captures_and_matching_release_activates() {
    let mut state = ChromeCaptureState::default();
    let device = DeviceId::from_raw(2);
    let action = WmActionId::from_raw(7);
    let targets = [target(Some(action))];

    assert_eq!(
        route(
            &mut state,
            device,
            button(true),
            PRESENTATION_EPOCH,
            &targets,
            false,
        ),
        ChromePointerDisposition::Captured
    );
    assert_eq!(
        route(
            &mut state,
            device,
            button(false),
            PRESENTATION_EPOCH,
            &targets,
            false,
        ),
        ChromePointerDisposition::Activated {
            output: OutputId::from_raw(1),
            action,
        }
    );
    assert!(state.capture(SeatId::from_raw(1)).is_none());
}

#[test]
fn strip_occlusion_and_actionless_cells_consume_without_capturing() {
    let mut state = ChromeCaptureState::default();
    let device = DeviceId::from_raw(2);
    let actionless = [target(None)];

    assert_eq!(
        route(
            &mut state,
            device,
            button(true),
            PRESENTATION_EPOCH,
            &actionless,
            false,
        ),
        ChromePointerDisposition::Consumed
    );
    assert_eq!(
        resolve_chrome_pointer_event(
            &mut state,
            SeatId::from_raw(1),
            device,
            InputEventKind::PointerMotion,
            Some(Point { x: 80.0, y: 7.0 }),
            Some(OutputId::from_raw(1)),
            PRESENTATION_EPOCH,
            &[],
            Some(Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 14,
            }),
            false,
        )
        .unwrap(),
        ChromePointerDisposition::Consumed
    );
    assert!(state.capture(SeatId::from_raw(1)).is_none());
}

#[test]
fn a_capture_ignores_the_wrong_device_or_button_and_waits_for_its_release() {
    let mut state = ChromeCaptureState::default();
    let device = DeviceId::from_raw(2);
    let action = WmActionId::from_raw(7);
    let targets = [target(Some(action))];
    assert_eq!(
        route(
            &mut state,
            device,
            button(true),
            PRESENTATION_EPOCH,
            &targets,
            false,
        ),
        ChromePointerDisposition::Captured
    );

    assert_eq!(
        route(
            &mut state,
            DeviceId::from_raw(3),
            button(false),
            PRESENTATION_EPOCH,
            &targets,
            false,
        ),
        ChromePointerDisposition::Consumed
    );
    assert!(state.capture(SeatId::from_raw(1)).is_some());
    assert_eq!(
        route(
            &mut state,
            device,
            InputEventKind::PointerButton {
                button: CHROME_PRIMARY_BUTTON + 1,
                pressed: false,
            },
            PRESENTATION_EPOCH,
            &targets,
            false,
        ),
        ChromePointerDisposition::Consumed
    );
    assert!(state.capture(SeatId::from_raw(1)).is_some());
    assert_eq!(
        route(
            &mut state,
            device,
            button(false),
            PRESENTATION_EPOCH,
            &targets,
            false,
        ),
        ChromePointerDisposition::Activated {
            output: OutputId::from_raw(1),
            action,
        }
    );
}

/// A policy commit that leaves the indicators alone leaves a click alive.
///
/// The hit target used to carry the projection commit serial, which advanced on
/// every policy commit, so any unrelated layout change under the pointer
/// cancelled a press already in flight. The target now identifies the
/// indicators it was measured against and nothing else.
#[test]
fn an_unrelated_policy_commit_does_not_cancel_a_capture() {
    let mut state = ChromeCaptureState::default();
    let device = DeviceId::from_raw(2);
    let action = WmActionId::from_raw(7);
    let targets = [target(Some(action))];

    assert_eq!(
        route(
            &mut state,
            device,
            button(true),
            PRESENTATION_EPOCH,
            &targets,
            false,
        ),
        ChromePointerDisposition::Captured
    );

    // The same indicators, republished by a later commit.
    assert_eq!(
        route(
            &mut state,
            device,
            button(false),
            PRESENTATION_EPOCH,
            &[target(Some(action))],
            false,
        ),
        ChromePointerDisposition::Activated {
            output: OutputId::from_raw(1),
            action,
        }
    );
}

#[test]
fn presentation_or_target_change_cancels_a_capture() {
    let mut state = ChromeCaptureState::default();
    let device = DeviceId::from_raw(2);
    let targets = [target(Some(WmActionId::from_raw(7)))];
    assert_eq!(
        route(
            &mut state,
            device,
            button(true),
            PRESENTATION_EPOCH,
            &targets,
            false,
        ),
        ChromePointerDisposition::Captured
    );
    assert_eq!(
        route(
            &mut state,
            device,
            button(false),
            PRESENTATION_EPOCH + 1,
            &targets,
            false,
        ),
        ChromePointerDisposition::Cancelled
    );

    assert_eq!(
        route(
            &mut state,
            device,
            button(true),
            PRESENTATION_EPOCH,
            &targets,
            false,
        ),
        ChromePointerDisposition::Captured
    );
    let mut changed = targets[0].clone();
    changed.publication_generation += 1;
    assert_eq!(
        route(
            &mut state,
            device,
            button(false),
            PRESENTATION_EPOCH,
            &[changed],
            false,
        ),
        ChromePointerDisposition::Cancelled
    );
}

#[test]
fn an_application_owned_sequence_bypasses_fresh_chrome_selection() {
    let mut state = ChromeCaptureState::default();
    assert_eq!(
        route(
            &mut state,
            DeviceId::from_raw(2),
            button(true),
            PRESENTATION_EPOCH,
            &[target(Some(WmActionId::from_raw(7)))],
            true,
        ),
        ChromePointerDisposition::Pass
    );
    assert!(state.capture(SeatId::from_raw(1)).is_none());
}
