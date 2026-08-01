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

use sophia_protocol::{
    Rect, SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAX_PAYLOAD_LEN, WM_API_VERSION, WmCommand,
    WmRequestKind, WmRequestPacket, WmResponsePacket, WmSessionDescriptor, decode_wm_request_frame,
    decode_wm_session_descriptor_frame, encode_wm_hello_frame, encode_wm_response_frame,
};
use sophia_x_authority::{XByteOrder, serve_x11_setup_socket_client_with_root_size};

use crate::{
    BridgeEngineUpdate, LegacyWmProfile, LegacyWmRequest, SYNTHETIC_ROOT_XID,
    SyntheticManageProfile, SyntheticXEvent, SyntheticXWindowId, X11WmBridgeError,
    X11WmBridgeState, XMONAD_ACTION_FOCUS_NEXT, XMONAD_ACTION_FOCUS_PREVIOUS,
    XMONAD_ACTION_NEXT_LAYOUT, translate_xmonad_profile_action,
};

const FIRST_DYNAMIC_ATOM: u32 = 256;
const BRIDGE_TIMEOUT: Duration = Duration::from_secs(3);
const QUIET_PERIOD: Duration = Duration::from_millis(80);
const IO_POLL: Duration = Duration::from_millis(20);
const XMONAD_RESIZE_TIMEOUT_MSEC: u32 = 2_000;
const XMONAD_ADMISSION_RESIZE_TIMEOUT_MSEC: u32 = 2_000;
const FIRST_PRIVATE_X_DISPLAY: u16 = 90;
const LAST_PRIVATE_X_DISPLAY: u16 = 4_095;

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
    Configure(SyntheticXWindowId, Rect),
    Unmap(SyntheticXWindowId),
    Destroy(SyntheticXWindowId),
    Key {
        keycode: u8,
        pressed: bool,
    },
    Button {
        window: SyntheticXWindowId,
        button: u8,
        pressed: bool,
    },
    Wake,
    QueryFocus(SyncSender<Option<SyntheticXWindowId>>),
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
    session: Option<WmSessionDescriptor>,
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
            config_dir,
        })
    }

    pub fn handle_request(
        &mut self,
        request: &WmRequestPacket,
    ) -> Result<WmResponsePacket, BridgeRuntimeError> {
        if self.profile == LegacyWmProfile::Xmonad {
            let session = self
                .session
                .as_ref()
                .ok_or_else(|| BridgeRuntimeError::new("WM session was not negotiated"))?;
            if let Some(response) = translate_xmonad_profile_action(request, session)? {
                if let Some(workspace) =
                    response.commands.iter().find_map(|command| match command {
                        WmCommand::ActivateWorkspace { workspace, .. } => Some(*workspace),
                        _ => None,
                    })
                {
                    let update = self
                        .bridge
                        .activate_workspace(request.transaction, workspace);
                    let _ = send_engine_update(
                        &self.bridge,
                        &update,
                        self.commands
                            .as_ref()
                            .ok_or_else(|| BridgeRuntimeError::new("legacy WM server stopped"))?,
                    )?;
                }
                return Ok(response);
            }
        }

        let profiled_key = match &request.kind {
            WmRequestKind::ActionActivated(activation)
                if self.profile == LegacyWmProfile::Xmonad =>
            {
                match activation.action.raw() {
                    XMONAD_ACTION_FOCUS_NEXT => Some(106),
                    XMONAD_ACTION_FOCUS_PREVIOUS => Some(107),
                    XMONAD_ACTION_NEXT_LAYOUT => Some(32),
                    _ => None,
                }
            }
            _ => None,
        };
        let profiled_focus = match &request.kind {
            WmRequestKind::ActionActivated(activation) => match activation.action.raw() {
                XMONAD_ACTION_FOCUS_NEXT => activation
                    .focused_surface
                    .and_then(|surface| self.bridge.cycle_focus_window(surface, true)),
                XMONAD_ACTION_FOCUS_PREVIOUS => activation
                    .focused_surface
                    .and_then(|surface| self.bridge.cycle_focus_window(surface, false)),
                _ => None,
            },
            WmRequestKind::FocusRequested(focus) => self.bridge.synthetic_window(focus.surface),
            _ => None,
        };

        while self.legacy.try_recv().is_ok() {}
        let update = self.bridge.apply_engine_request(request)?;
        let expected = send_engine_update(
            &self.bridge,
            &update,
            self.commands
                .as_ref()
                .ok_or_else(|| BridgeRuntimeError::new("legacy WM server stopped"))?,
        )?;
        if let Some(keycode) = profiled_key {
            let commands = self
                .commands
                .as_ref()
                .ok_or_else(|| BridgeRuntimeError::new("legacy WM server stopped"))?;
            for pressed in [true, false] {
                commands
                    .send(ServerCommand::Key { keycode, pressed })
                    .map_err(|_| BridgeRuntimeError::new("legacy WM server stopped"))?;
            }
            commands
                .send(ServerCommand::Wake)
                .map_err(|_| BridgeRuntimeError::new("legacy WM server stopped"))?;
        }
        if self.profile == LegacyWmProfile::Xmonad
            && matches!(request.kind, WmRequestKind::FocusRequested(_))
            && let Some(window) = profiled_focus
        {
            let commands = self
                .commands
                .as_ref()
                .ok_or_else(|| BridgeRuntimeError::new("legacy WM server stopped"))?;
            for pressed in [true, false] {
                commands
                    .send(ServerCommand::Button {
                        window,
                        button: 1,
                        pressed,
                    })
                    .map_err(|_| BridgeRuntimeError::new("legacy WM server stopped"))?;
            }
        }

        let started = Instant::now();
        let mut last_activity = started;
        let mut configured = BTreeMap::new();
        let mut focus = None;
        loop {
            let elapsed = started.elapsed();
            if elapsed >= BRIDGE_TIMEOUT {
                break;
            }
            let wait = IO_POLL.min(BRIDGE_TIMEOUT.saturating_sub(elapsed));
            match self.legacy.recv_timeout(wait) {
                Ok(request) => {
                    last_activity = Instant::now();
                    match request {
                        request @ LegacyWmRequest::ConfigureWindow { window, .. } => {
                            configured.insert(window, request);
                        }
                        request @ LegacyWmRequest::FocusWindow { .. } => focus = Some(request),
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    if expected
                        .iter()
                        .all(|window| configured.contains_key(window))
                        && last_activity.elapsed() >= QUIET_PERIOD
                    {
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

        if !expected
            .iter()
            .all(|window| configured.contains_key(window))
        {
            return Err(BridgeRuntimeError::new(format!(
                "legacy WM did not configure all {} synthetic windows within {} ms (configured {})",
                expected.len(),
                BRIDGE_TIMEOUT.as_millis(),
                configured.len()
            )));
        }
        let mut requests = if matches!(request.kind, WmRequestKind::FocusRequested(_)) {
            Vec::new()
        } else {
            configured.into_values().collect::<Vec<_>>()
        };
        let managed_focus = match &request.kind {
            WmRequestKind::ManageSurface(manage) => {
                self.bridge.synthetic_window(manage.node.surface)
            }
            _ => None,
        };
        if let Some(window) = profiled_focus {
            requests.push(LegacyWmRequest::FocusWindow { window });
        } else if let Some(window) = managed_focus {
            requests.push(LegacyWmRequest::FocusWindow { window });
        } else if let Some(focus) = focus {
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
        self.bridge
            .translate_legacy_requests(request.transaction, &requests, resize_timeout_msec)
            .map_err(Into::into)
    }

    pub fn profile(&self) -> LegacyWmProfile {
        self.profile
    }

    pub fn configure_session(
        &mut self,
        descriptor: WmSessionDescriptor,
    ) -> Result<(), BridgeRuntimeError> {
        if descriptor.api_version != WM_API_VERSION {
            return Err(BridgeRuntimeError::new("unsupported Sophia WM API version"));
        }
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
