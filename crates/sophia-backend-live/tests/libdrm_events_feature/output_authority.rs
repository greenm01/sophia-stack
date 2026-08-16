fn authority_capability(
    head: u64,
    output: u64,
    connector: &str,
    selected: (u32, u32),
) -> LibdrmNativeOutputCapability {
    let selected = LibdrmNativeOutputTiming::new(selected.0, selected.1, 60_000);
    LibdrmNativeOutputCapability::new(
        sophia_protocol::OutputId::from_raw(output),
        u32::try_from(head).unwrap(),
        connector,
        [
            LibdrmNativeOutputTiming::new(2_560, 1_440, 60_000),
            LibdrmNativeOutputTiming::new(1_920, 1_080, 60_000),
        ],
        Some(selected),
        selected,
        LibdrmNativeVrrPropertyDiscoveryStatus::Discovered,
    )
    .unwrap()
    .bind_head(sophia_engine::RenderHeadId::from_raw(head))
    .unwrap()
}

#[test]
fn output_authority_resolves_independent_modes_with_mirror_and_extended_groups() {
    use sophia_protocol::{
        OutputGroupMember, OutputHeadMapping, OutputHeadTargetProposal,
        OutputLogicalGroupProposal, OutputTopologyCandidate, OutputTopologyIntent,
        OutputTransform, OutputVrrPolicy, Rect,
    };

    let capabilities = vec![
        authority_capability(11, 1, "DP-1", (2_560, 1_440)),
        authority_capability(12, 1, "DP-2", (1_920, 1_080)),
        authority_capability(13, 2, "HDMI-A-1", (1_920, 1_080)),
    ];
    let outputs = vec![
        sophia_engine::HeadlessOutput {
            id: sophia_protocol::OutputId::from_raw(1),
            size: sophia_protocol::Size {
                width: 2_560,
                height: 1_440,
            },
            scale: 1,
        },
        sophia_engine::HeadlessOutput {
            id: sophia_protocol::OutputId::from_raw(2),
            size: sophia_protocol::Size {
                width: 1_920,
                height: 1_080,
            },
            scale: 1,
        },
    ];
    let snapshot = sophia_backend_live::project_live_output_authority_snapshot(
        &capabilities,
        &outputs,
        7,
    )
    .unwrap();
    assert_eq!(snapshot.heads[0].label, "Display 1");
    assert!(!snapshot.heads[0].label.contains("DP-"));
    let mode = |head: u64, width: i32| {
        snapshot
            .heads
            .iter()
            .find(|descriptor| descriptor.head.raw() == head)
            .unwrap()
            .modes
            .iter()
            .find(|mode| mode.pixel_size.width == width)
            .unwrap()
            .mode
    };
    let candidate = OutputTopologyCandidate {
        base_topology_epoch: 7,
        intent: OutputTopologyIntent::Apply,
        primary_group_index: 0,
        heads: vec![
            OutputHeadTargetProposal {
                head: sophia_protocol::DisplayHeadId::from_raw(11),
                head_generation: 1,
                mode: mode(11, 2_560),
                transform: OutputTransform::Normal,
                vrr: OutputVrrPolicy::Automatic,
            },
            OutputHeadTargetProposal {
                head: sophia_protocol::DisplayHeadId::from_raw(12),
                head_generation: 1,
                mode: mode(12, 1_920),
                transform: OutputTransform::Normal,
                vrr: OutputVrrPolicy::Disabled,
            },
            OutputHeadTargetProposal {
                head: sophia_protocol::DisplayHeadId::from_raw(13),
                head_generation: 1,
                mode: mode(13, 1_920),
                transform: OutputTransform::Normal,
                vrr: OutputVrrPolicy::Disabled,
            },
        ],
        groups: vec![
            OutputLogicalGroupProposal {
                output: sophia_protocol::OutputId::from_raw(1),
                logical: Rect {
                    x: 0,
                    y: 0,
                    width: 2_560,
                    height: 1_440,
                },
                members: vec![
                    OutputGroupMember {
                        head: sophia_protocol::DisplayHeadId::from_raw(11),
                        mapping: OutputHeadMapping::Exact,
                    },
                    OutputGroupMember {
                        head: sophia_protocol::DisplayHeadId::from_raw(12),
                        mapping: OutputHeadMapping::Fit,
                    },
                ],
            },
            OutputLogicalGroupProposal {
                output: sophia_protocol::OutputId::from_raw(2),
                logical: Rect {
                    x: 2_560,
                    y: 0,
                    width: 1_920,
                    height: 1_080,
                },
                members: vec![OutputGroupMember {
                    head: sophia_protocol::DisplayHeadId::from_raw(13),
                    mapping: OutputHeadMapping::Exact,
                }],
            },
        ],
    };
    let mut allocator = sophia_backend_live::LiveLogicalOutputAllocator::after(
        outputs.iter().map(|output| output.id),
    )
    .unwrap();
    let resolved = sophia_backend_live::resolve_live_output_topology_candidate(
        &snapshot,
        &capabilities,
        &candidate,
        &mut allocator,
    )
    .unwrap();
    assert_eq!(resolved.outputs.len(), 2);
    assert_eq!(resolved.targets.len(), 3);
    assert_eq!(resolved.targets[0].timing.width, 2_560);
    assert_eq!(resolved.targets[1].timing.width, 1_920);
    assert_eq!(resolved.targets[0].output, resolved.targets[1].output);
    assert_ne!(resolved.targets[1].output, resolved.targets[2].output);
    assert!(resolved.mirror_grouping.is_mirrored("DP-1"));
    assert!(!resolved.mirror_grouping.is_mirrored("HDMI-A-1"));
    let render_targets = resolved.head_render_targets();
    assert_eq!(render_targets[1].native_size.width, 1_920);
    assert_eq!(render_targets[1].mapping, OutputHeadMapping::Fit);
    assert_eq!(render_targets[1].target_generation, 2);

    let mut disabled_candidate = candidate;
    disabled_candidate.heads.pop();
    disabled_candidate.groups.pop();
    let disabled = sophia_backend_live::resolve_live_output_topology_candidate(
        &snapshot,
        &capabilities,
        &disabled_candidate,
        &mut allocator,
    )
    .unwrap();
    assert_eq!(
        disabled.disabled_heads,
        vec![sophia_backend_live::LiveOutputAuthorityDisabledHead {
            head: sophia_engine::RenderHeadId::from_raw(13),
            target_generation: 2,
        }]
    );
    assert_eq!(disabled.affected_heads().count(), 3);
}

#[test]
fn output_authority_uses_ceil_scaled_logical_extents_and_rejects_unrepresentable_modes() {
    let capability = authority_capability(21, 3, "DP-9", (1_920, 1_080));
    let snapshot = sophia_backend_live::project_live_output_authority_snapshot(
        &[capability],
        &[sophia_engine::HeadlessOutput {
            id: sophia_protocol::OutputId::from_raw(3),
            size: sophia_protocol::Size {
                width: 1_919,
                height: 1_079,
            },
            scale: 2,
        }],
        2,
    )
    .unwrap();
    assert_eq!(snapshot.groups[0].logical.width, 960);
    assert_eq!(snapshot.groups[0].logical.height, 540);

    let huge = LibdrmNativeOutputTiming::new(i32::MAX as u32 + 1, 1_080, 60_000);
    let huge = LibdrmNativeOutputCapability::new(
        sophia_protocol::OutputId::from_raw(3),
        9,
        "DP-9",
        [huge],
        Some(huge),
        huge,
        LibdrmNativeVrrPropertyDiscoveryStatus::Discovered,
    )
    .unwrap()
    .bind_head(sophia_engine::RenderHeadId::from_raw(21))
    .unwrap();
    assert!(matches!(
        sophia_backend_live::project_live_output_authority_snapshot(
            &[huge],
            &[sophia_engine::HeadlessOutput {
                id: sophia_protocol::OutputId::from_raw(3),
                size: sophia_protocol::Size {
                    width: 1_920,
                    height: 1_080,
                },
                scale: 1,
            }],
            3,
        ),
        Err(sophia_backend_live::LiveOutputAuthorityProjectionError::ModeExtentUnsupported(head))
            if head.raw() == 21
    ));
}

#[test]
fn rejected_native_projection_does_not_consume_a_fresh_output_identity() {
    use sophia_protocol::{
        DisplayHeadId, OutputGroupMember, OutputHeadMapping, OutputHeadTargetProposal,
        OutputId, OutputLogicalGroupProposal, OutputTopologyCandidate, OutputTopologyIntent,
        OutputTransform, OutputVrrPolicy, Rect,
    };

    // Connector names are private native data and can collide across cards.
    // The current name-based mirror grouping rejects this late, after all
    // protocol validation and mode projection have succeeded.
    let capabilities = vec![
        authority_capability(31, 1, "DP-1", (2_560, 1_440)),
        authority_capability(32, 1, "DP-1", (1_920, 1_080)),
    ];
    let outputs = [sophia_engine::HeadlessOutput {
        id: OutputId::from_raw(1),
        size: sophia_protocol::Size {
            width: 2_560,
            height: 1_440,
        },
        scale: 1,
    }];
    let snapshot = sophia_backend_live::project_live_output_authority_snapshot(
        &capabilities,
        &outputs,
        9,
    )
    .unwrap();
    let candidate = OutputTopologyCandidate {
        base_topology_epoch: 9,
        intent: OutputTopologyIntent::Apply,
        primary_group_index: 0,
        heads: snapshot
            .heads
            .iter()
            .map(|head| OutputHeadTargetProposal {
                head: head.head,
                head_generation: head.generation,
                mode: head.current_mode.unwrap(),
                transform: OutputTransform::Normal,
                vrr: OutputVrrPolicy::Disabled,
            })
            .collect(),
        groups: vec![OutputLogicalGroupProposal {
            output: OutputId::INVALID,
            logical: Rect {
                x: 0,
                y: 0,
                width: 2_560,
                height: 1_440,
            },
            members: [31, 32]
                .into_iter()
                .map(|head| OutputGroupMember {
                    head: DisplayHeadId::from_raw(head),
                    mapping: OutputHeadMapping::Fit,
                })
                .collect(),
        }],
    };
    let mut allocator = sophia_backend_live::LiveLogicalOutputAllocator::after(
        outputs.iter().map(|output| output.id),
    )
    .unwrap();
    assert!(matches!(
        sophia_backend_live::resolve_live_output_topology_candidate(
            &snapshot,
            &capabilities,
            &candidate,
            &mut allocator,
        ),
        Err(sophia_backend_live::LiveOutputAuthorityProjectionError::InvalidMirrorGrouping(_))
    ));
    assert_eq!(allocator.mint(), Some(OutputId::from_raw(2)));
}
