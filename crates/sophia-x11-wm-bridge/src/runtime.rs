use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt, symlink},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

mod wire;

use wire::*;

use sophia_protocol::{Rect, WmCommand, WmRequestKind, WmRequestPacket, WmResponsePacket};
use sophia_x_authority::{XByteOrder, serve_x11_setup_socket_client_with_root_size};

use crate::{
    BridgeEngineUpdate, LegacySessionDescriptor, LegacyWmProfile, LegacyWmRequest,
    SYNTHETIC_ROOT_XID, SyntheticManageProfile, SyntheticXEvent, SyntheticXWindowId,
    X11WmBridgeError, X11WmBridgeState, XMONAD_ACTION_DECREASE_MASTER_COUNT, XMONAD_ACTION_EXPAND,
    XMONAD_ACTION_FOCUS_MASTER, XMONAD_ACTION_FOCUS_NEXT, XMONAD_ACTION_FOCUS_PREVIOUS,
    XMONAD_ACTION_INCREASE_MASTER_COUNT, XMONAD_ACTION_NEXT_LAYOUT, XMONAD_ACTION_RESET_LAYOUT,
    XMONAD_ACTION_SHRINK, XMONAD_ACTION_SINK, XMONAD_ACTION_SWAP_DOWN, XMONAD_ACTION_SWAP_MASTER,
    XMONAD_ACTION_SWAP_UP, XMONAD_ACTION_TOGGLE_FLOATING, translate_xmonad_profile_action,
};

const FIRST_DYNAMIC_ATOM: u32 = 256;
const BRIDGE_TIMEOUT: Duration = Duration::from_secs(3);
const QUIET_PERIOD: Duration = Duration::from_millis(80);
// A grabbed key may be a legitimate state-dependent no-op. Give xmonad a
// bounded post-grab settling interval before declaring that quiet result; any
// observed reply restarts the same interval.
const PROFILE_ACTION_QUIET_PERIOD: Duration = Duration::from_millis(250);
const IO_POLL: Duration = Duration::from_millis(20);
const XMONAD_RESIZE_TIMEOUT_MSEC: u32 = 2_000;
const XMONAD_ADMISSION_RESIZE_TIMEOUT_MSEC: u32 = 2_000;
const FIRST_PRIVATE_X_DISPLAY: u16 = 90;
const LAST_PRIVATE_X_DISPLAY: u16 = 4_095;
const X11_ANY_KEY: u8 = 0;
const X11_ANY_MODIFIER: u16 = 1 << 15;
const X11_SHIFT_MASK: u16 = 1;
const X11_CONTROL_MASK: u16 = 1 << 2;
const X11_MOD1_MASK: u16 = 1 << 3;

#[derive(Debug)]
pub struct BridgeRuntimeError(String);
const MAX_PRIVATE_KEY_GRABS: usize = 256;

impl BridgeRuntimeError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for BridgeRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for BridgeRuntimeError {}

impl From<X11WmBridgeError> for BridgeRuntimeError {
    fn from(error: X11WmBridgeError) -> Self {
        Self::new(format!("bridge state rejected request: {error:?}"))
    }
}

#[derive(Clone, Debug)]
enum ServerCommand {
    Root(Rect),
    Map(SyntheticXWindowId, Rect, SyntheticManageProfile),
    Configure {
        window: SyntheticXWindowId,
        geometry: Rect,
        notify_root: bool,
    },
    ManageProfile {
        window: SyntheticXWindowId,
        profile: SyntheticManageProfile,
    },
    Unmap(SyntheticXWindowId),
    Destroy(SyntheticXWindowId),
    Key {
        chord: SyntheticKeyChord,
        pressed: bool,
    },
    ValidateKeyGrab {
        chord: SyntheticKeyChord,
        reply: SyncSender<bool>,
    },
    Button {
        window: SyntheticXWindowId,
        button: u8,
        modifiers: u16,
        root_x: Option<i16>,
        root_y: Option<i16>,
        pressed: bool,
    },
    PointerGesture {
        window: SyntheticXWindowId,
        button: u8,
        modifiers: u16,
        start_x: i16,
        start_y: i16,
        delta_x: i16,
        delta_y: i16,
    },
    Wake,
    QueryFocus(SyncSender<Option<SyntheticXWindowId>>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SyntheticKeyChord {
    keycode: u8,
    modifiers: u16,
}

#[derive(Debug, Default)]
struct LegacyResponseBatch {
    configured: BTreeMap<SyntheticXWindowId, LegacyWmRequest>,
    focus: Option<LegacyWmRequest>,
}

#[derive(Debug, Default)]
struct LegacyResponseExpectation {
    configured: BTreeSet<SyntheticXWindowId>,
    map_admissions: BTreeSet<SyntheticXWindowId>,
}

fn xmonad_profile_chord(action: u64) -> Option<SyntheticKeyChord> {
    let (keycode, modifiers) = match action {
        XMONAD_ACTION_FOCUS_NEXT => (b'j', X11_MOD1_MASK),
        XMONAD_ACTION_FOCUS_PREVIOUS => (b'k', X11_MOD1_MASK),
        XMONAD_ACTION_FOCUS_MASTER => (b'm', X11_MOD1_MASK),
        XMONAD_ACTION_SWAP_MASTER => (b'm', X11_MOD1_MASK | X11_SHIFT_MASK),
        XMONAD_ACTION_SWAP_DOWN => (b'j', X11_MOD1_MASK | X11_SHIFT_MASK),
        XMONAD_ACTION_SWAP_UP => (b'k', X11_MOD1_MASK | X11_SHIFT_MASK),
        XMONAD_ACTION_SHRINK => (b'h', X11_MOD1_MASK),
        XMONAD_ACTION_EXPAND => (b'l', X11_MOD1_MASK),
        XMONAD_ACTION_NEXT_LAYOUT => (b' ', X11_MOD1_MASK),
        XMONAD_ACTION_RESET_LAYOUT => (b' ', X11_MOD1_MASK | X11_SHIFT_MASK),
        XMONAD_ACTION_TOGGLE_FLOATING => (b' ', X11_MOD1_MASK | X11_CONTROL_MASK),
        XMONAD_ACTION_SINK => (b't', X11_MOD1_MASK),
        XMONAD_ACTION_INCREASE_MASTER_COUNT => (b',', X11_MOD1_MASK),
        XMONAD_ACTION_DECREASE_MASTER_COUNT => (b'.', X11_MOD1_MASK),
        _ => return None,
    };
    Some(SyntheticKeyChord { keycode, modifiers })
}

#[derive(Clone, Copy, Debug)]
struct XmonadFloatingToggle {
    window: SyntheticXWindowId,
    surface: sophia_protocol::SurfaceId,
    was_floating: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyWmLaunchSpec {
    executable: PathBuf,
    arguments: Vec<OsString>,
    private_executable_alias: Option<PathBuf>,
    profile: LegacyWmProfile,
}

impl LegacyWmLaunchSpec {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            arguments: Vec::new(),
            private_executable_alias: None,
            profile: LegacyWmProfile::LayoutOnly,
        }
    }

    pub fn arg(mut self, argument: impl Into<OsString>) -> Self {
        self.arguments.push(argument.into());
        self
    }

    pub fn with_profile(mut self, profile: LegacyWmProfile) -> Self {
        self.profile = profile;
        self
    }

    /// Stages the executable at a path relative to the bridge's private
    /// `XDG_CONFIG_HOME` and launches that alias. This is a generic escape hatch
    /// for WMs that re-exec a compiled/configured binary from a fixed location.
    pub fn with_private_executable_alias(mut self, relative_path: impl Into<PathBuf>) -> Self {
        self.private_executable_alias = Some(relative_path.into());
        self
    }
}

pub struct LegacyX11WmBridgeRuntime {
    bridge: X11WmBridgeState,
    commands: Option<SyncSender<ServerCommand>>,
    legacy: Receiver<LegacyWmRequest>,
    worker: Option<JoinHandle<Result<(), BridgeRuntimeError>>>,
    child: Child,
    display: u16,
    display_lease: File,
    config_dir: PathBuf,
    profile: LegacyWmProfile,
    session: Option<LegacySessionDescriptor>,
    request_failed: bool,
}

impl LegacyX11WmBridgeRuntime {
    pub fn start(spec: LegacyWmLaunchSpec) -> Result<Self, BridgeRuntimeError> {
        Self::start_with_root(
            spec,
            Rect {
                x: 0,
                y: 0,
                width: 1280,
                height: 720,
            },
        )
    }

    pub fn start_with_root(
        spec: LegacyWmLaunchSpec,
        initial_root: Rect,
    ) -> Result<Self, BridgeRuntimeError> {
        let executable = resolve_executable(&spec.executable)?;
        let profile = spec.profile;
        if let Some(relative_alias) = spec.private_executable_alias.as_deref() {
            validate_private_executable_alias(relative_alias)?;
        }
        let private_display = bind_private_display()?;
        let display = private_display.display;
        let socket_path = private_display.socket_path;
        let config_dir = std::env::temp_dir().join(format!(
            "sophia-x11-wm-bridge-{}-{display}",
            std::process::id()
        ));
        fs::create_dir_all(&config_dir).map_err(|error| {
            BridgeRuntimeError::new(format!(
                "failed to create private legacy WM config directory {}: {error}",
                config_dir.display()
            ))
        })?;
        let launch_path = if let Some(relative_alias) = spec.private_executable_alias {
            let staged = config_dir.join(relative_alias);
            let parent = staged.parent().ok_or_else(|| {
                BridgeRuntimeError::new("private legacy WM executable alias has no parent")
            })?;
            fs::create_dir_all(parent).map_err(|error| {
                BridgeRuntimeError::new(format!(
                    "failed to create private legacy WM runtime directory {}: {error}",
                    parent.display()
                ))
            })?;
            symlink(&executable, &staged).map_err(|error| {
                BridgeRuntimeError::new(format!(
                    "failed to stage private legacy WM executable: {error}"
                ))
            })?;
            staged
        } else {
            executable.clone()
        };

        let mut child = Command::new(&launch_path)
            .args(spec.arguments)
            .env_clear()
            .env("DISPLAY", format!(":{display}"))
            .env("HOME", &config_dir)
            .env("XDG_CONFIG_HOME", &config_dir)
            .env("XDG_CACHE_HOME", &config_dir)
            .env("XDG_DATA_HOME", &config_dir)
            .env("LANG", "C.UTF-8")
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| {
                BridgeRuntimeError::new(format!(
                    "failed to start legacy X11 WM {}: {error}",
                    executable.display()
                ))
            })?;

        let stream = match accept_private_legacy_wm(&private_display.listener, &mut child) {
            Ok(stream) => stream,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(&socket_path);
                let _ = fs::remove_dir_all(&config_dir);
                return Err(error);
            }
        };
        fs::remove_file(&socket_path).map_err(|error| {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_dir_all(&config_dir);
            BridgeRuntimeError::new(format!(
                "failed to unlink accepted private X socket {}: {error}",
                socket_path.display()
            ))
        })?;

        let (command_tx, command_rx) = mpsc::sync_channel(128);
        let (legacy_tx, legacy_rx) = mpsc::sync_channel(256);
        let worker = thread::spawn(move || {
            let mut stream = stream;
            serve_legacy_wm(&mut stream, command_rx, legacy_tx, initial_root)
        });

        Ok(Self {
            bridge: X11WmBridgeState::new(),
            commands: Some(command_tx),
            legacy: legacy_rx,
            worker: Some(worker),
            child,
            display,
            display_lease: private_display.lease,
            profile,
            session: None,
            request_failed: false,
            config_dir,
        })
    }

    pub fn handle_request(
        &mut self,
        request: &WmRequestPacket,
    ) -> Result<WmResponsePacket, BridgeRuntimeError> {
        if self.request_failed {
            return Err(BridgeRuntimeError::new(
                "legacy WM bridge must be restarted after a request failure",
            ));
        }
        let result = self.handle_request_once(request);
        if result.is_err() {
            // Legacy X requests carry no Sophia transaction identity. Once a
            // request fails, only process replacement can prove that no old
            // reply will be attributed to later Engine work.
            self.request_failed = true;
        }
        result
    }

    fn handle_request_once(
        &mut self,
        request: &WmRequestPacket,
    ) -> Result<WmResponsePacket, BridgeRuntimeError> {
        // The preceding successful request already reached quiet. This drain
        // is defensive channel cleanup, not the attribution boundary.
        while self.legacy.try_recv().is_ok() {}

        if self.profile == LegacyWmProfile::Xmonad {
            let session = self
                .session
                .as_ref()
                .ok_or_else(|| BridgeRuntimeError::new("WM session was not negotiated"))?;
            if let Some(response) = translate_xmonad_profile_action(request, session)? {
                let mut update = BridgeEngineUpdate {
                    transaction: request.transaction,
                    events: Vec::new(),
                };
                for command in &response.commands {
                    let direct = match *command {
                        WmCommand::ActivateWorkspace { workspace, .. } => Some(
                            self.bridge
                                .activate_workspace(request.transaction, workspace),
                        ),
                        WmCommand::AssignWorkspace { surface, workspace } => {
                            // A move packet carries the complete source view.
                            // Reconcile it before applying the speculative
                            // assignment returned to Engine.
                            let projection = self.bridge.apply_engine_request(request)?;
                            update.events.extend(projection.events);
                            Some(self.bridge.assign_workspace(
                                request.transaction,
                                surface,
                                workspace,
                            )?)
                        }
                        _ => None,
                    };
                    if let Some(direct) = direct {
                        update.events.extend(direct.events);
                    }
                }
                send_engine_update(
                    &self.bridge,
                    &update,
                    self.commands
                        .as_ref()
                        .ok_or_else(|| BridgeRuntimeError::new("legacy WM server stopped"))?,
                )?;
                if !update.events.is_empty() {
                    // Direct commands do not consume the WM's proposed layout,
                    // but their synthetic events must still settle before a
                    // successor request can own any legacy reply.
                    self.collect_legacy_responses(&BTreeSet::new(), false, QUIET_PERIOD)?;
                }
                return Ok(response);
            }
        }

        let profiled_chord = match &request.kind {
            WmRequestKind::ActionActivated(activation)
                if self.profile == LegacyWmProfile::Xmonad =>
            {
                if activation.action.raw() == XMONAD_ACTION_TOGGLE_FLOATING {
                    activation
                        .focused_surface
                        .and_then(|surface| {
                            activation.nodes.iter().find(|node| node.surface == surface)
                        })
                        .filter(|node| node.state.floating)
                        .map(|_| SyntheticKeyChord {
                            keycode: 116,
                            modifiers: X11_MOD1_MASK,
                        })
                } else {
                    xmonad_profile_chord(activation.action.raw())
                }
            }
            _ => None,
        };
        let floating_toggle = match &request.kind {
            WmRequestKind::ActionActivated(activation)
                if activation.action.raw() == XMONAD_ACTION_TOGGLE_FLOATING =>
            {
                activation.focused_surface.and_then(|surface| {
                    let node = activation
                        .nodes
                        .iter()
                        .find(|node| node.surface == surface)?;
                    Some(XmonadFloatingToggle {
                        window: self.bridge.synthetic_window(surface)?,
                        surface,
                        was_floating: node.state.floating,
                    })
                })
            }
            _ => None,
        };
        let pointer_gesture = match request.kind {
            WmRequestKind::PointerGestureCompleted(gesture) => Some(gesture),
            _ => None,
        };
        let profiled_focus = match &request.kind {
            WmRequestKind::ActionActivated(activation) => match activation.action.raw() {
                XMONAD_ACTION_FOCUS_NEXT => activation
                    .focused_surface
                    .and_then(|surface| self.bridge.cycle_focus_window(surface, true))
                    // Moving the focused window clears Engine focus before a
                    // later workspace view. A one-window workspace still has
                    // one exact xmonad target and needs no X11 event to find it.
                    .or_else(|| {
                        let [node] = activation.nodes.as_slice() else {
                            return None;
                        };
                        self.bridge.synthetic_window(node.surface)
                    }),
                XMONAD_ACTION_FOCUS_PREVIOUS => activation
                    .focused_surface
                    .and_then(|surface| self.bridge.cycle_focus_window(surface, false))
                    .or_else(|| {
                        let [node] = activation.nodes.as_slice() else {
                            return None;
                        };
                        self.bridge.synthetic_window(node.surface)
                    }),
                _ => None,
            },
            WmRequestKind::FocusRequested(focus) => self.bridge.synthetic_window(focus.surface),
            _ => None,
        };
        let deterministic_unfocused_cycle = matches!(
            &request.kind,
            WmRequestKind::ActionActivated(activation)
                if matches!(
                    activation.action.raw(),
                    XMONAD_ACTION_FOCUS_NEXT | XMONAD_ACTION_FOCUS_PREVIOUS
                )
                    && activation.focused_surface.is_none()
                    && activation.nodes.len() == 1
                    && profiled_focus.is_some()
        );

        let update = self.bridge.apply_engine_request(request)?;
        let expected = send_engine_update(
            &self.bridge,
            &update,
            self.commands
                .as_ref()
                .ok_or_else(|| BridgeRuntimeError::new("legacy WM server stopped"))?,
        )?;
        let managed_focus = match &request.kind {
            WmRequestKind::ManageSurface(manage) => {
                self.bridge.synthetic_window(manage.node.surface)
            }
            _ => None,
        };

        if profiled_chord.is_some()
            && !deterministic_unfocused_cycle
            && !expected.configured.is_empty()
        {
            // A sole retained node already has committed Engine geometry.
            // X11 permits its remap to produce no redundant configure, while
            // every action that still needs layout policy keeps this fence.
            self.collect_legacy_responses(&expected.map_admissions, false, QUIET_PERIOD)?;
        }
        if let Some(chord) = profiled_chord {
            let commands = self
                .commands
                .as_ref()
                .ok_or_else(|| BridgeRuntimeError::new("legacy WM server stopped"))?;
            let (grab_sender, grab_receiver) = mpsc::sync_channel(1);
            commands
                .send(ServerCommand::ValidateKeyGrab {
                    chord,
                    reply: grab_sender,
                })
                .map_err(|_| BridgeRuntimeError::new("legacy WM server stopped"))?;
            let grabbed = grab_receiver
                .recv_timeout(Duration::from_millis(500))
                .map_err(|_| BridgeRuntimeError::new("legacy WM key-grab query timed out"))?;
            if !grabbed {
                return Err(BridgeRuntimeError::new(format!(
                    "profile key chord was not registered by the legacy WM: keycode={} modifiers=0x{:x}",
                    chord.keycode, chord.modifiers
                )));
            }
            for pressed in [true, false] {
                commands
                    .send(ServerCommand::Key { chord, pressed })
                    .map_err(|_| BridgeRuntimeError::new("legacy WM server stopped"))?;
            }
            commands
                .send(ServerCommand::Wake)
                .map_err(|_| BridgeRuntimeError::new("legacy WM server stopped"))?;
        }
        if let Some(toggle) = floating_toggle.filter(|toggle| !toggle.was_floating) {
            let commands = self
                .commands
                .as_ref()
                .ok_or_else(|| BridgeRuntimeError::new("legacy WM server stopped"))?;
            for pressed in [true, false] {
                commands
                    .send(ServerCommand::Button {
                        window: toggle.window,
                        button: 1,
                        modifiers: X11_MOD1_MASK,
                        root_x: None,
                        root_y: None,
                        pressed,
                    })
                    .map_err(|_| BridgeRuntimeError::new("legacy WM server stopped"))?;
            }
            commands
                .send(ServerCommand::Wake)
                .map_err(|_| BridgeRuntimeError::new("legacy WM server stopped"))?;
        }
        if let Some(gesture) = pointer_gesture {
            let window = self
                .bridge
                .synthetic_window(gesture.surface)
                .ok_or_else(|| {
                    BridgeRuntimeError::new("pointer gesture targeted an unknown surface")
                })?;
            let button = match gesture.mode {
                sophia_protocol::WmPointerGestureMode::Move => 1,
                sophia_protocol::WmPointerGestureMode::Resize => 3,
            };
            let coordinate = |value: i32| {
                i16::try_from(value).unwrap_or(if value < 0 { i16::MIN } else { i16::MAX })
            };
            let commands = self
                .commands
                .as_ref()
                .ok_or_else(|| BridgeRuntimeError::new("legacy WM server stopped"))?;
            commands
                .send(ServerCommand::PointerGesture {
                    window,
                    button,
                    modifiers: X11_MOD1_MASK,
                    start_x: coordinate(gesture.start.x),
                    start_y: coordinate(gesture.start.y),
                    delta_x: coordinate(gesture.end.x.saturating_sub(gesture.start.x)),
                    delta_y: coordinate(gesture.end.y.saturating_sub(gesture.start.y)),
                })
                .map_err(|_| BridgeRuntimeError::new("legacy WM server stopped"))?;
        }
        let synchronized_focus = if self.profile == LegacyWmProfile::Xmonad {
            match request.kind {
                WmRequestKind::ManageSurface(_) => managed_focus,
                WmRequestKind::FocusRequested(_) => profiled_focus,
                _ => None,
            }
        } else {
            None
        };
        if let Some(window) = synchronized_focus {
            let commands = self
                .commands
                .as_ref()
                .ok_or_else(|| BridgeRuntimeError::new("legacy WM server stopped"))?;
            for pressed in [true, false] {
                commands
                    .send(ServerCommand::Button {
                        window,
                        button: 1,
                        modifiers: 0,
                        root_x: None,
                        root_y: None,
                        pressed,
                    })
                    .map_err(|_| BridgeRuntimeError::new("legacy WM server stopped"))?;
            }
        }
        let response_batch = if pointer_gesture.is_some() {
            self.collect_legacy_responses(&BTreeSet::new(), true, QUIET_PERIOD)?
        } else if profiled_chord.is_some() {
            // A registered grab proves that xmonad accepted the action. Some
            // valid actions are state-dependent no-ops, so quiet—not geometry
            // churn—is the response boundary.
            self.collect_legacy_responses(&BTreeSet::new(), false, PROFILE_ACTION_QUIET_PERIOD)?
        } else {
            self.collect_legacy_responses(&expected.configured, false, QUIET_PERIOD)?
        };
        let mut requests = if matches!(request.kind, WmRequestKind::FocusRequested(_)) {
            Vec::new()
        } else {
            response_batch.configured.into_values().collect::<Vec<_>>()
        };
        if let Some(window) = profiled_focus {
            requests.push(LegacyWmRequest::FocusWindow { window });
        } else if let Some(window) = managed_focus {
            requests.push(LegacyWmRequest::FocusWindow { window });
        } else if let Some(focus) = response_batch.focus {
            requests.push(focus);
        } else {
            let (focus_sender, focus_receiver) = mpsc::sync_channel(1);
            self.commands
                .as_ref()
                .ok_or_else(|| BridgeRuntimeError::new("legacy WM server stopped"))?
                .send(ServerCommand::QueryFocus(focus_sender))
                .map_err(|_| BridgeRuntimeError::new("legacy WM server stopped"))?;
            let queried_focus = focus_receiver
                .recv_timeout(Duration::from_millis(500))
                .map_err(|_| BridgeRuntimeError::new("legacy WM focus query timed out"))?;
            if let Some(window) = queried_focus {
                requests.push(LegacyWmRequest::FocusWindow { window });
            }
        }
        let resize_timeout_msec = if matches!(&request.kind, WmRequestKind::ManageSurface(_)) {
            XMONAD_ADMISSION_RESIZE_TIMEOUT_MSEC
        } else {
            XMONAD_RESIZE_TIMEOUT_MSEC
        };
        let response_output = match &request.kind {
            WmRequestKind::ManageSurface(manage) => Some(manage.output),
            WmRequestKind::RelayoutWorkspace(relayout) => Some(relayout.output),
            WmRequestKind::ActionActivated(activation) => Some(activation.output),
            WmRequestKind::FocusRequested(focus) => Some(focus.output),
            WmRequestKind::PointerGestureCompleted(gesture) => Some(gesture.output),
            WmRequestKind::SurfaceRemoved { .. } => None,
        };
        let mut response = self
            .bridge
            .translate_legacy_requests_for_output(
                request.transaction,
                &requests,
                resize_timeout_msec,
                response_output,
            )
            .map_err(BridgeRuntimeError::from)?;
        if let Some(toggle) = floating_toggle {
            response.commands.push(WmCommand::SetFloating {
                surface: toggle.surface,
                floating: !toggle.was_floating,
            });
        }
        if let WmRequestKind::ActionActivated(activation) = &request.kind
            && activation.action.raw() == XMONAD_ACTION_SINK
            && let Some(surface) = activation.focused_surface
        {
            response.commands.push(WmCommand::SetFloating {
                surface,
                floating: false,
            });
        }
        if let Some(gesture) = pointer_gesture {
            response.commands.push(WmCommand::SetFloating {
                surface: gesture.surface,
                floating: true,
            });
        }
        Ok(response)
    }

    fn collect_legacy_responses(
        &mut self,
        expected: &BTreeSet<SyntheticXWindowId>,
        require_activity: bool,
        quiet_period: Duration,
    ) -> Result<LegacyResponseBatch, BridgeRuntimeError> {
        let started = Instant::now();
        let mut last_activity = started;
        let mut observed_activity = false;
        let mut quiet_boundary = false;
        let mut batch = LegacyResponseBatch::default();
        loop {
            let elapsed = started.elapsed();
            if elapsed >= BRIDGE_TIMEOUT {
                break;
            }
            let wait = IO_POLL.min(BRIDGE_TIMEOUT.saturating_sub(elapsed));
            match self.legacy.recv_timeout(wait) {
                Ok(request) => {
                    observed_activity = true;
                    last_activity = Instant::now();
                    match request {
                        request @ LegacyWmRequest::ConfigureWindow { window, .. } => {
                            batch.configured.insert(window, request);
                        }
                        request @ LegacyWmRequest::FocusWindow { .. } => {
                            batch.focus = Some(request)
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    let expected_complete = expected
                        .iter()
                        .all(|window| batch.configured.contains_key(window));
                    if expected_complete
                        && (!require_activity || observed_activity)
                        && last_activity.elapsed() >= quiet_period
                    {
                        quiet_boundary = true;
                        break;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    let detail = self
                        .worker
                        .take()
                        .and_then(|worker| worker.join().ok())
                        .and_then(Result::err)
                        .map_or_else(
                            || "legacy WM server disconnected".to_owned(),
                            |error| format!("legacy WM server disconnected: {error}"),
                        );
                    return Err(BridgeRuntimeError::new(detail));
                }
            }
        }

        let expected_complete = expected
            .iter()
            .all(|window| batch.configured.contains_key(window));
        if !expected_complete {
            return Err(BridgeRuntimeError::new(format!(
                "legacy WM did not configure all {} synthetic windows within {} ms (configured {})",
                expected.len(),
                BRIDGE_TIMEOUT.as_millis(),
                batch.configured.len()
            )));
        }
        if require_activity && !observed_activity {
            return Err(BridgeRuntimeError::new(format!(
                "legacy WM produced no post-action response within {} ms",
                BRIDGE_TIMEOUT.as_millis()
            )));
        }
        if !quiet_boundary {
            return Err(BridgeRuntimeError::new(format!(
                "legacy WM response did not reach a quiet boundary within {} ms",
                BRIDGE_TIMEOUT.as_millis()
            )));
        }
        Ok(batch)
    }

    pub fn profile(&self) -> LegacyWmProfile {
        self.profile
    }

    pub fn configure_session(
        &mut self,
        descriptor: LegacySessionDescriptor,
    ) -> Result<(), BridgeRuntimeError> {
        self.session = Some(descriptor);
        Ok(())
    }

    pub fn bridge(&self) -> &X11WmBridgeState {
        &self.bridge
    }

    pub const fn private_display(&self) -> u16 {
        self.display
    }
}

fn validate_private_executable_alias(path: &Path) -> Result<(), BridgeRuntimeError> {
    if path.as_os_str().is_empty()
        || !path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
    {
        return Err(BridgeRuntimeError::new(
            "private legacy WM executable alias must be a non-empty relative path below XDG_CONFIG_HOME",
        ));
    }
    Ok(())
}

fn accept_private_legacy_wm(
    listener: &UnixListener,
    child: &mut Child,
) -> Result<UnixStream, BridgeRuntimeError> {
    listener.set_nonblocking(true).map_err(|error| {
        BridgeRuntimeError::new(format!("failed to configure private X listener: {error}"))
    })?;
    let started = Instant::now();
    loop {
        match listener.accept() {
            Ok((stream, _)) => return Ok(stream),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Err(error) => {
                return Err(BridgeRuntimeError::new(format!(
                    "failed to accept private legacy WM socket: {error}"
                )));
            }
        }
        if let Some(status) = child.try_wait().map_err(|error| {
            BridgeRuntimeError::new(format!("failed to inspect legacy WM process: {error}"))
        })? {
            return Err(BridgeRuntimeError::new(format!(
                "legacy WM exited before connecting to its private display: {status}"
            )));
        }
        if started.elapsed() >= BRIDGE_TIMEOUT {
            return Err(BridgeRuntimeError::new(format!(
                "legacy WM did not connect to its private display within {} ms",
                BRIDGE_TIMEOUT.as_millis()
            )));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn resolve_executable(path: &Path) -> Result<PathBuf, BridgeRuntimeError> {
    if path.components().count() > 1 {
        return fs::canonicalize(path).map_err(|error| {
            BridgeRuntimeError::new(format!(
                "failed to resolve legacy WM executable {}: {error}",
                path.display()
            ))
        });
    }
    let search_path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&search_path)
        .map(|directory| directory.join(path))
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| fs::canonicalize(candidate).ok())
        .ok_or_else(|| {
            BridgeRuntimeError::new(format!(
                "legacy WM executable '{}' was not found in PATH",
                path.display()
            ))
        })
}

impl Drop for LegacyX11WmBridgeRuntime {
    fn drop(&mut self) {
        self.commands.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let _ = fs::remove_dir_all(&self.config_dir);
        let _ = self.display_lease.unlock();
    }
}

include!("runtime/server.rs");
include!("runtime/dispatch.rs");
include!("runtime/replies.rs");
