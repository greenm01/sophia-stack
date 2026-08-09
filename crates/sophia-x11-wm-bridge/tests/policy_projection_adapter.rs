use sophia_engine::{PolicyProjectionReducer, WmWorkspaceState, adapt_v7_policy_plan};
use sophia_protocol::{
    LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot, LayoutNodeState, OutputId,
    PolicyOutputSnapshot, PolicyPresentationState, PolicyProjectionOutcome, PolicySceneSnapshot,
    PolicySurfaceKind, PolicySurfaceSnapshot, Rect, SOPHIA_WM_V1_BEHAVIOR_SCENARIOS,
    SurfaceConstraints, SurfaceId, TransactionId, WmRelayoutWorkspace, WmRequestKind,
    WmRequestPacket, WmResponsePacket, WorkspaceId, sophia_wm_v1_behavior_cause,
    sophia_wm_v1_behavior_scene,
};
use sophia_x11_wm_bridge::{LegacyWmRequest, X11WmBridgeState};

#[test]
fn legacy_x11_geometry_commits_through_the_canonical_projection_reducer() {
    let output = OutputId::from_raw(1);
    let workspace = WorkspaceId::from_raw(1);
    let bounds = rect(0, 0, 1200, 800);
    let nodes = vec![node(10), node(11)];
    let request_packet = WmRequestPacket {
        transaction: TransactionId::from_raw(71),
        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
            output,
            workspace,
            bounds,
            nodes: nodes.clone(),
        }),
    };
    let mut bridge = X11WmBridgeState::new();
    bridge.apply_engine_request(&request_packet).unwrap();
    let left = bridge.synthetic_window(nodes[0].surface).unwrap();
    let right = bridge.synthetic_window(nodes[1].surface).unwrap();
    let response = bridge
        .translate_legacy_requests_for_output(
            request_packet.transaction,
            &[
                LegacyWmRequest::ConfigureWindow {
                    window: left,
                    geometry: rect(0, 0, 600, 800),
                    z_index: 0,
                },
                LegacyWmRequest::ConfigureWindow {
                    window: right,
                    geometry: rect(600, 0, 600, 800),
                    z_index: 1,
                },
                LegacyWmRequest::FocusWindow { window: left },
            ],
            300,
            Some(output),
        )
        .unwrap();

    let mut workspace_state = WmWorkspaceState::new([(output, bounds)], 1).unwrap();
    for node in &nodes {
        workspace_state
            .register_surface(node.surface, workspace)
            .unwrap();
    }
    let plan = workspace_state.plan_response(&response, &[]).unwrap();
    let scene = PolicySceneSnapshot {
        generation: 1,
        active_output: output,
        outputs: vec![PolicyOutputSnapshot {
            output,
            generation: 1,
            focus: None,
            bounds,
            work_area: bounds,
        }],
        surfaces: nodes
            .iter()
            .map(|node| PolicySurfaceSnapshot {
                surface: node.surface,
                generation: node.generation,
                current_output: Some(output),
                kind: PolicySurfaceKind::Toplevel,
                capabilities: node.capabilities,
                constraints: node.constraints,
                exact_size: None,
                requested_state: PolicyPresentationState::default(),
                current_state: PolicyPresentationState::default(),
                transient_owner: node.transient_owner,
                geometry: node.geometry,
            })
            .collect(),
        session_operations: Vec::new(),
    };
    let mut reducer = PolicyProjectionReducer::new(scene.clone()).unwrap();
    reducer.connect(1).unwrap();
    let request = reducer.issue_request(vec![output]).unwrap();
    let proposal = adapt_v7_policy_plan(&request, &scene, &plan).unwrap();

    assert_eq!(
        reducer.apply_proposal(&proposal),
        PolicyProjectionOutcome::Committed
    );
    let committed = reducer.committed();
    assert_eq!(committed[0].placements.len(), 2);
    assert_eq!(committed[0].placements[1].geometry.x, 600);
    assert_eq!(committed[0].focus, Some(nodes[0].surface));
}

#[test]
fn legacy_x11_adapter_accepts_every_revision_one_behavior_scene() {
    for (index, scenario) in SOPHIA_WM_V1_BEHAVIOR_SCENARIOS.into_iter().enumerate() {
        let scene = sophia_wm_v1_behavior_scene(scenario).unwrap();
        let mut workspace_state = WmWorkspaceState::new(
            scene
                .outputs
                .iter()
                .map(|output| (output.output, output.work_area)),
            scene.outputs.len(),
        )
        .unwrap();
        let mut bridge = X11WmBridgeState::new();
        let transaction = TransactionId::from_raw(scene.generation + 100);
        let mut commands = Vec::new();

        for output in &scene.outputs {
            let workspace = workspace_state.output(output.output).unwrap().workspace;
            let surfaces = scene
                .surfaces
                .iter()
                .filter(|surface| surface.current_output == Some(output.output))
                .collect::<Vec<_>>();
            let nodes = surfaces
                .iter()
                .map(|surface| corpus_node(surface, workspace))
                .collect::<Vec<_>>();
            for node in &nodes {
                workspace_state
                    .register_surface(node.surface, workspace)
                    .unwrap();
            }
            bridge
                .apply_engine_request(&WmRequestPacket {
                    transaction,
                    kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                        output: output.output,
                        workspace,
                        bounds: output.work_area,
                        nodes: nodes.clone(),
                    }),
                })
                .unwrap();
            let count = i32::try_from(nodes.len()).unwrap();
            let width = output.work_area.width / count;
            let mut requests = nodes
                .iter()
                .enumerate()
                .map(|(index, node)| {
                    let index = i32::try_from(index).unwrap();
                    let x = output.work_area.x + index * width;
                    LegacyWmRequest::ConfigureWindow {
                        window: bridge.synthetic_window(node.surface).unwrap(),
                        geometry: rect(
                            x,
                            output.work_area.y,
                            if index + 1 == count {
                                output.work_area.x + output.work_area.width - x
                            } else {
                                width
                            },
                            output.work_area.height,
                        ),
                        z_index: index,
                    }
                })
                .collect::<Vec<_>>();
            if output.output == scene.active_output
                && let Some(focus) = output.focus
            {
                requests.push(LegacyWmRequest::FocusWindow {
                    window: bridge.synthetic_window(focus).unwrap(),
                });
            }
            commands.extend(
                bridge
                    .translate_legacy_requests_for_output(
                        transaction,
                        &requests,
                        300,
                        Some(output.output),
                    )
                    .unwrap()
                    .commands,
            );
        }

        let response = WmResponsePacket {
            transaction,
            commands,
            timeout_msec: 300,
        };
        let plan = workspace_state.plan_response(&response, &[]).unwrap();
        let mut reducer = PolicyProjectionReducer::new(scene.clone()).unwrap();
        reducer.connect(1).unwrap();
        let mut affected = scene
            .outputs
            .iter()
            .map(|output| output.output)
            .collect::<Vec<_>>();
        affected.sort_by_key(|output| (*output != scene.active_output, output.raw()));
        let request = reducer
            .issue_request_with_cause(affected, sophia_wm_v1_behavior_cause(scenario).unwrap())
            .unwrap();
        let proposal = adapt_v7_policy_plan(&request, &scene, &plan).unwrap();
        let outcome = match scenario {
            "timeout-discard" => reducer.timeout(proposal.request_id),
            "stale-discard" => {
                reducer
                    .observe_scene(
                        sophia_wm_v1_behavior_scene(SOPHIA_WM_V1_BEHAVIOR_SCENARIOS[index + 1])
                            .unwrap(),
                    )
                    .unwrap();
                reducer.apply_proposal(&proposal)
            }
            "invalid-discard" => {
                let mut invalid = proposal.clone();
                invalid.active_output = OutputId::from_raw(0);
                reducer.apply_proposal(&invalid)
            }
            _ => reducer.apply_proposal(&proposal),
        };
        assert_eq!(
            outcome,
            match scenario {
                "timeout-discard" => PolicyProjectionOutcome::TimedOut,
                "stale-discard" => PolicyProjectionOutcome::RejectedStale,
                "invalid-discard" => PolicyProjectionOutcome::RejectedInvalid,
                _ => PolicyProjectionOutcome::Committed,
            },
            "legacy adapter rejected behavior scenario {scenario}",
        );
    }
}

fn corpus_node(surface: &PolicySurfaceSnapshot, workspace: WorkspaceId) -> LayoutNodeSnapshot {
    LayoutNodeSnapshot {
        surface: surface.surface,
        workspace,
        kind: LayoutNodeKind::Toplevel,
        placement_preference: sophia_protocol::SurfacePlacementPreference::Default,
        transient_owner: surface.transient_owner,
        capabilities: surface.capabilities,
        state: LayoutNodeState::NORMAL,
        constraints: surface.constraints,
        geometry: surface.geometry,
        generation: surface.generation,
    }
}

fn node(index: u32) -> LayoutNodeSnapshot {
    LayoutNodeSnapshot {
        surface: SurfaceId::new(index, 1),
        workspace: WorkspaceId::from_raw(1),
        kind: LayoutNodeKind::Toplevel,
        placement_preference: sophia_protocol::SurfacePlacementPreference::Default,
        transient_owner: None,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        state: LayoutNodeState::NORMAL,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        geometry: rect(0, 0, 600, 800),
        generation: 1,
    }
}

const fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}
