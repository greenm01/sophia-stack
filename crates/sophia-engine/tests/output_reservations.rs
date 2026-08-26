use sophia_engine::{OutputWorkArea, reduce_output_work_areas};
use sophia_protocol::{
    AxisSpan, OutputEdge, OutputId, OutputReservation, Rect, SurfaceId, SurfaceOutputReservations,
};

fn reservation(
    surface: u32,
    edge: OutputEdge,
    depth: i32,
    start: i32,
    end: i32,
) -> SurfaceOutputReservations {
    SurfaceOutputReservations {
        surface: SurfaceId::new(surface, 1),
        reservations: vec![OutputReservation {
            edge,
            depth,
            span: AxisSpan { start, end },
        }],
    }
}

fn area(output: u64, full: Rect, work: Option<Rect>) -> OutputWorkArea {
    OutputWorkArea {
        output: OutputId::from_raw(output),
        full,
        work,
    }
}

#[test]
fn top_reservation_reduces_only_intersecting_output_span() {
    let left = Rect {
        x: 0,
        y: 0,
        width: 1280,
        height: 720,
    };
    let right = Rect {
        x: 1280,
        y: 0,
        width: 1280,
        height: 720,
    };

    assert_eq!(
        reduce_output_work_areas(
            Rect {
                x: 0,
                y: 0,
                width: 2560,
                height: 720,
            },
            [
                (OutputId::from_raw(1), left),
                (OutputId::from_raw(2), right)
            ],
            &[reservation(1, OutputEdge::Top, 18, 0, 1280)],
            &[]
        ),
        vec![
            area(
                1,
                left,
                Some(Rect {
                    x: 0,
                    y: 18,
                    width: 1280,
                    height: 702,
                }),
            ),
            area(2, right, Some(right)),
        ]
    );
}

#[test]
fn same_edge_uses_maximum_while_different_edges_combine() {
    let full = Rect {
        x: 0,
        y: 0,
        width: 1280,
        height: 720,
    };
    let reservations = [
        reservation(1, OutputEdge::Top, 18, 0, 1280),
        reservation(2, OutputEdge::Top, 24, 0, 1280),
        reservation(3, OutputEdge::Left, 40, 0, 720),
        reservation(4, OutputEdge::Bottom, 12, 0, 1280),
    ];

    assert_eq!(
        reduce_output_work_areas(full, [(OutputId::from_raw(1), full)], &reservations, &[])[0].work,
        Some(Rect {
            x: 40,
            y: 24,
            width: 1240,
            height: 684,
        })
    );
}

#[test]
fn root_relative_edge_depth_projects_into_offset_outputs() {
    let top = Rect {
        x: 0,
        y: 0,
        width: 1280,
        height: 720,
    };
    let bottom = Rect {
        x: 0,
        y: 720,
        width: 1280,
        height: 720,
    };
    let reduced = reduce_output_work_areas(
        Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 1440,
        },
        [
            (OutputId::from_raw(1), top),
            (OutputId::from_raw(2), bottom),
        ],
        &[reservation(1, OutputEdge::Bottom, 32, 0, 1280)],
        &[],
    );

    assert_eq!(reduced[0].work, Some(top));
    assert_eq!(
        reduced[1].work,
        Some(Rect {
            height: 688,
            ..bottom
        })
    );
}

#[test]
fn opposing_edges_that_consume_output_are_rejected() {
    let full = Rect {
        x: 0,
        y: 0,
        width: 1280,
        height: 720,
    };

    assert_eq!(
        reduce_output_work_areas(
            full,
            [(OutputId::from_raw(1), full)],
            &[
                reservation(1, OutputEdge::Top, 400, 0, 1280),
                reservation(2, OutputEdge::Bottom, 400, 0, 1280),
            ],
            &[]
        )[0]
        .work,
        None
    );
}

#[test]
fn malformed_or_out_of_root_reservations_do_not_change_work_area() {
    let full = Rect {
        x: 0,
        y: 0,
        width: 1280,
        height: 720,
    };

    assert_eq!(
        reduce_output_work_areas(
            full,
            [(OutputId::from_raw(1), full)],
            &[
                reservation(1, OutputEdge::Top, 0, 0, 1280),
                reservation(2, OutputEdge::Top, 18, -1, 1280),
                reservation(3, OutputEdge::Right, 1281, 0, 720),
            ],
            &[]
        )[0]
        .work,
        Some(full)
    );
}

#[test]
fn a_mode_change_reprojects_the_work_area_under_a_live_reservation() {
    // The output-authority row wants reservations settled with topology, not after
    // it. Reduction is pure in the output rect, so a mode change is the same
    // reservation against a different rect -- and the work area must follow the new
    // mode. A stale work area is worse than none: policy would lay out against
    // pixels the mode no longer has.
    let reservations = [reservation(1, OutputEdge::Top, 40, 0, 1920)];

    let before = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1440,
    };
    assert_eq!(
        reduce_output_work_areas(
            before,
            [(OutputId::from_raw(1), before)],
            &reservations,
            &[]
        ),
        vec![area(
            1,
            before,
            Some(Rect {
                x: 0,
                y: 40,
                width: 1920,
                height: 1400,
            })
        )]
    );

    // Same surface, same reservation, shorter mode.
    let after = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    assert_eq!(
        reduce_output_work_areas(after, [(OutputId::from_raw(1), after)], &reservations, &[]),
        vec![area(
            1,
            after,
            Some(Rect {
                x: 0,
                y: 40,
                width: 1920,
                height: 1040,
            })
        )],
        "the work area follows the new mode"
    );
}

#[test]
fn a_shrink_that_invalidates_a_reservation_releases_it_rather_than_preserving_it() {
    // Documented because it is a fail-open edge in an otherwise fail-closed model,
    // and it is reachable by an ordinary mode change rather than by malformed input.
    //
    // Reservations are root-relative, and a shrink can leave one outside the new
    // root -- here a span wider than the root that now contains it. Such a
    // reservation is filtered before reduction, so reduction succeeds and reports
    // the *full* output as available. An out-of-root reservation that arrives
    // malformed should be ignored, which is what that filter is for; one that was
    // valid until the mode changed is a different event with the same shape, and
    // the reducer cannot tell them apart because it holds no previous state.
    //
    // The consequence is bounded but real: between the mode change and the reserving
    // client republishing, policy may lay out under a bar that still occupies
    // pixels. Contrast `reduce_output_work_area` returning `None`, which callers
    // treat as "preserve what you had" -- the fail-closed path this case bypasses.
    let reservations = [reservation(1, OutputEdge::Top, 40, 0, 2560)];
    let after = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };

    assert_eq!(
        reduce_output_work_areas(after, [(OutputId::from_raw(1), after)], &reservations, &[]),
        vec![area(1, after, Some(after))],
        "the reservation is released, not preserved"
    );
}
