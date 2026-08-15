#[test]
fn mirror_group_generation_waits_for_every_head() {
    let output = OutputId::from_raw(7);
    let mut group = LiveProductionMirrorGroupLifecycle::new(output, [11, 12]).unwrap();

    assert_eq!(
        group.begin(LiveProductionNativeFrameId::from_raw(41)),
        LiveProductionMirrorGroupBegin::Started
    );
    assert_eq!(
        group.mark_submitted(11, LiveProductionNativeFrameId::from_raw(41)),
        LiveProductionMirrorHeadTransition::Accepted
    );
    assert_eq!(
        group.mark_submitted(12, LiveProductionNativeFrameId::from_raw(41)),
        LiveProductionMirrorHeadTransition::GroupReady
    );
    assert!(group.awaiting_flips());

    assert_eq!(
        group.mark_flipped(12, LiveProductionNativeFrameId::from_raw(41)),
        LiveProductionMirrorHeadTransition::Accepted
    );
    assert_eq!(group.completed_frame(), None);
    assert_eq!(
        group.mark_flipped(11, LiveProductionNativeFrameId::from_raw(41)),
        LiveProductionMirrorHeadTransition::GroupReady
    );
    assert!(!group.awaiting_flips());
    assert_eq!(
        group.take_completed_frame(),
        Some(LiveProductionNativeFrameId::from_raw(41))
    );
}

#[test]
fn mirror_group_rejects_mixed_generations_and_unknown_heads() {
    let output = OutputId::from_raw(7);
    let mut group = LiveProductionMirrorGroupLifecycle::new(output, [11, 12]).unwrap();
    let first = LiveProductionNativeFrameId::from_raw(41);
    let second = LiveProductionNativeFrameId::from_raw(42);

    assert_eq!(group.begin(first), LiveProductionMirrorGroupBegin::Started);
    assert_eq!(
        group.begin(second),
        LiveProductionMirrorGroupBegin::GenerationInFlight
    );
    assert_eq!(
        group.mark_submitted(99, first),
        LiveProductionMirrorHeadTransition::UnknownHead
    );
    assert_eq!(
        group.mark_submitted(11, second),
        LiveProductionMirrorHeadTransition::WrongGeneration
    );
    assert_eq!(
        group.mark_flipped(11, first),
        LiveProductionMirrorHeadTransition::NotSubmitted
    );
}

#[test]
fn mirror_group_initialization_is_connector_scoped() {
    let output = OutputId::from_raw(7);
    let mut group = LiveProductionMirrorGroupLifecycle::new(output, [11, 12]).unwrap();

    assert!(!group.initialized());
    assert_eq!(
        group.mark_initialized(11),
        LiveProductionMirrorHeadTransition::Accepted
    );
    assert!(!group.initialized());
    assert_eq!(
        group.mark_initialized(12),
        LiveProductionMirrorHeadTransition::GroupReady
    );
    assert!(group.initialized());
    assert_eq!(
        group.mark_initialized(12),
        LiveProductionMirrorHeadTransition::Duplicate
    );
}

#[test]
fn mirror_group_timing_uses_logical_generation_and_latest_physical_ust() {
    let output = OutputId::from_raw(7);
    let frame = LiveProductionNativeFrameId::from_raw(41);
    let mut group = LiveProductionMirrorGroupLifecycle::new(output, [11, 12]).unwrap();

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
    let mut group = LiveProductionMirrorGroupLifecycle::new(output, [11, 12]).unwrap();

    assert_eq!(group.begin(first), LiveProductionMirrorGroupBegin::Started);
    assert_eq!(
        group.mark_submitted(11, first),
        LiveProductionMirrorHeadTransition::Accepted
    );
    assert!(group.abort(first));
    assert!(group.failed());
    assert_eq!(group.begin(second), LiveProductionMirrorGroupBegin::Poisoned);
}
