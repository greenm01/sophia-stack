fn head_id(raw: u64) -> sophia_engine::RenderHeadId {
    sophia_engine::RenderHeadId::from_raw(raw)
}

#[test]
fn mirror_group_generation_waits_for_every_head() {
    let output = OutputId::from_raw(7);
    let mut group = LiveProductionMirrorGroupLifecycle::new(output, [head_id(11), head_id(12)]).unwrap();

    assert_eq!(
        group.begin(LiveProductionNativeFrameId::from_raw(41)),
        LiveProductionMirrorGroupBegin::Started
    );
    assert!(group.active_age().is_some());
    assert!(group.active_generation_hard_stalled(std::time::Duration::ZERO));
    assert_eq!(
        group.mark_submitted(head_id(11), LiveProductionNativeFrameId::from_raw(41)),
        LiveProductionMirrorHeadTransition::Accepted
    );
    assert_eq!(
        group.mark_submitted(head_id(12), LiveProductionNativeFrameId::from_raw(41)),
        LiveProductionMirrorHeadTransition::GroupReady
    );
    assert!(group.awaiting_flips());

    assert_eq!(
        group.mark_flipped(head_id(12), LiveProductionNativeFrameId::from_raw(41)),
        LiveProductionMirrorHeadTransition::Accepted
    );
    assert_eq!(group.completed_frame(), None);
    assert_eq!(
        group.mark_flipped(head_id(11), LiveProductionNativeFrameId::from_raw(41)),
        LiveProductionMirrorHeadTransition::GroupReady
    );
    assert!(!group.awaiting_flips());
    assert_eq!(
        group.take_completed_frame(),
        Some(LiveProductionNativeFrameId::from_raw(41))
    );
    assert_eq!(group.active_age(), None);
    assert!(!group.active_generation_hard_stalled(std::time::Duration::ZERO));
}

#[test]
fn mirror_group_rejects_mixed_generations_and_unknown_heads() {
    let output = OutputId::from_raw(7);
    let mut group = LiveProductionMirrorGroupLifecycle::new(output, [head_id(11), head_id(12)]).unwrap();
    let first = LiveProductionNativeFrameId::from_raw(41);
    let second = LiveProductionNativeFrameId::from_raw(42);

    assert_eq!(group.begin(first), LiveProductionMirrorGroupBegin::Started);
    assert_eq!(
        group.begin(second),
        LiveProductionMirrorGroupBegin::GenerationInFlight
    );
    assert_eq!(
        group.mark_submitted(head_id(99), first),
        LiveProductionMirrorHeadTransition::UnknownHead
    );
    assert_eq!(
        group.mark_submitted(head_id(11), second),
        LiveProductionMirrorHeadTransition::WrongGeneration
    );
    assert_eq!(
        group.mark_flipped(head_id(11), first),
        LiveProductionMirrorHeadTransition::NotSubmitted
    );
}

#[test]
fn mirror_group_initialization_is_head_scoped() {
    let output = OutputId::from_raw(7);
    let mut group = LiveProductionMirrorGroupLifecycle::new(output, [head_id(11), head_id(12)]).unwrap();

    assert!(!group.initialized());
    assert_eq!(
        group.mark_initialized(head_id(11)),
        LiveProductionMirrorHeadTransition::Accepted
    );
    assert!(!group.initialized());
    assert_eq!(
        group.mark_initialized(head_id(12)),
        LiveProductionMirrorHeadTransition::GroupReady
    );
    assert!(group.initialized());
    assert_eq!(
        group.mark_initialized(head_id(12)),
        LiveProductionMirrorHeadTransition::Duplicate
    );
}

#[test]
fn mirror_group_timing_uses_logical_generation_and_latest_physical_ust() {
    let output = OutputId::from_raw(7);
    let frame = LiveProductionNativeFrameId::from_raw(41);
    let mut group = LiveProductionMirrorGroupLifecycle::new(output, [head_id(11), head_id(12)]).unwrap();

    assert_eq!(group.begin(frame), LiveProductionMirrorGroupBegin::Started);
    assert!(group.observe_flip_timing(frame, 90, 8_000));
    assert!(group.observe_flip_timing(frame, 81, 9_000));
    assert_eq!(group.flip_timing(), Some((frame.raw(), 9_000)));
    assert!(!group.observe_flip_timing(
        LiveProductionNativeFrameId::from_raw(42),
        100,
        10_000
    ));
}

#[test]
fn aborted_mirror_group_cannot_admit_a_new_generation() {
    let output = OutputId::from_raw(7);
    let first = LiveProductionNativeFrameId::from_raw(41);
    let second = LiveProductionNativeFrameId::from_raw(42);
    let mut group = LiveProductionMirrorGroupLifecycle::new(output, [head_id(11), head_id(12)]).unwrap();

    assert_eq!(group.begin(first), LiveProductionMirrorGroupBegin::Started);
    assert_eq!(
        group.mark_submitted(head_id(11), first),
        LiveProductionMirrorHeadTransition::Accepted
    );
    assert!(group.abort(first));
    assert!(group.failed());
    assert_eq!(group.begin(second), LiveProductionMirrorGroupBegin::Poisoned);
}

#[test]
fn fast_mirror_head_cannot_advance_until_slow_head_submits_and_flips() {
    let output = OutputId::from_raw(7);
    let current = LiveProductionNativeFrameId::from_raw(41);
    let next = LiveProductionNativeFrameId::from_raw(42);
    let mut group = LiveProductionMirrorGroupLifecycle::new(output, [head_id(11), head_id(12)]).unwrap();

    assert_eq!(group.begin(current), LiveProductionMirrorGroupBegin::Started);
    assert_eq!(
        group.mark_submitted(head_id(12), current),
        LiveProductionMirrorHeadTransition::Accepted
    );
    assert_eq!(
        group.mark_flipped(head_id(12), current),
        LiveProductionMirrorHeadTransition::Accepted
    );

    let current_content = LiveProductionScanoutContent::RetainedMixed {
        frame: current,
        nonzero_rgb_pixels: 10,
    };
    let next_content = LiveProductionScanoutContent::RetainedMixed {
        frame: next,
        nonzero_rgb_pixels: 11,
    };
    // The slow head's completed worker result remains generation 41 even when
    // its one-deep pending slot already holds generation 42.
    let slow_work = live_production_mirror_head_work_frame(
        true,
        Some(current_content),
        Some(next_content),
    )
    .unwrap();
    let fast_work =
        live_production_mirror_head_work_frame(false, None, Some(next_content)).unwrap();
    assert_eq!(slow_work, current);
    assert_eq!(fast_work, next);

    // Head 12 has no KMS submission after its early callback, but it still
    // belongs to the active logical generation. Its queued successor must wait.
    assert!(!group.head_may_submit(head_id(12)));
    assert!(group.head_may_submit(head_id(11)));
    assert!(!group.head_may_submit_frame(head_id(12), fast_work));
    assert!(group.head_may_submit_frame(head_id(11), slow_work));
    assert_eq!(
        group.begin(next),
        LiveProductionMirrorGroupBegin::GenerationInFlight
    );

    assert_eq!(
        group.mark_submitted(head_id(11), current),
        LiveProductionMirrorHeadTransition::GroupReady
    );
    assert!(!group.head_may_submit(head_id(11)));
    assert!(!group.head_may_submit(head_id(12)));
    assert_eq!(
        group.mark_flipped(head_id(11), current),
        LiveProductionMirrorHeadTransition::GroupReady
    );

    assert!(group.head_may_submit(head_id(11)));
    assert!(group.head_may_submit(head_id(12)));
    assert!(group.head_may_submit_frame(head_id(12), fast_work));
    assert_eq!(group.begin(next), LiveProductionMirrorGroupBegin::Started);
    assert_eq!(
        group.mark_submitted(head_id(12), next),
        LiveProductionMirrorHeadTransition::Accepted
    );
}

#[test]
fn mirror_group_keeps_one_output_scoped_latest_successor() {
    let output = OutputId::from_raw(7);
    let current = LiveProductionNativeFrameId::from_raw(41);
    let next = LiveProductionNativeFrameId::from_raw(42);
    let latest = LiveProductionNativeFrameId::from_raw(43);
    let mut group = LiveProductionMirrorGroupLifecycle::new(output, [head_id(11), head_id(12)]).unwrap();

    assert_eq!(
        reduce_live_production_mirror_generation_queue_target(None, None),
        LiveProductionMirrorGenerationQueueTarget::Active
    );
    assert_eq!(group.begin(current), LiveProductionMirrorGroupBegin::Started);
    assert_eq!(
        reduce_live_production_mirror_generation_queue_target(group.active_frame(), None),
        LiveProductionMirrorGenerationQueueTarget::Successor
    );
    assert_eq!(
        reduce_live_production_mirror_generation_queue_target(group.active_frame(), Some(next)),
        LiveProductionMirrorGenerationQueueTarget::ReplaceSuccessor(next)
    );

    // Replacing the one logical successor cannot mutate either physical head's
    // ownership of the active generation.
    assert_eq!(
        reduce_live_production_mirror_generation_queue_target(group.active_frame(), Some(latest)),
        LiveProductionMirrorGenerationQueueTarget::ReplaceSuccessor(latest)
    );
    assert!(group.head_may_submit_frame(head_id(12), current));
    assert_eq!(
        group.mark_submitted(head_id(12), current),
        LiveProductionMirrorHeadTransition::Accepted
    );
}

#[test]
fn shutdown_drain_keeps_logical_frame_after_primary_flips_before_sibling() {
    let output = OutputId::from_raw(7);
    let frame = LiveProductionNativeFrameId::from_raw(12);
    let mut group = LiveProductionMirrorGroupLifecycle::new(output, [head_id(94), head_id(104)]).unwrap();

    assert_eq!(group.begin(frame), LiveProductionMirrorGroupBegin::Started);
    assert_eq!(
        group.mark_submitted(head_id(94), frame),
        LiveProductionMirrorHeadTransition::Accepted
    );
    assert_eq!(
        group.mark_flipped(head_id(94), frame),
        LiveProductionMirrorHeadTransition::Accepted
    );
    // The primary's per-head submitted_content has now moved to presented, but
    // the sibling's submit completes the logical submission join.
    assert_eq!(
        group.mark_submitted(head_id(104), frame),
        LiveProductionMirrorHeadTransition::GroupReady
    );
    assert!(group.awaiting_flips());
    assert_eq!(group.logically_submitted_frame(), Some(frame));

    assert_eq!(
        group.mark_flipped(head_id(104), frame),
        LiveProductionMirrorHeadTransition::GroupReady
    );
    assert_eq!(group.logically_submitted_frame(), None);
    assert_eq!(group.active_frame(), None);
    assert_eq!(group.completed_frame(), Some(frame));
    assert!(!group.failed());
}

#[test]
fn normal_mirror_retirement_cannot_reenter_scene_projection() {
    let source = include_str!("../../src/production_session/native_scanout.rs");
    let retirement = source
        .split_once("        pub fn retire_ready(\n")
        .expect("native scanout retains the normal retirement entry point")
        .1
        .split_once("        pub(crate) fn retire_ready_for_drain(\n")
        .expect("normal and drain retirement remain separate")
        .0;

    assert!(!retirement.contains("run_mirror_group_scene_tick"));
    assert!(!retirement.contains("CompositorBackendTickInput::default()"));
    assert!(retirement.contains("service_mirror_group_retirement"));
    assert!(retirement.contains("promote_queued_mirror_generation"));
}

#[test]
fn native_head_identity_is_wired_from_sessions_to_engine_registry() {
    // Production reachability for the opaque head boundary: the constructor
    // must build the backend head table from the session head records, admit
    // reduced head targets into the Engine registry, and route callbacks by
    // head. Deleting any of the three ends unwires the boundary and fails
    // this test rather than leaving a dead record type behind.
    let source = include_str!("../../src/production_session/native_scanout.rs");
    assert!(source.contains("LiveProductionNativeHeadTable::from_records(sessions.head_records"));
    assert!(source.contains("presentation_outputs.admit(target)"));
    assert!(source.contains("head.head == callback.head"));

    let session_source =
        include_str!("../../src/hardware_validation/atomic_scanout_card/session.rs");
    assert!(session_source.contains("allocator.mint()"));
    assert!(session_source.contains("head_records.push(crate::LiveNativeHeadRecord {"));
}
