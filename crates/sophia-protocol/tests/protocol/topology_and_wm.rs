#[test]
fn output_topology_validates_bounded_engine_facts() {
    let topology = OutputTopologySnapshot {
        generation: 7,
        primary: OutputId::from_raw(2),
        outputs: vec![
            OutputTopologyEntry {
                output: OutputId::from_raw(1),
                logical: Rect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                pixel_size: Size {
                    width: 1920,
                    height: 1080,
                },
                scale: 1,
                refresh_millihz: 60_000,
            },
            OutputTopologyEntry {
                output: OutputId::from_raw(2),
                logical: Rect {
                    x: 1920,
                    y: 0,
                    width: 1280,
                    height: 720,
                },
                pixel_size: Size {
                    width: 2560,
                    height: 1440,
                },
                scale: 2,
                refresh_millihz: 120_000,
            },
        ],
    };
    assert_eq!(
        topology.validate(),
        Ok(Size {
            width: 3200,
            height: 1080,
        })
    );
}

#[test]
fn output_topology_rejects_duplicate_and_unbounded_facts() {
    let mut topology = OutputTopologySnapshot::deterministic();
    topology.outputs.push(topology.outputs[0]);
    assert_eq!(
        topology.validate(),
        Err(OutputTopologyError::DuplicateOutput)
    );

    let mut topology = OutputTopologySnapshot::deterministic();
    topology.outputs[0].logical.width = i32::from(u16::MAX) + 1;
    assert_eq!(
        topology.validate(),
        Err(OutputTopologyError::RootSizeExceeded)
    );
}
#[test]
fn wm_api_v6_negotiation_and_opaque_application_actions_round_trip() {
    let hello = WmHello {
        api_version: WM_API_VERSION,
        capabilities: WmCapabilities::all_supported(),
        policy_generation: 1,
        chrome: WmChromePolicy::default(),
        bindings: vec![WmBindingRegistration {
            action: WmActionId::from_raw(7),
            keycode: 36,
            modifiers: WmModifierMask {
                bits: WmModifierMask::SUPER,
            },
        }],
    };
    assert_eq!(
        decode_wm_hello_frame(&encode_wm_hello_frame(&hello).unwrap()).unwrap(),
        hello
    );

    let descriptor = WmSessionDescriptor {
        api_version: WM_API_VERSION,
        workspaces: (1..=WM_DEFAULT_WORKSPACES)
            .map(|raw| WorkspaceId::from_raw(raw as u64))
            .collect(),
        active_workspaces: vec![WmOutputWorkspace {
            output: OutputId::from_raw(1),
            workspace: WorkspaceId::from_raw(1),
        }],
        session_actions: vec![
            WmSessionAction::LaunchApplication {
                application: SessionApplicationId::from_raw(1),
            },
            WmSessionAction::CloseFocused,
            WmSessionAction::Logout,
        ],
    };
    assert_eq!(
        decode_wm_session_descriptor_frame(
            &encode_wm_session_descriptor_frame(&descriptor).unwrap()
        )
        .unwrap(),
        descriptor
    );

    let workspace = WorkspaceId::from_raw(1);
    let node = LayoutNodeSnapshot {
        surface: SurfaceId::new(4, 1),
        workspace,
        kind: LayoutNodeKind::Toplevel,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        state: LayoutNodeState::NORMAL,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        geometry: Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        },
        generation: 1,
    };
    let request = WmRequestPacket {
        transaction: TransactionId::from_raw(22),
        kind: WmRequestKind::ActionActivated(WmActionActivation {
            action: WmActionId::from_raw(7),
            output: OutputId::from_raw(1),
            workspace,
            focused_surface: Some(node.surface),
            nodes: vec![node],
        }),
    };
    assert_eq!(
        decode_wm_request_frame(&encode_wm_request_frame(&request).unwrap()).unwrap(),
        request
    );

    let response = WmResponsePacket {
        transaction: request.transaction,
        commands: vec![
            WmCommand::ActivateWorkspace {
                output: OutputId::from_raw(1),
                workspace: WorkspaceId::from_raw(2),
            },
            WmCommand::RequestSessionAction {
                action: WmSessionAction::LaunchApplication {
                    application: SessionApplicationId::from_raw(1),
                },
                target: None,
            },
        ],
        timeout_msec: 300,
    };
    assert_eq!(
        decode_wm_response_frame(&encode_wm_response_frame(&response).unwrap()).unwrap(),
        response
    );
}

#[test]
fn wm_policy_update_and_ack_round_trip() {
    let update = WmPolicyUpdate {
        api_version: WM_API_VERSION,
        generation: 9,
        bindings: vec![WmBindingRegistration {
            action: WmActionId::from_raw(4),
            keycode: 57,
            modifiers: WmModifierMask {
                bits: WmModifierMask::SUPER,
            },
        }],
        chrome: WmChromePolicy {
            focus_ring: WmFocusRingStyle {
                enabled: true,
                width: 4,
                color: WmRgb8 {
                    red: 1,
                    green: 2,
                    blue: 3,
                },
            },
            frame: WmFrameStyle::default(),
        },
    };
    assert_eq!(
        decode_wm_policy_update_frame(&encode_wm_policy_update_frame(&update).unwrap()).unwrap(),
        update
    );
    let ack = WmPolicyAck {
        generation: 9,
        outcome: WmPolicyAckOutcome::Applied,
    };
    assert_eq!(
        decode_wm_policy_ack_frame(&encode_wm_policy_ack_frame(ack).unwrap()).unwrap(),
        ack
    );
}
