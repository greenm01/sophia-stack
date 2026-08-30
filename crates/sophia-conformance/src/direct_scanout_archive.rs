//! Typed identity binding and immutable archives for direct-scanout evidence.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::direct_scanout;

const IDENTITY_PREFIX: &str = "sophia_direct_scanout_identity schema=1 status=bound ";
const RESULT: &str = "sophia_direct_scanout schema=1 status=passed\n";
const ARCHIVE_FILES: [&str; 3] = ["manifest", "result.kdl", "session.log"];

pub struct BindEvidence<'a> {
    pub session_log: &'a Path,
    pub evidence: &'a Path,
    pub source_commit: &'a str,
    pub sophia_binary: &'a Path,
    pub client_binary: &'a Path,
    pub core_config: &'a Path,
    pub desktop_profile: &'a Path,
}

pub struct CreateArchive<'a> {
    pub repo: &'a Path,
    pub evidence: &'a Path,
    pub run_root: &'a Path,
    pub sophia_binary: &'a Path,
    pub client_binary: &'a Path,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Identity {
    source_commit: String,
    sophia_sha256: String,
    client: String,
    client_sha256: String,
    core_sha256: String,
    desktop_sha256: String,
}

pub fn bind_evidence(input: &BindEvidence<'_>) -> Result<(), String> {
    validate_commit(input.source_commit, "source commit")?;
    let client = input
        .client_binary
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("the client binary has no UTF-8 file name")?;
    if client.chars().any(char::is_whitespace) {
        return Err("the client binary name cannot contain whitespace".to_owned());
    }
    let identity = Identity {
        source_commit: input.source_commit.to_owned(),
        sophia_sha256: hash_file(input.sophia_binary)?,
        client: client.to_owned(),
        client_sha256: hash_file(input.client_binary)?,
        core_sha256: hash_file(input.core_config)?,
        desktop_sha256: hash_file(input.desktop_profile)?,
    };
    let session = fs::read(input.session_log)
        .map_err(|error| format!("could not read {}: {error}", input.session_log.display()))?;
    if let Some(parent) = input.evidence.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    let mut evidence = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(input.evidence)
        .map_err(|error| format!("could not create {}: {error}", input.evidence.display()))?;
    evidence
        .set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("could not protect {}: {error}", input.evidence.display()))?;
    evidence
        .write_all(&session)
        .and_then(|()| {
            if !session.ends_with(b"\n") {
                evidence.write_all(b"\n")?;
            }
            writeln!(
                evidence,
                "{IDENTITY_PREFIX}source_commit={} sophia_sha256={} client={} client_sha256={} core_sha256={} desktop_sha256={}",
                identity.source_commit,
                identity.sophia_sha256,
                identity.client,
                identity.client_sha256,
                identity.core_sha256,
                identity.desktop_sha256,
            )
        })
        .map_err(|error| format!("could not write {}: {error}", input.evidence.display()))
}

pub fn create_archive(input: &CreateArchive<'_>) -> Result<PathBuf, String> {
    direct_scanout::verify_standalone_logs(&[input.evidence.display().to_string()])?;
    let evidence_text = fs::read_to_string(input.evidence)
        .map_err(|error| format!("could not read {}: {error}", input.evidence.display()))?;
    let identity = parse_identity(&evidence_text)?;
    validate_commit(&identity.source_commit, "evidence source commit")?;
    require_commit(input.repo, &identity.source_commit, "evidence")?;
    require_signed_commit(input.repo, &identity.source_commit, "evidence")?;
    require_executable(input.sophia_binary)?;
    require_executable(input.client_binary)?;

    let evidence_sha256 = hash_file(input.evidence)?;
    let sophia_sha256 = hash_file(input.sophia_binary)?;
    let client_sha256 = hash_file(input.client_binary)?;
    if sophia_sha256 != identity.sophia_sha256 {
        return Err("the Sophia binary no longer matches the verified run".to_owned());
    }
    if client_sha256 != identity.client_sha256 {
        return Err("the client binary no longer matches the verified run".to_owned());
    }

    fs::create_dir_all(input.run_root)
        .map_err(|error| format!("could not create {}: {error}", input.run_root.display()))?;
    fs::set_permissions(input.run_root, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("could not protect {}: {error}", input.run_root.display()))?;
    if archive_with_hash_exists(input.run_root, &evidence_sha256)? {
        return Err("direct-scanout evidence is already archived".to_owned());
    }

    let run = reserve_run_directory(input.run_root)?;
    let result = write_archive(
        &run,
        input.evidence,
        &identity,
        &evidence_sha256,
        &sophia_sha256,
        &client_sha256,
    );
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&run);
        return Err(error);
    }
    verify_archive(input.repo, &run)?;
    Ok(run)
}

pub fn newest_archive(run_root: &Path) -> Result<PathBuf, String> {
    let mut runs = fs::read_dir(run_root)
        .map_err(|error| {
            format!(
                "direct-scanout archive is missing: {}: {error}",
                run_root.display()
            )
        })?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    runs.sort();
    runs.pop()
        .ok_or_else(|| format!("direct-scanout archive is missing: {}", run_root.display()))
}

pub fn verify_archive(repo: &Path, run: &Path) -> Result<(), String> {
    if !run.join("SHA256SUMS").is_file() {
        return Err(format!(
            "direct-scanout archive is missing: {}",
            run.display()
        ));
    }
    verify_checksums(run)?;
    let manifest = parse_manifest(&run.join("manifest"))?;
    if field(&manifest, "record_kind", run)? != "direct_scanout" {
        return Err(format!(
            "direct-scanout manifest records another kind of run: {}",
            run.display()
        ));
    }
    let source_commit = field(&manifest, "source_commit", run)?;
    require_commit(repo, source_commit, "archive")?;
    require_signed_commit(repo, source_commit, "archive")?;

    let evidence = run.join("session.log");
    let evidence_text = fs::read_to_string(&evidence)
        .map_err(|error| format!("could not read {}: {error}", evidence.display()))?;
    let identity = parse_identity(&evidence_text)?;
    if identity.source_commit != source_commit {
        return Err(format!(
            "direct-scanout manifest and evidence disagree on the source commit: {}",
            run.display()
        ));
    }
    if field(&manifest, "evidence_sha256", run)? != hash_file(&evidence)? {
        return Err(format!(
            "direct-scanout manifest does not describe its own evidence: {}",
            run.display()
        ));
    }
    let result = fs::read_to_string(run.join("result.kdl"))
        .map_err(|error| format!("could not read result: {error}"))?;
    if result != RESULT {
        return Err(format!(
            "direct-scanout archive has an invalid result: {}",
            run.display()
        ));
    }
    // An archive re-verifies under the rules that promoted it, not under
    // today's default. A run that opened an overlay proved something stronger
    // -- the return to composition and back -- and re-verifying it as an
    // ordinary run would quietly stop checking the harder half.
    //
    // The manifest says so when it can (`proof=overlay`), and the evidence
    // says so when the manifest predates the field. Both are consulted rather
    // than either alone: the field cannot be added to an already-checksummed
    // archive, and evidence alone would let a plain run that happened to
    // contain a stray record decide its own rules.
    let declared = manifest
        .get("proof")
        .map(|value| value.split(',').collect::<Vec<_>>())
        .unwrap_or_default();
    for (kind, marker) in [
        (
            "overlay",
            "sophia_live_direct_scanout_overlay_proof schema=1 ",
        ),
        (
            "cursor",
            "sophia_live_direct_scanout_cursor_proof schema=1 ",
        ),
    ] {
        if declared.contains(&kind) && !evidence_text.contains(marker) {
            return Err(format!(
                "direct-scanout manifest declares a {kind} proof its evidence does not contain: {}",
                run.display()
            ));
        }
    }
    let overlay = declared.contains(&"overlay")
        || evidence_text.contains("sophia_live_direct_scanout_overlay_proof schema=1 ");
    let cursor = declared.contains(&"cursor")
        || evidence_text.contains("sophia_live_direct_scanout_cursor_proof schema=1 ");
    direct_scanout::verify_standalone_logs_proving(
        &[evidence.display().to_string()],
        overlay,
        false,
        cursor,
    )?;
    Ok(())
}

fn write_archive(
    run: &Path,
    evidence: &Path,
    identity: &Identity,
    evidence_sha256: &str,
    sophia_sha256: &str,
    client_sha256: &str,
) -> Result<(), String> {
    let session = run.join("session.log");
    fs::copy(evidence, &session).map_err(|error| format!("could not archive evidence: {error}"))?;
    write_private(&run.join("result.kdl"), RESULT.as_bytes())?;
    let recorded_at = command_output("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"])?;
    // Which proof this run carried, so a later re-verification applies the
    // rules that promoted it rather than today's default. Read from the
    // evidence, because the evidence is what was proven.
    let archived = fs::read_to_string(&session)
        .map_err(|error| format!("could not read archived evidence: {error}"))?;
    let mut kinds = Vec::new();
    if archived.contains("sophia_live_direct_scanout_overlay_proof schema=1 ") {
        kinds.push("overlay");
    }
    if archived.contains("sophia_live_direct_scanout_cursor_proof schema=1 ") {
        kinds.push("cursor");
    }
    // A set, because one run can carry more than one proof, and a field that
    // held only the first would silently drop the rules of the rest.
    let proof = if kinds.is_empty() {
        String::new()
    } else {
        format!("proof={}\n", kinds.join(","))
    };
    let manifest = format!(
        "record_schema=1\nrecord_kind=direct_scanout\nrecorded_at_utc={}\nsource_commit={}\nevidence_sha256={}\nsophia_binary_sha256={}\nclient_binary_sha256={}\ncore_config_sha256={}\ndesktop_profile_sha256={}\n{proof}",
        recorded_at.trim(),
        identity.source_commit,
        evidence_sha256,
        sophia_sha256,
        client_sha256,
        identity.core_sha256,
        identity.desktop_sha256,
    );
    write_private(&run.join("manifest"), manifest.as_bytes())?;
    fs::set_permissions(&session, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("could not protect {}: {error}", session.display()))?;
    let mut checksums = String::new();
    for name in ARCHIVE_FILES {
        checksums.push_str(&format!("{}  {name}\n", hash_file(&run.join(name))?));
    }
    write_private(&run.join("SHA256SUMS"), checksums.as_bytes())
}

fn verify_checksums(run: &Path) -> Result<(), String> {
    let path = run.join("SHA256SUMS");
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let mut seen = BTreeSet::new();
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let hash = fields.next().unwrap_or_default();
        let name = fields.next().unwrap_or_default();
        if fields.next().is_some()
            || !ARCHIVE_FILES.contains(&name)
            || !is_hash(hash)
            || !seen.insert(name)
            || hash_file(&run.join(name)).as_deref() != Ok(hash)
        {
            return Err(format!(
                "direct-scanout archive checksum verification failed: {}",
                run.display()
            ));
        }
    }
    if seen.len() != ARCHIVE_FILES.len() {
        return Err(format!(
            "direct-scanout archive checksum verification failed: {}",
            run.display()
        ));
    }
    Ok(())
}

fn parse_identity(text: &str) -> Result<Identity, String> {
    let line = text
        .lines()
        .filter(|line| line.starts_with(IDENTITY_PREFIX))
        .last()
        .ok_or("direct-scanout evidence has no bound identity")?;
    let fields = parse_fields(
        line.strip_prefix(IDENTITY_PREFIX)
            .ok_or("direct-scanout evidence has an invalid identity")?,
        "identity",
    )?;
    let value = |name| {
        fields
            .get(name)
            .copied()
            .ok_or_else(|| format!("direct-scanout identity is missing {name}"))
    };
    let identity = Identity {
        source_commit: value("source_commit")?.to_owned(),
        sophia_sha256: value("sophia_sha256")?.to_owned(),
        client: value("client")?.to_owned(),
        client_sha256: value("client_sha256")?.to_owned(),
        core_sha256: value("core_sha256")?.to_owned(),
        desktop_sha256: value("desktop_sha256")?.to_owned(),
    };
    for (name, hash) in [
        ("sophia_sha256", &identity.sophia_sha256),
        ("client_sha256", &identity.client_sha256),
        ("core_sha256", &identity.core_sha256),
        ("desktop_sha256", &identity.desktop_sha256),
    ] {
        if !is_hash(hash) {
            return Err(format!("direct-scanout identity has an invalid {name}"));
        }
    }
    Ok(identity)
}

fn parse_manifest(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        let (name, value) = line.split_once('=').ok_or_else(|| {
            format!(
                "direct-scanout manifest has a malformed field: {}",
                path.display()
            )
        })?;
        if fields.insert(name.to_owned(), value.to_owned()).is_some() {
            return Err(format!(
                "direct-scanout manifest repeats {name}: {}",
                path.display()
            ));
        }
    }
    Ok(fields)
}

fn parse_fields<'a>(text: &'a str, record: &str) -> Result<BTreeMap<&'a str, &'a str>, String> {
    let mut fields = BTreeMap::new();
    for token in text.split_whitespace() {
        let (name, value) = token
            .split_once('=')
            .ok_or_else(|| format!("{record} has a malformed field {token:?}"))?;
        if fields.insert(name, value).is_some() {
            return Err(format!("{record} repeats field {name}"));
        }
    }
    Ok(fields)
}

fn field<'a>(
    manifest: &'a BTreeMap<String, String>,
    name: &str,
    run: &Path,
) -> Result<&'a str, String> {
    manifest.get(name).map(String::as_str).ok_or_else(|| {
        format!(
            "direct-scanout manifest is missing {name}: {}",
            run.display()
        )
    })
}

fn archive_with_hash_exists(run_root: &Path, wanted: &str) -> Result<bool, String> {
    for entry in fs::read_dir(run_root)
        .map_err(|error| format!("could not read {}: {error}", run_root.display()))?
    {
        let path = entry
            .map_err(|error| format!("could not read archive entry: {error}"))?
            .path();
        let manifest = path.join("manifest");
        if manifest.is_file()
            && parse_manifest(&manifest)?
                .get("evidence_sha256")
                .is_some_and(|hash| hash == wanted)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn reserve_run_directory(run_root: &Path) -> Result<PathBuf, String> {
    for sequence in 1u32.. {
        let run = run_root.join(format!("{sequence:04}"));
        match fs::create_dir(&run) {
            Ok(()) => {
                fs::set_permissions(&run, fs::Permissions::from_mode(0o700))
                    .map_err(|error| format!("could not protect {}: {error}", run.display()))?;
                return Ok(run);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(format!("could not create {}: {error}", run.display())),
        }
    }
    unreachable!("u32 archive sequence exhausted")
}

fn write_private(path: &Path, contents: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| format!("could not create {}: {error}", path.display()))?;
    file.write_all(contents)
        .map_err(|error| format!("could not write {}: {error}", path.display()))
}

fn hash_file(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub fn sha256(path: &Path) -> Result<String, String> {
    hash_file(path)
}

fn is_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_commit(value: &str, label: &str) -> Result<(), String> {
    if value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!("direct-scanout {label} is invalid"))
    }
}

fn require_executable(path: &Path) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|_| {
        format!(
            "direct-scanout evidence binary is unavailable: {}",
            path.display()
        )
    })?;
    if metadata.is_file() && metadata.permissions().mode() & 0o111 != 0 {
        Ok(())
    } else {
        Err(format!(
            "direct-scanout evidence binary is unavailable: {}",
            path.display()
        ))
    }
}

fn require_commit(repo: &Path, commit: &str, source: &str) -> Result<(), String> {
    let object = format!("{commit}^{{commit}}");
    if git_status(repo, &["cat-file", "-e", &object])? {
        Ok(())
    } else {
        Err(format!(
            "direct-scanout {source} names a commit this checkout does not have: {commit}"
        ))
    }
}

fn require_signed_commit(repo: &Path, commit: &str, source: &str) -> Result<(), String> {
    if git_status(repo, &["verify-commit", commit])? {
        Ok(())
    } else {
        Err(format!(
            "direct-scanout {source} names a commit without a valid signature: {commit}"
        ))
    }
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

fn command_output(program: &str, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("could not run {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!("{program} exited with {}", output.status));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("{program} emitted non-UTF-8: {error}"))
}
