use std::path::PathBuf;

use sophia_protocol::{
    LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot, LayoutNodeState, OutputId, Rect,
    SurfaceConstraints, SurfaceId, TransactionId, WM_API_VERSION, WmActionActivation, WmActionId,
    WmCommand, WmFocusRequest, WmManageSurface, WmOutputWorkspace, WmRelayoutWorkspace,
    WmRequestKind, WmRequestPacket, WmSessionDescriptor, WorkspaceId,
};
use sophia_x11_wm_bridge::{
    LegacyWmLaunchSpec, LegacyWmProfile, LegacyX11WmBridgeRuntime, XMONAD_ACTION_NEXT_LAYOUT,
    run_wm_socket_server,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        Some("serve-socket") => {
            let socket = args
                .iter()
                .find_map(|arg| arg.strip_prefix("--socket="))
                .ok_or("missing --socket=PATH")?;
            let executable = args
                .iter()
                .find_map(|arg| arg.strip_prefix("--wm="))
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("SOPHIA_LEGACY_X11_WM").map(PathBuf::from))
                .ok_or("missing --wm=PATH or SOPHIA_LEGACY_X11_WM")?;
            let launch = args
                .iter()
                .filter_map(|arg| arg.strip_prefix("--wm-arg="))
                .fold(LegacyWmLaunchSpec::new(executable), |launch, argument| {
                    launch.arg(argument)
                });
            let launch = match args
                .iter()
                .find_map(|arg| arg.strip_prefix("--wm-private-alias="))
            {
                Some(alias) => launch.with_private_executable_alias(alias),
                None => launch,
            };
            let profile = match args.iter().find_map(|arg| arg.strip_prefix("--profile=")) {
                None | Some("layout-only") => LegacyWmProfile::LayoutOnly,
                Some("xmonad") => LegacyWmProfile::Xmonad,
                Some(profile) => {
                    return Err(format!("unsupported legacy WM profile {profile:?}").into());
                }
            };
            let launch = launch.with_profile(profile);
            run_wm_socket_server(socket, launch)?;
        }
        Some("xmonad-smoke" | "smoke") => {
            let xmonad = args
                .iter()
                .find_map(|arg| arg.strip_prefix("--xmonad="))
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("SOPHIA_XMONAD_BIN").map(PathBuf::from))
                .unwrap_or_else(|| PathBuf::from("xmonad"));
            run_xmonad_smoke(xmonad)?;
        }
        _ => {
            return Err(
                "usage: sophia-x11-wm-bridge serve-socket --socket=PATH --wm=PATH [--profile=layout-only|xmonad] [--wm-arg=ARG ...] [--wm-private-alias=RELATIVE]\n       sophia-x11-wm-bridge xmonad-smoke [--xmonad=PATH]"
                    .into(),
            );
        }
    }
    Ok(())
}

fn run_xmonad_smoke(xmonad: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let workspace = WorkspaceId::from_raw(1);
    let bounds = Rect {
        x: 0,
        y: 14,
        width: 2560,
        height: 1426,
    };
    let launch = LegacyWmLaunchSpec::new(xmonad)
        .with_private_executable_alias("xmonad/xmonad-x86_64-linux")
        .with_profile(LegacyWmProfile::Xmonad);
    let mut runtime = LegacyX11WmBridgeRuntime::start_with_root(launch.clone(), bounds)?;
    runtime.configure_session(WmSessionDescriptor {
        api_version: WM_API_VERSION,
        workspaces: vec![workspace],
        active_workspaces: vec![WmOutputWorkspace {
            output: OutputId::from_raw(1),
            workspace,
        }],
        session_actions: Vec::new(),
    })?;
    let first = WmRequestPacket {
        transaction: TransactionId::from_raw(1),
        kind: WmRequestKind::ManageSurface(WmManageSurface {
            output: OutputId::from_raw(1),
            workspace,
            bounds,
            node: node(10, workspace, bounds),
        }),
    };
    let first_response = runtime.handle_request(&first)?;
    let first_placements = response_placements(&first_response);
    if first_placements != vec![(10, bounds)] {
        return Err(format!(
            "xmonad did not place the first managed window: actual={first_placements:?}"
        )
        .into());
    }
    let second = WmRequestPacket {
        transaction: TransactionId::from_raw(2),
        kind: WmRequestKind::ManageSurface(WmManageSurface {
            output: OutputId::from_raw(1),
            workspace,
            bounds,
            node: node(
                11,
                workspace,
                Rect {
                    x: 80,
                    y: 60,
                    ..bounds
                },
            ),
        }),
    };
    let response = runtime.handle_request(&second)?;
    let actual = response_placements(&response);
    let expected = vec![
        (
            11,
            Rect {
                x: 0,
                y: 14,
                width: 1280,
                height: 1426,
            },
        ),
        (
            10,
            Rect {
                x: 1280,
                y: 14,
                width: 1280,
                height: 1426,
            },
        ),
    ];
    if response.transaction != second.transaction || actual != expected {
        return Err(format!(
            "xmonad did not produce the strict sequential two-tile response: transaction={:?} actual={actual:?}",
            response.transaction
        )
        .into());
    }
    let third = WmRequestPacket {
        transaction: TransactionId::from_raw(3),
        kind: WmRequestKind::ManageSurface(WmManageSurface {
            output: OutputId::from_raw(1),
            workspace,
            bounds,
            node: node(
                12,
                workspace,
                Rect {
                    x: 80,
                    y: 60,
                    ..bounds
                },
            ),
        }),
    };
    let response = runtime.handle_request(&third)?;
    let actual = response_placements(&response);
    let expected = vec![
        (
            12,
            Rect {
                x: 0,
                y: 14,
                width: 1280,
                height: 1426,
            },
        ),
        (
            11,
            Rect {
                x: 1280,
                y: 14,
                width: 1280,
                height: 713,
            },
        ),
        (
            10,
            Rect {
                x: 1280,
                y: 727,
                width: 1280,
                height: 713,
            },
        ),
    ];
    if response.transaction != third.transaction || actual != expected {
        return Err(format!(
            "xmonad did not produce the strict sequential three-tile response: transaction={:?} actual={actual:?}",
            response.transaction
        )
        .into());
    }
    let layout = runtime.handle_request(&WmRequestPacket {
        transaction: TransactionId::from_raw(4),
        kind: WmRequestKind::ActionActivated(WmActionActivation {
            action: WmActionId::from_raw(XMONAD_ACTION_NEXT_LAYOUT),
            output: OutputId::from_raw(1),
            workspace,
            focused_surface: Some(SurfaceId::new(12, 1)),
            nodes: expected
                .iter()
                .map(|(raw, geometry)| node(*raw, workspace, *geometry))
                .collect(),
        }),
    })?;
    let mirror = response_placements(&layout);
    let expected_mirror = vec![
        (
            12,
            Rect {
                x: 0,
                y: 14,
                width: 2560,
                height: 713,
            },
        ),
        (
            11,
            Rect {
                x: 0,
                y: 727,
                width: 1280,
                height: 713,
            },
        ),
        (
            10,
            Rect {
                x: 1280,
                y: 727,
                width: 1280,
                height: 713,
            },
        ),
    ];
    if layout.transaction != TransactionId::from_raw(4) || mirror != expected_mirror {
        return Err(format!(
            "xmonad did not produce the strict Tall-to-Mirror action response: transaction={:?} actual={mirror:?}",
            layout.transaction
        )
        .into());
    }
    let focus_target = SurfaceId::new(10, 1);
    let focus = runtime.handle_request(&WmRequestPacket {
        transaction: TransactionId::from_raw(5),
        kind: WmRequestKind::FocusRequested(WmFocusRequest {
            surface: focus_target,
            output: OutputId::from_raw(1),
            workspace,
        }),
    })?;
    if !response_placements(&focus).is_empty()
        || !focus
            .commands
            .contains(&WmCommand::FocusSurface(focus_target))
    {
        return Err(format!(
            "xmonad did not focus the requested opaque surface without relayout: commands={:?}",
            focus.commands
        )
        .into());
    }
    let recovery_extent = sophia_protocol::Size {
        width: 500,
        height: 500,
    };
    let recovery_geometry = Rect {
        x: 80,
        y: 60,
        width: recovery_extent.width,
        height: recovery_extent.height,
    };
    let mut recovery_node = node(12, workspace, recovery_geometry);
    recovery_node.capabilities.resizable = false;
    recovery_node.constraints = SurfaceConstraints {
        min_size: Some(recovery_extent),
        max_size: Some(recovery_extent),
    };
    let recovery = runtime.handle_request(&WmRequestPacket {
        transaction: TransactionId::from_raw(6),
        kind: WmRequestKind::ManageSurface(WmManageSurface {
            output: OutputId::from_raw(1),
            workspace,
            bounds,
            node: recovery_node,
        }),
    })?;
    let recovery_placements = response_placements(&recovery);
    if !recovery_placements.iter().any(|(surface, geometry)| {
        *surface == 12
            && geometry.width == recovery_extent.width
            && geometry.height == recovery_extent.height
    }) || !recovery.commands.iter().any(|command| {
        matches!(
            command,
            WmCommand::ConfigureSurface(request)
                if request.surface == SurfaceId::new(12, 1)
                    && request.size == recovery_extent
        )
    }) {
        return Err(format!(
            "xmonad did not apply the generic fixed-extent recovery profile: commands={:?}",
            recovery.commands
        )
        .into());
    }
    let released = runtime.handle_request(&WmRequestPacket {
        transaction: TransactionId::from_raw(7),
        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
            output: OutputId::from_raw(1),
            workspace,
            bounds,
            nodes: recovery_placements
                .iter()
                .map(|(raw, geometry)| node(*raw, workspace, *geometry))
                .collect(),
        }),
    })?;
    let released_placements = response_placements(&released);
    if released_placements != expected_mirror
        || !released
            .commands
            .contains(&WmCommand::FocusSurface(SurfaceId::new(12, 1)))
    {
        return Err(format!(
            "xmonad did not preserve the master/focus stack across recovery-constraint release: commands={:?}",
            released.commands
        )
        .into());
    }

    drop(runtime);
    let mut restarted = LegacyX11WmBridgeRuntime::start_with_root(launch, bounds)?;
    restarted.configure_session(WmSessionDescriptor {
        api_version: WM_API_VERSION,
        workspaces: vec![workspace],
        active_workspaces: vec![WmOutputWorkspace {
            output: OutputId::from_raw(1),
            workspace,
        }],
        session_actions: Vec::new(),
    })?;
    let committed_seed = restarted.handle_request(&WmRequestPacket {
        transaction: TransactionId::from_raw(8),
        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
            output: OutputId::from_raw(1),
            workspace,
            bounds,
            nodes: vec![
                node(
                    10,
                    workspace,
                    Rect {
                        x: 1280,
                        y: 14,
                        width: 1280,
                        height: 1426,
                    },
                ),
                node(
                    11,
                    workspace,
                    Rect {
                        x: 0,
                        y: 14,
                        width: 1280,
                        height: 1426,
                    },
                ),
            ],
        }),
    })?;
    if response_placements(&committed_seed)
        != vec![
            (
                11,
                Rect {
                    x: 0,
                    y: 14,
                    width: 1280,
                    height: 1426,
                },
            ),
            (
                10,
                Rect {
                    x: 1280,
                    y: 14,
                    width: 1280,
                    height: 1426,
                },
            ),
        ]
    {
        return Err(format!(
            "fresh xmonad did not restore the committed two-window seed: commands={:?}",
            committed_seed.commands
        )
        .into());
    }
    let replayed_manage = restarted.handle_request(&WmRequestPacket {
        transaction: TransactionId::from_raw(9),
        kind: WmRequestKind::ManageSurface(WmManageSurface {
            output: OutputId::from_raw(1),
            workspace,
            bounds,
            node: node(
                12,
                workspace,
                Rect {
                    x: 80,
                    y: 60,
                    ..bounds
                },
            ),
        }),
    })?;
    let restarted_three = response_placements(&replayed_manage);
    if restarted_three != expected
        || !replayed_manage
            .commands
            .contains(&WmCommand::FocusSurface(SurfaceId::new(12, 1)))
    {
        return Err(format!(
            "fresh xmonad did not replay the pending admission after the committed seed: commands={:?}",
            replayed_manage.commands
        )
        .into());
    }
    let post_recovery = restarted.handle_request(&WmRequestPacket {
        transaction: TransactionId::from_raw(10),
        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
            output: OutputId::from_raw(1),
            workspace,
            bounds,
            nodes: restarted_three
                .iter()
                .map(|(raw, geometry)| node(*raw, workspace, *geometry))
                .collect(),
        }),
    })?;
    if response_placements(&post_recovery) != expected
        || !post_recovery
            .commands
            .contains(&WmCommand::FocusSurface(SurfaceId::new(12, 1)))
    {
        return Err(format!(
            "fresh xmonad did not retain the replayed admission as master/focus: commands={:?}",
            post_recovery.commands
        )
        .into());
    }
    println!(
        "real-xmonad-sequential-three-window-smoke: pass transaction={} layout_transaction={} focus_transaction={} recovery_transaction={} release_transaction={} restart_seed_transaction={} restart_manage_transaction={} restart_release_transaction={} master={:?} stack_top={:?} stack_bottom={:?} mirror={mirror:?} recovery={recovery_geometry:?}",
        response.transaction.raw(),
        layout.transaction.raw(),
        focus.transaction.raw(),
        recovery.transaction.raw(),
        released.transaction.raw(),
        committed_seed.transaction.raw(),
        replayed_manage.transaction.raw(),
        post_recovery.transaction.raw(),
        actual[0].1,
        actual[1].1,
        actual[2].1,
    );
    Ok(())
}

fn response_placements(response: &sophia_protocol::WmResponsePacket) -> Vec<(u32, Rect)> {
    let placements = response
        .commands
        .iter()
        .filter_map(|command| match command {
            WmCommand::RenderSurface(placement) => Some(placement),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut actual = placements
        .iter()
        .map(|placement| (placement.surface.index(), placement.geometry))
        .collect::<Vec<_>>();
    actual.sort_by_key(|(_, geometry)| (geometry.x, geometry.y));
    actual
}

fn node(raw: u32, workspace: WorkspaceId, geometry: Rect) -> LayoutNodeSnapshot {
    LayoutNodeSnapshot {
        surface: SurfaceId::new(raw, 1),
        workspace,
        kind: LayoutNodeKind::Toplevel,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        state: LayoutNodeState::NORMAL,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        geometry,
        generation: 1,
    }
}
