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
