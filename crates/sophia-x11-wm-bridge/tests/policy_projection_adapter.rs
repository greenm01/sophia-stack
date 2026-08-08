use sophia_engine::{PolicyProjectionReducer, WmWorkspaceState, adapt_v7_policy_plan};
use sophia_protocol::{
    LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot, LayoutNodeState, OutputId,
    PolicyOutputSnapshot, PolicyPresentationState, PolicyProjectionOutcome, PolicySceneSnapshot,
    PolicySurfaceKind, PolicySurfaceSnapshot, Rect, SurfaceConstraints, SurfaceId, TransactionId,
    WmRelayoutWorkspace, WmRequestKind, WmRequestPacket, WorkspaceId,
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
