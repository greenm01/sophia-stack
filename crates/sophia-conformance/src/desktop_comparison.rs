//! Typed orchestration and evidence reduction for the diagnostic desktop matrix.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const XLIBRE_COMMIT: &str = "56be9f4320ef121dc5d4bc40a6365d995512d3bc";
pub const NIRI_VERSION: &str = "26.04";
pub const KITTY_VERSION: &str = "0.48.2";
pub const FIREFOX_VERSION: &str = "154";
pub const TOPOLOGY: &str = "DP-1:2560x1440@60000+DP-2:1920x1080@60000";

const STACKS: [&str; 3] = ["sophia", "xlibre-xmonad", "niri"];
const SHORT_WORKLOADS: [&str; 4] = ["kitty-60s", "firefox-local", "resize", "kitty-burst-16"];
const CONFIGS: [&str; 4] = [
    "validation/desktop-comparison/config/sophia.kdl",
    "validation/desktop-comparison/config/xlibre-xmonad.kdl",
    "validation/desktop-comparison/config/niri.kdl",
    "validation/desktop-comparison/firefox/index.html",
];

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ScheduledSample {
    order: usize,
    stack: String,
    workload: String,
    repetition: u8,
}

#[derive(Clone, Debug)]
struct Sample {
    scheduled: ScheduledSample,
    duration_msec: u64,
    pss_peak_kib: u64,
    rss_peak_kib: u64,
    cpu_msec: u64,
    frame_mean_usec: u64,
}

pub fn prepare(
    repo: &Path,
    run: &Path,
    kernel: &str,
    mesa: &str,
    gpu: &str,
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
    let mut manifest = format!(
        "desktop_comparison_manifest schema=1 status=prepared diagnostic_only=true source_commit={source_commit} candidate_signature=verified kernel={kernel} mesa={mesa} gpu={gpu} topology={TOPOLOGY} kitty={KITTY_VERSION} firefox={FIREFOX_VERSION}\n"
    );
    manifest.push_str(&format!(
        "desktop_comparison_stack schema=1 id=sophia version={source_commit} backend=native\n"
    ));
    manifest.push_str(&format!(
        "desktop_comparison_stack schema=1 id=xlibre-xmonad version={XLIBRE_COMMIT} xmonad=0.18.1.9 backend=native\n"
    ));
    manifest.push_str(&format!(
        "desktop_comparison_stack schema=1 id=niri version={NIRI_VERSION} path=/usr/bin/niri backend=native\n"
    ));
    for config in CONFIGS {
        let path = repo.join(config);
        manifest.push_str(&format!(
            "desktop_comparison_input schema=1 path={config} sha256={}\n",
            digest_file(&path)?
        ));
    }
    write_new(&run.join("manifest.kdl"), manifest.as_bytes())?;

    let schedule = schedule();
    let mut encoded = String::new();
    for item in &schedule {
        encoded.push_str(&format!(
            "desktop_comparison_schedule schema=1 order={} stack={} workload={} repetition={} backend=native\n",
            item.order, item.stack, item.workload, item.repetition
        ));
    }
    write_new(&run.join("schedule.kdl"), encoded.as_bytes())?;
    rewrite_checksums(run, &[])?;
    Ok(vec![format!(
        "desktop_comparison_prepare schema=1 status=complete run={} source_commit={} samples={}",
        run.display(),
        source_commit,
        schedule.len()
    )])
}

/// Ingest one native-stack adapter log and bind it to the prepared schedule.
pub fn run_sample(repo: &Path, run: &Path, raw_log: &Path) -> Result<Vec<String>, String> {
    verify_prepared_inputs(repo, run)?;
    verify_checksums(run)?;
    let source = fs::read_to_string(raw_log)
        .map_err(|error| format!("could not read sample log {}: {error}", raw_log.display()))?;
    let sample = parse_sample(&source, &source_commit(run)?)?;
    let scheduled = schedule();
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

pub fn verify(repo: &Path, run: &Path) -> Result<Vec<String>, String> {
    verify_prepared_inputs(repo, run)?;
    verify_checksums(run)?;
    let candidate = source_commit(run)?;
    let expected = schedule();
    let expected_set = expected.iter().cloned().collect::<BTreeSet<_>>();
    let mut observed = BTreeSet::new();
    for entry in sample_paths(run)? {
        let source = fs::read_to_string(&entry)
            .map_err(|error| format!("could not read {}: {error}", entry.display()))?;
        let sample = parse_sample(&source, &candidate)?;
        if !observed.insert(sample.scheduled.clone()) {
            return Err("comparison contains a duplicate scheduled sample".to_owned());
        }
    }
    if observed != expected_set {
        let missing = expected_set.difference(&observed).count();
        let unexpected = observed.difference(&expected_set).count();
        return Err(format!(
            "comparison matrix is incomplete: missing={missing} unexpected={unexpected}"
        ));
    }
    Ok(vec![format!(
        "desktop_comparison_verify schema=1 status=complete diagnostic_only=true samples={} relative_performance_gate=false",
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
            "desktop_comparison_report schema=1 status=diagnostic stack={stack} workload={workload} samples={count} duration_mean_msec={} pss_peak_mean_kib={} rss_peak_mean_kib={} cpu_mean_msec={} frame_mean_usec={} verdict=none",
            sum(|sample| sample.duration_msec),
            sum(|sample| sample.pss_peak_kib),
            sum(|sample| sample.rss_peak_kib),
            sum(|sample| sample.cpu_msec),
            sum(|sample| sample.frame_mean_usec),
        ));
    }
    Ok(lines)
}

fn schedule() -> Vec<ScheduledSample> {
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
    for stack in STACKS {
        result.push(ScheduledSample {
            order,
            stack: stack.to_owned(),
            workload: "soak-2h".to_owned(),
            repetition: 1,
        });
        order += 1;
    }
    result
}

fn parse_sample(source: &str, candidate: &str) -> Result<Sample, String> {
    let records = source
        .lines()
        .filter(|line| line.starts_with("desktop_comparison_sample schema=1 status=complete "))
        .collect::<Vec<_>>();
    if records.len() != 1 {
        return Err(format!(
            "sample log requires exactly one completion record; found {}",
            records.len()
        ));
    }
    let fields = fields(records[0])?;
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
    let numeric = |name| {
        required(name)?
            .parse::<u64>()
            .map_err(|_| format!("sample {name} is not an integer"))
    };
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
    Ok(Sample {
        scheduled: ScheduledSample {
            order: usize::try_from(numeric("order")?).map_err(|_| "sample order is too large")?,
            stack: stack.to_owned(),
            workload: workload.to_owned(),
            repetition: u8::try_from(numeric("repetition")?)
                .map_err(|_| "sample repetition is too large")?,
        },
        duration_msec,
        pss_peak_kib: numeric("pss_peak_kib")?,
        rss_peak_kib: numeric("rss_peak_kib")?,
        cpu_msec: numeric("cpu_msec")?,
        frame_mean_usec: numeric("frame_mean_usec")?,
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
    let schedule_file = fs::read_to_string(run.join("schedule.kdl"))
        .map_err(|error| format!("comparison schedule is missing: {error}"))?;
    let expected_schedule = schedule()
        .iter()
        .map(|item| format!(
            "desktop_comparison_schedule schema=1 order={} stack={} workload={} repetition={} backend=native\n",
            item.order, item.stack, item.workload, item.repetition
        ))
        .collect::<String>();
    if schedule_file != expected_schedule {
        return Err("comparison schedule differs from the typed matrix".to_owned());
    }
    for config in CONFIGS {
        let expected = format!(
            "desktop_comparison_input schema=1 path={config} sha256={}",
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

fn source_commit(run: &Path) -> Result<String, String> {
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

fn verify_checksums(run: &Path) -> Result<(), String> {
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
