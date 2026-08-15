#[test]
fn mirror_group_generation_waits_for_every_head() {
    let output = OutputId::from_raw(7);
    let mut group = LiveProductionMirrorGroupLifecycle::new(output, [11, 12]).unwrap();

    assert_eq!(
        group.begin(LiveProductionNativeFrameId::from_raw(41)),
        LiveProductionMirrorGroupBegin::Started
    );
    assert!(group.active_age().is_some());
    assert!(group.active_generation_hard_stalled(std::time::Duration::ZERO));
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
    assert_eq!(group.active_age(), None);
    assert!(!group.active_generation_hard_stalled(std::time::Duration::ZERO));
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

#[test]
fn fast_mirror_head_cannot_advance_until_slow_head_submits_and_flips() {
    let output = OutputId::from_raw(7);
    let current = LiveProductionNativeFrameId::from_raw(41);
    let next = LiveProductionNativeFrameId::from_raw(42);
    let mut group = LiveProductionMirrorGroupLifecycle::new(output, [11, 12]).unwrap();

    assert_eq!(group.begin(current), LiveProductionMirrorGroupBegin::Started);
    assert_eq!(
        group.mark_submitted(12, current),
        LiveProductionMirrorHeadTransition::Accepted
    );
    assert_eq!(
        group.mark_flipped(12, current),
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
    assert!(!group.connector_may_submit(12));
    assert!(group.connector_may_submit(11));
    assert!(!group.connector_may_submit_frame(12, fast_work));
    assert!(group.connector_may_submit_frame(11, slow_work));
    assert_eq!(
        group.begin(next),
        LiveProductionMirrorGroupBegin::GenerationInFlight
    );

    assert_eq!(
        group.mark_submitted(11, current),
        LiveProductionMirrorHeadTransition::GroupReady
    );
    assert!(!group.connector_may_submit(11));
    assert!(!group.connector_may_submit(12));
    assert_eq!(
        group.mark_flipped(11, current),
        LiveProductionMirrorHeadTransition::GroupReady
    );

    assert!(group.connector_may_submit(11));
    assert!(group.connector_may_submit(12));
    assert!(group.connector_may_submit_frame(12, fast_work));
    assert_eq!(group.begin(next), LiveProductionMirrorGroupBegin::Started);
    assert_eq!(
        group.mark_submitted(12, next),
        LiveProductionMirrorHeadTransition::Accepted
    );
}

#[test]
fn cleanup_blocked_mirror_head_preserves_reserved_generation_before_worker_start() {
    let output = OutputId::from_raw(7);
    let current = LiveProductionNativeFrameId::from_raw(41);
    let next = LiveProductionNativeFrameId::from_raw(42);
    let latest = LiveProductionNativeFrameId::from_raw(43);
    let current_content = LiveProductionScanoutContent::RetainedMixed {
        frame: current,
        nonzero_rgb_pixels: 10,
    };
    let next_content = LiveProductionScanoutContent::RetainedMixed {
        frame: next,
        nonzero_rgb_pixels: 11,
    };
    let mut group = LiveProductionMirrorGroupLifecycle::new(output, [11, 12]).unwrap();

    // Queue reservation happens before either exporter is polled. Head 11 has
    // captured generation 41; head 12 is cleanup-blocked with 41 still pending.
    assert_eq!(group.begin(current), LiveProductionMirrorGroupBegin::Started);
    assert_eq!(
        reduce_live_production_mirror_head_queue_target(
            group.active_frame(),
            false,
            Some(current_content),
        ),
        LiveProductionMirrorHeadQueueTarget::Deferred
    );
    assert_eq!(
        reduce_live_production_mirror_head_queue_target(
            group.active_frame(),
            true,
            Some(next_content),
        ),
        LiveProductionMirrorHeadQueueTarget::Pending
    );

    // Staging generation 42 in the deferred slot leaves the slow head's real
    // exporter identity at 41, so it remains eligible after cleanup succeeds.
    let pending = Some(current_content);
    let deferred = Some(next_content);
    let slow_work = live_production_mirror_head_work_frame(false, None, pending).unwrap();
    assert_eq!(slow_work, current);
    assert_eq!(deferred.unwrap().frame(), next);
    let latest_content = LiveProductionScanoutContent::RetainedMixed {
        frame: latest,
        nonzero_rgb_pixels: 12,
    };
    assert_eq!(
        reduce_live_production_mirror_head_queue_target(
            group.active_frame(),
            false,
            Some(current_content),
        ),
        LiveProductionMirrorHeadQueueTarget::Deferred
    );
    // The production slot is one-deep/latest-wins: replacing its deferred value
    // never touches the still-pending active content.
    let deferred = Some(latest_content);
    assert_eq!(pending.unwrap().frame(), current);
    assert_eq!(deferred.unwrap().frame(), latest);
    assert!(group.connector_may_submit_frame(12, slow_work));
    assert_eq!(
        group.mark_submitted(12, slow_work),
        LiveProductionMirrorHeadTransition::Accepted
    );
}

#[test]
fn shutdown_drain_keeps_logical_frame_after_primary_flips_before_sibling() {
    let output = OutputId::from_raw(7);
    let frame = LiveProductionNativeFrameId::from_raw(12);
    let mut group = LiveProductionMirrorGroupLifecycle::new(output, [94, 104]).unwrap();

    assert_eq!(group.begin(frame), LiveProductionMirrorGroupBegin::Started);
    assert_eq!(
        group.mark_submitted(94, frame),
        LiveProductionMirrorHeadTransition::Accepted
    );
    assert_eq!(
        group.mark_flipped(94, frame),
        LiveProductionMirrorHeadTransition::Accepted
    );
    // The primary's per-head submitted_content has now moved to presented, but
    // the sibling's submit completes the logical submission join.
    assert_eq!(
        group.mark_submitted(104, frame),
        LiveProductionMirrorHeadTransition::GroupReady
    );
    assert!(group.awaiting_flips());
    assert_eq!(group.logically_submitted_frame(), Some(frame));

    assert_eq!(
        group.mark_flipped(104, frame),
        LiveProductionMirrorHeadTransition::GroupReady
    );
    assert_eq!(group.logically_submitted_frame(), None);
}
