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
