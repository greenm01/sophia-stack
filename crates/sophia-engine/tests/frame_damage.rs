use sophia_engine::{
    CompositorDisplayCommand, CompositorDisplayList, HeadlessOutput, OutputFrameDamageError,
    OutputFramePresentationState, OutputRepaintPlan, SurfaceChromeStyle, output_frame_damage,
    output_frame_damage_snapshot, surface_chrome_display_list,
};
use sophia_protocol::{
    BufferSource, CommittedSurfaceState, OutputId, Rect, Region, Size, SurfaceId,
};

fn output(id: u64) -> HeadlessOutput {
    HeadlessOutput {
        id: OutputId::from_raw(id),
        size: Size {
            width: 200,
            height: 100,
        },
        scale: 1,
    }
}

fn committed(surface: SurfaceId, geometry: Rect, generation: u64) -> CommittedSurfaceState {
    CommittedSurfaceState {
        surface,
        committed_generation: generation,
        geometry,
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::CpuBuffer {
                handle: u64::from(surface.index()),
            },
            Size {
                width: geometry.width,
                height: geometry.height,
            },
        ),
        damage: Region::single(geometry),
    }
}

#[test]
fn initial_output_snapshot_requires_full_damage_and_stable_state_requires_none() {
    let output = output(1);
    let surface = SurfaceId::new(1, 1);
    let committed = committed(
        surface,
        Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        },
        1,
    );
    let display_list = surface_chrome_display_list(
        output.id,
        &[surface],
        std::slice::from_ref(&committed),
        Some(surface),
        SurfaceChromeStyle::default(),
    )
    .unwrap();
    let snapshot =
        output_frame_damage_snapshot(output, display_list, std::slice::from_ref(&committed), None)
            .unwrap();

    assert_eq!(
        output_frame_damage(None, &snapshot).unwrap(),
        Region::single(Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 100,
        })
    );
    assert!(
        output_frame_damage(Some(&snapshot), &snapshot)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn client_generation_geometry_and_stacking_changes_damage_old_and_new_extents() {
    let output = output(1);
    let first = SurfaceId::new(1, 1);
    let second = SurfaceId::new(2, 1);
    let first_rect = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 100,
    };
    let second_rect = Rect {
        x: 100,
        y: 0,
        width: 100,
        height: 100,
    };
    let before_states = [
        committed(first, first_rect, 1),
        committed(second, second_rect, 1),
    ];
    let after_states = [
        committed(first, first_rect, 2),
        committed(second, second_rect, 1),
    ];
    let before = output_frame_damage_snapshot(
        output,
        surface_chrome_display_list(
            output.id,
            &[first, second],
            &before_states,
            None,
            SurfaceChromeStyle::default(),
        )
        .unwrap(),
        &before_states,
        None,
    )
    .unwrap();
    let generation_changed = output_frame_damage_snapshot(
        output,
        surface_chrome_display_list(
            output.id,
            &[first, second],
            &after_states,
            None,
            SurfaceChromeStyle::default(),
        )
        .unwrap(),
        &after_states,
        None,
    )
    .unwrap();
    let generation_damage = output_frame_damage(Some(&before), &generation_changed).unwrap();
    assert_eq!(generation_damage.rects, [first_rect, first_rect]);

    let reordered = output_frame_damage_snapshot(
        output,
        surface_chrome_display_list(
            output.id,
            &[second, first],
            &after_states,
            None,
            SurfaceChromeStyle::default(),
        )
        .unwrap(),
        &after_states,
        None,
    )
    .unwrap();
    let reorder_damage = output_frame_damage(Some(&generation_changed), &reordered).unwrap();
    assert_eq!(reorder_damage.rects.len(), 4);
    assert!(reorder_damage.rects.contains(&first_rect));
    assert!(reorder_damage.rects.contains(&second_rect));
}

#[test]
fn compositor_focus_and_software_cursor_share_one_combined_damage_region() {
    let output = output(1);
    let first = SurfaceId::new(1, 1);
    let second = SurfaceId::new(2, 1);
    let states = [
        committed(
            first,
            Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            },
            1,
        ),
        committed(
            second,
            Rect {
                x: 100,
                y: 0,
                width: 100,
                height: 100,
            },
            1,
        ),
    ];
    let before_cursor = Rect {
        x: 10,
        y: 10,
        width: 16,
        height: 16,
    };
    let after_cursor = Rect {
        x: 30,
        y: 20,
        width: 16,
        height: 16,
    };
    let before = output_frame_damage_snapshot(
        output,
        surface_chrome_display_list(
            output.id,
            &[first, second],
            &states,
            Some(first),
            SurfaceChromeStyle::default(),
        )
        .unwrap(),
        &states,
        Some(before_cursor),
    )
    .unwrap();
    let after = output_frame_damage_snapshot(
        output,
        surface_chrome_display_list(
            output.id,
            &[first, second],
            &states,
            Some(second),
            SurfaceChromeStyle::default(),
        )
        .unwrap(),
        &states,
        Some(after_cursor),
    )
    .unwrap();

    let damage = output_frame_damage(Some(&before), &after).unwrap();
    assert_eq!(damage.rects.len(), 10);
    assert!(damage.rects.contains(&before_cursor));
    assert!(damage.rects.contains(&after_cursor));
}

#[test]
fn snapshot_rejects_cross_output_and_duplicate_surface_streams() {
    let output = output(1);
    assert_eq!(
        output_frame_damage_snapshot(
            output,
            CompositorDisplayList::empty(OutputId::from_raw(2)),
            &[],
            None
        ),
        Err(OutputFrameDamageError::OutputMismatch)
    );

    let surface = SurfaceId::new(1, 1);
    let duplicate = CompositorDisplayList {
        output: output.id,
        commands: vec![
            CompositorDisplayCommand::Surface { surface },
            CompositorDisplayCommand::Surface { surface },
        ],
    };
    assert_eq!(
        output_frame_damage_snapshot(output, duplicate, &[], None),
        Err(OutputFrameDamageError::DuplicateSurface)
    );
}

#[test]
fn presentation_lifecycle_plans_combined_client_damage_against_presented_state() {
    let output = output(1);
    let surface = SurfaceId::new(1, 1);
    let geometry = Rect {
        x: 10,
        y: 10,
        width: 50,
        height: 40,
    };
    let before_state = committed(surface, geometry, 1);
    let after_state = committed(surface, geometry, 2);
    let list = CompositorDisplayList {
        output: output.id,
        commands: vec![CompositorDisplayCommand::Surface { surface }],
    };
    let before = output_frame_damage_snapshot(
        output,
        list.clone(),
        std::slice::from_ref(&before_state),
        None,
    )
    .unwrap();
    let after =
        output_frame_damage_snapshot(output, list, std::slice::from_ref(&after_state), None)
            .unwrap();
    let mut presentation = OutputFramePresentationState::new(output).unwrap();
    presentation.queue(before).unwrap();
    presentation.mark_initial_presented().unwrap();

    let queued = presentation.queue(after).unwrap();
    assert!(queued.compositor_damage.is_empty());
    assert_eq!(queued.damage.rects, [geometry, geometry]);
    assert!(matches!(
        queued.repaint,
        OutputRepaintPlan::Partial {
            damaged_pixels: 2_000,
            ..
        }
    ));
    presentation.mark_submitted().unwrap();
    presentation.mark_presented().unwrap();
}

#[test]
fn damage_reducer_rejects_mutated_snapshot_invariants() {
    let output = output(1);
    let surface = SurfaceId::new(1, 1);
    let state = committed(
        surface,
        Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        },
        1,
    );
    let mut snapshot = output_frame_damage_snapshot(
        output,
        CompositorDisplayList {
            output: output.id,
            commands: vec![CompositorDisplayCommand::Surface { surface }],
        },
        std::slice::from_ref(&state),
        None,
    )
    .unwrap();
    snapshot.surfaces.push(snapshot.surfaces[0]);

    assert_eq!(
        output_frame_damage(None, &snapshot),
        Err(OutputFrameDamageError::DuplicateSurface)
    );
}
