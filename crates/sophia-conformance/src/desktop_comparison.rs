//! Typed orchestration and evidence reduction for the diagnostic desktop matrix.

mod capture;
mod capture_owner;
mod host;

pub use capture::{CaptureReplay, replay_attempt};
pub use capture_owner::{
    attest_session, attest_session_auto, capture_next, finalize_next, install_reference, preflight,
    qualify, run_stream,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const XLIBRE_COMMIT: &str = "56be9f4320ef121dc5d4bc40a6365d995512d3bc";
pub const NIRI_VERSION: &str = "26.04";
pub const KITTY_VERSION: &str = "0.48.2";
pub const FIREFOX_VERSION: &str = "155";
pub const TOPOLOGY: &str = "DP-1:2560x1440@60000+DP-2:1920x1080@60000";
pub const XMONAD_VERSION: &str = "0.18.1";
pub const XMONAD_CONTRIB_VERSION: &str = "0.18.2";

pub(crate) const STACKS: [&str; 3] = ["sophia", "xlibre-xmonad", "niri"];
pub(crate) const SHORT_WORKLOADS: [&str; 4] =
    ["kitty-60s", "firefox-local", "resize", "kitty-burst-16"];
pub(crate) const CONFIGS: [&str; 14] = [
    "validation/desktop-comparison/config/sophia.kdl",
    "validation/desktop-comparison/config/xlibre-xmonad.kdl",
    "validation/desktop-comparison/config/niri.kdl",
    "validation/desktop-comparison/firefox/index.html",
    "validation/desktop-comparison/firefox/user.js",
    "tools/desktop_comparison_tracefs.sh",
    "tools/desktop_comparison_tty3.sh",
    "tools/start_sophia_tty3.sh",
    "tools/run_sophia_session.sh",
    "tools/sophia_tty_mode.py",
    "tools/lib/session_terminal.sh",
    "validation/desktop-comparison/profiles/hagia.kdl",
    "validation/desktop-comparison/profiles/niri.kdl",
    "validation/desktop-comparison/profiles/xmonad.hs",
];

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ScheduledSample {
    pub order: usize,
    pub stack: String,
    pub workload: String,
    pub repetition: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ComparisonLane {
    Interactive,
    OptionalSoak,
}

impl ComparisonLane {
    const fn token(self) -> &'static str {
        match self {
            Self::Interactive => "interactive",
            Self::OptionalSoak => "optional-soak",
        }
    }
}

#[derive(Clone, Debug)]
struct Sample {
    scheduled: ScheduledSample,
    duration_msec: u64,
    processes: u64,
    pss_peak_kib: u64,
    rss_peak_kib: u64,
    anonymous_peak_kib: u64,
    private_dirty_peak_kib: u64,
    cpu_msec: u64,
    minor_faults: u64,
    major_faults: u64,
    threads_peak: u64,
    fds_peak: u64,
    stack_processes: u64,
    stack_pss_peak_kib: u64,
    stack_rss_peak_kib: u64,
    stack_cpu_msec: u64,
    stack_threads_peak: u64,
    stack_fds_peak: u64,
    workload_processes: u64,
    workload_pss_peak_kib: u64,
    workload_rss_peak_kib: u64,
    workload_cpu_msec: u64,
    workload_threads_peak: u64,
    workload_fds_peak: u64,
    launch_msec: u64,
    settle_msec: u64,
    resize_msec: u64,
    frame_mean_usec: u64,
    frame_p50_usec: u64,
    frame_p95_usec: u64,
    frame_p99_usec: u64,
    frame_max_usec: u64,
    frame_deliveries: u64,
    frame_duplicates: u64,
}

fn comparison_binaries(repo: &Path) -> Result<[(&'static str, PathBuf); 6], String> {
    let configured =
        |name: &str, fallback: PathBuf| std::env::var_os(name).map_or(fallback, PathBuf::from);
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("HOME is unset; reference binary defaults cannot be resolved")?;
    let xlibre_prefix = configured(
        "SOPHIA_DESKTOP_COMPARISON_XLIBRE_PREFIX",
        home.join(".local/opt/xlibre-56be9f4320ef"),
    );
    let binaries = [
        (
            "sophia_sha256",
            configured(
                "SOPHIA_DESKTOP_COMPARISON_SOPHIA_BIN",
                repo.join("target/release/sophia"),
            ),
        ),
        (
            "hagia_sha256",
            configured(
                "SOPHIA_DESKTOP_COMPARISON_HAGIA_BIN",
                repo.join("../hagia/hagia"),
            ),
        ),
        (
            "narthex_sha256",
            configured(
                "SOPHIA_DESKTOP_COMPARISON_NARTHEX_BIN",
                repo.join("../narthex/narthex"),
            ),
        ),
        ("xlibre_sha256", xlibre_prefix.join("bin/Xorg")),
        ("xmonad_sha256", xlibre_prefix.join("bin/xmonad")),
        (
            "niri_sha256",
            configured(
                "SOPHIA_DESKTOP_COMPARISON_NIRI_BIN",
                PathBuf::from("/usr/bin/niri"),
            ),
        ),
    ];
    for (name, path) in &binaries {
        let metadata = fs::metadata(path).map_err(|error| {
            format!(
                "comparison binary {name} is unavailable at {}: {error}",
                path.display()
            )
        })?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
            return Err(format!(
                "comparison binary {name} is not executable: {}",
                path.display()
            ));
        }
    }
    Ok(binaries)
}

pub fn prepare(repo: &Path, run: &Path) -> Result<Vec<String>, String> {
    verify_host_tool_versions()?;
    let identity = host::detect()?;
    prepare_with_identity(
        repo,
        run,
        &identity.kernel,
        &identity.mesa,
        &identity.gpu,
        ComparisonLane::Interactive,
    )
}

pub fn prepare_optional_soak(repo: &Path, run: &Path) -> Result<Vec<String>, String> {
    verify_host_tool_versions()?;
    let identity = host::detect()?;
    prepare_with_identity(
        repo,
        run,
        &identity.kernel,
        &identity.mesa,
        &identity.gpu,
        ComparisonLane::OptionalSoak,
    )
}

/// Verifies every mutable host executable used by a prepared comparison run.
///
/// This belongs before preparation and before graphical takeover. Capture also
/// repeats it so a package update cannot silently mix versions within a run.
pub fn verify_host_tool_versions() -> Result<(), String> {
    require_tool_version("kitty", &["--version"], KITTY_VERSION)?;
    require_tool_version("firefox", &["--version"], FIREFOX_VERSION)?;
    require_tool_version("niri", &["--version"], NIRI_VERSION)
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
    if !output.status.success() || !version_output_matches(&observed, expected) {
        return Err(format!(
            "{program} version mismatch: expected token {expected:?}, observed {:?}",
            observed.trim()
        ));
    }
    Ok(())
}

fn version_output_matches(observed: &str, expected: &str) -> bool {
    observed.split_ascii_whitespace().any(|token| {
        token == expected
            || token
                .strip_prefix(expected)
                .is_some_and(|suffix| suffix.starts_with('.'))
    })
}

fn prepare_with_identity(
    repo: &Path,
    run: &Path,
    kernel: &str,
    mesa: &str,
    gpu: &str,
    lane: ComparisonLane,
) -> Result<Vec<String>, String> {
    if run.exists() {
        return Err(format!("comparison run already exists: {}", run.display()));
    }
    for (name, value) in [("kernel", kernel), ("mesa", mesa), ("gpu", gpu)] {
        require_token(name, value)?;
    }
    require_clean_worktree(&git_output(repo, &["status", "--porcelain"])?)?;
    let source_commit = git_output(repo, &["rev-parse", "HEAD"])?;
    let signature = Command::new("git")
        .args([
            "-C",
            repo.to_string_lossy().as_ref(),
            "verify-commit",
            "HEAD",
        ])
        .output()
        .map_err(|error| format!("could not verify candidate signature: {error}"))?;
    if !signature.status.success() {
        return Err("desktop comparison requires a signed Sophia candidate".to_owned());
    }

    fs::create_dir_all(run.join("samples"))
        .map_err(|error| format!("could not create comparison run: {error}"))?;
    fs::create_dir(run.join("attempts"))
        .map_err(|error| format!("could not create comparison attempt root: {error}"))?;
    let mut binary_fields = String::new();
    for (name, path) in comparison_binaries(repo)? {
        binary_fields.push_str(&format!(" {name}={}", digest_file(&path)?));
    }
    let mut manifest = format!(
        "desktop_comparison_manifest schema=4 status=prepared diagnostic_only=true raw_capture_required=true acquisition=terminal_free_visible lane={} optional_soak=separate source_commit={source_commit} candidate_signature=verified kernel={kernel} mesa={mesa} gpu={gpu} topology={TOPOLOGY} kitty={KITTY_VERSION} firefox={FIREFOX_VERSION}{binary_fields}\n",
        lane.token(),
    );
    manifest.push_str(&format!(
        "desktop_comparison_stack schema=2 id=sophia version={source_commit} backend=native\n"
    ));
    manifest.push_str(&format!(
        "desktop_comparison_stack schema=2 id=xlibre-xmonad version={XLIBRE_COMMIT} xmonad={XMONAD_VERSION} xmonad_contrib={XMONAD_CONTRIB_VERSION} backend=native\n"
    ));
    manifest.push_str(&format!(
        "desktop_comparison_stack schema=2 id=niri version={NIRI_VERSION} path=/usr/bin/niri backend=native\n"
    ));
    for config in CONFIGS {
        let path = repo.join(config);
        manifest.push_str(&format!(
            "desktop_comparison_input schema=2 path={config} sha256={}\n",
            digest_file(&path)?
        ));
    }
    write_new(&run.join("manifest.kdl"), manifest.as_bytes())?;

    let schedule = schedule_for_lane(lane);
    let mut encoded = String::new();
    for item in &schedule {
        encoded.push_str(&format!(
            "desktop_comparison_schedule schema=2 order={} stack={} workload={} repetition={} backend=native\n",
            item.order, item.stack, item.workload, item.repetition
        ));
    }
    write_new(&run.join("schedule.kdl"), encoded.as_bytes())?;
    rewrite_checksums(run, &[])?;
    Ok(vec![format!(
        "desktop_comparison_prepare schema=4 status=complete run={} source_commit={} lane={} samples={}",
        run.display(),
        source_commit,
        lane.token(),
        schedule.len()
    )])
}
pub fn require_candidate_checkout(repo: &Path, run: &Path) -> Result<(), String> {
    let expected = source_commit(run)?;
    let observed = git_output(repo, &["rev-parse", "HEAD"])?;
    if observed != expected {
        return Err(format!(
            "comparison candidate checkout mismatch: expected {expected}, observed {observed}"
        ));
    }
    require_clean_worktree(&git_output(repo, &["status", "--porcelain"])?)
}

pub fn verify_prepared_binaries(repo: &Path, run: &Path) -> Result<(), String> {
    let manifest = fs::read_to_string(run.join("manifest.kdl"))
        .map_err(|error| format!("comparison manifest is missing: {error}"))?;
    let first = manifest
        .lines()
        .next()
        .ok_or("comparison manifest is empty")?;
    let identities = fields(first)?;
    for (name, path) in comparison_binaries(repo)? {
        let expected = identities
            .get(name)
            .ok_or_else(|| format!("comparison manifest lacks {name}"))?;
        if digest_file(&path)? != *expected {
            return Err(format!(
                "prepared comparison binary changed: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

pub fn status(repo: &Path, run: &Path) -> Result<Vec<String>, String> {
    verify_prepared_inputs(repo, run)?;
    verify_checksums(run)?;
    verify_no_pending_capture(run)?;
    let observed = observed_samples(run)?;
    verify_raw_attempts(run, &observed)?;
    let expected = schedule_for_run(run)?;
    match next_scheduled(run)? {
        Some(item) => Ok(vec![format!(
            "desktop_comparison_status schema=1 status=pending completed={} total={} next_order={} next_stack={} next_workload={} next_repetition={}",
            item.order.saturating_sub(1),
            expected.len(),
            item.order,
            item.stack,
            item.workload,
            item.repetition,
        )]),
        None => Ok(vec![format!(
            "desktop_comparison_status schema=1 status=complete completed={} total={}",
            expected.len(),
            expected.len(),
        )]),
    }
}

fn verify_no_pending_capture(run: &Path) -> Result<(), String> {
    let incoming = run.join("incoming");
    if incoming.exists()
        && fs::read_dir(&incoming)
            .map_err(|error| format!("could not inspect pending captures: {error}"))?
            .next()
            .is_some()
    {
        return Err(format!(
            "partial comparison capture requires diagnosis: {}",
            incoming.display()
        ));
    }
    Ok(())
}

fn observed_samples(run: &Path) -> Result<BTreeSet<ScheduledSample>, String> {
    let candidate = source_commit(run)?;
    let mut observed = BTreeSet::new();
    for entry in sample_paths(run)? {
        let source = fs::read_to_string(&entry)
            .map_err(|error| format!("could not read {}: {error}", entry.display()))?;
        let sample = parse_sample(&source, &candidate)?;
        if !observed.insert(sample.scheduled.clone()) {
            return Err("comparison contains a duplicate scheduled sample".to_owned());
        }
    }
    Ok(observed)
}

pub fn next_scheduled(run: &Path) -> Result<Option<ScheduledSample>, String> {
    let expected = schedule_for_run(run)?;
    let observed = observed_samples(run)?;
    for (index, item) in expected.iter().enumerate() {
        if !observed.contains(item) {
            if expected[index.saturating_add(1)..]
                .iter()
                .any(|later| observed.contains(later))
            {
                return Err("comparison samples are not a contiguous schedule prefix".to_owned());
            }
            return Ok(Some(item.clone()));
        }
    }
    if observed.len() != expected.len() {
        return Err("comparison contains an unexpected scheduled sample".to_owned());
    }
    Ok(None)
}

/// Ingest one native-stack adapter log and bind it to the prepared schedule.
fn run_sample(repo: &Path, run: &Path, raw_log: &Path) -> Result<Vec<String>, String> {
    verify_prepared_inputs(repo, run)?;
    verify_checksums(run)?;
    let source = fs::read_to_string(raw_log)
        .map_err(|error| format!("could not read sample log {}: {error}", raw_log.display()))?;
    let sample = parse_sample(&source, &source_commit(run)?)?;
    let scheduled = schedule_for_run(run)?;
    if !scheduled.iter().any(|item| item == &sample.scheduled) {
        return Err(format!(
            "sample is not in the prepared schedule: {}/{}/{} order={}",
            sample.scheduled.stack,
            sample.scheduled.workload,
            sample.scheduled.repetition,
            sample.scheduled.order
        ));
    }
    let relative = PathBuf::from("samples")
        .join(&sample.scheduled.stack)
        .join(format!(
            "{}-{}.log",
            sample.scheduled.workload, sample.scheduled.repetition
        ));
    let destination = run.join(&relative);
    if destination.exists() {
        return Err(format!(
            "comparison sample already exists: {}",
            destination.display()
        ));
    }
    fs::create_dir_all(destination.parent().expect("sample has a parent"))
        .map_err(|error| format!("could not create sample directory: {error}"))?;
    fs::copy(raw_log, &destination)
        .map_err(|error| format!("could not bind sample log: {error}"))?;
    append_checksum(run, &relative)?;
    Ok(vec![format!(
        "desktop_comparison_run schema=1 status=recorded order={} stack={} workload={} repetition={} sha256={}",
        sample.scheduled.order,
        sample.scheduled.stack,
        sample.scheduled.workload,
        sample.scheduled.repetition,
        digest_file(&destination)?
    )])
}

pub fn bind_attempt(repo: &Path, run: &Path, raw_attempt: &Path) -> Result<Vec<String>, String> {
    verify_prepared_inputs(repo, run)?;
    verify_checksums(run)?;
    let observed = observed_samples(run)?;
    verify_raw_attempts(run, &observed)?;
    let next = next_scheduled(run)?
        .ok_or_else(|| "desktop comparison matrix is already complete".to_owned())?;
    let preview = replay_attempt(run, raw_attempt)?;
    if preview.order != next.order
        || preview.stack != next.stack
        || preview.workload != next.workload
        || preview.repetition != next.repetition
    {
        return Err(format!(
            "capture does not match next schedule row: expected order={} stack={} workload={} repetition={}",
            next.order, next.stack, next.workload, next.repetition
        ));
    }
    let attempt_root = run.join("attempts");
    fs::create_dir_all(&attempt_root)
        .map_err(|error| format!("could not create attempt root: {error}"))?;
    let destination = attempt_root.join(format!(
        "{:02}-{}-{}-{}",
        next.order, next.stack, next.workload, next.repetition
    ));
    let archived = capture::archive_attempt(run, raw_attempt, &destination)?;
    if archived != preview {
        return Err("archived comparison attempt differs from its source replay".to_owned());
    }
    let mut lines = run_sample(repo, run, &destination.join("result.kdl"))?;
    lines.insert(
        0,
        format!(
            "desktop_comparison_bind schema=2 status=complete order={} stack={} workload={} repetition={} attempt={}",
            next.order,
            next.stack,
            next.workload,
            next.repetition,
            destination.display(),
        ),
    );
    Ok(lines)
}
fn raw_capture_required(run: &Path) -> Result<bool, String> {
    let manifest = fs::read_to_string(run.join("manifest.kdl"))
        .map_err(|error| format!("comparison manifest is missing: {error}"))?;
    Ok(manifest.lines().next().is_some_and(|line| {
        line.split_ascii_whitespace()
            .any(|token| token == "raw_capture_required=true")
    }))
}

fn verify_raw_attempts(
    run: &Path,
    observed_samples: &BTreeSet<ScheduledSample>,
) -> Result<(), String> {
    if !raw_capture_required(run)? {
        return Ok(());
    }
    let root = run.join("attempts");
    let entries = fs::read_dir(&root)
        .map_err(|error| format!("comparison attempt root is missing: {error}"))?;
    let mut observed_attempts = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("could not read comparison attempt: {error}"))?;
        if !entry.path().is_dir()
            || entry
                .file_name()
                .to_str()
                .is_none_or(|name| name.ends_with(".partial"))
        {
            return Err("comparison attempt root contains an unsealed entry".to_owned());
        }
        let replay = capture::verify_archived_attempt(run, &entry.path())?;
        let scheduled = ScheduledSample {
            order: replay.order,
            stack: replay.stack.clone(),
            workload: replay.workload.clone(),
            repetition: replay.repetition,
        };
        if !observed_attempts.insert(scheduled.clone()) {
            return Err("comparison contains duplicate raw attempt evidence".to_owned());
        }
        let sample_path = run.join("samples").join(&scheduled.stack).join(format!(
            "{}-{}.log",
            scheduled.workload, scheduled.repetition
        ));
        let bound = fs::read_to_string(&sample_path)
            .map_err(|error| format!("bound sample is missing for raw attempt: {error}"))?;
        if bound != format!("{}\n", replay.sample_record) {
            return Err("bound sample does not match its raw attempt replay".to_owned());
        }
    }
    if observed_attempts != *observed_samples {
        return Err("raw attempt set does not cover the bound sample matrix".to_owned());
    }
    Ok(())
}

pub fn verify(repo: &Path, run: &Path) -> Result<Vec<String>, String> {
    verify_prepared_inputs(repo, run)?;
    verify_checksums(run)?;
    verify_no_pending_capture(run)?;
    let expected = schedule_for_run(run)?;
    let expected_set = expected.iter().cloned().collect::<BTreeSet<_>>();
    let observed = observed_samples(run)?;
    if observed != expected_set {
        let missing = expected_set.difference(&observed).count();
        let unexpected = observed.difference(&expected_set).count();
        return Err(format!(
            "comparison matrix is incomplete: missing={missing} unexpected={unexpected}"
        ));
    }
    verify_raw_attempts(run, &observed)?;
    Ok(vec![format!(
        "desktop_comparison_verify schema=2 status=complete diagnostic_only=true samples={} relative_performance_gate=false",
        observed.len()
    )])
}

pub fn report(repo: &Path, run: &Path) -> Result<Vec<String>, String> {
    let mut lines = verify(repo, run)?;
    let candidate = source_commit(run)?;
    let mut groups = BTreeMap::<(String, String), Vec<Sample>>::new();
    for entry in sample_paths(run)? {
        let source = fs::read_to_string(&entry)
            .map_err(|error| format!("could not read {}: {error}", entry.display()))?;
        let sample = parse_sample(&source, &candidate)?;
        groups
            .entry((
                sample.scheduled.stack.clone(),
                sample.scheduled.workload.clone(),
            ))
            .or_default()
            .push(sample);
    }
    for ((stack, workload), samples) in groups {
        let count = u64::try_from(samples.len()).unwrap_or(u64::MAX);
        let sum = |field: fn(&Sample) -> u64| {
            samples.iter().map(field).fold(0u64, u64::saturating_add) / count
        };
        lines.push(format!(
            "desktop_comparison_report schema=3 status=diagnostic stack={stack} workload={workload} samples={count} duration_mean_msec={} processes_peak_mean={} pss_peak_mean_kib={} rss_peak_mean_kib={} anonymous_peak_mean_kib={} private_dirty_peak_mean_kib={} cpu_mean_msec={} minor_faults_mean={} major_faults_mean={} threads_peak_mean={} fds_peak_mean={} stack_processes_peak_mean={} stack_pss_peak_mean_kib={} stack_rss_peak_mean_kib={} stack_cpu_mean_msec={} stack_threads_peak_mean={} stack_fds_peak_mean={} workload_processes_peak_mean={} workload_pss_peak_mean_kib={} workload_rss_peak_mean_kib={} workload_cpu_mean_msec={} workload_threads_peak_mean={} workload_fds_peak_mean={} launch_mean_msec={} settle_mean_msec={} resize_p95_mean_msec={} frame_deliveries_mean={} frame_duplicates_mean={} frame_mean_usec={} frame_p50_mean_usec={} frame_p95_mean_usec={} frame_p99_mean_usec={} frame_max_mean_usec={} crashes=0 sample_loss=0 verdict=none",
            sum(|sample| sample.duration_msec),
            sum(|sample| sample.processes),
            sum(|sample| sample.pss_peak_kib),
            sum(|sample| sample.rss_peak_kib),
            sum(|sample| sample.anonymous_peak_kib),
            sum(|sample| sample.private_dirty_peak_kib),
            sum(|sample| sample.cpu_msec),
            sum(|sample| sample.minor_faults),
            sum(|sample| sample.major_faults),
            sum(|sample| sample.threads_peak),
            sum(|sample| sample.fds_peak),
            sum(|sample| sample.stack_processes),
            sum(|sample| sample.stack_pss_peak_kib),
            sum(|sample| sample.stack_rss_peak_kib),
            sum(|sample| sample.stack_cpu_msec),
            sum(|sample| sample.stack_threads_peak),
            sum(|sample| sample.stack_fds_peak),
            sum(|sample| sample.workload_processes),
            sum(|sample| sample.workload_pss_peak_kib),
            sum(|sample| sample.workload_rss_peak_kib),
            sum(|sample| sample.workload_cpu_msec),
            sum(|sample| sample.workload_threads_peak),
            sum(|sample| sample.workload_fds_peak),
            sum(|sample| sample.launch_msec),
            sum(|sample| sample.settle_msec),
            sum(|sample| sample.resize_msec),
            sum(|sample| sample.frame_deliveries),
            sum(|sample| sample.frame_duplicates),
            sum(|sample| sample.frame_mean_usec),
            sum(|sample| sample.frame_p50_usec),
            sum(|sample| sample.frame_p95_usec),
            sum(|sample| sample.frame_p99_usec),
            sum(|sample| sample.frame_max_usec),
        ));
    }
    Ok(lines)
}

pub fn schedule() -> Vec<ScheduledSample> {
    let mut result = Vec::new();
    let mut order = 1usize;
    for workload in SHORT_WORKLOADS {
        for repetition in 1..=3u8 {
            let rotation = usize::from(repetition - 1);
            for offset in 0..STACKS.len() {
                result.push(ScheduledSample {
                    order,
                    stack: STACKS[(rotation + offset) % STACKS.len()].to_owned(),
                    workload: workload.to_owned(),
                    repetition,
                });
                order += 1;
            }
        }
    }
    result
}

pub fn optional_soak_schedule() -> Vec<ScheduledSample> {
    vec![ScheduledSample {
        order: 1,
        stack: "sophia".to_owned(),
        workload: "soak-2h".to_owned(),
        repetition: 1,
    }]
}

fn schedule_for_lane(lane: ComparisonLane) -> Vec<ScheduledSample> {
    match lane {
        ComparisonLane::Interactive => schedule(),
        ComparisonLane::OptionalSoak => optional_soak_schedule(),
    }
}

fn schedule_for_run(run: &Path) -> Result<Vec<ScheduledSample>, String> {
    Ok(schedule_for_lane(prepared_lane(run)?))
}

fn prepared_lane(run: &Path) -> Result<ComparisonLane, String> {
    let manifest = fs::read_to_string(run.join("manifest.kdl"))
        .map_err(|error| format!("comparison manifest is missing: {error}"))?;
    match fields(manifest.lines().next().unwrap_or_default())?
        .get("lane")
        .copied()
    {
        Some("interactive") => Ok(ComparisonLane::Interactive),
        Some("optional-soak") => Ok(ComparisonLane::OptionalSoak),
        Some(lane) => Err(format!("comparison manifest names unknown lane {lane:?}")),
        None => Err("comparison manifest lacks lane".to_owned()),
    }
}

fn parse_sample(source: &str, candidate: &str) -> Result<Sample, String> {
    let records = source
        .lines()
        .filter(|line| {
            line.starts_with("desktop_comparison_sample schema=1 status=complete ")
                || line.starts_with("desktop_comparison_sample schema=2 status=complete ")
                || line.starts_with("desktop_comparison_sample schema=3 status=complete ")
                || line.starts_with("desktop_comparison_sample schema=4 status=complete ")
        })
        .collect::<Vec<_>>();
    if records.len() != 1 {
        return Err(format!(
            "sample log requires exactly one completion record; found {}",
            records.len()
        ));
    }
    let fields = fields(records[0])?;
    let schema = fields.get("schema").copied().unwrap_or_default();
    let required = |name| {
        fields
            .get(name)
            .copied()
            .ok_or_else(|| format!("sample record lacks {name}"))
    };
    let stack = required("stack")?;
    if !STACKS.contains(&stack) {
        return Err(format!("unknown comparison stack {stack:?}"));
    }
    let workload = required("workload")?;
    if !SHORT_WORKLOADS.contains(&workload) && workload != "soak-2h" {
        return Err(format!("unknown comparison workload {workload:?}"));
    }
    let expected_version = match stack {
        "sophia" => candidate,
        "xlibre-xmonad" => XLIBRE_COMMIT,
        "niri" => NIRI_VERSION,
        _ => unreachable!(),
    };
    for (name, expected) in [
        ("backend", "native"),
        ("topology", TOPOLOGY),
        ("kitty", KITTY_VERSION),
        ("firefox", FIREFOX_VERSION),
        ("stack_version", expected_version),
        ("crashes", "0"),
        ("sample_loss", "0"),
    ] {
        if required(name)? != expected {
            return Err(format!(
                "sample {name} does not match the prepared contract"
            ));
        }
    }
    if matches!(schema, "2" | "3" | "4") {
        for (name, expected) in [("frame_source", "kernel_drm"), ("teardown", "clean")] {
            if required(name)? != expected {
                return Err(format!(
                    "sample {name} does not match the raw-capture contract"
                ));
            }
        }
    }
    let numeric = |name| {
        required(name)?
            .parse::<u64>()
            .map_err(|_| format!("sample {name} is not an integer"))
    };
    let compatible_numeric = |name: &str, fallback: u64| {
        fields.get(name).map_or(Ok(fallback), |value| {
            value
                .parse::<u64>()
                .map_err(|_| format!("sample {name} is not an integer"))
        })
    };
    if matches!(schema, "2" | "3" | "4") {
        for name in [
            "resource_samples",
            "anonymous_peak_kib",
            "private_dirty_peak_kib",
            "minor_faults",
            "major_faults",
            "resize_samples",
            "resize_p50_usec",
            "resize_p95_usec",
            "resize_p99_usec",
            "resize_max_usec",
            "frame_p50_usec",
            "frame_p95_usec",
            "frame_p99_usec",
            "frame_max_usec",
            "native_samples",
        ] {
            let _ = numeric(name)?;
        }
        let _ = required("native_timing")?;
        let _ = required("native_source")?;
    }
    if matches!(schema, "3" | "4") {
        for (name, expected) in [
            ("controller_outside_supervisor", "true"),
            ("visible_dp1", "true"),
            ("focused_owned", "true"),
            ("foreign_toplevels", "0"),
        ] {
            if required(name)? != expected {
                return Err(format!(
                    "sample {name} does not match the visibility contract"
                ));
            }
        }
        if numeric("visibility_samples")? == 0 {
            return Err("sample visibility_samples must be positive".to_owned());
        }
    }
    let duration_msec = numeric("duration_msec")?;
    let minimum_duration = match workload {
        "kitty-60s" => 60_000,
        "soak-2h" => 7_200_000,
        _ => 1,
    };
    if duration_msec < minimum_duration {
        return Err(format!("sample duration is below {minimum_duration}ms"));
    }
    for name in [
        "processes",
        "pss_peak_kib",
        "rss_peak_kib",
        "threads_peak",
        "fds_peak",
        "frame_samples",
        "frame_mean_usec",
    ] {
        if numeric(name)? == 0 {
            return Err(format!("sample {name} must be positive"));
        }
    }
    for name in ["cpu_msec", "launch_msec", "settle_msec", "resize_msec"] {
        let _ = numeric(name)?;
    }
    if schema == "4" {
        for name in [
            "stack_processes",
            "stack_pss_peak_kib",
            "stack_rss_peak_kib",
            "stack_threads_peak",
            "stack_fds_peak",
            "workload_processes",
            "workload_pss_peak_kib",
            "workload_rss_peak_kib",
            "workload_threads_peak",
            "workload_fds_peak",
            "frame_deliveries",
        ] {
            if numeric(name)? == 0 {
                return Err(format!("sample {name} must be positive"));
            }
        }
        for name in ["stack_cpu_msec", "workload_cpu_msec", "frame_duplicates"] {
            let _ = numeric(name)?;
        }
        if numeric("frame_deliveries")? < numeric("frame_samples")?.saturating_add(1) {
            return Err(
                "sample frame delivery population is smaller than unique frames".to_owned(),
            );
        }
    }
    Ok(Sample {
        scheduled: ScheduledSample {
            order: usize::try_from(numeric("order")?).map_err(|_| "sample order is too large")?,
            stack: stack.to_owned(),
            workload: workload.to_owned(),
            repetition: u8::try_from(numeric("repetition")?)
                .map_err(|_| "sample repetition is too large")?,
        },
        duration_msec,
        processes: numeric("processes")?,
        pss_peak_kib: numeric("pss_peak_kib")?,
        rss_peak_kib: numeric("rss_peak_kib")?,
        anonymous_peak_kib: compatible_numeric("anonymous_peak_kib", 0)?,
        private_dirty_peak_kib: compatible_numeric("private_dirty_peak_kib", 0)?,
        cpu_msec: numeric("cpu_msec")?,
        minor_faults: compatible_numeric("minor_faults", 0)?,
        major_faults: compatible_numeric("major_faults", 0)?,
        threads_peak: numeric("threads_peak")?,
        fds_peak: numeric("fds_peak")?,
        stack_processes: compatible_numeric("stack_processes", 0)?,
        stack_pss_peak_kib: compatible_numeric("stack_pss_peak_kib", 0)?,
        stack_rss_peak_kib: compatible_numeric("stack_rss_peak_kib", 0)?,
        stack_cpu_msec: compatible_numeric("stack_cpu_msec", 0)?,
        stack_threads_peak: compatible_numeric("stack_threads_peak", 0)?,
        stack_fds_peak: compatible_numeric("stack_fds_peak", 0)?,
        workload_processes: compatible_numeric("workload_processes", 0)?,
        workload_pss_peak_kib: compatible_numeric("workload_pss_peak_kib", 0)?,
        workload_rss_peak_kib: compatible_numeric("workload_rss_peak_kib", 0)?,
        workload_cpu_msec: compatible_numeric("workload_cpu_msec", 0)?,
        workload_threads_peak: compatible_numeric("workload_threads_peak", 0)?,
        workload_fds_peak: compatible_numeric("workload_fds_peak", 0)?,
        launch_msec: numeric("launch_msec")?,
        settle_msec: numeric("settle_msec")?,
        resize_msec: numeric("resize_msec")?,
        frame_mean_usec: numeric("frame_mean_usec")?,
        frame_p50_usec: compatible_numeric("frame_p50_usec", numeric("frame_mean_usec")?)?,
        frame_p95_usec: compatible_numeric("frame_p95_usec", numeric("frame_mean_usec")?)?,
        frame_p99_usec: compatible_numeric("frame_p99_usec", numeric("frame_mean_usec")?)?,
        frame_max_usec: compatible_numeric("frame_max_usec", numeric("frame_mean_usec")?)?,
        frame_deliveries: compatible_numeric(
            "frame_deliveries",
            numeric("frame_samples")?.saturating_add(1),
        )?,
        frame_duplicates: compatible_numeric("frame_duplicates", 0)?,
    })
}

fn fields(line: &str) -> Result<BTreeMap<&str, &str>, String> {
    let mut fields = BTreeMap::new();
    for token in line.split_ascii_whitespace() {
        let Some((name, value)) = token.split_once('=') else {
            continue;
        };
        if fields.insert(name, value).is_some() {
            return Err(format!("desktop comparison record repeats field {name}"));
        }
    }
    Ok(fields)
}

fn verify_prepared_inputs(repo: &Path, run: &Path) -> Result<(), String> {
    let manifest = fs::read_to_string(run.join("manifest.kdl"))
        .map_err(|error| format!("comparison manifest is missing: {error}"))?;
    let acquisition = manifest.lines().next().unwrap_or_default();
    if !acquisition.starts_with("desktop_comparison_manifest schema=4 status=prepared ")
        || !acquisition
            .split_ascii_whitespace()
            .any(|field| field == "acquisition=terminal_free_visible")
        || !acquisition
            .split_ascii_whitespace()
            .any(|field| field == "optional_soak=separate")
    {
        return Err(
            "comparison run predates the terminal-free visibility and optional-soak contract"
                .to_owned(),
        );
    }
    let schedule_file = fs::read_to_string(run.join("schedule.kdl"))
        .map_err(|error| format!("comparison schedule is missing: {error}"))?;
    let expected_schedule = schedule_for_run(run)?
        .iter()
        .map(|item| format!(
            "desktop_comparison_schedule schema=2 order={} stack={} workload={} repetition={} backend=native\n",
            item.order, item.stack, item.workload, item.repetition
        ))
        .collect::<String>();
    if schedule_file != expected_schedule {
        return Err("comparison schedule differs from the typed matrix".to_owned());
    }
    for config in CONFIGS {
        let expected = format!(
            "desktop_comparison_input schema=2 path={config} sha256={}",
            digest_file(&repo.join(config))?
        );
        if !manifest.lines().any(|line| line == expected) {
            return Err(format!(
                "comparison input changed after preparation: {config}"
            ));
        }
    }
    Ok(())
}

pub(crate) fn source_commit(run: &Path) -> Result<String, String> {
    let manifest = fs::read_to_string(run.join("manifest.kdl"))
        .map_err(|error| format!("comparison manifest is missing: {error}"))?;
    let first = manifest
        .lines()
        .next()
        .ok_or("comparison manifest is empty")?;
    fields(first)?
        .get("source_commit")
        .map(|value| (*value).to_owned())
        .ok_or_else(|| "comparison manifest lacks source_commit".to_owned())
}

fn sample_paths(run: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    let root = run.join("samples");
    for stack in fs::read_dir(&root).map_err(|error| format!("sample root is missing: {error}"))? {
        let stack = stack.map_err(|error| format!("could not read sample stack: {error}"))?;
        if !stack.path().is_dir() {
            return Err("sample root contains a non-directory entry".to_owned());
        }
        for sample in fs::read_dir(stack.path())
            .map_err(|error| format!("could not read sample directory: {error}"))?
        {
            let sample = sample.map_err(|error| format!("could not read sample entry: {error}"))?;
            if sample.path().extension().and_then(|value| value.to_str()) != Some("log") {
                return Err("sample directory contains a non-log entry".to_owned());
            }
            paths.push(sample.path());
        }
    }
    paths.sort();
    Ok(paths)
}

fn rewrite_checksums(run: &Path, extra: &[PathBuf]) -> Result<(), String> {
    let mut paths = vec![PathBuf::from("manifest.kdl"), PathBuf::from("schedule.kdl")];
    paths.extend_from_slice(extra);
    let mut output = String::new();
    for relative in paths {
        output.push_str(&format!(
            "{}  {}\n",
            digest_file(&run.join(&relative))?,
            relative.display()
        ));
    }
    fs::write(run.join("checksums.sha256"), output)
        .map_err(|error| format!("could not write comparison checksums: {error}"))
}

fn append_checksum(run: &Path, relative: &Path) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .append(true)
        .open(run.join("checksums.sha256"))
        .map_err(|error| format!("could not open comparison checksums: {error}"))?;
    writeln!(
        file,
        "{}  {}",
        digest_file(&run.join(relative))?,
        relative.display()
    )
    .map_err(|error| format!("could not append comparison checksum: {error}"))
}

pub(crate) fn verify_checksums(run: &Path) -> Result<(), String> {
    let checksums = fs::read_to_string(run.join("checksums.sha256"))
        .map_err(|error| format!("comparison checksums are missing: {error}"))?;
    let mut paths = BTreeSet::new();
    for line in checksums.lines() {
        let (expected, path) = line
            .split_once("  ")
            .ok_or("comparison checksum line is malformed")?;
        if !paths.insert(path.to_owned()) {
            return Err(format!("duplicate comparison checksum for {path}"));
        }
        if digest_file(&run.join(path))? != expected {
            return Err(format!("comparison checksum mismatch: {path}"));
        }
    }
    let expected_count = sample_paths(run)?.len().saturating_add(2);
    if paths.len() != expected_count {
        return Err("comparison checksum set does not cover every raw artifact".to_owned());
    }
    Ok(())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|error| format!("could not create {}: {error}", path.display()))
}

fn digest_file(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn require_token(name: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.chars().any(char::is_whitespace) || value.contains('=') {
        return Err(format!(
            "{name} identity must be one nonempty key-value-safe token"
        ));
    }
    Ok(())
}

fn require_clean_worktree(status: &str) -> Result<(), String> {
    if status.is_empty() {
        Ok(())
    } else {
        Err("desktop comparison requires a clean Sophia worktree".to_owned())
    }
}

fn git_output(repo: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(arguments)
        .output()
        .map_err(|error| format!("could not run git: {error}"))?;
    if !output.status.success() {
        return Err(format!("git {} failed", arguments.join(" ")));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|_| "git output was not UTF-8".to_owned())
}

#[path = "../tests/support/desktop_comparison.rs"]
mod tests;
