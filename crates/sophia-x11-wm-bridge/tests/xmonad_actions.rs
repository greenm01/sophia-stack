use std::{
    collections::VecDeque,
    env,
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    thread,
    time::Duration,
};

use sophia_protocol::{
    LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot, LayoutNodeState, OutputId, Rect,
    SurfaceConstraints, SurfaceId, TransactionId, WM_API_VERSION, WmActionActivation, WmActionId,
    WmCommand, WmManageSurface, WmOutputWorkspace, WmPointerGestureCompleted, WmPointerGestureMode,
    WmPointerPosition, WmRelayoutWorkspace, WmRequestKind, WmRequestPacket, WmSessionDescriptor,
    WorkspaceId,
};
use sophia_x11_wm_bridge::{
    LegacyWmLaunchSpec, LegacyWmProfile, LegacyX11WmBridgeRuntime, XMONAD_ACTION_FOCUS_NEXT,
    XMONAD_ACTION_NEXT_LAYOUT,
};

const ROOT: u32 = 0x20;
const FOCUS_NEXT_KEYCODE: u8 = 106;
const NEXT_LAYOUT_KEYCODE: u8 = 32;
const MOD1_MASK: u16 = 1 << 3;
const BOUNDS: Rect = Rect {
    x: 0,
    y: 14,
    width: 2560,
    height: 1426,
};

#[test]
fn delayed_next_layout_fixture_process() {
    if is_private_bridge_child() {
        run_fixture(FixtureBehavior::DelayedLayout);
    }
}

#[test]
fn missing_grab_fixture_process() {
    if is_private_bridge_child() {
        run_fixture(FixtureBehavior::MissingLayoutGrab);
    }
}

#[test]
fn partial_reconciliation_fixture_process() {
    if is_private_bridge_child() {
        run_fixture(FixtureBehavior::PartialFocusReconciliation);
    }
}

#[test]
fn manage_focus_fixture_process() {
    if is_private_bridge_child() {
        run_fixture(FixtureBehavior::ManageFocus);
    }
}

#[test]
fn pointer_gesture_fixture_process() {
    if is_private_bridge_child() {
        run_fixture(FixtureBehavior::PointerGesture);
    }
}

#[test]
fn next_layout_ignores_pre_action_reconciliation_and_waits_for_delayed_wm_output() {
    let mut runtime = fixture_runtime("delayed_next_layout_fixture_process");
    configure_session(&mut runtime);
    let tall = manage_three_surfaces(&mut runtime);
    assert_eq!(tall, tall_layout());

    let response = runtime
        .handle_request(&next_layout_request(TransactionId::from_raw(4)))
        .unwrap();

    assert_eq!(response.transaction, TransactionId::from_raw(4));
    assert_eq!(response_placements(&response), mirror_layout());
}

#[test]
fn next_layout_fails_closed_when_the_wm_did_not_grab_its_profile_chord() {
    let mut runtime = fixture_runtime("missing_grab_fixture_process");
    configure_session(&mut runtime);
    manage_three_surfaces(&mut runtime);

    let error = runtime
        .handle_request(&next_layout_request(TransactionId::from_raw(4)))
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("profile key chord was not registered by the legacy WM"),
        "unexpected error: {error}"
    );
}

#[test]
fn focus_action_accepts_partial_pre_action_reconciliation() {
    let mut runtime = fixture_runtime("partial_reconciliation_fixture_process");
    configure_session(&mut runtime);
    let tall = manage_three_surfaces(&mut runtime);
    assert_eq!(tall, tall_layout());

    let response = runtime
        .handle_request(&focus_next_request(TransactionId::from_raw(4)))
        .unwrap();

    assert_eq!(response.transaction, TransactionId::from_raw(4));
    assert!(
        response
            .commands
            .contains(&WmCommand::FocusSurface(SurfaceId::new(10, 1)))
    );
}

#[test]
fn manage_focus_remains_the_legacy_wm_focus_at_the_next_relayout() {
    let mut runtime = fixture_runtime("manage_focus_fixture_process");
    configure_session(&mut runtime);
    let tall = manage_three_surfaces(&mut runtime);
    assert_eq!(tall, tall_layout());

    let response = runtime
        .handle_request(&relayout_request(TransactionId::from_raw(4)))
        .unwrap();

    assert_eq!(response.transaction, TransactionId::from_raw(4));
    assert_eq!(response_placements(&response), tall_layout());
    assert!(
        response
            .commands
            .contains(&WmCommand::FocusSurface(SurfaceId::new(12, 1)))
    );
}

#[test]
fn completed_move_gesture_is_floated_and_committed_atomically() {
    let mut runtime = fixture_runtime("pointer_gesture_fixture_process");
    configure_session(&mut runtime);
    manage_three_surfaces(&mut runtime);
    let surface = SurfaceId::new(12, 1);

    let response = runtime
        .handle_request(&WmRequestPacket {
            transaction: TransactionId::from_raw(40),
            kind: WmRequestKind::PointerGestureCompleted(WmPointerGestureCompleted {
                surface,
                output: OutputId::from_raw(1),
                workspace: WorkspaceId::from_raw(1),
                mode: WmPointerGestureMode::Move,
                start: WmPointerPosition { x: 100, y: 100 },
                end: WmPointerPosition { x: 300, y: 260 },
            }),
        })
        .unwrap();

    assert!(response.commands.contains(&WmCommand::SetFloating {
        surface,
        floating: true,
    }));
    assert!(response.commands.iter().any(|command| matches!(
        command,
        WmCommand::RenderSurface(placement)
            if placement.surface == surface
                && placement.geometry == Rect { x: 200, y: 160, width: 640, height: 480 }
    )));
}

fn fixture_runtime(test_name: &str) -> LegacyX11WmBridgeRuntime {
    let executable = env::current_exe().unwrap();
    let launch = LegacyWmLaunchSpec::new(executable)
        .arg("--exact")
        .arg(test_name)
        .arg("--nocapture")
        .with_profile(LegacyWmProfile::Xmonad);
    LegacyX11WmBridgeRuntime::start_with_root(launch, BOUNDS).unwrap()
}

fn configure_session(runtime: &mut LegacyX11WmBridgeRuntime) {
    runtime
        .configure_session(WmSessionDescriptor {
            api_version: WM_API_VERSION,
            workspaces: vec![WorkspaceId::from_raw(1)],
            active_workspaces: vec![WmOutputWorkspace {
                output: OutputId::from_raw(1),
                workspace: WorkspaceId::from_raw(1),
            }],
            session_actions: Vec::new(),
        })
        .unwrap();
}

fn manage_three_surfaces(runtime: &mut LegacyX11WmBridgeRuntime) -> Vec<(u32, Rect)> {
    let mut placements = Vec::new();
    for raw in 10..=12 {
        let request = WmRequestPacket {
            transaction: TransactionId::from_raw(u64::from(raw)),
            kind: WmRequestKind::ManageSurface(WmManageSurface {
                output: OutputId::from_raw(1),
                workspace: WorkspaceId::from_raw(1),
                bounds: BOUNDS,
                node: node(raw, BOUNDS),
            }),
        };
        placements = response_placements(&runtime.handle_request(&request).unwrap());
    }
    placements
}

fn next_layout_request(transaction: TransactionId) -> WmRequestPacket {
    WmRequestPacket {
        transaction,
        kind: WmRequestKind::ActionActivated(WmActionActivation {
            action: WmActionId::from_raw(XMONAD_ACTION_NEXT_LAYOUT),
            output: OutputId::from_raw(1),
            workspace: WorkspaceId::from_raw(1),
            focused_surface: Some(SurfaceId::new(12, 1)),
            nodes: tall_layout()
                .into_iter()
                .map(|(raw, geometry)| node(raw, geometry))
                .collect(),
        }),
    }
}

fn relayout_request(transaction: TransactionId) -> WmRequestPacket {
    WmRequestPacket {
        transaction,
        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
            output: OutputId::from_raw(1),
            workspace: WorkspaceId::from_raw(1),
            bounds: BOUNDS,
            nodes: tall_layout()
                .into_iter()
                .map(|(raw, geometry)| node(raw, geometry))
                .collect(),
        }),
    }
}

fn focus_next_request(transaction: TransactionId) -> WmRequestPacket {
    WmRequestPacket {
        transaction,
        kind: WmRequestKind::ActionActivated(WmActionActivation {
            action: WmActionId::from_raw(XMONAD_ACTION_FOCUS_NEXT),
            output: OutputId::from_raw(1),
            workspace: WorkspaceId::from_raw(1),
            focused_surface: Some(SurfaceId::new(12, 1)),
            nodes: tall_layout()
                .into_iter()
                .map(|(raw, geometry)| node(raw, geometry))
                .collect(),
        }),
    }
}

fn node(raw: u32, geometry: Rect) -> LayoutNodeSnapshot {
    LayoutNodeSnapshot {
        surface: SurfaceId::new(raw, 1),
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
        geometry,
        generation: 1,
    }
}

fn response_placements(response: &sophia_protocol::WmResponsePacket) -> Vec<(u32, Rect)> {
    let mut placements = response
        .commands
        .iter()
        .filter_map(|command| match command {
            WmCommand::RenderSurface(placement) => {
                Some((placement.surface.index(), placement.geometry))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    placements.sort_by_key(|(_, geometry)| (geometry.x, geometry.y));
    placements
}

fn tall_layout() -> Vec<(u32, Rect)> {
    vec![
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
    ]
}

fn mirror_layout() -> Vec<(u32, Rect)> {
    vec![
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
    ]
}

fn is_private_bridge_child() -> bool {
    env::var_os("HOME")
        .map(PathBuf::from)
        .as_deref()
        .and_then(std::path::Path::file_name)
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("sophia-x11-wm-bridge-"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureBehavior {
    DelayedLayout,
    MissingLayoutGrab,
    PartialFocusReconciliation,
    ManageFocus,
    PointerGesture,
}

fn run_fixture(behavior: FixtureBehavior) {
    let mut stream = connect_private_display();
    let (next_layout_keycode, mut pending_events) =
        query_keycode(&mut stream, u32::from(NEXT_LAYOUT_KEYCODE));
    assert_eq!(next_layout_keycode, NEXT_LAYOUT_KEYCODE);
    match behavior {
        FixtureBehavior::DelayedLayout => write_grab_key(&mut stream, next_layout_keycode),
        FixtureBehavior::MissingLayoutGrab => {}
        FixtureBehavior::PartialFocusReconciliation => {
            write_grab_key(&mut stream, FOCUS_NEXT_KEYCODE)
        }
        FixtureBehavior::ManageFocus => {}
        FixtureBehavior::PointerGesture => {}
    }

    let mut windows = Vec::new();
    let mut mirror = false;
    let mut partial_reconciliation_sent = false;
    let mut focus_action_seen = false;
    loop {
        let event = pending_events.pop_front().unwrap_or_else(|| {
            let mut event = [0_u8; 32];
            stream.read_exact(&mut event).unwrap();
            event
        });
        match event[0] & 0x7f {
            20 => {
                let window = read_u32(&event, 8);
                windows.push(window);
                write_layout(&mut stream, &windows, mirror);
            }
            22 if behavior == FixtureBehavior::PartialFocusReconciliation => {
                if windows.len() == 3 && partial_reconciliation_sent && !focus_action_seen {
                    panic!("existing-node reconciliation was not coalesced");
                }
                if windows.len() == 3 && !focus_action_seen {
                    write_configure_window(&mut stream, windows[0], tall_geometries(3)[0]);
                    stream.flush().unwrap();
                    partial_reconciliation_sent = true;
                }
            }
            22 => write_layout(&mut stream, &windows, mirror),
            2 if event[1] == NEXT_LAYOUT_KEYCODE && read_u16(&event, 28) == MOD1_MASK => {
                thread::sleep(Duration::from_millis(160));
                mirror = true;
                write_layout(&mut stream, &windows, mirror);
            }
            2 if event[1] == FOCUS_NEXT_KEYCODE && read_u16(&event, 28) == MOD1_MASK => {
                focus_action_seen = true;
                write_set_input_focus(&mut stream, windows[0]);
            }
            4 if behavior == FixtureBehavior::PointerGesture => {
                let window = read_u32(&event, 12);
                write_query_pointer_and_grab(&mut stream, window);
            }
            4 => write_set_input_focus(&mut stream, read_u32(&event, 12)),
            6 if behavior == FixtureBehavior::PointerGesture => {
                let window = read_u32(&event, 12);
                write_configure_window(
                    &mut stream,
                    window,
                    Rect {
                        x: 200,
                        y: 160,
                        width: 640,
                        height: 480,
                    },
                );
                stream.flush().unwrap();
            }
            5 => {}
            2 | 3 => {}
            event_type => panic!("unexpected synthetic X event {event_type}"),
        }
    }
}

fn write_query_pointer_and_grab(stream: &mut UnixStream, window: u32) {
    let mut query = vec![38, 0, 2, 0];
    query.extend_from_slice(&window.to_le_bytes());
    stream.write_all(&query).unwrap();
    stream.flush().unwrap();
    let _ = read_reply_ignoring_events(stream);

    let mut grab = vec![26, 0, 6, 0];
    grab.extend_from_slice(&ROOT.to_le_bytes());
    grab.extend_from_slice(&((1_u16 << 3) | (1_u16 << 6)).to_le_bytes());
    grab.extend_from_slice(&[1, 1]);
    grab.extend_from_slice(&0_u32.to_le_bytes());
    grab.extend_from_slice(&0_u32.to_le_bytes());
    grab.extend_from_slice(&0_u32.to_le_bytes());
    stream.write_all(&grab).unwrap();
    stream.flush().unwrap();
    let reply = read_reply_ignoring_events(stream);
    assert_eq!(reply[0], 1);
}

fn read_reply_ignoring_events(stream: &mut UnixStream) -> [u8; 32] {
    loop {
        let mut packet = [0_u8; 32];
        stream.read_exact(&mut packet).unwrap();
        if packet[0] == 1 {
            return packet;
        }
    }
}

fn connect_private_display() -> UnixStream {
    let display = env::var("DISPLAY")
        .unwrap()
        .strip_prefix(':')
        .unwrap()
        .parse::<u16>()
        .unwrap();
    let mut stream = UnixStream::connect(format!("/tmp/.X11-unix/X{display}")).unwrap();
    stream
        .write_all(&[b'l', 0, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0])
        .unwrap();

    let mut setup_header = [0_u8; 8];
    stream.read_exact(&mut setup_header).unwrap();
    assert_eq!(setup_header[0], 1, "private X setup failed");
    let remaining = usize::from(read_u16(&setup_header, 6)) * 4;
    let mut setup_body = vec![0_u8; remaining];
    stream.read_exact(&mut setup_body).unwrap();
    stream
}

fn query_keycode(stream: &mut UnixStream, keysym: u32) -> (u8, VecDeque<[u8; 32]>) {
    const FIRST_KEYCODE: u8 = 8;
    const KEYCODE_COUNT: u8 = 248;

    stream
        .write_all(&[101, 0, 2, 0, FIRST_KEYCODE, KEYCODE_COUNT, 0, 0])
        .unwrap();
    stream.flush().unwrap();
    let mut pending_events = VecDeque::new();
    let header = loop {
        let mut packet = [0_u8; 32];
        stream.read_exact(&mut packet).unwrap();
        if packet[0] == 1 {
            break packet;
        }
        pending_events.push_back(packet);
    };
    assert_eq!(header[0], 1, "GetKeyboardMapping failed");
    assert_eq!(header[1], 1, "fixture requires one keysym per keycode");
    let mapping_len = read_u32(&header, 4) as usize;
    assert_eq!(mapping_len, usize::from(KEYCODE_COUNT));
    let mut mapping = vec![0_u8; mapping_len * 4];
    stream.read_exact(&mut mapping).unwrap();
    let offset = mapping
        .chunks_exact(4)
        .position(|entry| u32::from_le_bytes(entry.try_into().unwrap()) == keysym)
        .expect("profile keysym must exist in the private keyboard map");
    (
        FIRST_KEYCODE + u8::try_from(offset).unwrap(),
        pending_events,
    )
}

fn write_grab_key(stream: &mut UnixStream, keycode: u8) {
    let mut request = vec![33, 1, 4, 0];
    request.extend_from_slice(&ROOT.to_le_bytes());
    request.extend_from_slice(&MOD1_MASK.to_le_bytes());
    request.push(keycode);
    request.extend_from_slice(&[1, 1, 0, 0, 0]);
    stream.write_all(&request).unwrap();
    stream.flush().unwrap();
}

fn write_set_input_focus(stream: &mut UnixStream, window: u32) {
    let mut request = vec![42, 0, 3, 0];
    request.extend_from_slice(&window.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    stream.write_all(&request).unwrap();
    stream.flush().unwrap();
}

fn write_layout(stream: &mut UnixStream, windows: &[u32], mirror: bool) {
    let geometries = if mirror {
        mirror_geometries(windows.len())
    } else {
        tall_geometries(windows.len())
    };
    for (&window, geometry) in windows.iter().rev().zip(geometries) {
        write_configure_window(stream, window, geometry);
    }
    stream.flush().unwrap();
}

fn tall_geometries(count: usize) -> Vec<Rect> {
    match count {
        0 => Vec::new(),
        1 => vec![BOUNDS],
        2 => vec![
            Rect {
                width: 1280,
                ..BOUNDS
            },
            Rect {
                x: 1280,
                width: 1280,
                ..BOUNDS
            },
        ],
        3 => tall_layout()
            .into_iter()
            .map(|(_, geometry)| geometry)
            .collect(),
        _ => panic!("fixture supports at most three windows"),
    }
}

fn mirror_geometries(count: usize) -> Vec<Rect> {
    match count {
        0 => Vec::new(),
        1 => vec![BOUNDS],
        2 => vec![
            Rect {
                height: 713,
                ..BOUNDS
            },
            Rect {
                y: 727,
                height: 713,
                ..BOUNDS
            },
        ],
        3 => mirror_layout()
            .into_iter()
            .map(|(_, geometry)| geometry)
            .collect(),
        _ => panic!("fixture supports at most three windows"),
    }
}

fn write_configure_window(stream: &mut UnixStream, window: u32, geometry: Rect) {
    let mut request = vec![12, 0, 7, 0];
    request.extend_from_slice(&window.to_le_bytes());
    request.extend_from_slice(&0xf_u16.to_le_bytes());
    request.extend_from_slice(&[0, 0]);
    for value in [
        geometry.x as u32,
        geometry.y as u32,
        geometry.width as u32,
        geometry.height as u32,
    ] {
        request.extend_from_slice(&value.to_le_bytes());
    }
    stream.write_all(&request).unwrap();
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
