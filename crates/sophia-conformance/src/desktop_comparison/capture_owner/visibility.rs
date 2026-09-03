//! Passive, stack-native observation for desktop-comparison workloads.
//!
//! Application identity is consumed only here, by the trusted conformance
//! owner. The normalized evidence carries counts and placement facts; none of
//! the identity used for correlation crosses the blind WM boundary.

use super::parse_proc_stat;
use niri_ipc::{Request, Response};
use std::fs;
use std::time::{Duration, Instant};
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt as _, MapState, Window};
use x11rb::rust_connection::RustConnection;

const DP1_WIDTH: i64 = 2_560;
const DP1_HEIGHT: i64 = 1_440;
const MINIMUM_APPLICATION_EXTENT: u16 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ProcessIdentity {
    pid: u32,
    start_ticks: u64,
}

impl ProcessIdentity {
    pub(super) fn read(pid: u32) -> Result<Self, String> {
        let source = fs::read_to_string(format!("/proc/{pid}/stat"))
            .map_err(|error| format!("could not bind workload process {pid}: {error}"))?;
        Ok(Self {
            pid,
            start_ticks: parse_proc_stat(&source)?.start_ticks,
        })
    }

    pub(super) const fn pid(self) -> u32 {
        self.pid
    }

    fn owns(self, candidate: u32) -> bool {
        let mut current = candidate;
        for _ in 0..256 {
            let Ok(source) = fs::read_to_string(format!("/proc/{current}/stat")) else {
                return false;
            };
            let Ok(stat) = parse_proc_stat(&source) else {
                return false;
            };
            if current == self.pid {
                return stat.start_ticks == self.start_ticks;
            }
            if stat.ppid == 0 || stat.ppid == current {
                return false;
            }
            current = stat.ppid;
        }
        false
    }
}

pub(super) fn process_descends_from(candidate: u32, root: u32) -> Result<bool, String> {
    let root = ProcessIdentity::read(root)?;
    Ok(root.owns(candidate))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct VisibilityObservation {
    pub(super) owned_toplevels: u64,
    pub(super) visible_dp1: u64,
    pub(super) foreign_toplevels: u64,
    pub(super) focused_visible_dp1: bool,
}

impl VisibilityObservation {
    pub(super) fn require_empty(self) -> Result<(), String> {
        if self.owned_toplevels == 0
            && self.visible_dp1 == 0
            && self.foreign_toplevels == 0
            && !self.focused_visible_dp1
        {
            Ok(())
        } else {
            Err("comparison session contains an application before workload launch".to_owned())
        }
    }

    pub(super) fn require_visible(self) -> Result<(), String> {
        if self.owned_toplevels == 0 {
            return Err("comparison workload has no owned toplevel".to_owned());
        }
        if self.foreign_toplevels != 0 {
            return Err("comparison session contains a foreign application toplevel".to_owned());
        }
        if self.visible_dp1 == 0 {
            return Err("comparison workload is ready but not visible on DP-1".to_owned());
        }
        if !self.focused_visible_dp1 {
            return Err(
                "comparison workload is not the focused visible DP-1 application".to_owned(),
            );
        }
        Ok(())
    }
}

pub(super) enum VisibilityProbe {
    X11(X11Probe),
    Niri(NiriProbe),
}

impl VisibilityProbe {
    pub(super) fn connect(stack: &str) -> Result<Self, String> {
        match stack {
            "sophia" | "xlibre-xmonad" => X11Probe::connect().map(Self::X11),
            "niri" => NiriProbe::connect().map(Self::Niri),
            _ => Err("prepared schedule contains an unknown stack".to_owned()),
        }
    }

    pub(super) fn observe(
        &mut self,
        roots: &[ProcessIdentity],
    ) -> Result<VisibilityObservation, String> {
        match self {
            Self::X11(probe) => probe.observe(roots),
            Self::Niri(probe) => probe.observe(roots),
        }
    }

    pub(super) fn wait_visible(
        &mut self,
        roots: &[ProcessIdentity],
        timeout: Duration,
    ) -> Result<VisibilityObservation, String> {
        let deadline = Instant::now() + timeout;
        let mut last = None;
        while Instant::now() < deadline {
            match self.observe(roots) {
                Ok(observation) if observation.require_visible().is_ok() => {
                    return Ok(observation);
                }
                Ok(observation) => last = Some(observation),
                Err(_) => {}
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let detail = last.map_or_else(
            || "no observation was available".to_owned(),
            |observation| {
                format!(
                    "owned={} visible_dp1={} foreign={} focused_visible_dp1={}",
                    observation.owned_toplevels,
                    observation.visible_dp1,
                    observation.foreign_toplevels,
                    observation.focused_visible_dp1,
                )
            },
        );
        Err(format!(
            "comparison workload did not become focused and visible on DP-1 within 30 seconds ({detail})"
        ))
    }
}

pub(super) struct X11Probe {
    connection: RustConnection,
    root: Window,
    wm_pid: Atom,
}

impl X11Probe {
    fn connect() -> Result<Self, String> {
        let (connection, screen) = x11rb::connect(None)
            .map_err(|error| format!("could not connect visibility probe to X11: {error}"))?;
        let root = connection
            .setup()
            .roots
            .get(screen)
            .ok_or("X11 visibility probe selected an unknown screen")?
            .root;
        let wm_pid = connection
            .intern_atom(false, b"_NET_WM_PID")
            .map_err(|error| format!("could not request _NET_WM_PID: {error}"))?
            .reply()
            .map_err(|error| format!("could not resolve _NET_WM_PID: {error}"))?
            .atom;
        Ok(Self {
            connection,
            root,
            wm_pid,
        })
    }

    fn observe(&self, roots: &[ProcessIdentity]) -> Result<VisibilityObservation, String> {
        let focus = self
            .connection
            .get_input_focus()
            .map_err(|error| format!("could not query X11 input focus: {error}"))?
            .reply()
            .map_err(|error| format!("could not read X11 input focus: {error}"))?
            .focus;
        let focused_toplevel = self.top_level_for(focus);
        let children = self
            .connection
            .query_tree(self.root)
            .map_err(|error| format!("could not query X11 root tree: {error}"))?
            .reply()
            .map_err(|error| format!("could not read X11 root tree: {error}"))?
            .children;
        let mut result = VisibilityObservation::default();
        for window in children {
            let Ok(attributes) = self.connection.get_window_attributes(window) else {
                continue;
            };
            let Ok(attributes) = attributes.reply() else {
                continue;
            };
            if attributes.map_state != MapState::VIEWABLE || attributes.override_redirect {
                continue;
            }
            let Ok(geometry) = self.connection.get_geometry(window) else {
                continue;
            };
            let Ok(geometry) = geometry.reply() else {
                continue;
            };
            if geometry.width < MINIMUM_APPLICATION_EXTENT
                || geometry.height < MINIMUM_APPLICATION_EXTENT
            {
                continue;
            }
            let Ok(translated) = self
                .connection
                .translate_coordinates(window, self.root, 0, 0)
            else {
                continue;
            };
            let Ok(translated) = translated.reply() else {
                continue;
            };
            let owner = self.window_pid(window);
            let owned = owner.is_some_and(|pid| roots.iter().any(|root| root.owns(pid)));
            if !owned {
                result.foreign_toplevels = result.foreign_toplevels.saturating_add(1);
                continue;
            }
            result.owned_toplevels = result.owned_toplevels.saturating_add(1);
            let center_x = i64::from(translated.dst_x) + i64::from(geometry.width) / 2;
            let center_y = i64::from(translated.dst_y) + i64::from(geometry.height) / 2;
            let on_dp1 = (0..DP1_WIDTH).contains(&center_x) && (0..DP1_HEIGHT).contains(&center_y);
            if on_dp1 {
                result.visible_dp1 = result.visible_dp1.saturating_add(1);
                if focused_toplevel == Some(window) {
                    result.focused_visible_dp1 = true;
                }
            }
        }
        Ok(result)
    }

    fn window_pid(&self, window: Window) -> Option<u32> {
        let cookie = self
            .connection
            .get_property(false, window, self.wm_pid, AtomEnum::CARDINAL, 0, 1)
            .ok()?;
        let reply = cookie.reply().ok()?;
        reply.value32()?.next()
    }

    fn top_level_for(&self, focus: Window) -> Option<Window> {
        if focus == 0 || focus == self.root {
            return None;
        }
        let mut current = focus;
        for _ in 0..256 {
            let tree = self.connection.query_tree(current).ok()?.reply().ok()?;
            if tree.parent == self.root {
                return Some(current);
            }
            if tree.parent == 0 || tree.parent == current {
                return None;
            }
            current = tree.parent;
        }
        None
    }
}

pub(super) struct NiriProbe {
    socket: niri_ipc::socket::Socket,
}

impl NiriProbe {
    fn connect() -> Result<Self, String> {
        niri_ipc::socket::Socket::connect()
            .map(|socket| Self { socket })
            .map_err(|error| format!("could not connect to niri IPC: {error}"))
    }

    fn observe(&mut self, roots: &[ProcessIdentity]) -> Result<VisibilityObservation, String> {
        let workspaces = match self
            .socket
            .send(Request::Workspaces)
            .map_err(|error| format!("could not request niri workspaces: {error}"))?
            .map_err(|error| format!("niri refused workspace observation: {error}"))?
        {
            Response::Workspaces(workspaces) => workspaces,
            _ => return Err("niri returned the wrong workspace response".to_owned()),
        };
        let windows = match self
            .socket
            .send(Request::Windows)
            .map_err(|error| format!("could not request niri windows: {error}"))?
            .map_err(|error| format!("niri refused window observation: {error}"))?
        {
            Response::Windows(windows) => windows,
            _ => return Err("niri returned the wrong window response".to_owned()),
        };
        let active_dp1 = workspaces
            .iter()
            .filter(|workspace| workspace.is_active && workspace.output.as_deref() == Some("DP-1"))
            .map(|workspace| workspace.id)
            .collect::<std::collections::BTreeSet<_>>();
        let mut result = VisibilityObservation::default();
        for window in windows {
            let owned = window
                .pid
                .and_then(|pid| u32::try_from(pid).ok())
                .is_some_and(|pid| roots.iter().any(|root| root.owns(pid)));
            if !owned {
                result.foreign_toplevels = result.foreign_toplevels.saturating_add(1);
                continue;
            }
            result.owned_toplevels = result.owned_toplevels.saturating_add(1);
            let on_dp1 = window
                .workspace_id
                .is_some_and(|workspace| active_dp1.contains(&workspace));
            if on_dp1 {
                result.visible_dp1 = result.visible_dp1.saturating_add(1);
                if window.is_focused {
                    result.focused_visible_dp1 = true;
                }
            }
        }
        Ok(result)
    }
}

pub(super) fn format_record(
    phase: &str,
    sequence: u64,
    monotonic_usec: u64,
    observation: VisibilityObservation,
) -> String {
    format!(
        "desktop_comparison_visibility schema=1 phase={phase} seq={sequence} monotonic_usec={monotonic_usec} owned_toplevels={} visible_dp1={} foreign_toplevels={} focused_visible_dp1={}\n",
        observation.owned_toplevels,
        observation.visible_dp1,
        observation.foreign_toplevels,
        observation.focused_visible_dp1,
    )
}
