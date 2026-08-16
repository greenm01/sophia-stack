fn topology_plan_selection(
    raw: u32,
    size: sophia_protocol::Size,
) -> sophia_backend_live::LibdrmNativePrimaryPlaneSelection {
    sophia_backend_live::LibdrmNativePrimaryPlaneSelection::new(
        drm::control::from_u32(raw).unwrap(),
        drm::control::from_u32(raw + 100).unwrap(),
        drm::control::from_u32(raw + 200).unwrap(),
        size,
        None,
    )
}

fn topology_plan_mode() -> drm::control::Mode {
    drm::control::Mode::from(drm_ffi::drm_mode_modeinfo::default())
}

#[test]
fn live_native_topology_plan_resolves_every_enabled_and_disabled_head_without_mutation() {
    use sophia_backend_live::{
        LiveOutputAuthorityDisabledHead, LiveOutputAuthorityHeadTarget,
        LiveProductionNativeTopologyCurrentHead, LiveProductionNativeTopologyDisposition,
        NativeMirrorGrouping,
    };
    use sophia_protocol::{
        OutputHeadMapping, OutputId, OutputTransform, OutputVrrPolicy, Size,
    };

    let output_one = OutputId::from_raw(1);
    let output_two = OutputId::from_raw(2);
    let head_one = RenderHeadId::from_raw(11);
    let head_two = RenderHeadId::from_raw(12);
    let large = Size {
        width: 2_560,
        height: 1_440,
    };
    let small = Size {
        width: 1_920,
        height: 1_080,
    };
    let current = [
        LiveProductionNativeTopologyCurrentHead::new(
            head_one,
            0,
            output_one,
            topology_plan_selection(1, large),
            4,
        ),
        LiveProductionNativeTopologyCurrentHead::new(
            head_two,
            1,
            output_one,
            topology_plan_selection(2, small),
            7,
        ),
    ];
    let resolved = sophia_backend_live::LiveResolvedOutputTopology {
        primary_output: output_one,
        outputs: vec![sophia_engine::HeadlessOutput {
            id: output_one,
            size: large,
            scale: 1,
        }],
        logical_viewports: vec![sophia_backend_live::LiveOutputAuthorityLogicalViewport {
            output: output_one,
            logical: Rect {
                x: 0,
                y: 0,
                width: large.width,
                height: large.height,
            },
        }],
        disabled_heads: vec![LiveOutputAuthorityDisabledHead {
            head: head_two,
            target_generation: 8,
        }],
        targets: vec![LiveOutputAuthorityHeadTarget {
            head: head_one,
            target_generation: 5,
            output: output_one,
            timing: sophia_backend_live::LibdrmNativeOutputTiming::new(2_560, 1_440, 60_000),
            native_size: large,
            transform: OutputTransform::Normal,
            mapping: OutputHeadMapping::Exact,
            vrr: OutputVrrPolicy::Disabled,
        }],
        mirror_grouping: NativeMirrorGrouping::none(),
    };
    let mut mode_reads = Vec::new();
    let plan = sophia_backend_live::plan_live_production_native_topology(
        &current,
        &resolved,
        |current, timing| {
            mode_reads.push((current.head, timing));
            Ok(Some(topology_plan_mode()))
        },
    )
    .unwrap();
    assert_eq!(mode_reads.len(), 1, "disabled heads need no mode lookup");
    assert_eq!(plan.primary_output, output_one);
    assert_eq!(plan.heads.len(), 2);
    assert!(matches!(
        plan.heads[0].disposition,
        LiveProductionNativeTopologyDisposition::Enabled { output, selection, .. }
            if output == output_one && selection.size() == large
    ));
    assert_eq!(
        plan.heads[1].disposition,
        LiveProductionNativeTopologyDisposition::Disabled
    );
    assert_eq!(
        current[1].output, output_one,
        "projection must not mutate the published binding"
    );

    let mut split = resolved;
    split.outputs.push(sophia_engine::HeadlessOutput {
        id: output_two,
        size: small,
        scale: 1,
    });
    split.logical_viewports.push(
        sophia_backend_live::LiveOutputAuthorityLogicalViewport {
            output: output_two,
            logical: Rect {
                x: large.width,
                y: 0,
                width: small.width,
                height: small.height,
            },
        },
    );
    split.disabled_heads.clear();
    split.targets.push(LiveOutputAuthorityHeadTarget {
        head: head_two,
        target_generation: 8,
        output: output_two,
        timing: sophia_backend_live::LibdrmNativeOutputTiming::new(1_920, 1_080, 60_000),
        native_size: small,
        transform: OutputTransform::Normal,
        mapping: OutputHeadMapping::Exact,
        vrr: OutputVrrPolicy::Disabled,
    });
    let split_plan = sophia_backend_live::plan_live_production_native_topology(
        &current,
        &split,
        |_current, _timing| Ok(Some(topology_plan_mode())),
    )
    .unwrap();
    assert!(matches!(
        split_plan.heads[1].disposition,
        LiveProductionNativeTopologyDisposition::Enabled { output, .. } if output == output_two
    ));
}

#[test]
fn live_native_topology_plan_rejects_incomplete_coverage_and_stale_generations() {
    use sophia_backend_live::{
        LiveOutputAuthorityHeadTarget, LiveProductionNativeTopologyCurrentHead,
        LiveProductionNativeTopologyPlanError, NativeMirrorGrouping,
    };
    use sophia_protocol::{
        OutputHeadMapping, OutputId, OutputTransform, OutputVrrPolicy, Size,
    };

    let output = OutputId::from_raw(1);
    let size = Size {
        width: 1_280,
        height: 720,
    };
    let head_one = RenderHeadId::from_raw(21);
    let head_two = RenderHeadId::from_raw(22);
    let current = [
        LiveProductionNativeTopologyCurrentHead::new(
            head_one,
            0,
            output,
            topology_plan_selection(3, size),
            1,
        ),
        LiveProductionNativeTopologyCurrentHead::new(
            head_two,
            0,
            output,
            topology_plan_selection(4, size),
            1,
        ),
    ];
    let mut resolved = sophia_backend_live::LiveResolvedOutputTopology {
        primary_output: output,
        outputs: vec![sophia_engine::HeadlessOutput {
            id: output,
            size,
            scale: 1,
        }],
        logical_viewports: vec![sophia_backend_live::LiveOutputAuthorityLogicalViewport {
            output,
            logical: Rect {
                x: 0,
                y: 0,
                width: size.width,
                height: size.height,
            },
        }],
        disabled_heads: Vec::new(),
        targets: vec![LiveOutputAuthorityHeadTarget {
            head: head_one,
            target_generation: 2,
            output,
            timing: sophia_backend_live::LibdrmNativeOutputTiming::new(1_280, 720, 60_000),
            native_size: size,
            transform: OutputTransform::Normal,
            mapping: OutputHeadMapping::Exact,
            vrr: OutputVrrPolicy::Disabled,
        }],
        mirror_grouping: NativeMirrorGrouping::none(),
    };
    assert_eq!(
        sophia_backend_live::plan_live_production_native_topology(
            &current,
            &resolved,
            |_current, _timing| Ok(Some(topology_plan_mode())),
        ),
        Err(LiveProductionNativeTopologyPlanError::MissingCandidateHead(
            head_two
        ))
    );

    resolved.disabled_heads.push(
        sophia_backend_live::LiveOutputAuthorityDisabledHead {
            head: head_two,
            target_generation: 2,
        },
    );
    resolved.targets[0].target_generation = 1;
    assert_eq!(
        sophia_backend_live::plan_live_production_native_topology(
            &current,
            &resolved,
            |_current, _timing| Ok(Some(topology_plan_mode())),
        ),
        Err(LiveProductionNativeTopologyPlanError::InvalidGeneration(
            head_one
        ))
    );
}

fn topology_apply_plan(card_indices: &[usize]) -> sophia_backend_live::LiveProductionNativeTopologyPlan {
    use sophia_backend_live::{
        LiveOutputAuthorityLogicalViewport, LiveProductionNativeTopologyDisposition,
        LiveProductionNativeTopologyHeadPlan, LiveProductionNativeTopologyPlan,
    };
    use sophia_protocol::{OutputHeadMapping, OutputTransform, OutputVrrPolicy};

    let output = OutputId::from_raw(1);
    let size = Size {
        width: 1_920,
        height: 1_080,
    };
    LiveProductionNativeTopologyPlan {
        primary_output: output,
        outputs: vec![sophia_engine::HeadlessOutput {
            id: output,
            size,
            scale: 1,
        }],
        logical_viewports: vec![LiveOutputAuthorityLogicalViewport {
            output,
            logical: Rect {
                x: 0,
                y: 0,
                width: size.width,
                height: size.height,
            },
        }],
        heads: card_indices
            .iter()
            .copied()
            .enumerate()
            .map(|(index, card_index)| {
                let head = RenderHeadId::from_raw(index as u64 + 1);
                let previous_selection = topology_plan_selection(index as u32 + 1, size);
                LiveProductionNativeTopologyHeadPlan {
                    head,
                    card_index,
                    previous_output: output,
                    previous_selection,
                    previous_target_generation: 1,
                    candidate_target_generation: 2,
                    disposition: LiveProductionNativeTopologyDisposition::Enabled {
                        output,
                        selection: previous_selection,
                        transform: OutputTransform::Normal,
                        mapping: OutputHeadMapping::Exact,
                        vrr: OutputVrrPolicy::Disabled,
                    },
                }
            })
            .collect(),
    }
}

#[test]
fn topology_apply_coordinator_rolls_back_the_accepted_card_prefix_in_reverse_order() {
    use sophia_backend_live::{
        LiveProductionNativeTopologyApplyCoordinator as Coordinator,
        LiveProductionNativeTopologyApplyPhase as Phase,
        LiveProductionNativeTopologyApplyTransition as Transition, NativeTopologySubmitOutcome,
    };

    let mut coordinator = Coordinator::new(&topology_apply_plan(&[9, 3, 7])).unwrap();
    assert_eq!(coordinator.phase(), Phase::Prepared);
    assert_eq!(coordinator.begin_apply(), Transition::Accepted);

    // Cards are canonicalized independently of head discovery order.
    assert_eq!(coordinator.current_card_index(), Some(3));
    assert!(matches!(
        coordinator.observe_apply(3, NativeTopologySubmitOutcome::Accepted),
        Transition::CardApplied { card_index: 3, .. }
    ));
    assert_eq!(coordinator.current_card_index(), Some(7));
    assert!(matches!(
        coordinator.observe_apply(7, NativeTopologySubmitOutcome::Accepted),
        Transition::CardApplied { card_index: 7, .. }
    ));
    assert_eq!(coordinator.current_card_index(), Some(9));
    assert_eq!(
        coordinator.observe_apply(9, NativeTopologySubmitOutcome::Rejected),
        Transition::RollbackRequired {
            failed_card_index: 9
        }
    );

    assert_eq!(coordinator.phase(), Phase::RollingBack);
    assert_eq!(coordinator.current_card_index(), Some(7));
    assert!(matches!(
        coordinator.observe_rollback(7, NativeTopologySubmitOutcome::Accepted),
        Transition::CardRolledBack { card_index: 7, .. }
    ));
    assert_eq!(coordinator.current_card_index(), Some(3));
    assert!(matches!(
        coordinator.observe_rollback(3, NativeTopologySubmitOutcome::Accepted),
        Transition::RolledBack { card_index: 3, .. }
    ));
    assert_eq!(coordinator.phase(), Phase::RolledBack);
    assert_eq!(coordinator.current_card_index(), None);
}

#[test]
fn topology_apply_coordinator_retries_busy_and_distinguishes_unmutated_failure() {
    use sophia_backend_live::{
        LiveProductionNativeTopologyApplyCoordinator as Coordinator,
        LiveProductionNativeTopologyApplyPhase as Phase,
        LiveProductionNativeTopologyApplyTransition as Transition, NativeTopologySubmitOutcome,
    };

    let mut coordinator = Coordinator::new(&topology_apply_plan(&[4, 4])).unwrap();
    assert_eq!(coordinator.begin_apply(), Transition::Accepted);
    assert_eq!(coordinator.current_heads().len(), 2);
    assert_eq!(
        coordinator.observe_apply(4, NativeTopologySubmitOutcome::Busy),
        Transition::Retry
    );
    assert_eq!(coordinator.current_card_index(), Some(4));
    assert!(matches!(
        coordinator.observe_apply(4, NativeTopologySubmitOutcome::Accepted),
        Transition::Applied { card_index: 4, .. }
    ));
    assert_eq!(coordinator.phase(), Phase::Applied);

    let mut rejected = Coordinator::new(&topology_apply_plan(&[2, 5])).unwrap();
    assert_eq!(rejected.begin_apply(), Transition::Accepted);
    assert_eq!(
        rejected.observe_apply(2, NativeTopologySubmitOutcome::Rejected),
        Transition::FailedWithoutMutation { card_index: 2 }
    );
    assert_eq!(rejected.phase(), Phase::Failed);
    assert_eq!(rejected.current_card_index(), None);
}

#[test]
fn published_topology_projection_uses_published_viewports_and_live_native_targets() {
    use sophia_backend_live::{
        LibdrmNativeOutputTiming, LiveProductionNativeTopologyCurrentHead,
        project_live_production_published_topology,
    };
    use sophia_protocol::{
        DisplayHeadId, DisplayModeId, OutputAuthoritySnapshot, OutputGroupMember,
        OutputHeadDescriptor, OutputHeadMapping, OutputLogicalGroupState, OutputModeDescriptor,
        OutputTransformSet,
    };

    let left = OutputId::from_raw(1);
    let right = OutputId::from_raw(2);
    let left_head = RenderHeadId::from_raw(31);
    let right_head = RenderHeadId::from_raw(32);
    let left_size = Size {
        width: 2_560,
        height: 1_440,
    };
    let right_size = Size {
        width: 1_920,
        height: 1_080,
    };
    let descriptor = |head: RenderHeadId, generation, size: Size| OutputHeadDescriptor {
        head: DisplayHeadId::from_raw(head.raw()),
        generation,
        label: format!("Display {}", head.raw()),
        connected: true,
        enabled: true,
        current_mode: Some(DisplayModeId::from_raw(head.raw() * 10)),
        transforms: OutputTransformSet::NORMAL,
        vrr_capable: false,
        modes: vec![OutputModeDescriptor {
            mode: DisplayModeId::from_raw(head.raw() * 10),
            pixel_size: size,
            refresh_millihz: 60_000,
            preferred: true,
        }],
    };
    let snapshot = OutputAuthoritySnapshot {
        topology_epoch: 8,
        primary_output: left,
        heads: vec![
            descriptor(left_head, 4, left_size),
            descriptor(right_head, 7, right_size),
        ],
        groups: vec![
            OutputLogicalGroupState {
                output: left,
                generation: 3,
                logical: Rect {
                    x: 0,
                    y: 0,
                    width: 1_280,
                    height: 720,
                },
                members: vec![OutputGroupMember {
                    head: DisplayHeadId::from_raw(left_head.raw()),
                    mapping: OutputHeadMapping::Exact,
                }],
            },
            OutputLogicalGroupState {
                output: right,
                generation: 5,
                logical: Rect {
                    x: 1_280,
                    y: 0,
                    width: 1_920,
                    height: 1_080,
                },
                members: vec![OutputGroupMember {
                    head: DisplayHeadId::from_raw(right_head.raw()),
                    mapping: OutputHeadMapping::Fit,
                }],
            },
        ],
    };
    let current = [
        LiveProductionNativeTopologyCurrentHead::new(
            left_head,
            0,
            left,
            topology_plan_selection(10, left_size),
            4,
        ),
        LiveProductionNativeTopologyCurrentHead::new(
            right_head,
            1,
            right,
            topology_plan_selection(20, right_size),
            7,
        ),
    ];

    let projected = project_live_production_published_topology(
        &current,
        &snapshot,
        |head| {
            Ok(LibdrmNativeOutputTiming::new(
                head.selection.size().width as u32,
                head.selection.size().height as u32,
                60_000,
            ))
        },
    )
    .unwrap();

    assert_eq!(projected.logical_viewports[0].logical, snapshot.groups[0].logical);
    assert_eq!(projected.logical_viewports[1].logical, snapshot.groups[1].logical);
    assert_eq!(projected.targets[0].native_size, left_size);
    assert_eq!(projected.targets[0].target_generation, 4);
    assert_eq!(projected.targets[1].native_size, right_size);
    assert_eq!(projected.targets[1].target_generation, 7);
    assert_eq!(projected.targets[1].mapping, OutputHeadMapping::Fit);
}

#[test]
fn topology_resource_cohort_requires_complete_candidate_and_rollback_owners() {
    use sophia_backend_live::{
        LiveProductionNativeTopologyCandidateResource as Candidate,
        LiveProductionNativeTopologyDisposition as Disposition,
        LiveProductionNativeTopologyResourceCohort as Cohort,
        LiveProductionNativeTopologyResourceTransition as Transition,
    };

    let mut plan = topology_apply_plan(&[0, 1]);
    let enabled = plan.heads[0].head;
    let disabled = plan.heads[1].head;
    plan.heads[1].disposition = Disposition::Disabled;
    let mut resources = Cohort::<String, String>::new(&plan).unwrap();

    assert_eq!(
        resources
            .prepare_candidate_enabled(enabled, "candidate-enabled".into())
            .unwrap(),
        Transition::Accepted
    );
    assert_eq!(
        resources
            .prepare_candidate_disabled(disabled, "candidate-disabled".into())
            .unwrap(),
        Transition::Accepted
    );
    assert!(!resources.ready(), "candidate coverage alone must not permit apply");
    assert_eq!(
        resources
            .prepare_rollback(enabled, "rollback-enabled".into())
            .unwrap(),
        Transition::Accepted
    );
    assert!(!resources.ready());
    assert_eq!(
        resources
            .prepare_rollback(disabled, "rollback-disabled".into())
            .unwrap(),
        Transition::Ready
    );
    assert!(resources.ready());
    assert_eq!(resources.card_heads(0), vec![enabled]);
    assert_eq!(resources.card_heads(1), vec![disabled]);
    assert!(matches!(resources.candidate(enabled), Some(Candidate::Enabled(_))));
    assert!(matches!(
        resources.candidate(disabled),
        Some(Candidate::Disabled(_))
    ));

    let rejection = resources
        .prepare_candidate_enabled(enabled, "duplicate".into())
        .unwrap_err();
    assert_eq!(rejection.transition, Transition::Duplicate);
    assert_eq!(rejection.owner, "duplicate");
    let rejection = resources
        .prepare_candidate_enabled(disabled, "wrong-role".into())
        .unwrap_err();
    assert_eq!(rejection.transition, Transition::WrongDisposition);
    assert_eq!(rejection.owner, "wrong-role");

    let (candidate, rollback) = resources.into_remaining();
    assert_eq!(candidate.len(), 2);
    assert_eq!(rollback.len(), 2);
}
