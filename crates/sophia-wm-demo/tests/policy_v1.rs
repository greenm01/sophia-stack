use sophia_protocol::{
    LayoutNodeCapabilities, OutputId, PolicyOutputSnapshot, PolicyPresentationState,
    PolicyProjectionRequest, PolicyRequestCause, PolicySceneSnapshot, PolicySurfaceKind,
    PolicySurfaceSnapshot, Rect, Size, SurfaceConstraints, SurfaceId, TransactionId,
};
use sophia_wm_demo::tile_policy_scene;

#[test]
fn reference_policy_tiles_only_the_complete_affected_output() {
    let output = OutputId::from_raw(1);
    let scene = PolicySceneSnapshot {
        generation: 4,
        active_output: output,
        outputs: vec![PolicyOutputSnapshot {
            output,
            generation: 1,
            focus: Some(SurfaceId::new(1, 1)),
            bounds: Rect {
                x: 10,
                y: 20,
                width: 101,
                height: 80,
            },
            work_area: Rect {
                x: 10,
                y: 20,
                width: 101,
                height: 80,
            },
        }],
        surfaces: vec![
            surface(1, Some(output)),
            surface(2, Some(output)),
            surface(3, None),
        ],
        session_operations: Vec::new(),
    };
    let request = PolicyProjectionRequest {
        connection_epoch: 7,
        request_id: 8,
        scene_generation: 4,
        policy_generation: 1,
        affected_outputs: vec![output],
        cause: PolicyRequestCause::SceneChanged,
    };

    let proposal = tile_policy_scene(TransactionId::from_raw(9), &scene, &request).unwrap();

    assert_eq!(proposal.outputs.len(), 1);
    assert_eq!(proposal.outputs[0].placements.len(), 2);
    assert_eq!(proposal.outputs[0].placements[0].geometry.width, 50);
    assert_eq!(proposal.outputs[0].placements[1].geometry.width, 51);
    assert_eq!(proposal.outputs[0].focus, Some(SurfaceId::new(1, 1)));
}

fn surface(index: u32, current_output: Option<OutputId>) -> PolicySurfaceSnapshot {
    PolicySurfaceSnapshot {
        surface: SurfaceId::new(index, 1),
        generation: 1,
        current_output,
        kind: PolicySurfaceKind::Toplevel,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        constraints: SurfaceConstraints {
            min_size: Some(Size {
                width: 10,
                height: 10,
            }),
            max_size: None,
        },
        exact_size: None,
        requested_state: PolicyPresentationState::default(),
        current_state: PolicyPresentationState::default(),
        transient_owner: None,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 40,
        },
    }
}
