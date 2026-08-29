//! Development orchestration for the direct-scanout physical gate.

use std::ffi::OsString;
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{direct_scanout, direct_scanout_archive, profile};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Probe {
    pub width: u32,
    pub height: u32,
    pub hold_seconds: u32,
    pub workload: String,
    /// Whether the session drives an overlay over a directly scanned frame and
    /// the verification requires the return to composition.
    pub overlay_proof: bool,
}

impl Default for Probe {
    fn default() -> Self {
        Self {
            width: 2560,
            height: 1440,
            hold_seconds: 20,
            workload: "kitty".to_owned(),
            overlay_proof: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateReport {
    pub source_commit: String,
    pub sophia_sha256: String,
    pub client: PathBuf,
    pub client_sha256: String,
    pub archive: PathBuf,
}

impl Probe {
    pub fn from_arguments(arguments: &[String]) -> Result<Self, String> {
        let mut overlay_proof = false;
        let arguments = arguments
            .iter()
            .filter(|argument| {
                let flag = argument.as_str() == "--overlay-proof";
                overlay_proof |= flag;
                !flag
            })
            .cloned()
            .collect::<Vec<_>>();
        let arguments = arguments.as_slice();
        if arguments.len() > 4 {
            return Err(
                "direct-scanout run accepts WIDTH HEIGHT HOLD WORKLOAD as positional arguments"
                    .to_owned(),
            );
        }
        let mut probe = Self::default();
        if let Some(value) = arguments.first() {
            probe.width = positive(value, "width")?;
        }
        if let Some(value) = arguments.get(1) {
            probe.height = positive(value, "height")?;
        }
        if let Some(value) = arguments.get(2) {
            probe.hold_seconds = positive(value, "hold")?;
        }
        if let Some(value) = arguments.get(3) {
            probe.workload = value.to_owned();
        }
        if !["kitty", "glxgears", "vkcube", "xterm"].contains(&probe.workload.as_str()) {
            return Err(format!(
                "workload must be kitty, glxgears, vkcube, or xterm, got {:?}",
                probe.workload
            ));
        }
        probe.overlay_proof = overlay_proof;
        Ok(probe)
    }
}

pub fn run_probe(repo: &Path, probe: &Probe, client: Option<&Path>) -> Result<(), String> {
    profile::check_every_profile(&[])?;
    let mut command = Command::new(repo.join("tools/start_sophia_tty3.sh"));
    command
        .current_dir(repo)
        .env("SOPHIA_TTY_PROFILE", "standalone")
        .env("SOPHIA_SESSION_VERBOSE_TRACE", "true")
        .env("SOPHIA_ENABLE_DIRECT_SCANOUT", "1")
        .env("SOPHIA_STANDALONE_WORKLOAD", &probe.workload)
        .env("SOPHIA_STANDALONE_WIDTH", probe.width.to_string())
        .env("SOPHIA_STANDALONE_HEIGHT", probe.height.to_string())
        .env(
            "SOPHIA_STANDALONE_HOLD_SECONDS",
            probe.hold_seconds.to_string(),
        );
    if probe.overlay_proof {
        command.env("SOPHIA_DIRECT_OVERLAY_PROOF", "1");
    }
    if let Some(client) = client {
        command.env("SOPHIA_STANDALONE_APP_BIN", client);
    }
    let status = command
        .status()
        .map_err(|error| format!("could not start the direct-scanout session: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("the direct-scanout session exited with {status}"))
    }
}

pub fn run_gate(repo: &Path) -> Result<GateReport, String> {
    run_gate_with(repo, &Probe::default())
}

pub fn run_gate_with(repo: &Path, probe: &Probe) -> Result<GateReport, String> {
    require_tty3()?;
    let source_commit = git_output(repo, &["rev-parse", "HEAD"])?;
    if !git_output(repo, &["status", "--short"])?.is_empty() {
        return Err("Sophia worktree must be clean before the physical proof".to_owned());
    }
    if !git_status(repo, &["verify-commit", &source_commit])? {
        return Err("physical-proof HEAD lacks a valid signature".to_owned());
    }
    let upstream = git_output(
        repo,
        &["rev-parse", "--verify", "refs/remotes/origin/master"],
    )?;
    if source_commit != upstream {
        return Err(format!(
            "physical-proof HEAD must equal origin/master: HEAD={source_commit} origin/master={upstream}"
        ));
    }

    let client = std::env::var_os("SOPHIA_STANDALONE_APP_BIN")
        .map(PathBuf::from)
        .map_or_else(|| find_program("kitty"), Ok)?;
    let core = repo.join("tools/fixtures/direct_scanout_core.kdl");
    let desktop = repo.join("tools/fixtures/direct_scanout_desktop.kdl");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let build = Command::new(cargo)
        .current_dir(repo)
        .args([
            "build",
            "--quiet",
            "--release",
            "--offline",
            "-p",
            "sophia-cli",
            "--features",
            "atomic-scanout-live",
        ])
        .status()
        .map_err(|error| format!("could not build the physical-proof binary: {error}"))?;
    if !build.success() {
        return Err(format!("the physical-proof build exited with {build}"));
    }
    if git_output(repo, &["rev-parse", "HEAD"])? != source_commit
        || !git_output(repo, &["status", "--short"])?.is_empty()
    {
        return Err("Sophia source identity changed during the physical-proof build".to_owned());
    }

    let sophia = repo.join("target/release/sophia");
    let sophia_sha256 = direct_scanout_archive::sha256(&sophia)?;
    let client_sha256 = direct_scanout_archive::sha256(&client)?;

    let state_home = state_home()?;
    let session_log = state_home.join("sophia/standalone-session/session.log");
    match fs::remove_file(&session_log) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "could not remove prior evidence {}: {error}",
                session_log.display()
            ));
        }
    }
    run_probe(repo, probe, Some(&client))?;
    if !session_log.is_file() {
        return Err(format!(
            "the direct-scanout session produced no evidence: {}",
            session_log.display()
        ));
    }

    let evidence = std::env::var_os("SOPHIA_DIRECT_SCANOUT_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/sophia-direct-scanout.log"));
    direct_scanout_archive::bind_evidence(&direct_scanout_archive::BindEvidence {
        session_log: &session_log,
        evidence: &evidence,
        source_commit: &source_commit,
        sophia_binary: &sophia,
        client_binary: &client,
        core_config: &core,
        desktop_profile: &desktop,
    })?;
    direct_scanout::verify_standalone_logs_with_overlay(
        &[evidence.display().to_string()],
        probe.overlay_proof,
    )?;
    let run_root = std::env::var_os("SOPHIA_DIRECT_SCANOUT_RUN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| state_home.join("sophia/promotion/direct-scanout-runs"));
    let archive = direct_scanout_archive::create_archive(&direct_scanout_archive::CreateArchive {
        repo,
        evidence: &evidence,
        run_root: &run_root,
        sophia_binary: &sophia,
        client_binary: &client,
    })?;
    Ok(GateReport {
        source_commit,
        sophia_sha256,
        client,
        client_sha256,
        archive,
    })
}

fn positive(value: &str, name: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{name} must be a positive integer, got {value:?}"))
}

/// Which terminal this process is attached to, read from the descriptor
/// rather than from a subprocess.
///
/// `Command::output` closes the child's stdin, so asking `tty(1)` what our
/// terminal is asks it about a null descriptor: it answered "not a tty" and
/// exited 1 on a real tty3, which refused the gate before it could start.
/// The descriptor is right here; nothing needs to be asked.
fn current_terminal() -> Result<PathBuf, String> {
    std::fs::read_link("/proc/self/fd/0")
        .map_err(|error| format!("could not read this process's terminal: {error}"))
}

/// Whether this terminal is the one the gate runs on.
///
/// Split from the descriptor read so the decision is testable without a
/// terminal: the read is one syscall and the decision is the rule.
pub fn is_gate_terminal(terminal: &Path) -> bool {
    terminal == Path::new("/dev/tty3")
}

fn require_tty3() -> Result<(), String> {
    if !std::io::stdin().is_terminal() {
        return Err("switch to tty3, log in, and run: just direct-scanout-gate".to_owned());
    }
    let terminal = current_terminal()?;
    if is_gate_terminal(&terminal) {
        Ok(())
    } else {
        Err(format!(
            "switch to tty3, log in, and run: just direct-scanout-gate (current terminal: {})",
            terminal.display()
        ))
    }
}

fn state_home() -> Result<PathBuf, String> {
    if let Some(state) = std::env::var_os("XDG_STATE_HOME") {
        return Ok(PathBuf::from(state));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".local/state"))
        .ok_or_else(|| "HOME and XDG_STATE_HOME are unset".to_owned())
}

fn find_program(name: &str) -> Result<PathBuf, String> {
    let path = std::env::var_os("PATH").ok_or("PATH is unset")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| format!("the direct-scanout gate requires {name}"))
}

fn git_output(repo: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(arguments)
        .output()
        .map_err(|error| format!("could not run git: {error}"))?;
    if !output.status.success() {
        return Err(format!("git exited with {}", output.status));
    }
    String::from_utf8(output.stdout)
        .map(|text| text.trim().to_owned())
        .map_err(|error| format!("git emitted non-UTF-8: {error}"))
}

fn git_status(repo: &Path, arguments: &[&str]) -> Result<bool, String> {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(arguments)
        .status()
        .map(|status| status.success())
        .map_err(|error| format!("could not run git: {error}"))
}
