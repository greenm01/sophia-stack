use std::path::PathBuf;

use sophia_protocol::{
    LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot, LayoutNodeState, OutputId, Rect,
    SurfaceConstraints, SurfaceId, TransactionId, WM_API_VERSION, WmCommand, WmFocusRequest,
    WmManageSurface, WmOutputWorkspace, WmRequestKind, WmRequestPacket, WmSessionDescriptor,
    WorkspaceId,
};
use sophia_x11_wm_bridge::{
    LegacyWmLaunchSpec, LegacyWmProfile, LegacyX11WmBridgeRuntime, run_wm_socket_server,
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
    let mut runtime = LegacyX11WmBridgeRuntime::start_with_root(launch, bounds)?;
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
    let focus_target = SurfaceId::new(10, 1);
    let focus = runtime.handle_request(&WmRequestPacket {
        transaction: TransactionId::from_raw(4),
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
    println!(
        "real-xmonad-sequential-three-window-smoke: pass transaction={} focus_transaction={} master={:?} stack_top={:?} stack_bottom={:?}",
        response.transaction.raw(),
        focus.transaction.raw(),
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
