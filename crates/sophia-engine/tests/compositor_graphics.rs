use sophia_engine::{
    CompositorDisplayCommand, CompositorDisplayListError, CompositorNodeId,
    FocusedSurfaceBorderEdge, FocusedSurfaceBorderStyle, compositor_display_list_damage,
    focused_surface_display_list,
};
use sophia_protocol::{BufferSource, CommittedSurfaceState, OutputId, Rect, Region, SurfaceId};

fn committed(surface: SurfaceId, geometry: Rect, generation: u64) -> CommittedSurfaceState {
    CommittedSurfaceState {
        surface,
        committed_generation: generation,
        geometry,
        buffer: BufferSource::CpuBuffer {
            handle: u64::from(surface.index()),
        },
        damage: Region::single(geometry),
    }
}

#[test]
fn focused_border_is_inserted_after_the_matching_committed_surface() {
    let output = OutputId::from_raw(1);
    let first = SurfaceId::new(1, 1);
    let focused = SurfaceId::new(2, 1);
    let geometry = Rect {
        x: 100,
        y: 40,
        width: 300,
        height: 200,
    };
    let list = focused_surface_display_list(
        output,
        &[first, focused],
        &[
            committed(first, Rect { x: 0, ..geometry }, 3),
            committed(focused, geometry, 7),
        ],
        Some(focused),
        FocusedSurfaceBorderStyle::default(),
    )
    .unwrap();

    assert_eq!(list.output, output);
    assert_eq!(list.commands.len(), 6);
    assert_eq!(
        list.commands[0],
        CompositorDisplayCommand::Surface { surface: first }
    );
    assert_eq!(
        list.commands[1],
        CompositorDisplayCommand::Surface { surface: focused }
    );
    let rects = list.solid_rects().collect::<Vec<_>>();
    assert_eq!(rects.len(), 4);
    assert_ne!(rects[0].generation, 0);
    assert_eq!(
        rects[0].node,
        CompositorNodeId::FocusedSurfaceBorder {
            surface: focused,
            edge: FocusedSurfaceBorderEdge::Top,
        }
    );
    assert_eq!(
        rects.iter().map(|rect| rect.geometry).collect::<Vec<_>>(),
        [
            Rect {
                x: 100,
                y: 40,
                width: 300,
                height: 2,
            },
            Rect {
                x: 100,
                y: 238,
                width: 300,
                height: 2,
            },
            Rect {
                x: 100,
                y: 42,
                width: 2,
                height: 196,
            },
            Rect {
                x: 398,
                y: 42,
                width: 2,
                height: 196,
            },
        ]
    );
}

#[test]
fn hidden_or_uncommitted_focus_produces_no_border() {
    let output = OutputId::from_raw(1);
    let visible = SurfaceId::new(1, 1);
    let hidden = SurfaceId::new(2, 1);
    let states = [
        committed(
            visible,
            Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            },
            1,
        ),
        committed(
            hidden,
            Rect {
                x: 100,
                y: 0,
                width: 100,
                height: 100,
            },
            1,
        ),
    ];

    let hidden_list = focused_surface_display_list(
        output,
        &[visible],
        &states,
        Some(hidden),
        FocusedSurfaceBorderStyle::default(),
    )
    .unwrap();
    assert_eq!(hidden_list.solid_rects().count(), 0);

    let missing = SurfaceId::new(3, 1);
    let missing_list = focused_surface_display_list(
        output,
        &[visible, missing],
        &states,
        Some(missing),
        FocusedSurfaceBorderStyle::default(),
    )
    .unwrap();
    assert_eq!(missing_list.solid_rects().count(), 0);
}

#[test]
fn display_list_damage_covers_old_and_new_focus_extents_once_per_edge() {
    let output = OutputId::from_raw(1);
    let first = SurfaceId::new(1, 1);
    let second = SurfaceId::new(2, 1);
    let states = [
        committed(
            first,
            Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 80,
            },
            1,
        ),
        committed(
            second,
            Rect {
                x: 100,
                y: 0,
                width: 100,
                height: 80,
            },
            1,
        ),
    ];
    let first_list = focused_surface_display_list(
        output,
        &[first, second],
        &states,
        Some(first),
        FocusedSurfaceBorderStyle::default(),
    )
    .unwrap();
    let stable_list = focused_surface_display_list(
        output,
        &[first, second],
        &states,
        Some(first),
        FocusedSurfaceBorderStyle::default(),
    )
    .unwrap();
    let pixel_generation_only = focused_surface_display_list(
        output,
        &[first, second],
        &[
            committed(first, states[0].geometry, 99),
            committed(second, states[1].geometry, 100),
        ],
        Some(first),
        FocusedSurfaceBorderStyle::default(),
    )
    .unwrap();
    let second_list = focused_surface_display_list(
        output,
        &[first, second],
        &states,
        Some(second),
        FocusedSurfaceBorderStyle::default(),
    )
    .unwrap();

    assert!(compositor_display_list_damage(&first_list, &stable_list).is_empty());
    assert!(
        compositor_display_list_damage(&first_list, &pixel_generation_only).is_empty(),
        "client pixel generations must not damage stable compositor chrome"
    );
    let damage = compositor_display_list_damage(&first_list, &second_list);
    assert_eq!(damage.rects.len(), 8);
    assert!(damage.rects.iter().any(|rect| rect.x == 0));
    assert!(damage.rects.iter().any(|rect| rect.x == 100));
}

#[test]
fn display_list_rejects_invalid_duplicate_and_over_capacity_surface_streams() {
    let output = OutputId::from_raw(1);
    let surface = SurfaceId::new(1, 1);
    assert_eq!(
        focused_surface_display_list(
            output,
            &[surface, surface],
            &[],
            None,
            FocusedSurfaceBorderStyle::default(),
        ),
        Err(CompositorDisplayListError::DuplicateSurface)
    );
    assert_eq!(
        focused_surface_display_list(
            output,
            &[SurfaceId::INVALID],
            &[],
            None,
            FocusedSurfaceBorderStyle::default(),
        ),
        Err(CompositorDisplayListError::InvalidSurface)
    );

    let surfaces = (0..=sophia_engine::MAX_COMPOSITOR_DISPLAY_COMMANDS)
        .map(|index| SurfaceId::new(u32::try_from(index).unwrap(), 1))
        .collect::<Vec<_>>();
    assert_eq!(
        focused_surface_display_list(
            output,
            &surfaces,
            &[],
            None,
            FocusedSurfaceBorderStyle::default(),
        ),
        Err(CompositorDisplayListError::CapacityExceeded)
    );
}
