use super::*;
use std::collections::BTreeMap;

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

#[test]
fn startup_submission_requirements_follow_head_identity_across_reordering() {
    let first = sophia_engine::RenderHeadId::from_raw(11);
    let second = sophia_engine::RenderHeadId::from_raw(22);
    let required = BTreeMap::from([(first, 4), (second, 9)]);

    assert_eq!(
        startup_required_submission_for_head(Some(&required), second),
        Some(9),
    );
    assert_eq!(
        startup_required_submission_for_head(Some(&required), first),
        Some(4),
    );
    assert_eq!(
        startup_required_submission_for_head(
            Some(&required),
            sophia_engine::RenderHeadId::from_raw(33),
        ),
        None,
    );
}
