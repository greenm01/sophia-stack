use sophia_engine::{
    CURSOR_SHAKE_RESTORE_DELAY_MSEC, CursorShakeAction, CursorShakeDetector, MAX_CURSOR_EDGE,
};

/// Four moves that reverse three times on one axis, 50 ms apart.
fn shake(detector: &mut CursorShakeDetector) -> Option<CursorShakeAction> {
    let mut last = None;
    for (step, x) in [0, 100, 0, 100, 0].into_iter().enumerate() {
        last = detector.observe_motion(true, x, 0, step as u64 * 50);
    }
    last
}

#[test]
fn three_reversals_on_one_axis_enlarge_the_cursor() {
    let mut detector = CursorShakeDetector::default();
    assert_eq!(shake(&mut detector), Some(CursorShakeAction::Enlarge));
    assert!(detector.is_enlarged());
}

#[test]
fn a_cursor_already_enlarged_is_not_enlarged_again() {
    // The caller repaints a buffer on every action, so a second Enlarge would
    // be a second raster of a cursor that already looks like that.
    let mut detector = CursorShakeDetector::default();
    assert_eq!(shake(&mut detector), Some(CursorShakeAction::Enlarge));
    for (step, x) in [0, 100, 0, 100, 0].into_iter().enumerate() {
        let action = detector.observe_motion(true, x, 0, 1_000 + step as u64 * 50);
        assert_ne!(action, Some(CursorShakeAction::Enlarge));
    }
    assert!(detector.is_enlarged());
}

#[test]
fn jitter_below_the_minimum_delta_is_not_a_gesture() {
    // A hand resting on a mouse produces reversals all day. Whatever else the
    // detector does, it must not enlarge the cursor of someone sitting still.
    let mut detector = CursorShakeDetector::default();
    for step in 0..40u64 {
        let x = if step % 2 == 0 { 0 } else { 8 };
        assert_eq!(detector.observe_motion(true, x, 0, step * 40), None);
    }
    assert!(!detector.is_enlarged());
}

#[test]
fn reversals_too_far_apart_never_accumulate() {
    // Slower than a shake is someone using the pointer. The gap check is what
    // separates the two, so a waggle at half a second a stroke stays inert.
    let mut detector = CursorShakeDetector::default();
    for (step, x) in [0, 100, 0, 100, 0, 100, 0].into_iter().enumerate() {
        assert_eq!(detector.observe_motion(true, x, 0, step as u64 * 500), None);
    }
    assert!(!detector.is_enlarged());
}

#[test]
fn reversals_split_across_axes_do_not_add_up() {
    // Three direction changes are only a shake if they are the same shake.
    // Changing axis is a new gesture, not a continuation of the old one.
    let mut detector = CursorShakeDetector::default();
    let moves = [(0, 0), (100, 0), (0, 0), (0, 100), (0, 0), (100, 0)];
    for (step, (x, y)) in moves.into_iter().enumerate() {
        assert_ne!(
            detector.observe_motion(true, x, y, step as u64 * 50),
            Some(CursorShakeAction::Enlarge)
        );
    }
    assert!(!detector.is_enlarged());
}

#[test]
fn the_cursor_restores_once_the_pointer_has_stopped() {
    let mut detector = CursorShakeDetector::default();
    assert_eq!(shake(&mut detector), Some(CursorShakeAction::Enlarge));
    // The shake ended at 200 ms.
    let due = 200 + CURSOR_SHAKE_RESTORE_DELAY_MSEC;
    assert_eq!(detector.tick(true, due - 1), None);
    assert_eq!(detector.tick(true, due), Some(CursorShakeAction::Restore));
    assert!(!detector.is_enlarged());
    // And only once: a repaint per tick would be a repaint per frame.
    assert_eq!(detector.tick(true, due + 5_000), None);
}

#[test]
fn moving_while_enlarged_keeps_the_cursor_large() {
    // The point of the gesture is finding the pointer, which is not finished
    // until it has been moved somewhere. The countdown starts when it stops.
    let mut detector = CursorShakeDetector::default();
    assert_eq!(shake(&mut detector), Some(CursorShakeAction::Enlarge));
    let due = 200 + CURSOR_SHAKE_RESTORE_DELAY_MSEC;
    assert_eq!(detector.observe_motion(true, 500, 0, due - 10), None);
    assert_eq!(detector.tick(true, due), None);
    assert!(detector.is_enlarged());
    assert_eq!(
        detector.tick(true, due - 10 + CURSOR_SHAKE_RESTORE_DELAY_MSEC),
        Some(CursorShakeAction::Restore)
    );
}

#[test]
fn turning_the_feature_off_restores_a_cursor_it_had_enlarged() {
    // Otherwise a profile reload that disabled the gesture would leave the
    // pointer stuck at twice its size until something happened to move.
    let mut detector = CursorShakeDetector::default();
    assert_eq!(shake(&mut detector), Some(CursorShakeAction::Enlarge));
    assert_eq!(
        detector.observe_motion(false, 0, 0, 1_000),
        Some(CursorShakeAction::Restore)
    );
    assert!(!detector.is_enlarged());

    let mut ticked = CursorShakeDetector::default();
    assert_eq!(shake(&mut ticked), Some(CursorShakeAction::Enlarge));
    assert_eq!(ticked.tick(false, 201), Some(CursorShakeAction::Restore));
}

#[test]
fn the_enlarged_size_doubles_small_cursors_and_steps_large_ones() {
    assert_eq!(CursorShakeDetector::enlarged_size(16), 40);
    assert_eq!(CursorShakeDetector::enlarged_size(24), 48);
    // Past this point doubling outruns the ceiling, so the step is what is
    // left doing useful work.
    assert_eq!(CursorShakeDetector::enlarged_size(48), 96);
    assert_eq!(CursorShakeDetector::enlarged_size(64), MAX_CURSOR_EDGE);
    // A base at the ceiling cannot grow, and says so by not changing -- the
    // caller compares and declines rather than rastering the same cursor.
    assert_eq!(
        CursorShakeDetector::enlarged_size(MAX_CURSOR_EDGE),
        MAX_CURSOR_EDGE
    );
}
