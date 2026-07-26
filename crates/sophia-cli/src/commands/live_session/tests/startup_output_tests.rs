use super::*;

#[test]
fn startup_frame_barrier_is_scoped_to_intersecting_outputs() {
    let focused = Rect {
        x: 100,
        y: 50,
        width: 640,
        height: 480,
    };
    let primary = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let secondary = Rect {
        x: 1920,
        y: 0,
        width: 1920,
        height: 1080,
    };

    assert!(rects_intersect(focused, primary));
    assert!(!rects_intersect(focused, secondary));
    assert_eq!(startup_submission_requirement(4, 3, true), 4);
    assert_eq!(startup_submission_requirement(1, 1, true), 2);
    assert_eq!(startup_submission_requirement(1, 0, false), 0);
}
