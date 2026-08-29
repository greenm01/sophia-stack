use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sophia_conformance::{direct_scanout, direct_scanout_archive, direct_scanout_gate};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "sophia-conformance-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn passing_log() -> String {
    [
        "sophia_live_session schema=16 status=bounded_complete display=:77 runtime_surfaces=0 wm_policy=disabled wm_restarts=0",
        "sophia_live_session_present schema=2 status=retired transaction=242 surface=2097166 source=2560x1440 target=2560x1440_0_0 clip=2560x1440_0_0 unit_scale=true",
        "sophia_live_native_resources schema=12 status=complete direct_scanout_attempts=30 direct_scanout_flips=30 direct_scanout_tests=1 direct_scanout_test_rejections=0 direct_scanout_refusals=0 direct_scanout_unsupported=0 direct_scanout_fallbacks=0",
        "sophia_live_direct_scanout_verdicts schema=2 status=complete eligible=32 layer_count=26 layer_not_active=0 layer_resampled=0 layer_offset=0 layer_not_head_sized=0 layer_clipped=0 layer_not_dma_buf=0 layer_translucent=0 composition_required=0 composed_cursor=0",
        "sophia_live_direct_scanout schema=1 status=exported output=1 scene_generation=299 reason=none",
        "sophia_live_direct_scanout schema=1 status=test_passed output=1 scene_generation=299 reason=none",
        "sophia_live_direct_scanout schema=1 status=flipped output=1 scene_generation=299 reason=none",
        "",
    ]
    .join("\n")
}

fn write_log(directory: &TempDir, text: &str) -> String {
    let path = directory.path().join("session.log");
    fs::write(&path, text).unwrap();
    path.display().to_string()
}

#[test]
fn standalone_verification_accepts_the_promoted_schema() {
    let directory = TempDir::new();
    let log = write_log(&directory, &passing_log());
    let report = direct_scanout::verify_standalone_logs(&[log]).unwrap();
    assert!(report.iter().any(|line| line.contains("30 flips")));
}

#[test]
fn duplicate_telemetry_fields_fail_closed() {
    let directory = TempDir::new();
    let text = passing_log().replace(
        "direct_scanout_attempts=30",
        "direct_scanout_attempts=30 direct_scanout_attempts=30",
    );
    let log = write_log(&directory, &text);
    let error = direct_scanout::verify_logs(&[log]).unwrap_err();
    assert!(error.contains("repeats field direct_scanout_attempts"));
}

#[test]
fn an_episode_cannot_flip_before_export() {
    let directory = TempDir::new();
    let text = passing_log().replace(
        "sophia_live_direct_scanout schema=1 status=exported output=1 scene_generation=299 reason=none\n",
        "",
    );
    let log = write_log(&directory, &text);
    let error = direct_scanout::verify_logs(&[log]).unwrap_err();
    assert!(error.contains("scene never exported"));
}

#[test]
fn standalone_verification_rejects_window_manager_chrome() {
    let directory = TempDir::new();
    let text = passing_log().replace("wm_policy=disabled", "wm_policy=required");
    let log = write_log(&directory, &text);
    let error = direct_scanout::verify_standalone_logs(&[log]).unwrap_err();
    assert!(error.contains("window manager ran"));
}

#[test]
fn probe_arguments_are_typed_before_the_display_is_taken() {
    let error = direct_scanout_gate::Probe::from_arguments(&[
        "zero".to_owned(),
        "1440".to_owned(),
        "20".to_owned(),
        "kitty".to_owned(),
    ])
    .unwrap_err();
    assert!(error.contains("width must be a positive integer"));

    let error = direct_scanout_gate::Probe::from_arguments(&[
        "2560".to_owned(),
        "1440".to_owned(),
        "20".to_owned(),
        "unknown".to_owned(),
    ])
    .unwrap_err();
    assert!(error.contains("workload must be"));
}

#[test]
fn identity_binding_preserves_the_archive_schema() {
    let directory = TempDir::new();
    let session = directory.path().join("input.log");
    let evidence = directory.path().join("evidence.log");
    let sophia = directory.path().join("sophia");
    let client = directory.path().join("kitty");
    let core = directory.path().join("core.kdl");
    let desktop = directory.path().join("desktop.kdl");
    fs::write(&session, passing_log()).unwrap();
    fs::write(&sophia, "sophia").unwrap();
    fs::write(&client, "client").unwrap();
    fs::write(&core, "core").unwrap();
    fs::write(&desktop, "desktop").unwrap();

    direct_scanout_archive::bind_evidence(&direct_scanout_archive::BindEvidence {
        session_log: &session,
        evidence: &evidence,
        source_commit: "0123456789012345678901234567890123456789",
        sophia_binary: &sophia,
        client_binary: &client,
        core_config: &core,
        desktop_profile: &desktop,
    })
    .unwrap();

    let text = fs::read_to_string(evidence).unwrap();
    let identity = text.lines().last().unwrap();
    assert!(
        identity.starts_with("sophia_direct_scanout_identity schema=1 status=bound source_commit=")
    );
    assert!(identity.contains(" client=kitty "));
    assert!(identity.ends_with(&format!(
        "desktop_sha256={}",
        direct_scanout_archive::sha256(&desktop).unwrap()
    )));
}
