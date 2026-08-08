use sophia_protocol::{
    LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot, LayoutNodeState, NamespaceId,
    OutputId, Rect, SessionApplicationId, Size, SurfaceConstraints, SurfaceId, TransactionId,
    WM_API_VERSION, WmActionActivation, WmActionId, WmCommand, WmFocusRequest, WmModifierMask,
    WmOutputWorkspace, WmRelayoutWorkspace, WmRequestKind, WmRequestPacket, WmSessionAction,
    WmSessionDescriptor, WorkspaceId,
};
use sophia_x11_wm_bridge::{
    LegacyWmProfile, LegacyWmRequest, SYNTHETIC_ROOT_XID, SyntheticXEvent, X11WmBridgeError,
    X11WmBridgeState, XMONAD_ACTION_APPLICATION_1, XMONAD_ACTION_APPLICATION_2,
    XMONAD_ACTION_APPLICATION_3, XMONAD_ACTION_TOGGLE_FLOATING, translate_xmonad_profile_action,
};

#[test]
fn compatibility_profiles_leave_compositor_chrome_to_engine_config() {
    let hello = LegacyWmProfile::Xmonad.hello();

    assert_eq!(
        hello.capabilities.bits & sophia_protocol::WmCapabilities::POLICY_CHROME_V2,
        0
    );
}

fn node(raw: u32) -> LayoutNodeSnapshot {
    LayoutNodeSnapshot {
        surface: SurfaceId::new(raw, 1),
        workspace: WorkspaceId::from_raw(1),
        kind: LayoutNodeKind::Toplevel,
        placement_preference: sophia_protocol::SurfacePlacementPreference::Default,
        transient_owner: None,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        state: LayoutNodeState::NORMAL,
        constraints: SurfaceConstraints {
            min_size: Some(Size {
                width: 100,
                height: 80,
            }),
            max_size: None,
        },
        geometry: Rect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        },
        generation: 1,
    }
}

#[test]
fn xmonad_launches_the_terminal_with_super_enter() {
    let hello = LegacyWmProfile::Xmonad.hello();
    let binding = hello
        .bindings
        .iter()
        .find(|binding| binding.action.raw() == XMONAD_ACTION_APPLICATION_1)
        .expect("xmonad terminal binding");
    assert_eq!(binding.keycode, 28);
    assert_eq!(binding.modifiers.bits, WmModifierMask::SUPER);
}

#[test]
fn xmonad_registers_super_shift_space_as_the_floating_toggle() {
    let binding = LegacyWmProfile::Xmonad
        .hello()
        .bindings
        .into_iter()
        .find(|binding| binding.action.raw() == XMONAD_ACTION_TOGGLE_FLOATING)
        .expect("xmonad floating toggle binding");

    assert_eq!(binding.keycode, 57);
    assert_eq!(
        binding.modifiers.bits,
        WmModifierMask::SUPER | WmModifierMask::SHIFT
    );
}

#[test]
fn hinted_dialog_remains_managed_and_exports_only_opaque_transient_facts() {
    let mut bridge = X11WmBridgeState::new();
    let owner = node(30);
    let mut dialog = node(31);
    dialog.kind = LayoutNodeKind::Dialog;
    dialog.placement_preference = sophia_protocol::SurfacePlacementPreference::Floating;
    dialog.transient_owner = Some(owner.surface);
    dialog.state.floating = true;
    let request = WmRequestPacket {
        transaction: TransactionId::from_raw(76),
        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
            output: OutputId::from_raw(1),
            workspace: WorkspaceId::from_raw(1),
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1200,
                height: 800,
            },
            nodes: vec![owner, dialog],
        }),
    };

    bridge.apply_engine_request(&request).unwrap();
    let owner_window = bridge.synthetic_window(SurfaceId::new(30, 1)).unwrap();
    let dialog_window = bridge.synthetic_window(SurfaceId::new(31, 1)).unwrap();
    let profile = bridge.synthetic_manage_profile(dialog_window).unwrap();

    assert_eq!(profile.kind, LayoutNodeKind::Dialog);
    assert_eq!(profile.transient_for, Some(owner_window.raw()));

    let mut unattached = node(32);
    unattached.kind = LayoutNodeKind::Dialog;
    unattached.state.floating = true;
    bridge
        .apply_engine_request(&WmRequestPacket {
            transaction: TransactionId::from_raw(77),
            kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                output: OutputId::from_raw(1),
                workspace: WorkspaceId::from_raw(1),
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 1200,
                    height: 800,
                },
                nodes: vec![unattached],
            }),
        })
        .unwrap();
    let profile = bridge
        .synthetic_manage_profile(bridge.synthetic_window(SurfaceId::new(32, 1)).unwrap())
        .unwrap();
    assert_eq!(profile.transient_for, Some(SYNTHETIC_ROOT_XID));
}

#[test]
fn xmonad_application_bindings_keep_semantic_slots_when_the_launcher_is_absent() {
    let workspace = WorkspaceId::from_raw(1);
    let output = OutputId::from_raw(1);
    let terminal = WmSessionAction::LaunchApplication {
        application: SessionApplicationId::from_raw(1),
    };
    let browser = WmSessionAction::LaunchApplication {
        application: SessionApplicationId::from_raw(3),
    };
    let session = WmSessionDescriptor {
        api_version: WM_API_VERSION,
        workspaces: vec![workspace],
        active_workspaces: vec![WmOutputWorkspace { output, workspace }],
        session_actions: vec![
            WmSessionAction::CloseFocused,
            WmSessionAction::Logout,
            terminal,
            browser,
        ],
    };
    let activate = |action| WmRequestPacket {
        transaction: TransactionId::from_raw(action),
        kind: WmRequestKind::ActionActivated(WmActionActivation {
            action: WmActionId::from_raw(action),
            output,
            workspace,
            focused_surface: None,
            nodes: Vec::new(),
        }),
    };

    let response =
        translate_xmonad_profile_action(&activate(XMONAD_ACTION_APPLICATION_3), &session)
            .unwrap()
            .unwrap();
    assert_eq!(
        response.commands,
        vec![WmCommand::RequestSessionAction {
            action: browser,
            target: None,
        }]
    );
    assert_eq!(
        translate_xmonad_profile_action(&activate(XMONAD_ACTION_APPLICATION_2), &session,),
        Err(X11WmBridgeError::UnavailableSessionAction)
    );
}

#[test]
fn translates_two_synthetic_legacy_wm_tiles_without_metadata() {
    let transaction = TransactionId::from_raw(71);
    let request = WmRequestPacket {
        transaction,
        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
            output: OutputId::from_raw(1),
            workspace: WorkspaceId::from_raw(1),
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1200,
                height: 800,
            },
            nodes: vec![node(10), node(11)],
        }),
    };
    let mut bridge = X11WmBridgeState::new();
    let update = bridge.apply_engine_request(&request).unwrap();
    assert_eq!(update.events.len(), 3);
    assert!(matches!(
        update.events[1],
        SyntheticXEvent::MapRequest { .. }
    ));
    assert!(matches!(
        update.events[2],
        SyntheticXEvent::MapRequest { .. }
    ));

    let left = bridge.synthetic_window(SurfaceId::new(10, 1)).unwrap();
    let right = bridge.synthetic_window(SurfaceId::new(11, 1)).unwrap();
    let response = bridge
        .translate_legacy_requests(
            transaction,
            &[
                LegacyWmRequest::ConfigureWindow {
                    window: left,
                    geometry: Rect {
                        x: 0,
                        y: 0,
                        width: 600,
                        height: 800,
                    },
                    z_index: 0,
                },
                LegacyWmRequest::ConfigureWindow {
                    window: right,
                    geometry: Rect {
                        x: 600,
                        y: 0,
                        width: 600,
                        height: 800,
                    },
                    z_index: 1,
                },
                LegacyWmRequest::FocusWindow { window: left },
            ],
            300,
        )
        .unwrap();

    assert_eq!(response.commands.len(), 5);
    assert!(
        response
            .commands
            .contains(&WmCommand::FocusSurface(SurfaceId::new(10, 1)))
    );
    assert!(!format!("{response:?}").contains(&format!("{:?}", NamespaceId::from_raw(99))));
}

#[test]
fn workspace_activation_unmaps_hidden_windows_and_remaps_only_the_target_workspace() {
    let transaction = TransactionId::from_raw(73);
    let mut bridge = X11WmBridgeState::new();
    let first = WmRequestPacket {
        transaction,
        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
            output: OutputId::from_raw(1),
            workspace: WorkspaceId::from_raw(1),
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1200,
                height: 800,
            },
            nodes: vec![node(20), node(21)],
        }),
    };
    bridge.apply_engine_request(&first).unwrap();

    let hidden = bridge.activate_workspace(TransactionId::from_raw(74), WorkspaceId::from_raw(2));
    assert_eq!(
        hidden
            .events
            .iter()
            .filter(|event| matches!(event, SyntheticXEvent::UnmapNotify { .. }))
            .count(),
        2
    );
    let hidden_window = bridge
        .synthetic_window(SurfaceId::new(20, 1))
        .expect("managed surface has a synthetic window");
    let hidden_focus = bridge
        .translate_legacy_requests(
            TransactionId::from_raw(74),
            &[LegacyWmRequest::FocusWindow {
                window: hidden_window,
            }],
            300,
        )
        .unwrap();
    assert!(
        hidden_focus
            .commands
            .iter()
            .all(|command| !matches!(command, WmCommand::FocusSurface(_))),
        "hidden legacy focus crossed the blind-WM boundary"
    );

    let restored = bridge.activate_workspace(TransactionId::from_raw(75), WorkspaceId::from_raw(1));
    assert_eq!(
        restored
            .events
            .iter()
            .filter(|event| matches!(event, SyntheticXEvent::MapRequest { .. }))
            .count(),
        2
    );
}

#[test]
fn hidden_legacy_geometry_cannot_cross_the_workspace_projection() {
    let output = OutputId::from_raw(1);
    let first_workspace = WorkspaceId::from_raw(1);
    let second_workspace = WorkspaceId::from_raw(2);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 1200,
        height: 800,
    };
    let hidden = node(22);
    let mut visible = node(23);
    visible.workspace = second_workspace;
    let mut bridge = X11WmBridgeState::new();

    for (transaction, workspace, nodes) in [
        (76, first_workspace, vec![hidden.clone()]),
        (77, second_workspace, vec![visible.clone()]),
    ] {
        bridge
            .apply_engine_request(&WmRequestPacket {
                transaction: TransactionId::from_raw(transaction),
                kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                    output,
                    workspace,
                    bounds,
                    nodes,
                }),
            })
            .unwrap();
    }

    let hidden_window = bridge.synthetic_window(hidden.surface).unwrap();
    let visible_window = bridge.synthetic_window(visible.surface).unwrap();
    let response = bridge
        .translate_legacy_requests(
            TransactionId::from_raw(78),
            &[
                LegacyWmRequest::ConfigureWindow {
                    window: hidden_window,
                    geometry: bounds,
                    z_index: 0,
                },
                LegacyWmRequest::ConfigureWindow {
                    window: visible_window,
                    geometry: bounds,
                    z_index: 0,
                },
                LegacyWmRequest::FocusWindow {
                    window: hidden_window,
                },
                LegacyWmRequest::FocusWindow {
                    window: visible_window,
                },
            ],
            300,
        )
        .unwrap();

    assert_eq!(response.commands.len(), 3);
    assert!(matches!(
        response.commands[0],
        WmCommand::ConfigureSurface(request) if request.surface == visible.surface
    ));
    assert!(matches!(
        response.commands[1],
        WmCommand::RenderSurface(placement) if placement.surface == visible.surface
    ));
    assert_eq!(
        response.commands[2],
        WmCommand::FocusSurface(visible.surface)
    );
}

#[test]
fn complete_workspace_projection_unmaps_an_omitted_surface() {
    let workspace = WorkspaceId::from_raw(1);
    let first = node(23);
    let second = node(24);
    let mut bridge = X11WmBridgeState::new();
    bridge
        .apply_engine_request(&WmRequestPacket {
            transaction: TransactionId::from_raw(78),
            kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                output: OutputId::from_raw(1),
                workspace,
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 1200,
                    height: 800,
                },
                nodes: vec![first.clone(), second.clone()],
            }),
        })
        .unwrap();
    let omitted_window = bridge.synthetic_window(first.surface).unwrap();

    let update = bridge
        .apply_engine_request(&WmRequestPacket {
            transaction: TransactionId::from_raw(79),
            kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                output: OutputId::from_raw(1),
                workspace,
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 1200,
                    height: 800,
                },
                nodes: vec![second],
            }),
        })
        .unwrap();

    assert!(update.events.contains(&SyntheticXEvent::UnmapNotify {
        window: omitted_window,
    }));
    assert_eq!(bridge.synthetic_window(first.surface), Some(omitted_window));
    let response = bridge
        .translate_legacy_requests(
            TransactionId::from_raw(80),
            &[
                LegacyWmRequest::ConfigureWindow {
                    window: omitted_window,
                    geometry: Rect {
                        x: 0,
                        y: 0,
                        width: 600,
                        height: 800,
                    },
                    z_index: 0,
                },
                LegacyWmRequest::FocusWindow {
                    window: omitted_window,
                },
            ],
            300,
        )
        .unwrap();
    assert!(response.commands.is_empty());
}

#[test]
fn direct_workspace_assignment_reconciles_cached_membership() {
    let workspace_one = WorkspaceId::from_raw(1);
    let workspace_two = WorkspaceId::from_raw(2);
    let surface = SurfaceId::new(25, 1);
    let mut bridge = X11WmBridgeState::new();
    bridge
        .apply_engine_request(&WmRequestPacket {
            transaction: TransactionId::from_raw(81),
            kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                output: OutputId::from_raw(1),
                workspace: workspace_one,
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 1200,
                    height: 800,
                },
                nodes: vec![node(25)],
            }),
        })
        .unwrap();
    let window = bridge.synthetic_window(surface).unwrap();

    let assigned = bridge
        .assign_workspace(TransactionId::from_raw(82), surface, workspace_two)
        .unwrap();
    assert!(
        assigned
            .events
            .contains(&SyntheticXEvent::UnmapNotify { window })
    );
    assert!(
        bridge
            .translate_legacy_requests(
                TransactionId::from_raw(83),
                &[LegacyWmRequest::FocusWindow { window }],
                300,
            )
            .unwrap()
            .commands
            .is_empty()
    );

    let activated = bridge.activate_workspace(TransactionId::from_raw(84), workspace_two);
    assert!(
        activated
            .events
            .contains(&SyntheticXEvent::MapRequest { window })
    );
}

#[test]
fn client_size_constraints_bound_both_configure_and_render_geometry() {
    let transaction = TransactionId::from_raw(72);
    let mut constrained = node(12);
    constrained.constraints.max_size = Some(Size {
        width: 320,
        height: 240,
    });
    let request = WmRequestPacket {
        transaction,
        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
            output: OutputId::from_raw(1),
            workspace: WorkspaceId::from_raw(1),
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1200,
                height: 800,
            },
            nodes: vec![constrained],
        }),
    };
    let mut bridge = X11WmBridgeState::new();
    bridge.apply_engine_request(&request).unwrap();
    let window = bridge.synthetic_window(SurfaceId::new(12, 1)).unwrap();
    let response = bridge
        .translate_legacy_requests(
            transaction,
            &[LegacyWmRequest::ConfigureWindow {
                window,
                geometry: Rect {
                    x: 20,
                    y: 30,
                    width: 1200,
                    height: 800,
                },
                z_index: 0,
            }],
            300,
        )
        .unwrap();

    assert!(response.commands.iter().any(|command| matches!(
        command,
        WmCommand::ConfigureSurface(request)
            if request.size == Size { width: 320, height: 240 }
    )));
    assert!(response.commands.iter().any(|command| matches!(
        command,
        WmCommand::RenderSurface(placement)
            if placement.geometry == Rect { x: 20, y: 30, width: 320, height: 240 }
    )));
}

#[test]
fn completed_gesture_clamps_the_entire_frame_to_its_current_output() {
    let output_one = OutputId::from_raw(1);
    let output_two = OutputId::from_raw(2);
    let workspace_two = WorkspaceId::from_raw(2);
    let mut second = node(13);
    second.workspace = workspace_two;
    let mut bridge = X11WmBridgeState::new();
    for (transaction, output, workspace, bounds, nodes) in [
        (
            73,
            output_one,
            WorkspaceId::from_raw(1),
            Rect {
                x: 0,
                y: 0,
                width: 1200,
                height: 800,
            },
            vec![node(12)],
        ),
        (
            74,
            output_two,
            workspace_two,
            Rect {
                x: 1200,
                y: 0,
                width: 800,
                height: 600,
            },
            vec![second],
        ),
    ] {
        bridge
            .apply_engine_request(&WmRequestPacket {
                transaction: TransactionId::from_raw(transaction),
                kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                    output,
                    workspace,
                    bounds,
                    nodes,
                }),
            })
            .unwrap();
    }
    let window = bridge.synthetic_window(SurfaceId::new(13, 1)).unwrap();

    let response = bridge
        .translate_legacy_requests_for_output(
            TransactionId::from_raw(75),
            &[LegacyWmRequest::ConfigureWindow {
                window,
                geometry: Rect {
                    x: -300,
                    y: 700,
                    width: 1000,
                    height: 900,
                },
                z_index: 4,
            }],
            300,
            Some(output_two),
        )
        .unwrap();

    assert!(response.commands.iter().any(|command| matches!(
        command,
        WmCommand::RenderSurface(placement)
            if placement.geometry == Rect {
                x: 1200,
                y: 0,
                width: 800,
                height: 600,
            }
    )));
}

#[test]
fn focus_request_preserves_the_existing_blind_synthetic_topology() {
    let mut bridge = X11WmBridgeState::new();
    bridge
        .apply_engine_request(&WmRequestPacket {
            transaction: TransactionId::from_raw(80),
            kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                output: OutputId::from_raw(1),
                workspace: WorkspaceId::from_raw(1),
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 1200,
                    height: 800,
                },
                nodes: vec![node(30), node(31)],
            }),
        })
        .unwrap();
    let target = SurfaceId::new(31, 1);
    let window = bridge.synthetic_window(target).unwrap();

    let update = bridge
        .apply_engine_request(&WmRequestPacket {
            transaction: TransactionId::from_raw(81),
            kind: WmRequestKind::FocusRequested(WmFocusRequest {
                surface: target,
                output: OutputId::from_raw(1),
                workspace: WorkspaceId::from_raw(1),
            }),
        })
        .unwrap();

    assert!(update.events.is_empty());
    assert_eq!(bridge.synthetic_window(target), Some(window));
}

#[test]
fn exact_constraint_change_updates_the_synthetic_property_in_place() {
    let transaction = TransactionId::from_raw(90);
    let surface = SurfaceId::new(40, 1);
    let mut bridge = X11WmBridgeState::new();
    let initial = WmRequestPacket {
        transaction,
        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
            output: OutputId::from_raw(1),
            workspace: WorkspaceId::from_raw(1),
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1200,
                height: 800,
            },
            nodes: vec![node(40)],
        }),
    };
    bridge.apply_engine_request(&initial).unwrap();
    let old_window = bridge.synthetic_window(surface).unwrap();
    let initial_profile = bridge.synthetic_manage_profile(old_window).unwrap();

    let mut fixed = node(40);
    fixed.capabilities.resizable = false;
    fixed.constraints = SurfaceConstraints {
        min_size: Some(Size {
            width: 500,
            height: 500,
        }),
        max_size: Some(Size {
            width: 500,
            height: 500,
        }),
    };
    let recovery = WmRequestPacket {
        transaction: TransactionId::from_raw(91),
        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
            output: OutputId::from_raw(1),
            workspace: WorkspaceId::from_raw(1),
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1200,
                height: 800,
            },
            nodes: vec![fixed],
        }),
    };
    let update = bridge.apply_engine_request(&recovery).unwrap();
    let new_window = bridge.synthetic_window(surface).unwrap();

    assert_eq!(new_window, old_window);
    assert!(
        update
            .events
            .contains(&SyntheticXEvent::PropertyNotify { window: old_window })
    );
    assert!(!update.events.iter().any(|event| matches!(
        event,
        SyntheticXEvent::DestroyNotify { .. } | SyntheticXEvent::MapRequest { .. }
    )));
    assert_eq!(
        bridge
            .synthetic_manage_profile(new_window)
            .unwrap()
            .constraints,
        Some(SurfaceConstraints {
            min_size: Some(Size {
                width: 500,
                height: 500,
            }),
            max_size: Some(Size {
                width: 500,
                height: 500,
            }),
        })
    );
    let hints = bridge
        .synthetic_manage_profile(new_window)
        .unwrap()
        .icccm_normal_hints()
        .unwrap();
    assert_eq!(hints[0], (1 << 4) | (1 << 5));
    assert_eq!(&hints[5..9], &[500, 500, 500, 500]);

    let released = WmRequestPacket {
        transaction: TransactionId::from_raw(92),
        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
            output: OutputId::from_raw(1),
            workspace: WorkspaceId::from_raw(1),
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1200,
                height: 800,
            },
            nodes: vec![node(40)],
        }),
    };
    let update = bridge.apply_engine_request(&released).unwrap();

    assert_eq!(bridge.synthetic_window(surface), Some(old_window));
    assert!(
        update
            .events
            .contains(&SyntheticXEvent::PropertyNotify { window: old_window })
    );
    assert!(!update.events.iter().any(|event| matches!(
        event,
        SyntheticXEvent::DestroyNotify { .. } | SyntheticXEvent::MapRequest { .. }
    )));
    assert_eq!(
        bridge.synthetic_manage_profile(old_window).unwrap(),
        initial_profile
    );
}
