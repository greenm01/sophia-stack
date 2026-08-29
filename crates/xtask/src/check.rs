//! Canonical deterministic repository checks.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Stdio};

pub fn run(repo: &Path, arguments: &[String]) -> Result<(), String> {
    match arguments {
        [] => all(repo),
        [subject] if subject == "layout" => layout(repo),
        [subject] => Err(format!("unknown check subject {subject:?}")),
        _ => Err("check accepts at most one subject".to_owned()),
    }
}

fn all(repo: &Path) -> Result<(), String> {
    command(repo, "cargo", &["fmt", "--all", "--check"])?;
    command(repo, "git", &["diff", "--check"])?;
    command_quiet(
        repo,
        "cargo",
        &[
            "metadata",
            "--no-deps",
            "--offline",
            "--format-version",
            "1",
        ],
    )?;
    command(
        repo,
        "cargo",
        &["test", "--offline", "--workspace", "--all-features"],
    )?;
    command(
        repo,
        "cargo",
        &[
            "clippy",
            "--offline",
            "--workspace",
            "--all-features",
            "--all-targets",
        ],
    )?;
    sophia_conformance::profile::check_every_profile(&[])?;
    layout(repo)?;
    anchored_readers(repo)?;
    for tool in [
        "tools/check_direct_scanout_verifier.sh",
        "tools/check_direct_scanout_archive_verifier.sh",
        "tools/check_sophia_standalone_vkcube_verifier.sh",
        "tools/check_hagia_native_matchers.sh",
        "tools/check_mirror_group_physical_verifier.sh",
    ] {
        command(repo, tool, &[])?;
    }
    Ok(())
}

/// Session-log readers must find their records by marker, not by line start.
///
/// Records reach a session log two ways: printed by the session itself, bare;
/// or emitted through `tracing`, decorated with a timestamp, level, module
/// path, and ANSI colour. A reader anchored to the line start sees only the
/// first kind -- and which kind carries a given record is a fact about
/// plumbing, not about evidence, so it changes without anyone deciding to
/// change it.
///
/// That is not hypothetical. The episode-order rules were anchored, saw none
/// of their records, and reported `episode_sessions=0` in every gate summary
/// from archive 0001 onward while never once running. A passing physical run
/// was then refused by a rule that had never worked.
///
/// One reader stays anchored on purpose, and this names it rather than
/// letting an allowlist grow silently.
fn anchored_readers(repo: &Path) -> Result<(), String> {
    let source = repo.join("crates/sophia-conformance/src");
    let mut offenders = BTreeSet::new();
    for entry in std::fs::read_dir(&source)
        .map_err(|error| format!("could not read {}: {error}", source.display()))?
    {
        let path = entry
            .map_err(|error| format!("could not read a conformance source entry: {error}"))?
            .path();
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        if sophia_conformance::direct_scanout::ANCHORED_READER_ALLOWLIST.contains(&name.as_str()) {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        for (index, line) in text.lines().enumerate() {
            if line.contains("starts_with(\"sophia_") || line.contains("strip_prefix(\"sophia_") {
                offenders.insert(format!("{name}:{}", index + 1));
            }
        }
    }
    if offenders.is_empty() {
        return Ok(());
    }
    Err(format!(
        "these conformance readers anchor a session record to the line start, so a \n`tracing`-decorated record is invisible to them:\n  {}\nUse `record_after_marker`. If a reader genuinely parses bare stdout rather \nthan a session log, add its file to ANCHORED_READER_ALLOWLIST with the reason.",
        offenders.into_iter().collect::<Vec<_>>().join("\n  ")
    ))
}

fn layout(repo: &Path) -> Result<(), String> {
    let output = Command::new(repo.join("tools/audit_source_layout.sh"))
        .current_dir(repo)
        .output()
        .map_err(|error| format!("could not run source-layout audit: {error}"))?;
    let mut text = String::from_utf8(output.stdout)
        .map_err(|error| format!("source-layout audit emitted non-UTF-8: {error}"))?;
    text.push_str(
        &String::from_utf8(output.stderr)
            .map_err(|error| format!("source-layout audit emitted non-UTF-8: {error}"))?,
    );
    let observed = text
        .lines()
        .filter_map(normalize_layout_error)
        .collect::<BTreeSet<_>>();
    let ledger_path = repo.join("docs/source-layout-debt.txt");
    let ledger = std::fs::read_to_string(&ledger_path)
        .map_err(|error| format!("could not read {}: {error}", ledger_path.display()))?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if observed == ledger {
        return Ok(());
    }
    let introduced = observed.difference(&ledger).cloned().collect::<Vec<_>>();
    let retired = ledger.difference(&observed).cloned().collect::<Vec<_>>();
    Err(format!(
        "source-layout debt ledger changed\nnew: {}\nretired: {}",
        display_set(&introduced),
        display_set(&retired)
    ))
}

fn normalize_layout_error(line: &str) -> Option<String> {
    let message = line.strip_prefix("error: ")?;
    if let Some(path) = message.strip_prefix("inline tests in ") {
        return Some(format!("inline-tests {path}"));
    }
    if let Some(path) = message.strip_prefix("direct library printing in ") {
        return Some(format!("direct-print {path}"));
    }
    let (path, rest) = message.split_once(" has ")?;
    if rest
        .split_once(" lines")
        .is_some_and(|(count, _)| count.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Some(format!("large {path}"));
    }
    None
}

fn display_set(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_owned()
    } else {
        format!("\n  {}", values.join("\n  "))
    }
}

fn command(repo: &Path, program: &str, arguments: &[&str]) -> Result<(), String> {
    let status = Command::new(program)
        .current_dir(repo)
        .args(arguments)
        .status()
        .map_err(|error| format!("could not run {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {arguments:?} exited with {status}"))
    }
}

fn command_quiet(repo: &Path, program: &str, arguments: &[&str]) -> Result<(), String> {
    let status = Command::new(program)
        .current_dir(repo)
        .args(arguments)
        .stdout(Stdio::null())
        .status()
        .map_err(|error| format!("could not run {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {arguments:?} exited with {status}"))
    }
}
