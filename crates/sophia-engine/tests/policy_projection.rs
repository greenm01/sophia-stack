use sophia_engine::{PolicyProjectionReducer, WmWorkspaceState, adapt_v7_policy_plan};
use sophia_protocol::{
    LayoutNodeCapabilities, OutputId, PolicyOutputProjection, PolicyOutputSnapshot,
    PolicyPresentationState, PolicyProjectionIndicator, PolicyProjectionOutcome,
    PolicyProjectionOutputStatus, PolicyProjectionProposal, PolicyRequestCause,
    PolicySceneSnapshot, PolicySurfaceKind, PolicySurfacePlacement, PolicySurfaceSnapshot,
    PolicyTransform, Rect, SurfaceConstraints, SurfaceId, TransactionId, Transform, WmActionId,
    WmCommand, WmResponsePacket, WorkspaceId,
};

#[test]
fn duplicate_surface_across_outputs_rejects_without_partial_commit() {
    let scene = scene(1, &[surface(1), surface(2)]);
    let mut reducer = PolicyProjectionReducer::new(scene).unwrap();
    reducer.connect(1).unwrap();
    let request = reducer.issue_request(vec![output(1), output(2)]).unwrap();
    let proposal = proposal(
        &request,
        7,
        vec![
            projected(output(1), vec![placed(surface_id(1), 1, rect(0, 0))], None),
            projected(
                output(2),
                vec![placed(surface_id(1), 1, rect(100, 0))],
                None,
            ),
        ],
    );

    assert_eq!(
        reducer.apply_proposal(&proposal),
        PolicyProjectionOutcome::RejectedInvalid
    );
    assert_eq!(reducer.commit_serial(), 0);
    assert!(
        reducer
            .committed()
            .iter()
            .all(|projection| projection.placements.is_empty())
    );
}

#[test]
fn one_proposal_replaces_both_outputs_atomically() {
    let scene = scene(1, &[surface(1), surface(2)]);
    let mut reducer = PolicyProjectionReducer::new(scene).unwrap();
    reducer.connect(1).unwrap();
    let request = reducer.issue_request(vec![output(1), output(2)]).unwrap();
    let proposal = proposal(
        &request,
        8,
        vec![
            projected(
                output(1),
                vec![placed(surface_id(1), 1, rect(0, 0))],
                Some(surface_id(1)),
            ),
            projected(
                output(2),
                vec![placed(surface_id(2), 1, rect(100, 0))],
                Some(surface_id(2)),
            ),
        ],
    );

    assert_eq!(
        reducer.apply_proposal(&proposal),
        PolicyProjectionOutcome::Committed
    );
    assert_eq!(reducer.commit_serial(), 1);
    assert_eq!(
        reducer
            .committed()
            .iter()
            .map(|projection| projection.placements.len())
            .collect::<Vec<_>>(),
        vec![1, 1]
    );
}

#[test]
fn staged_projection_preserves_last_good_until_frontend_commit() {
    let scene = scene(1, &[surface(1)]);
    let mut reducer = PolicyProjectionReducer::new(scene).unwrap();
    reducer.connect(1).unwrap();
    let request = reducer.issue_request(vec![output(1)]).unwrap();
    let proposal = proposal(
        &request,
        8,
        vec![projected(
            output(1),
            vec![placed(surface_id(1), 1, rect(0, 0))],
            Some(surface_id(1)),
        )],
    );

    let staged = reducer.stage_proposal(&proposal).unwrap();
    assert_eq!(reducer.commit_serial(), 0);
    assert!(reducer.committed()[0].placements.is_empty());
    assert_eq!(
        reducer.commit_staged(staged),
        PolicyProjectionOutcome::Committed
    );
    assert_eq!(reducer.commit_serial(), 1);
    assert_eq!(reducer.committed()[0].placements.len(), 1);
}

#[test]
fn frontend_fact_change_invalidates_a_staged_projection() {
    let mut reducer = PolicyProjectionReducer::new(scene(1, &[surface(1)])).unwrap();
    reducer.connect(1).unwrap();
    let request = reducer.issue_request(vec![output(1)]).unwrap();
    let proposal = proposal(
        &request,
        8,
        vec![projected(
            output(1),
            vec![placed(surface_id(1), 1, rect(0, 0))],
            Some(surface_id(1)),
        )],
    );
    let staged = reducer.stage_proposal(&proposal).unwrap();
    reducer.observe_scene(scene(2, &[surface(1)])).unwrap();

    assert_eq!(
        reducer.commit_staged(staged),
        PolicyProjectionOutcome::RejectedStale
    );
    assert_eq!(reducer.commit_serial(), 0);
    assert!(reducer.committed()[0].placements.is_empty());
    assert!(
        reducer.issue_request(vec![output(1)]).is_ok(),
        "the exact stale request must be retired so fresh policy work can proceed"
    );
}

#[test]
fn guessed_future_generation_cannot_become_current() {
    let mut reducer = PolicyProjectionReducer::new(scene(1, &[surface(1)])).unwrap();
    reducer.connect(1).unwrap();
    let request = reducer.issue_request(vec![output(1)]).unwrap();
    reducer.observe_scene(scene(2, &[surface(1)])).unwrap();
    let mut guessed = proposal(
        &request,
        9,
        vec![projected(
            output(1),
            vec![placed(surface_id(1), 1, rect(0, 0))],
            None,
        )],
    );
    guessed.base_generation = 2;

    assert_eq!(
        reducer.apply_proposal(&guessed),
        PolicyProjectionOutcome::RejectedStale
    );
    assert_eq!(reducer.commit_serial(), 0);
}

#[test]
fn disconnect_preserves_layout_and_scene_removal_prunes_only_dead_surface() {
    let mut reducer = PolicyProjectionReducer::new(scene(1, &[surface(1), surface(2)])).unwrap();
    reducer.connect(3).unwrap();
    let request = reducer.issue_request(vec![output(1)]).unwrap();
    let proposal = proposal(
        &request,
        10,
        vec![projected(
            output(1),
            vec![
                placed(surface_id(1), 1, rect(0, 0)),
                placed(surface_id(2), 1, rect(50, 0)),
            ],
            Some(surface_id(2)),
        )],
    );
    assert_eq!(
        reducer.apply_proposal(&proposal),
        PolicyProjectionOutcome::Committed
    );
    let committed = reducer.committed();
    assert_eq!(reducer.disconnect(3), PolicyProjectionOutcome::Disconnected);
    assert_eq!(reducer.committed(), committed);

    reducer.observe_scene(scene(2, &[surface(1)])).unwrap();
    let first = reducer
        .committed()
        .into_iter()
        .find(|projection| projection.output == output(1))
        .unwrap();
    assert_eq!(first.placements.len(), 1);
    assert_eq!(first.placements[0].surface, surface_id(1));
    assert_eq!(first.focus, None);
}

#[test]
fn descriptors_commit_atomically_and_clear_at_the_connection_boundary() {
    let mut reducer = PolicyProjectionReducer::new(scene(1, &[surface(1)])).unwrap();
    reducer.connect(5).unwrap();
    let request = reducer.issue_request(vec![output(1)]).unwrap();
    let mut candidate = proposal(&request, 44, vec![projected(output(1), Vec::new(), None)]);
    candidate.indicators.push(PolicyProjectionIndicator {
        output: output(1),
        slot: 0,
        indicator: 9,
        action: Some(WmActionId::from_raw(11)),
        state_bits: 1,
        label: "1".into(),
    });
    candidate
        .output_statuses
        .push(PolicyProjectionOutputStatus {
            output: output(1),
            focus_bits: 0,
            layout: "Scroller".into(),
        });

    assert_eq!(
        reducer.apply_proposal(&candidate),
        PolicyProjectionOutcome::Committed
    );
    let committed = reducer.indicator_publication();
    assert_eq!(committed.connection_epoch, Some(5));
    assert_eq!(committed.projection_commit_serial, 1);
    assert_eq!(committed.indicators[0].label, "1");

    assert_eq!(reducer.disconnect(5), PolicyProjectionOutcome::Disconnected);
    let cleared = reducer.indicator_publication();
    assert_eq!(cleared.connection_epoch, None);
    assert!(cleared.indicators.is_empty());
    assert!(cleared.output_statuses.is_empty());
    assert!(cleared.generation > committed.generation);
    assert_eq!(reducer.commit_serial(), 1);
}

#[test]
fn invalid_descriptor_preserves_the_last_good_projection_and_publication() {
    let mut reducer = PolicyProjectionReducer::new(scene(1, &[surface(1)])).unwrap();
    reducer.connect(1).unwrap();
    let request = reducer.issue_request(vec![output(1)]).unwrap();
    let mut invalid = proposal(&request, 45, vec![projected(output(1), Vec::new(), None)]);
    invalid.indicators.push(PolicyProjectionIndicator {
        output: output(1),
        slot: 0,
        indicator: 0,
        action: None,
        state_bits: 0,
        label: "bad".into(),
    });
    let before = reducer.indicator_publication();

    assert_eq!(
        reducer.apply_proposal(&invalid),
        PolicyProjectionOutcome::RejectedInvalid
    );
    assert_eq!(reducer.indicator_publication(), before);
    assert_eq!(reducer.commit_serial(), 0);
}

#[test]
fn api_v7_workspace_plan_enters_the_same_projection_reducer() {
    let scene = scene(1, &[surface(1)]);
    let mut reducer = PolicyProjectionReducer::new(scene.clone()).unwrap();
    reducer.connect(1).unwrap();
    let request = reducer.issue_request(vec![output(1), output(2)]).unwrap();

    let workspace = WorkspaceId::from_raw(1);
    let mut state = WmWorkspaceState::new(
        [
            (
                output(1),
                Rect {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 100,
                },
            ),
            (
                output(2),
                Rect {
                    x: 100,
                    y: 0,
                    width: 100,
                    height: 100,
                },
            ),
        ],
        2,
    )
    .unwrap();
    state.register_surface(surface_id(1), workspace).unwrap();
    let response = WmResponsePacket {
        transaction: TransactionId::from_raw(12),
        commands: vec![
            WmCommand::RenderSurface(sophia_protocol::SurfacePlacement {
                surface: surface_id(1),
                geometry: rect(0, 0),
                z_index: 0,
                crop: None,
                transform: Transform::IDENTITY,
            }),
            WmCommand::FocusSurface(surface_id(1)),
        ],
        timeout_msec: 100,
    };
    let plan = state.plan_response(&response, &[]).unwrap();
    let proposal = adapt_v7_policy_plan(&request, &scene, &plan).unwrap();

    assert_eq!(
        reducer.apply_proposal(&proposal),
        PolicyProjectionOutcome::Committed
    );
    let committed = reducer.committed();
    assert_eq!(committed[0].focus, Some(surface_id(1)));
    assert!(committed[1].placements.is_empty());
}

#[test]
fn snapshot_membership_and_focus_are_the_initial_and_committed_truth() {
    let mut initial = scene(1, &[surface(1)]);
    initial.surfaces[0].current_output = Some(output(1));
    initial.outputs[0].focus = Some(surface_id(1));
    let mut reducer = PolicyProjectionReducer::new(initial).unwrap();
    assert_eq!(reducer.committed()[0].placements.len(), 1);
    assert_eq!(reducer.committed()[0].focus, Some(surface_id(1)));

    reducer.connect(1).unwrap();
    let request = reducer.issue_request(vec![output(1)]).unwrap();
    let hide = proposal(&request, 13, vec![projected(output(1), Vec::new(), None)]);
    assert_eq!(
        reducer.apply_proposal(&hide),
        PolicyProjectionOutcome::Committed
    );
    assert_eq!(reducer.scene().surfaces[0].current_output, None);
    assert_eq!(reducer.scene().outputs[0].focus, None);
}

#[test]
fn repeated_action_token_with_distinct_activation_serials_is_not_coalesced() {
    let mut reducer = PolicyProjectionReducer::new(scene(1, &[surface(1)])).unwrap();
    reducer.connect(1).unwrap();

    for activation_serial in [41, 42] {
        let request = reducer
            .issue_request_with_cause(
                vec![output(1)],
                PolicyRequestCause::Action {
                    activation_serial,
                    action: WmActionId::from_raw(7),
                },
            )
            .unwrap();
        assert_eq!(
            request.cause,
            PolicyRequestCause::Action {
                activation_serial,
                action: WmActionId::from_raw(7),
            }
        );
        assert_eq!(
            reducer.apply_proposal(&proposal(
                &request,
                activation_serial,
                vec![projected(output(1), Vec::new(), None)],
            )),
            PolicyProjectionOutcome::Committed
        );
    }

    assert_eq!(reducer.commit_serial(), 2);
}

#[test]
fn work_area_must_be_nonempty_and_inside_output_bounds() {
    let mut invalid = scene(1, &[surface(1)]);
    invalid.outputs[0].work_area = Rect {
        x: 0,
        y: 0,
        width: 101,
        height: 100,
    };

    assert!(PolicyProjectionReducer::new(invalid).is_err());
}

#[test]
fn committed_presentation_state_is_reflected_in_the_next_snapshot() {
    let mut reducer = PolicyProjectionReducer::new(scene(1, &[surface(1)])).unwrap();
    reducer.connect(1).unwrap();
    let request = reducer.issue_request(vec![output(1)]).unwrap();
    let mut placement = placed(surface_id(1), 1, rect(0, 0));
    placement.presentation.maximized = true;

    assert_eq!(
        reducer.apply_proposal(&proposal(
            &request,
            44,
            vec![projected(output(1), vec![placement], Some(surface_id(1)))],
        )),
        PolicyProjectionOutcome::Committed
    );
    assert!(reducer.scene().surfaces[0].current_state.maximized);
}

fn scene(generation: u64, surfaces: &[PolicySurfaceSnapshot]) -> PolicySceneSnapshot {
    PolicySceneSnapshot {
        generation,
        outputs: vec![
            PolicyOutputSnapshot {
                output: output(1),
                generation: 1,
                focus: None,
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 100,
                },
                work_area: Rect {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 100,
                },
            },
            PolicyOutputSnapshot {
                output: output(2),
                generation: 1,
                focus: None,
                bounds: Rect {
                    x: 100,
                    y: 0,
                    width: 100,
                    height: 100,
                },
                work_area: Rect {
                    x: 100,
                    y: 0,
                    width: 100,
                    height: 100,
                },
            },
        ],
        surfaces: surfaces.to_vec(),
        session_operations: Vec::new(),
    }
}

fn surface(raw: u32) -> PolicySurfaceSnapshot {
    PolicySurfaceSnapshot {
        surface: surface_id(raw),
        generation: 1,
        current_output: None,
        kind: PolicySurfaceKind::Toplevel,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        exact_size: None,
        requested_state: PolicyPresentationState::default(),
        current_state: PolicyPresentationState::default(),
        transient_owner: None,
        geometry: rect(0, 0),
    }
}

fn proposal(
    request: &sophia_protocol::PolicyProjectionRequest,
    transaction: u64,
    outputs: Vec<PolicyOutputProjection>,
) -> PolicyProjectionProposal {
    PolicyProjectionProposal {
        transaction: TransactionId::from_raw(transaction),
        connection_epoch: request.connection_epoch,
        request_id: request.request_id,
        base_generation: request.scene_generation,
        outputs,
        indicators: Vec::new(),
        output_statuses: Vec::new(),
    }
}

fn projected(
    output: OutputId,
    placements: Vec<PolicySurfacePlacement>,
    focus: Option<SurfaceId>,
) -> PolicyOutputProjection {
    PolicyOutputProjection {
        output,
        placements,
        focus,
    }
}

fn placed(surface: SurfaceId, generation: u64, geometry: Rect) -> PolicySurfacePlacement {
    PolicySurfacePlacement {
        surface,
        surface_generation: generation,
        geometry,
        requested_size: None,
        crop: None,
        transform: PolicyTransform::Identity,
        presentation: PolicyPresentationState::default(),
    }
}

fn output(raw: u64) -> OutputId {
    OutputId::from_raw(raw)
}

fn surface_id(index: u32) -> SurfaceId {
    SurfaceId::new(index, 1)
}

fn rect(x: i32, y: i32) -> Rect {
    Rect {
        x,
        y,
        width: 50,
        height: 50,
    }
}
