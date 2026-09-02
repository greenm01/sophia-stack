//! Exact XLibre and xmonad reference installation.

use super::super::{XLIBRE_COMMIT, XMONAD_CONTRIB_VERSION, XMONAD_VERSION, git_output};
use super::write_new;
use sha2::Digest as _;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn install_reference(
    repo: &Path,
    xlibre_source: &Path,
    prefix: &Path,
) -> Result<Vec<String>, String> {
    if !prefix.is_absolute() {
        return Err("desktop comparison reference prefix must be absolute".to_owned());
    }
    let commit = git_output(xlibre_source, &["rev-parse", "HEAD"])?;
    if commit != XLIBRE_COMMIT {
        return Err(format!(
            "XLibre source must be pinned at {XLIBRE_COMMIT}; observed {commit}"
        ));
    }
    let worktree = git_output(xlibre_source, &["status", "--porcelain"])?;
    if !worktree.is_empty() {
        return Err(
            "desktop comparison reference installation requires a clean XLibre worktree".to_owned(),
        );
    }
    let xlibre = ["Xorg", "XLibre"]
        .into_iter()
        .map(|name| prefix.join("bin").join(name))
        .find(|path| path.is_file())
        .ok_or_else(|| {
            format!(
                "dedicated prefix has no XLibre server under {}/bin",
                prefix.display()
            )
        })?;
    let version = Command::new(&xlibre)
        .arg("-version")
        .output()
        .map_err(|error| format!("could not inspect staged XLibre server: {error}"))?;
    let version_text = format!(
        "{}{}",
        String::from_utf8_lossy(&version.stdout),
        String::from_utf8_lossy(&version.stderr)
    );
    if !version_text.contains("XLibre") {
        return Err("staged server identifies as X.Org rather than XLibre".to_owned());
    }
    let [xmonad_package, xmonad_contrib_package] = ghc_package_ids()?;

    let bin = prefix.join("bin");
    let identity = prefix.join("share/sophia-desktop-comparison");
    fs::create_dir_all(&bin)
        .and_then(|()| fs::create_dir_all(&identity))
        .map_err(|error| format!("could not create reference identity directories: {error}"))?;
    let xmonad = bin.join("xmonad");
    let partial = bin.join("xmonad.sophia-comparison.partial");
    let build = identity.join("xmonad-build.partial");
    for path in [
        &xmonad,
        &partial,
        &build,
        &identity.join("xlibre-commit"),
        &identity.join("xmonad-version"),
        &identity.join("xmonad-contrib-version"),
        &identity.join("xmonad-profile-sha256"),
    ] {
        if path.exists() {
            return Err(format!(
                "reference installation refuses existing artifact: {}",
                path.display()
            ));
        }
    }

    fs::create_dir(&build)
        .map_err(|error| format!("could not create isolated xmonad build directory: {error}"))?;
    let profile = repo.join("validation/desktop-comparison/profiles/xmonad.hs");
    let compilation = Command::new("ghc")
        .args(["-O2", "-threaded", "-package-id"])
        .arg(&xmonad_package)
        .arg("-package-id")
        .arg(&xmonad_contrib_package)
        .arg("-outputdir")
        .arg(&build)
        .arg(&profile)
        .arg("-o")
        .arg(&partial)
        .status()
        .map_err(|error| format!("could not compile isolated xmonad profile: {error}"));
    let cleanup = fs::remove_dir_all(&build)
        .map_err(|error| format!("could not clean isolated xmonad build directory: {error}"));
    let status = compilation?;
    cleanup?;
    if !status.success() {
        if partial.exists() {
            fs::remove_file(&partial)
                .map_err(|error| format!("could not remove failed xmonad executable: {error}"))?;
        }
        return Err("isolated xmonad profile compilation failed".to_owned());
    }
    let compiled_version = match Command::new(&partial).arg("--version").output() {
        Ok(output) => output,
        Err(error) => {
            let _ = fs::remove_file(&partial);
            return Err(format!(
                "could not inspect compiled xmonad profile: {error}"
            ));
        }
    };
    let compiled_version_text = format!(
        "{}{}",
        String::from_utf8_lossy(&compiled_version.stdout),
        String::from_utf8_lossy(&compiled_version.stderr)
    );
    if !compiled_version.status.success()
        || compiled_version_text.trim() != format!("xmonad {XMONAD_VERSION}")
    {
        fs::remove_file(&partial)
            .map_err(|error| format!("could not remove mismatched xmonad executable: {error}"))?;
        return Err(format!(
            "compiled xmonad version mismatch: expected {:?}, observed {:?}",
            format!("xmonad {XMONAD_VERSION}"),
            compiled_version_text.trim(),
        ));
    }
    fs::rename(&partial, &xmonad)
        .map_err(|error| format!("could not seal isolated xmonad executable: {error}"))?;
    let profile_digest = format!(
        "{:x}",
        sha2::Sha256::digest(
            fs::read(&profile)
                .map_err(|error| format!("could not hash xmonad profile: {error}"))?
        )
    );
    write_new(
        &identity.join("xlibre-commit"),
        format!("{XLIBRE_COMMIT}\n").as_bytes(),
    )?;
    write_new(
        &identity.join("xmonad-version"),
        format!("{XMONAD_VERSION}\n").as_bytes(),
    )?;
    write_new(
        &identity.join("xmonad-contrib-version"),
        format!("{XMONAD_CONTRIB_VERSION}\n").as_bytes(),
    )?;
    write_new(
        &identity.join("xmonad-profile-sha256"),
        format!("{profile_digest}\n").as_bytes(),
    )?;
    Ok(vec![format!(
        "desktop_comparison_reference_install schema=1 status=complete prefix={} xlibre_commit={} xmonad={} xmonad_contrib={} profile_sha256={}",
        prefix.display(),
        XLIBRE_COMMIT,
        XMONAD_VERSION,
        XMONAD_CONTRIB_VERSION,
        profile_digest,
    )])
}

fn ghc_package_ids() -> Result<[String; 2], String> {
    let output = Command::new("ghc")
        .args(["-e", ":show packages"])
        .output()
        .map_err(|error| format!("could not inspect xmonad build packages: {error}"))?;
    let observed = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() {
        return Err(format!(
            "GHC could not report its active package environment: {:?}",
            observed.trim()
        ));
    }
    let resolve = |name: &str, version: &str| {
        let prefix = format!("{name}-{version}-");
        let mut matches = observed.lines().filter_map(|line| {
            line.trim()
                .strip_prefix("-package-id ")
                .filter(|package| package.starts_with(&prefix))
        });
        let package = matches.next().ok_or_else(|| {
            format!(
                "GHC package mismatch: expected {name} {version}, observed {:?}",
                observed.trim()
            )
        })?;
        if matches.next().is_some() {
            return Err(format!(
                "GHC package environment contains multiple {name} {version} identities"
            ));
        }
        Ok(package.to_owned())
    };
    Ok([
        resolve("xmonad", XMONAD_VERSION)?,
        resolve("xmonad-contrib", XMONAD_CONTRIB_VERSION)?,
    ])
}
