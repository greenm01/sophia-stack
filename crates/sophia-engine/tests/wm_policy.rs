use sophia_engine::{
    WmPolicyApplyOutcome, WmPolicyError, WmShortcutDecision, WmShortcutRegistry, WmShortcutRouter,
    WmWorkspaceState,
};
use sophia_protocol::{
    OutputId, Rect, SeatId, SurfaceId, TransactionId, WM_API_VERSION, WmActionId,
    WmBindingRegistration, WmCapabilities, WmCommand, WmHello, WmModifierMask, WmPolicyAckOutcome,
    WmPolicyUpdate, WmResponsePacket, WmSessionAction, WorkspaceId,
};

fn bounds(x: i32) -> Rect {
    Rect {
        x,
        y: 0,
        width: 1280,
        height: 720,
    }
}

#[test]
fn output_point_lookup_uses_half_open_multi_output_bounds() {
    let output_one = OutputId::from_raw(1);
    let output_two = OutputId::from_raw(2);
    let state =
        WmWorkspaceState::new([(output_one, bounds(0)), (output_two, bounds(1280))], 9).unwrap();

    assert_eq!(state.output_at_point(0, 0), Some(output_one));
    assert_eq!(state.output_at_point(1279, 719), Some(output_one));
    assert_eq!(state.output_at_point(1280, 0), Some(output_two));
    assert_eq!(state.output_at_point(2559, 719), Some(output_two));
    assert_eq!(state.output_at_point(2560, 0), None);
    assert_eq!(state.output_at_point(1280, 720), None);
}

#[test]
fn chrome_capability_is_explicit_and_blocks_unadvertised_policy_changes() {
    let chrome = sophia_protocol::WmChromePolicy::default();
    let registry = WmShortcutRegistry::from_hello(&WmHello {
        api_version: WM_API_VERSION,
        capabilities: WmCapabilities {
            bits: WmCapabilities::BINDINGS
                | WmCapabilities::WORKSPACES
                | WmCapabilities::SESSION_ACTIONS,
        },
        policy_generation: 1,
        chrome,
        bindings: Vec::new(),
    })
    .unwrap();
    assert!(!registry.supports_chrome_policy());
    let mut router = WmShortcutRouter::new(registry);
    let mut changed = chrome;
    changed.focus_ring.width = 6;

    assert_eq!(
        router.apply_policy_update(&WmPolicyUpdate {
            api_version: WM_API_VERSION,
            generation: 2,
            bindings: Vec::new(),
            chrome: changed,
        }),
        WmPolicyApplyOutcome::Acknowledged(sophia_protocol::WmPolicyAck {
            generation: 2,
            outcome: WmPolicyAckOutcome::RejectedInvalid,
        })
    );
}

#[test]
fn physical_shortcut_router_tracks_super_per_seat_and_suppresses_repeats() {
    let action = WmActionId::from_raw(7);
    let registry = WmShortcutRegistry::from_hello(&WmHello {
        api_version: WM_API_VERSION,
        capabilities: WmCapabilities::all_supported(),
        policy_generation: 1,
        chrome: sophia_protocol::WmChromePolicy::default(),
        bindings: vec![WmBindingRegistration {
            action,
            keycode: 36,
            modifiers: WmModifierMask {
                bits: WmModifierMask::SUPER,
            },
        }],
    })
    .unwrap();
    let mut router = WmShortcutRouter::new(registry);
    let seat = SeatId::from_raw(1);

    assert!(!router.route_key(seat, 125, true).consumed);
    assert_eq!(
        router.route_key(seat, 36, true),
        WmShortcutDecision {
            action: Some(action),
            consumed: true,
        }
    );
    assert_eq!(
        router.route_key(seat, 36, true),
        WmShortcutDecision {
            action: None,
            consumed: true,
        }
    );
    assert!(!router.route_key(seat, 125, false).consumed);
    assert_eq!(
        router.route_key(seat, 36, false),
        WmShortcutDecision {
            action: None,
            consumed: true,
        }
    );
}

#[test]
fn workspace_activation_swaps_visible_workspaces_without_mutating_source() {
    let output_one = OutputId::from_raw(1);
    let output_two = OutputId::from_raw(2);
    let state =
        WmWorkspaceState::new([(output_one, bounds(0)), (output_two, bounds(1280))], 9).unwrap();
    let response = WmResponsePacket {
        transaction: TransactionId::from_raw(9),
        commands: vec![WmCommand::ActivateWorkspace {
            output: output_one,
            workspace: WorkspaceId::from_raw(2),
        }],
        timeout_msec: 300,
    };

    let plan = state.plan_response(&response, &[]).unwrap();

    assert_eq!(
        state.output(output_one).unwrap().workspace,
        WorkspaceId::from_raw(1)
    );
    assert_eq!(
        plan.candidate.output(output_one).unwrap().workspace,
        WorkspaceId::from_raw(2)
    );
    assert_eq!(
        plan.candidate.output(output_two).unwrap().workspace,
        WorkspaceId::from_raw(1)
    );
    assert_eq!(plan.affected_outputs, vec![output_one, output_two]);
}

#[test]
fn output_bounds_update_preserves_workspace_and_focus_policy() {
    let output = OutputId::from_raw(1);
    let surface = SurfaceId::new(5, 1);
    let mut state = WmWorkspaceState::new([(output, bounds(0))], 9).unwrap();
    state
        .register_surface(surface, WorkspaceId::from_raw(1))
        .unwrap();
    state = state
        .plan_response(
            &WmResponsePacket {
                transaction: TransactionId::from_raw(10),
                commands: vec![WmCommand::FocusSurface(surface)],
                timeout_msec: 300,
            },
            &[],
        )
        .unwrap()
        .candidate;
    let work_area = Rect {
        x: 0,
        y: 28,
        width: 1280,
        height: 692,
    };

    assert!(state.update_output_bounds(output, work_area).unwrap());
    assert!(!state.update_output_bounds(output, work_area).unwrap());
    assert_eq!(state.output(output).unwrap().bounds, work_area);
    assert_eq!(
        state.output(output).unwrap().workspace,
        WorkspaceId::from_raw(1)
    );
    assert_eq!(state.output(output).unwrap().focus, Some(surface));
}

#[test]
fn floating_state_is_transactional_and_survives_unrelated_policy_updates() {
    let output = OutputId::from_raw(1);
    let surface = SurfaceId::new(50, 1);
    let mut state = WmWorkspaceState::new([(output, bounds(0))], 9).unwrap();
    state
        .register_surface(surface, WorkspaceId::from_raw(1))
        .unwrap();

    state = state
        .plan_response(
            &WmResponsePacket {
                transaction: TransactionId::from_raw(11),
                commands: vec![WmCommand::SetFloating {
                    surface,
                    floating: true,
                }],
                timeout_msec: 300,
            },
            &[],
        )
        .unwrap()
        .candidate;
    assert!(state.surface_floating(surface));

    let next = state
        .plan_response(
            &WmResponsePacket {
                transaction: TransactionId::from_raw(12),
                commands: vec![WmCommand::FocusSurface(surface)],
                timeout_msec: 300,
            },
            &[],
        )
        .unwrap()
        .candidate;
    assert!(next.surface_floating(surface));
}

#[test]
fn output_bounds_update_rejects_unknown_or_empty_outputs() {
    let output = OutputId::from_raw(1);
    let mut state = WmWorkspaceState::new([(output, bounds(0))], 9).unwrap();

    assert_eq!(
        state.update_output_bounds(OutputId::from_raw(2), bounds(0)),
        Err(WmPolicyError::UnknownOutput)
    );
    assert_eq!(
        state.update_output_bounds(
            output,
            Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 720,
            }
        ),
        Err(WmPolicyError::InvalidOutputBounds)
    );
}

#[test]
fn copying_output_bounds_preserves_candidate_workspace_and_focus() {
    let output = OutputId::from_raw(1);
    let surface = SurfaceId::new(6, 1);
    let mut current = WmWorkspaceState::new([(output, bounds(0))], 9).unwrap();
    current
        .update_output_bounds(
            output,
            Rect {
                x: 0,
                y: 28,
                width: 1280,
                height: 692,
            },
        )
        .unwrap();
    let mut candidate = WmWorkspaceState::new([(output, bounds(0))], 9).unwrap();
    candidate
        .register_surface(surface, WorkspaceId::from_raw(1))
        .unwrap();
    candidate = candidate
        .plan_response(
            &WmResponsePacket {
                transaction: TransactionId::from_raw(13),
                commands: vec![WmCommand::FocusSurface(surface)],
                timeout_msec: 300,
            },
            &[],
        )
        .unwrap()
        .candidate;

    assert!(candidate.copy_output_bounds_from(&current).unwrap());
    assert_eq!(
        candidate.output(output).unwrap().bounds,
        current.output(output).unwrap().bounds
    );
    assert_eq!(candidate.output(output).unwrap().focus, Some(surface));
    assert_eq!(
        candidate.surface_workspace(surface),
        Some(WorkspaceId::from_raw(1))
    );
}

#[test]
fn workspace_activation_restores_focus_owned_by_each_workspace() {
    let output = OutputId::from_raw(1);
    let surface = SurfaceId::new(5, 1);
    let mut state = WmWorkspaceState::new([(output, bounds(0))], 9).unwrap();
    state
        .register_surface(surface, WorkspaceId::from_raw(1))
        .unwrap();
    let focus = WmResponsePacket {
        transaction: TransactionId::from_raw(10),
        commands: vec![WmCommand::FocusSurface(surface)],
        timeout_msec: 300,
    };
    state = state.plan_response(&focus, &[]).unwrap().candidate;
    assert_eq!(state.output(output).unwrap().focus, Some(surface));

    let hide = WmResponsePacket {
        transaction: TransactionId::from_raw(11),
        commands: vec![WmCommand::ActivateWorkspace {
            output,
            workspace: WorkspaceId::from_raw(2),
        }],
        timeout_msec: 300,
    };
    state = state.plan_response(&hide, &[]).unwrap().candidate;
    assert_eq!(state.output(output).unwrap().focus, None);

    let restore = WmResponsePacket {
        transaction: TransactionId::from_raw(12),
        commands: vec![WmCommand::ActivateWorkspace {
            output,
            workspace: WorkspaceId::from_raw(1),
        }],
        timeout_msec: 300,
    };
    state = state.plan_response(&restore, &[]).unwrap().candidate;
    assert_eq!(state.output(output).unwrap().focus, Some(surface));
}

#[test]
fn workspace_plan_moves_focus_and_validates_named_actions_atomically() {
    let output = OutputId::from_raw(1);
    let surface = SurfaceId::new(4, 1);
    let mut state = WmWorkspaceState::new([(output, bounds(0))], 9).unwrap();
    state
        .register_surface(surface, WorkspaceId::from_raw(1))
        .unwrap();
    let response = WmResponsePacket {
        transaction: TransactionId::from_raw(10),
        commands: vec![
            WmCommand::AssignWorkspace {
                surface,
                workspace: WorkspaceId::from_raw(2),
            },
            WmCommand::RequestSessionAction {
                action: WmSessionAction::LaunchApplication {
                    application: sophia_protocol::SessionApplicationId::from_raw(1),
                },
                target: None,
            },
        ],
        timeout_msec: 300,
    };

    let plan = state
        .plan_response(
            &response,
            &[WmSessionAction::LaunchApplication {
                application: sophia_protocol::SessionApplicationId::from_raw(1),
            }],
        )
        .unwrap();

    assert_eq!(
        plan.candidate.surface_workspace(surface),
        Some(WorkspaceId::from_raw(2))
    );
    assert_eq!(
        plan.session_action,
        Some((
            WmSessionAction::LaunchApplication {
                application: sophia_protocol::SessionApplicationId::from_raw(1),
            },
            None,
        ))
    );
    assert_eq!(
        state.surface_workspace(surface),
        Some(WorkspaceId::from_raw(1))
    );
}

#[test]
fn workspace_plan_rejects_unadvertised_or_duplicate_side_effects() {
    let output = OutputId::from_raw(1);
    let state = WmWorkspaceState::new([(output, bounds(0))], 9).unwrap();
    let unadvertised = WmResponsePacket {
        transaction: TransactionId::from_raw(11),
        commands: vec![WmCommand::RequestSessionAction {
            action: WmSessionAction::LaunchApplication {
                application: sophia_protocol::SessionApplicationId::from_raw(2),
            },
            target: None,
        }],
        timeout_msec: 300,
    };
    assert_eq!(
        state.plan_response(&unadvertised, &[]),
        Err(WmPolicyError::UnadvertisedSessionAction)
    );

    let duplicate = WmResponsePacket {
        transaction: TransactionId::from_raw(12),
        commands: vec![
            WmCommand::ActivateWorkspace {
                output,
                workspace: WorkspaceId::from_raw(2),
            },
            WmCommand::ActivateWorkspace {
                output,
                workspace: WorkspaceId::from_raw(3),
            },
        ],
        timeout_msec: 300,
    };
    assert_eq!(
        state.plan_response(&duplicate, &[]),
        Err(WmPolicyError::DuplicateOutputCommand)
    );
}
