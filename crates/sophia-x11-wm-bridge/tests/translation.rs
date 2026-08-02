use sophia_protocol::{
    LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot, LayoutNodeState, NamespaceId,
    OutputId, Rect, SessionApplicationId, Size, SurfaceConstraints, SurfaceId, TransactionId,
    WM_API_VERSION, WmActionActivation, WmActionId, WmCommand, WmFocusRequest, WmModifierMask,
    WmOutputWorkspace, WmRelayoutWorkspace, WmRequestKind, WmRequestPacket, WmSessionAction,
    WmSessionDescriptor, WorkspaceId,
};
use sophia_x11_wm_bridge::{
    LegacyWmProfile, LegacyWmRequest, SyntheticXEvent, X11WmBridgeError, X11WmBridgeState,
    XMONAD_ACTION_APPLICATION_1, XMONAD_ACTION_APPLICATION_2, XMONAD_ACTION_APPLICATION_3,
    translate_xmonad_profile_action,
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
