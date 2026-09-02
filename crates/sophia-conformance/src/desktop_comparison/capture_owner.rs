//! Local owner for one prepared desktop-comparison schedule row.
//!
//! Stack startup and tracefs privilege remain narrow OS adapters. This owner
//! admits an owner-only session attestation, launches the repository workload,
//! samples the common process population, normalizes kernel DRM completion
//! events, and binds only a replayable raw attempt.

mod reference;
mod trace;
mod workload;

pub use reference::install_reference;
use trace::{TraceOwner, tracefs_root};
use workload::WorkloadOwner;
pub use workload::run_stream;

use super::{
    FIREFOX_VERSION, KITTY_VERSION, NIRI_VERSION, ScheduledSample, TOPOLOGY, XLIBRE_COMMIT,
    XMONAD_CONTRIB_VERSION, XMONAD_VERSION, bind_attempt, next_scheduled, source_commit, status,
};
use sha2::Digest as _;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

const SESSION_PREFIX: &str = "desktop_comparison_session schema=1 status=ready ";
const RESOURCE_INTERVAL: Duration = Duration::from_secs(1);
const READY_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Eq, PartialEq)]
struct SessionAttestation {
    stack: String,
    stack_version: String,
    topology: String,
    supervisor_pid: u32,
    supervisor_start_ticks: u64,
    crtc: u64,
    native_timing: String,
    native_source: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ResourceSnapshot {
    processes: u64,
    pss_kib: u64,
    rss_kib: u64,
    anonymous_kib: u64,
    private_dirty_kib: u64,
    cpu_ticks: u64,
    minor_faults: u64,
    major_faults: u64,
    threads: u64,
    fds: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ProcStat {
    ppid: u32,
    start_ticks: u64,
    cpu_ticks: u64,
    minor_faults: u64,
    major_faults: u64,
    threads: u64,
}
pub fn attest_session(
    repo: &Path,
    run: &Path,
    supervisor_pid: u32,
    crtc: u64,
) -> Result<Vec<String>, String> {
    let _ = status(repo, run)?;
    let scheduled = next_scheduled(run)?
        .ok_or_else(|| "desktop comparison matrix is already complete".to_owned())?;
    let proc_root = PathBuf::from(format!("/proc/{supervisor_pid}"));
    let proc_status = fs::read_to_string(proc_root.join("status"))
        .map_err(|error| format!("could not inspect session supervisor: {error}"))?;
    let uid = proc_status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|rest| rest.split_ascii_whitespace().next())
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or("session supervisor does not expose a numeric UID")?;
    if uid != current_uid()? {
        return Err("session supervisor is not owned by the invoking user".to_owned());
    }
    let executable = fs::read_link(proc_root.join("exe"))
        .map_err(|error| format!("could not identify session supervisor executable: {error}"))?;
    let executable_name = executable
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("session supervisor executable name is not UTF-8")?;
    let expected_names: &[&str] = match scheduled.stack.as_str() {
        "sophia" => &["sophia"],
        "xlibre-xmonad" => &["Xorg", "XLibre"],
        "niri" => &["niri"],
        _ => return Err("prepared schedule contains an unknown stack".to_owned()),
    };
    if !expected_names.contains(&executable_name) {
        return Err(format!(
            "session supervisor executable {executable_name:?} does not match stack {}",
            scheduled.stack
        ));
    }
    if scheduled.stack == "xlibre-xmonad" {
        let prefix = executable
            .parent()
            .and_then(Path::parent)
            .ok_or("XLibre executable does not live under a versioned prefix")?;
        let identity =
            fs::read_to_string(prefix.join("share/sophia-desktop-comparison/xlibre-commit"))
                .map_err(|error| format!("XLibre source identity is missing: {error}"))?;
        if identity.trim() != XLIBRE_COMMIT {
            return Err("XLibre source identity does not match the pinned commit".to_owned());
        }
    }
    let stat = fs::read_to_string(proc_root.join("stat"))
        .map_err(|error| format!("could not read session supervisor start identity: {error}"))?;
    let start_ticks = parse_proc_stat(&stat)?.start_ticks;
    let stack_version = match scheduled.stack.as_str() {
        "sophia" => source_commit(run)?,
        "xlibre-xmonad" => XLIBRE_COMMIT.to_owned(),
        "niri" => NIRI_VERSION.to_owned(),
        _ => unreachable!(),
    };
    let native_source = match scheduled.stack.as_str() {
        "sophia" => "sophia-engine",
        "xlibre-xmonad" => "x-present",
        "niri" => "presentation-time",
        _ => unreachable!(),
    };
    let path = session_attestation_path()?;
    let parent = path
        .parent()
        .ok_or("session attestation path has no parent")?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create session attestation directory: {error}"))?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("could not protect session attestation directory: {error}"))?;
    let partial = parent.join(format!("session.kdl.{supervisor_pid}.partial"));
    write_new(
        &partial,
        format!(
            "desktop_comparison_session schema=1 status=ready stack={} stack_version={} topology={} supervisor_pid={} supervisor_start_ticks={} crtc={} native_timing=not_exposed native_source={}\n",
            scheduled.stack,
            stack_version,
            TOPOLOGY,
            supervisor_pid,
            start_ticks,
            crtc,
            native_source,
        )
        .as_bytes(),
    )?;
    fs::set_permissions(&partial, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("could not protect session attestation: {error}"))?;
    fs::rename(&partial, &path)
        .map_err(|error| format!("could not publish session attestation: {error}"))?;
    Ok(vec![format!(
        "desktop_comparison_attest schema=1 status=ready order={} stack={} supervisor_pid={} crtc={} path={}",
        scheduled.order,
        scheduled.stack,
        supervisor_pid,
        crtc,
        path.display(),
    )])
}

pub fn preflight(repo: &Path, run: &Path) -> Result<Vec<String>, String> {
    let _ = status(repo, run)?;
    let scheduled = next_scheduled(run)?
        .ok_or_else(|| "desktop comparison matrix is already complete".to_owned())?;
    let attestation_path = session_attestation_path()?;
    let attestation = read_attestation(&attestation_path)?;
    validate_session(run, &scheduled, &attestation)?;
    validate_active_profile(repo, &attestation)?;
    require_tool_version("kitty", &["--version"], KITTY_VERSION)?;
    require_tool_version("firefox", &["--version"], FIREFOX_VERSION)?;
    if scheduled.stack == "niri" {
        require_tool_version("niri", &["--version"], NIRI_VERSION)?;
    }
    let helper = repo.join("tools/desktop_comparison_tracefs.sh");
    if !helper.is_file() {
        return Err(format!(
            "kernel timing adapter is missing: {}",
            helper.display()
        ));
    }
    let tracefs = tracefs_root().ok_or(
        "kernel DRM timing is unavailable: tracefs or drm_vblank_event_delivered is missing",
    )?;
    Ok(vec![format!(
        "desktop_comparison_preflight schema=1 status=ready order={} stack={} workload={} repetition={} session={} tracefs={} frame_source=kernel_drm",
        scheduled.order,
        scheduled.stack,
        scheduled.workload,
        scheduled.repetition,
        attestation_path.display(),
        tracefs.display(),
    )])
}

pub fn capture_next(repo: &Path, run: &Path) -> Result<Vec<String>, String> {
    preflight(repo, run)?;
    let scheduled = next_scheduled(run)?
        .ok_or_else(|| "desktop comparison matrix is already complete".to_owned())?;
    let attestation = read_attestation(&session_attestation_path()?)?;
    let incoming = run.join("incoming");
    if incoming.exists()
        && fs::read_dir(&incoming)
            .map_err(|error| format!("could not inspect pending captures: {error}"))?
            .next()
            .is_some()
    {
        return Err(format!(
            "a partial capture needs diagnosis before retry: {}",
            incoming.display()
        ));
    }
    fs::create_dir_all(&incoming)
        .map_err(|error| format!("could not create pending capture root: {error}"))?;
    let attempt = incoming.join(format!(
        "{:02}-{}-{}-{}.partial",
        scheduled.order, scheduled.stack, scheduled.workload, scheduled.repetition
    ));
    fs::create_dir(&attempt)
        .map_err(|error| format!("could not create pending capture: {error}"))?;

    let measured = measure(repo, run, &attempt, &scheduled, &attestation);
    let result = match measured {
        Ok(()) => bind_attempt(repo, run, &attempt),
        Err(error) => Err(format!(
            "{error}; partial capture retained at {}",
            attempt.display()
        )),
    };
    if result.is_ok() {
        fs::remove_dir_all(&attempt)
            .map_err(|error| format!("capture bound but pending copy was not removed: {error}"))?;
    }
    result
}

fn measure(
    repo: &Path,
    run: &Path,
    attempt: &Path,
    scheduled: &ScheduledSample,
    attestation: &SessionAttestation,
) -> Result<(), String> {
    let duration = if scheduled.workload == "soak-2h" {
        Duration::from_secs(2 * 60 * 60)
    } else {
        Duration::from_secs(60)
    };
    let raw_trace = attempt.join("kernel-trace.raw");
    let mut trace = TraceOwner::start(
        &repo.join("tools/desktop_comparison_tracefs.sh"),
        attempt,
        &raw_trace,
    )?;
    let mut workload = WorkloadOwner::launch(repo, attempt, scheduled, duration)?;
    trace.begin()?;
    workload.begin_measured_work()?;

    let started = Instant::now();
    let clock_ticks = clock_ticks_per_second()?;
    let mut roots = BTreeSet::from([attestation.supervisor_pid]);
    roots.extend(workload.root_pids());
    roots.extend(stack_auxiliary_roots(repo, &scheduled.stack)?);
    let baseline = sample_process_population(Path::new("/proc"), &roots)?;
    let mut resources = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(attempt.join("resources.log"))
        .map_err(|error| format!("could not create resource evidence: {error}"))?;
    let samples = duration.as_secs();
    for sequence in 1..=samples {
        let due = started + RESOURCE_INTERVAL.saturating_mul(sequence as u32);
        sleep_until(due);
        validate_supervisor(attestation)?;
        if workload.exited_early()? {
            return Err("comparison workload exited before its measured window".to_owned());
        }
        let snapshot = sample_process_population(Path::new("/proc"), &roots)?;
        writeln!(
            resources,
            "desktop_comparison_resource schema=1 seq={sequence} monotonic_usec={} processes={} pss_kib={} rss_kib={} anonymous_kib={} private_dirty_kib={} cpu_msec={} minor_faults={} major_faults={} threads={} fds={}",
            elapsed_micros(started),
            snapshot.processes,
            snapshot.pss_kib,
            snapshot.rss_kib,
            snapshot.anonymous_kib,
            snapshot.private_dirty_kib,
            snapshot.cpu_ticks.saturating_sub(baseline.cpu_ticks).saturating_mul(1_000) / clock_ticks,
            snapshot.minor_faults.saturating_sub(baseline.minor_faults),
            snapshot.major_faults.saturating_sub(baseline.major_faults),
            snapshot.threads,
            snapshot.fds,
        )
        .map_err(|error| format!("could not append resource evidence: {error}"))?;
    }
    resources
        .sync_all()
        .map_err(|error| format!("could not sync resource evidence: {error}"))?;

    let duration_msec = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    trace.finish()?;
    workload.finish(attempt)?;
    normalize_kernel_trace(
        &raw_trace,
        &attempt.join("kernel-frames.log"),
        attestation.crtc,
    )?;
    fs::remove_file(&raw_trace)
        .map_err(|error| format!("could not remove normalized trace source: {error}"))?;
    write_new(
        &attempt.join("native.log"),
        format!(
            "desktop_comparison_native_timing schema=1 availability={} source={} samples=0\n",
            attestation.native_timing, attestation.native_source,
        )
        .as_bytes(),
    )?;
    let candidate = source_commit(run)?;
    let version = match scheduled.stack.as_str() {
        "sophia" => candidate.as_str(),
        "xlibre-xmonad" => XLIBRE_COMMIT,
        "niri" => NIRI_VERSION,
        _ => return Err("prepared schedule contains an unknown stack".to_owned()),
    };
    write_new(
        &attempt.join("attempt.kdl"),
        format!(
            "desktop_comparison_attempt schema=1 status=measured order={} stack={} workload={} repetition={} backend=native stack_version={} topology={} kitty={} firefox={} duration_msec={} crashes=0 sample_loss=0 teardown=clean\n",
            scheduled.order,
            scheduled.stack,
            scheduled.workload,
            scheduled.repetition,
            version,
            TOPOLOGY,
            KITTY_VERSION,
            FIREFOX_VERSION,
            duration_msec,
        )
        .as_bytes(),
    )
}

fn validate_session(
    run: &Path,
    scheduled: &ScheduledSample,
    attestation: &SessionAttestation,
) -> Result<(), String> {
    let candidate = source_commit(run)?;
    let expected_version = match scheduled.stack.as_str() {
        "sophia" => candidate.as_str(),
        "xlibre-xmonad" => XLIBRE_COMMIT,
        "niri" => NIRI_VERSION,
        _ => return Err("prepared schedule contains an unknown stack".to_owned()),
    };
    if attestation.stack != scheduled.stack
        || attestation.stack_version != expected_version
        || attestation.topology != TOPOLOGY
    {
        return Err(
            "active session attestation does not match the exact next schedule row".to_owned(),
        );
    }
    if attestation.native_timing != "not_exposed" {
        return Err(
            "local capture currently requires native_timing=not_exposed; kernel DRM remains authoritative"
                .to_owned(),
        );
    }
    validate_supervisor(attestation)
}

fn validate_active_profile(repo: &Path, attestation: &SessionAttestation) -> Result<(), String> {
    let expected = expected_profile(repo, &attestation.stack)?;
    match attestation.stack.as_str() {
        "sophia" => {
            let observed =
                process_environment(attestation.supervisor_pid, "SOPHIA_DESKTOP_PROFILE")
                    .ok_or("Sophia supervisor does not expose SOPHIA_DESKTOP_PROFILE")?;
            require_same_path(&observed, &expected, "Sophia desktop profile")
        }
        "niri" => {
            if let Some(observed) = process_environment(attestation.supervisor_pid, "NIRI_CONFIG") {
                return require_same_path(&observed, &expected, "niri profile");
            }
            let command = fs::read(format!("/proc/{}/cmdline", attestation.supervisor_pid))
                .map_err(|error| format!("could not inspect niri command line: {error}"))?;
            let expected = expected.to_string_lossy();
            if command
                .split(|byte| *byte == 0)
                .any(|argument| argument == expected.as_bytes())
            {
                Ok(())
            } else {
                Err("niri supervisor is not using the repository comparison profile".to_owned())
            }
        }
        "xlibre-xmonad" => stack_auxiliary_roots(repo, &attestation.stack).map(|_| ()),
        _ => Err("active session names an unknown comparison stack".to_owned()),
    }
}

fn expected_profile(repo: &Path, stack: &str) -> Result<PathBuf, String> {
    let relative = match stack {
        "sophia" => "validation/desktop-comparison/profiles/hagia.kdl",
        "xlibre-xmonad" => "validation/desktop-comparison/profiles/xmonad.hs",
        "niri" => "validation/desktop-comparison/profiles/niri.kdl",
        _ => return Err("unknown comparison stack profile".to_owned()),
    };
    fs::canonicalize(repo.join(relative))
        .map_err(|error| format!("could not resolve comparison profile {relative}: {error}"))
}

fn process_environment(pid: u32, name: &str) -> Option<PathBuf> {
    let environment = fs::read(format!("/proc/{pid}/environ")).ok()?;
    environment.split(|byte| *byte == 0).find_map(|entry| {
        let separator = entry.iter().position(|byte| *byte == b'=')?;
        let (key, value) = entry.split_at(separator);
        (key == name.as_bytes())
            .then(|| PathBuf::from(String::from_utf8_lossy(&value[1..]).into_owned()))
    })
}

fn require_same_path(observed: &Path, expected: &Path, name: &str) -> Result<(), String> {
    let observed = fs::canonicalize(observed)
        .map_err(|error| format!("could not resolve observed {name}: {error}"))?;
    if observed == expected {
        Ok(())
    } else {
        Err(format!(
            "{name} mismatch: expected {}, observed {}",
            expected.display(),
            observed.display()
        ))
    }
}

fn stack_auxiliary_roots(repo: &Path, stack: &str) -> Result<Vec<u32>, String> {
    if stack != "xlibre-xmonad" {
        return Ok(Vec::new());
    }
    let uid = current_uid()?;
    let mut roots = Vec::new();
    for entry in
        fs::read_dir("/proc").map_err(|error| format!("could not enumerate xmonad: {error}"))?
    {
        let entry = entry.map_err(|error| format!("could not inspect xmonad process: {error}"))?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(executable) = fs::read_link(entry.path().join("exe")) else {
            continue;
        };
        if executable.file_name().and_then(|name| name.to_str()) != Some("xmonad") {
            continue;
        }
        let Ok(status) = fs::read_to_string(entry.path().join("status")) else {
            continue;
        };
        let process_uid = status
            .lines()
            .find_map(|line| line.strip_prefix("Uid:"))
            .and_then(|rest| rest.split_ascii_whitespace().next())
            .and_then(|value| value.parse::<u32>().ok());
        if process_uid == Some(uid) {
            roots.push(pid);
        }
    }
    if roots.len() != 1 {
        return Err(format!(
            "XLibre comparison requires exactly one owned xmonad process; found {}",
            roots.len()
        ));
    }
    let executable = fs::read_link(format!("/proc/{}/exe", roots[0]))
        .map_err(|error| format!("could not identify xmonad executable: {error}"))?;
    let prefix = executable
        .parent()
        .and_then(Path::parent)
        .ok_or("xmonad executable does not live under a versioned prefix")?;
    let identity = prefix.join("share/sophia-desktop-comparison");
    let version = fs::read_to_string(identity.join("xmonad-version"))
        .map_err(|error| format!("xmonad version identity is missing: {error}"))?;
    if version.trim() != XMONAD_VERSION {
        return Err(format!(
            "xmonad version identity does not match {XMONAD_VERSION}"
        ));
    }
    let contrib = fs::read_to_string(identity.join("xmonad-contrib-version"))
        .map_err(|error| format!("xmonad-contrib version identity is missing: {error}"))?;
    if contrib.trim() != XMONAD_CONTRIB_VERSION {
        return Err(format!(
            "xmonad-contrib version identity does not match {XMONAD_CONTRIB_VERSION}"
        ));
    }
    let expected_profile = expected_profile(repo, stack)?;
    let expected_digest = format!(
        "{:x}",
        sha2::Sha256::digest(
            fs::read(&expected_profile)
                .map_err(|error| format!("could not hash xmonad profile: {error}"))?
        )
    );
    let observed_digest = fs::read_to_string(identity.join("xmonad-profile-sha256"))
        .map_err(|error| format!("xmonad profile identity is missing: {error}"))?;
    if observed_digest.trim() != expected_digest {
        return Err(
            "running xmonad was not built from the repository comparison profile".to_owned(),
        );
    }
    Ok(roots)
}

fn validate_supervisor(attestation: &SessionAttestation) -> Result<(), String> {
    let source = fs::read_to_string(format!("/proc/{}/stat", attestation.supervisor_pid))
        .map_err(|error| format!("attested session supervisor is not alive: {error}"))?;
    let observed = parse_proc_stat(&source)?;
    if observed.start_ticks != attestation.supervisor_start_ticks {
        return Err("attested session supervisor PID was reused".to_owned());
    }
    Ok(())
}

fn session_attestation_path() -> Result<PathBuf, String> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .ok_or("XDG_RUNTIME_DIR is unset; no active comparison session can be admitted")?;
    Ok(PathBuf::from(runtime)
        .join("sophia-desktop-comparison")
        .join("session.kdl"))
}

fn read_attestation(path: &Path) -> Result<SessionAttestation, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("comparison session attestation is missing: {error}"))?;
    if !metadata.is_file() || metadata.uid() != current_uid()? || metadata.mode() & 0o077 != 0 {
        return Err("comparison session attestation must be an owner-only regular file".to_owned());
    }
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read comparison session attestation: {error}"))?;
    let line = one_record(&source, SESSION_PREFIX)?;
    let fields = record_fields(line)?;
    let required = |name: &str| {
        fields
            .get(name)
            .cloned()
            .ok_or_else(|| format!("comparison session attestation lacks {name}"))
    };
    let number = |name: &str| {
        required(name)?
            .parse::<u64>()
            .map_err(|_| format!("comparison session {name} is not an integer"))
    };
    let native_timing = required("native_timing")?;
    let native_source = required("native_source")?;
    require_token("native_source", &native_source)?;
    Ok(SessionAttestation {
        stack: required("stack")?,
        stack_version: required("stack_version")?,
        topology: required("topology")?,
        supervisor_pid: u32::try_from(number("supervisor_pid")?)
            .map_err(|_| "comparison supervisor PID is too large")?,
        supervisor_start_ticks: number("supervisor_start_ticks")?,
        crtc: number("crtc")?,
        native_timing,
        native_source,
    })
}

fn current_uid() -> Result<u32, String> {
    let status = fs::read_to_string("/proc/self/status")
        .map_err(|error| format!("could not read current process identity: {error}"))?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|rest| rest.split_ascii_whitespace().next())
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| "current process identity lacks a numeric UID".to_owned())
}

fn sample_process_population(
    proc_root: &Path,
    roots: &BTreeSet<u32>,
) -> Result<ResourceSnapshot, String> {
    let mut processes = BTreeMap::new();
    for entry in fs::read_dir(proc_root)
        .map_err(|error| format!("could not enumerate process population: {error}"))?
    {
        let entry = entry.map_err(|error| format!("could not read process entry: {error}"))?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(source) = fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        if let Ok(stat) = parse_proc_stat(&source) {
            processes.insert(pid, stat);
        }
    }
    if roots.iter().any(|pid| !processes.contains_key(pid)) {
        return Err("a sampled process root disappeared".to_owned());
    }

    let selected = processes
        .keys()
        .copied()
        .filter(|pid| descends_from(*pid, roots, &processes))
        .collect::<Vec<_>>();
    let mut total = ResourceSnapshot::default();
    for pid in selected {
        let Some(stat) = processes.get(&pid) else {
            continue;
        };
        let process = proc_root.join(pid.to_string());
        let memory = match fs::read_to_string(process.join("smaps_rollup")) {
            Ok(memory) => memory,
            Err(error) if roots.contains(&pid) => {
                return Err(format!(
                    "sampled process root {pid} has no readable memory population: {error}"
                ));
            }
            Err(_) => continue,
        };
        let fd_count = match fs::read_dir(process.join("fd")) {
            Ok(entries) => entries.filter_map(Result::ok).count(),
            Err(error) if roots.contains(&pid) => {
                return Err(format!(
                    "sampled process root {pid} has no readable fd population: {error}"
                ));
            }
            Err(_) => continue,
        };
        total.processes = total.processes.saturating_add(1);
        total.pss_kib = total.pss_kib.saturating_add(memory_kib(&memory, "Pss:"));
        total.rss_kib = total.rss_kib.saturating_add(memory_kib(&memory, "Rss:"));
        total.anonymous_kib = total
            .anonymous_kib
            .saturating_add(memory_kib(&memory, "Anonymous:"));
        total.private_dirty_kib = total
            .private_dirty_kib
            .saturating_add(memory_kib(&memory, "Private_Dirty:"));
        total.cpu_ticks = total.cpu_ticks.saturating_add(stat.cpu_ticks);
        total.minor_faults = total.minor_faults.saturating_add(stat.minor_faults);
        total.major_faults = total.major_faults.saturating_add(stat.major_faults);
        total.threads = total.threads.saturating_add(stat.threads);
        total.fds = total
            .fds
            .saturating_add(u64::try_from(fd_count).unwrap_or(u64::MAX));
    }
    if total.processes == 0
        || total.pss_kib == 0
        || total.rss_kib == 0
        || total.threads == 0
        || total.fds == 0
    {
        return Err("sampled process population is empty or unreadable".to_owned());
    }
    Ok(total)
}

fn descends_from(mut pid: u32, roots: &BTreeSet<u32>, processes: &BTreeMap<u32, ProcStat>) -> bool {
    for _ in 0..=processes.len() {
        if roots.contains(&pid) {
            return true;
        }
        let Some(stat) = processes.get(&pid) else {
            return false;
        };
        if stat.ppid == 0 || stat.ppid == pid {
            return false;
        }
        pid = stat.ppid;
    }
    false
}

pub(super) fn parse_proc_stat(source: &str) -> Result<ProcStat, String> {
    let close = source
        .rfind(')')
        .ok_or("process stat lacks a command terminator")?;
    let fields = source[close + 1..]
        .split_ascii_whitespace()
        .collect::<Vec<_>>();
    let number = |index: usize, name: &str| {
        fields
            .get(index)
            .ok_or_else(|| format!("process stat lacks {name}"))?
            .parse::<u64>()
            .map_err(|_| format!("process stat {name} is not an integer"))
    };
    Ok(ProcStat {
        ppid: u32::try_from(number(1, "ppid")?).map_err(|_| "process ppid is too large")?,
        minor_faults: number(7, "minor_faults")?,
        major_faults: number(9, "major_faults")?,
        cpu_ticks: number(11, "utime")?.saturating_add(number(12, "stime")?),
        threads: number(17, "threads")?,
        start_ticks: number(19, "start_ticks")?,
    })
}

fn memory_kib(source: &str, field: &str) -> u64 {
    source
        .lines()
        .find_map(|line| line.strip_prefix(field))
        .and_then(|rest| rest.split_ascii_whitespace().next())
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn clock_ticks_per_second() -> Result<u64, String> {
    let output = Command::new("getconf")
        .arg("CLK_TCK")
        .output()
        .map_err(|error| format!("could not query process clock rate: {error}"))?;
    if !output.status.success() {
        return Err("getconf CLK_TCK failed".to_owned());
    }
    String::from_utf8(output.stdout)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| "getconf CLK_TCK did not return a positive integer".to_owned())
}

pub(super) fn normalize_kernel_trace(
    source: &Path,
    destination: &Path,
    expected_crtc: u64,
) -> Result<(), String> {
    let trace = fs::read_to_string(source)
        .map_err(|error| format!("could not read kernel timing trace: {error}"))?;
    if trace
        .lines()
        .any(|line| line.contains("LOST") && line.contains("EVENTS"))
    {
        return Err("kernel DRM trace reports lost events".to_owned());
    }
    let mut timestamps = Vec::new();
    for line in trace.lines() {
        let Some((prefix, payload)) = line.split_once("drm_vblank_event_delivered:") else {
            continue;
        };
        let crtc = payload
            .split_ascii_whitespace()
            .find_map(|token| token.trim_end_matches(',').strip_prefix("crtc="))
            .and_then(|value| value.parse::<u64>().ok());
        if crtc != Some(expected_crtc) {
            continue;
        }
        let timestamp = prefix
            .split_ascii_whitespace()
            .rev()
            .find_map(|token| token.strip_suffix(':'))
            .and_then(|value| value.parse::<f64>().ok())
            .map(|seconds| (seconds * 1_000_000.0).round() as u64)
            .ok_or("kernel DRM trace contains an invalid event timestamp")?;
        if timestamps
            .last()
            .is_some_and(|previous| *previous >= timestamp)
        {
            return Err("kernel DRM completion timestamps are not strictly monotonic".to_owned());
        }
        timestamps.push(timestamp);
    }
    if timestamps.len() < 3 {
        return Err(format!(
            "kernel DRM timing population is incomplete: found {} delivered events for CRTC {expected_crtc}",
            timestamps.len()
        ));
    }
    let mut output = String::new();
    for (index, timestamp) in timestamps.into_iter().enumerate() {
        output.push_str(&format!(
            "desktop_comparison_kernel_frame schema=1 seq={} crtc={} ust_usec={}\n",
            index + 1,
            expected_crtc,
            timestamp,
        ));
    }
    write_new(destination, output.as_bytes())
}

fn require_tool_version(program: &str, arguments: &[&str], expected: &str) -> Result<(), String> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("could not run {program} version preflight: {error}"))?;
    let observed = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() || !observed.contains(expected) {
        return Err(format!(
            "{program} version mismatch: expected token {expected:?}, observed {:?}",
            observed.trim()
        ));
    }
    Ok(())
}

fn timing_population(values: &[u64]) -> (u64, u64, u64, u64) {
    if values.is_empty() {
        return (0, 0, 0, 0);
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let pick = |percentile: usize| {
        let rank = sorted.len().saturating_mul(percentile).saturating_add(99) / 100;
        sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
    };
    (pick(50), pick(95), pick(99), *sorted.last().unwrap_or(&0))
}

fn one_record<'a>(source: &'a str, prefix: &str) -> Result<&'a str, String> {
    let records = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    if records.len() != 1 || !records[0].starts_with(prefix) {
        return Err(format!(
            "comparison session requires exactly one {prefix:?} record"
        ));
    }
    Ok(records[0])
}

fn record_fields(line: &str) -> Result<BTreeMap<String, String>, String> {
    let mut fields = BTreeMap::new();
    for token in line.split_ascii_whitespace() {
        let Some((name, value)) = token.split_once('=') else {
            continue;
        };
        if fields.insert(name.to_owned(), value.to_owned()).is_some() {
            return Err(format!("comparison record repeats field {name}"));
        }
    }
    Ok(fields)
}

fn require_token(name: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.chars().any(char::is_whitespace) || value.contains('=') {
        Err(format!("{name} must be one nonempty key-value-safe token"))
    } else {
        Ok(())
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|error| format!("could not create {}: {error}", path.display()))
}

fn elapsed_micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn sleep_until(deadline: Instant) {
    if let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        thread::sleep(remaining);
    }
}
