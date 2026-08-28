const SLOT_DAMAGE_OUTPUT: sophia_protocol::Size = sophia_protocol::Size {
    width: 800,
    height: 600,
};

fn slot_damage_output() -> sophia_engine::HeadlessOutput {
    sophia_engine::HeadlessOutput {
        id: sophia_protocol::OutputId::from_raw(1),
        size: SLOT_DAMAGE_OUTPUT,
        scale: 1,
    }
}

fn tile(x: i32) -> sophia_protocol::Rect {
    sophia_protocol::Rect {
        x,
        y: 0,
        width: 100,
        height: 100,
    }
}

/// Four fixed tiles. The set and its order never change, so a snapshot differs
/// from its predecessor only where a surface committed a new generation --
/// which is what buffer age has to reach back across. Adding or reordering
/// surfaces is a stacking change and Engine damages every extent for it, so a
/// fixture that grew its surface list would measure that instead.
const SLOT_DAMAGE_TILES: [i32; 4] = [0, 200, 400, 600];

fn slot_damage_committed(
    surface: sophia_protocol::SurfaceId,
    geometry: sophia_protocol::Rect,
    generation: u64,
) -> sophia_protocol::CommittedSurfaceState {
    sophia_protocol::CommittedSurfaceState {
        surface,
        committed_generation: generation,
        geometry,
        content: sophia_protocol::SurfaceContentSet::singleton(
            sophia_protocol::BufferSource::CpuBuffer {
                handle: u64::from(surface.index()),
            },
            sophia_protocol::Size {
                width: geometry.width,
                height: geometry.height,
            },
        ),
        damage: sophia_protocol::Region::single(geometry),
    }
}

/// A snapshot of the four tiles at the given committed generations, built
/// through Engine's own reduction. The display list carries surfaces alone:
/// chrome would repaint borders on tiles that did not change.
fn slot_damage_snapshot(generations: [u64; 4]) -> sophia_engine::OutputFrameDamageSnapshot {
    let output = slot_damage_output();
    let committed: Vec<sophia_protocol::CommittedSurfaceState> = SLOT_DAMAGE_TILES
        .iter()
        .zip(generations)
        .enumerate()
        .map(|(index, (x, generation))| {
            slot_damage_committed(
                sophia_protocol::SurfaceId::new(
                    u32::try_from(index + 1).expect("fixture surface index"),
                    1,
                ),
                tile(*x),
                generation,
            )
        })
        .collect();
    let display_list = sophia_engine::CompositorDisplayList {
        output: output.id,
        commands: committed
            .iter()
            .map(|state| sophia_engine::CompositorDisplayCommand::Surface {
                surface: state.surface,
            })
            .collect(),
    };
    sophia_engine::output_frame_damage_snapshot(output, display_list, &committed, None)
        .expect("engine snapshot for the slot damage fixture")
}

fn slot(index: usize) -> LiveRendererFrameSlotId {
    LiveRendererFrameSlotId::from_index(index).expect("slot index within pool capacity")
}

fn slot_damage_covers(rects: &[sophia_protocol::Rect], probe: sophia_protocol::Rect) -> bool {
    rects.iter().any(|rect| {
        rect.x <= probe.x
            && rect.y <= probe.y
            && rect.x + i32::from(rect.width) >= probe.x + i32::from(probe.width)
            && rect.y + i32::from(rect.height) >= probe.y + i32::from(probe.height)
    })
}

#[test]
fn slot_damage_history_refuses_a_repaint_it_has_no_history_for() {
    let mut history = LiveRendererSlotDamageHistory::new();
    let current = slot_damage_snapshot([1, 1, 1, 1]);

    let plan = history.plan(
        slot(0),
        LiveRendererSlotBufferAge::new(1),
        Some(&current),
        SLOT_DAMAGE_OUTPUT,
    );

    assert_eq!(
        plan,
        LiveRendererSlotRepaint::Full {
            reason: LiveRendererSlotFullRepaintReason::NoHistory
        }
    );
    assert_eq!(history.metrics().full_repaints, 1);
    assert_eq!(history.metrics().partial_repaints, 0);
}

#[test]
fn slot_damage_history_refuses_a_buffer_whose_age_the_driver_would_not_report() {
    let mut history = LiveRendererSlotDamageHistory::new();
    let first = slot_damage_snapshot([1, 1, 1, 1]);
    history.record(slot(0), first.clone());

    let plan = history.plan(
        slot(0),
        LiveRendererSlotBufferAge::UNKNOWN,
        Some(&first),
        SLOT_DAMAGE_OUTPUT,
    );

    assert_eq!(
        plan,
        LiveRendererSlotRepaint::Full {
            reason: LiveRendererSlotFullRepaintReason::UnknownBufferAge
        }
    );
}

#[test]
fn slot_damage_history_reaches_past_the_immediately_previous_render() {
    // The slot is written three times, then hands back the buffer it used for
    // the first of them. The work owed is every change since that render, not
    // the change since the most recent one.
    let mut history = LiveRendererSlotDamageHistory::new();
    let first = slot_damage_snapshot([1, 1, 1, 1]);
    let second = slot_damage_snapshot([1, 2, 1, 1]);
    let third = slot_damage_snapshot([1, 2, 3, 1]);
    history.record(slot(0), first);
    history.record(slot(0), second);
    history.record(slot(0), third.clone());

    let fourth = slot_damage_snapshot([1, 2, 3, 4]);
    let age_one = history.plan(
        slot(0),
        LiveRendererSlotBufferAge::new(1),
        Some(&fourth),
        SLOT_DAMAGE_OUTPUT,
    );
    let age_three = history.plan(
        slot(0),
        LiveRendererSlotBufferAge::new(3),
        Some(&fourth),
        SLOT_DAMAGE_OUTPUT,
    );

    let (
        LiveRendererSlotRepaint::Partial { damage: near },
        LiveRendererSlotRepaint::Partial { damage: far },
    ) = (age_one, age_three)
    else {
        panic!("both ages have retained history and bounded damage");
    };
    // Age one: the buffer already holds every tile but the newest.
    assert!(slot_damage_covers(&near, tile(600)));
    assert!(!slot_damage_covers(&near, tile(200)));
    assert!(!slot_damage_covers(&near, tile(400)));
    // Age three: the buffer predates the second and third tiles as well, and
    // the repaint owes all of them. Painting only the newest would leave two
    // stale tiles in a frame that is otherwise presentable.
    assert!(slot_damage_covers(&far, tile(200)));
    assert!(slot_damage_covers(&far, tile(400)));
    assert!(slot_damage_covers(&far, tile(600)));
}

#[test]
fn slot_damage_history_refuses_a_buffer_older_than_it_retains() {
    let mut history = LiveRendererSlotDamageHistory::new();
    let snapshot = slot_damage_snapshot([1, 1, 1, 1]);
    history.record(slot(0), snapshot.clone());

    let plan = history.plan(
        slot(0),
        LiveRendererSlotBufferAge::new(2),
        Some(&snapshot),
        SLOT_DAMAGE_OUTPUT,
    );

    assert_eq!(
        plan,
        LiveRendererSlotRepaint::Full {
            reason: LiveRendererSlotFullRepaintReason::BeyondHistoryDepth
        }
    );
}

#[test]
fn slot_damage_history_retains_only_its_bounded_depth() {
    let mut history = LiveRendererSlotDamageHistory::new();
    let snapshot = slot_damage_snapshot([1, 1, 1, 1]);
    for _ in 0..(LIVE_RENDERER_SLOT_DAMAGE_HISTORY_DEPTH + 3) {
        history.record(slot(0), snapshot.clone());
    }

    assert_eq!(
        history.depth(slot(0)),
        LIVE_RENDERER_SLOT_DAMAGE_HISTORY_DEPTH
    );
}

#[test]
fn slot_damage_history_forgets_a_slot_whose_bundle_was_rebuilt() {
    let mut history = LiveRendererSlotDamageHistory::new();
    let snapshot = slot_damage_snapshot([1, 1, 1, 1]);
    history.record(slot(0), snapshot.clone());
    history.record(slot(1), snapshot.clone());

    history.invalidate(slot(0));

    assert_eq!(history.depth(slot(0)), 0);
    assert_eq!(history.metrics().invalidations, 1);
    // One slot's rebuild says nothing about another's buffers.
    assert_eq!(history.depth(slot(1)), 1);
    assert_eq!(
        history.plan(
            slot(0),
            LiveRendererSlotBufferAge::new(1),
            Some(&snapshot),
            SLOT_DAMAGE_OUTPUT
        ),
        LiveRendererSlotRepaint::Full {
            reason: LiveRendererSlotFullRepaintReason::NoHistory
        }
    );
}

#[test]
fn slot_damage_history_keeps_each_slot_separate() {
    // A fresher slot must not shorten a staler slot's repaint.
    let mut history = LiveRendererSlotDamageHistory::new();
    let first = slot_damage_snapshot([1, 1, 1, 1]);
    let second = slot_damage_snapshot([1, 2, 1, 1]);
    history.record(slot(0), first);
    history.record(slot(1), second.clone());

    assert_eq!(history.depth(slot(0)), 1);
    assert_eq!(history.depth(slot(1)), 1);
    assert_eq!(
        history.plan(
            slot(2),
            LiveRendererSlotBufferAge::new(1),
            Some(&second),
            SLOT_DAMAGE_OUTPUT
        ),
        LiveRendererSlotRepaint::Full {
            reason: LiveRendererSlotFullRepaintReason::NoHistory
        }
    );
}

#[test]
fn slot_damage_history_owes_nothing_for_a_scene_its_buffer_already_holds() {
    let mut history = LiveRendererSlotDamageHistory::new();
    let snapshot = slot_damage_snapshot([1, 1, 1, 1]);
    history.record(slot(0), snapshot.clone());

    let plan = history.plan(
        slot(0),
        LiveRendererSlotBufferAge::new(1),
        Some(&snapshot),
        SLOT_DAMAGE_OUTPUT,
    );

    assert_eq!(
        plan,
        LiveRendererSlotRepaint::Partial { damage: Vec::new() }
    );
    assert_eq!(history.metrics().partial_repaints, 1);
}


