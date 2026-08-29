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

/// A session that flipped, opened an overlay, composed while it was up, and
/// resumed flipping only after a fresh validating commit.
fn overlay_log() -> String {
    [
        "sophia_live_session schema=16 status=bounded_complete display=:77 runtime_surfaces=0 wm_policy=disabled wm_restarts=0",
        "sophia_live_native_resources schema=12 status=complete direct_scanout_attempts=30 direct_scanout_flips=30 direct_scanout_tests=2 direct_scanout_test_rejections=0 direct_scanout_refusals=0 direct_scanout_unsupported=0 direct_scanout_fallbacks=0",
        "sophia_live_direct_scanout_verdicts schema=2 status=complete eligible=32 layer_count=26 layer_not_active=0 layer_resampled=0 layer_offset=0 layer_not_head_sized=0 layer_clipped=0 layer_not_dma_buf=0 layer_translucent=0 composition_required=12 composed_cursor=0",
        "sophia_live_session_present schema=2 status=retired transaction=242 surface=2097166 source=2560x1440 target=2560x1440_0_0 clip=2560x1440_0_0 unit_scale=true",
        "sophia_live_direct_scanout schema=1 status=exported output=1 scene_generation=299 reason=none",
        "sophia_live_direct_scanout schema=1 status=test_passed output=1 scene_generation=299 reason=none",
        "sophia_live_direct_scanout schema=1 status=flipped output=1 scene_generation=299 reason=none",
        "sophia_live_direct_scanout_overlay_proof schema=1 status=activated output=1 flips_before=10",
        "sophia_live_direct_scanout_geometry schema=2 status=composition_required command=rect output=1 head_width=2560 head_height=1440 layer_x=0 layer_y=0 layer_width=2560 layer_height=1440",
        "sophia_live_session_present schema=2 status=retired transaction=243 surface=2097166 source=2560x1440 target=2560x1440_0_0 clip=2560x1440_0_0 unit_scale=true",
        "sophia_live_direct_scanout_overlay_proof schema=1 status=withdrawn output=0 flips_before=10",
        "sophia_live_direct_scanout schema=1 status=exported output=1 scene_generation=400 reason=none",
        "sophia_live_direct_scanout schema=1 status=test_passed output=1 scene_generation=400 reason=none",
        "sophia_live_direct_scanout schema=1 status=flipped output=1 scene_generation=400 reason=none",
        "",
    ]
    .join("\n")
}

fn overlay_verification(text: &str) -> Result<Vec<String>, String> {
    let directory = TempDir::new();
    let log = write_log(&directory, text);
    direct_scanout::verify_standalone_logs_with_overlay(&[log], true)
}

#[test]
fn overlay_verification_accepts_a_return_to_composition() {
    let report = overlay_verification(&overlay_log()).unwrap();
    assert!(
        report
            .iter()
            .any(|line| line.contains("sophia_direct_scanout_overlay schema=1 status=returned")),
        "the overlay window should be reported: {report:?}"
    );
}

/// A run that never opened one is a different proof, not a failed one. Every
/// existing caller and both archives verify through the same entry.
#[test]
fn a_run_without_an_overlay_still_verifies_without_the_requirement() {
    let directory = TempDir::new();
    let log = write_log(&directory, &passing_log());
    direct_scanout::verify_standalone_logs(&[log]).unwrap();
}

#[test]
fn a_run_without_an_overlay_is_refused_when_one_was_required() {
    let error = overlay_verification(&passing_log()).unwrap_err();
    assert!(error.contains("never activated"), "{error}");
}

/// An activation ends the eligibility episode, so a flip inside the window
/// reached the plane under a stamp the activation invalidated.
#[test]
fn a_flip_while_the_overlay_is_up_is_refused() {
    let text = overlay_log().replace(
        "sophia_live_direct_scanout_geometry schema=2 status=composition_required command=rect output=1 head_width=2560 head_height=1440 layer_x=0 layer_y=0 layer_width=2560 layer_height=1440",
        "sophia_live_direct_scanout schema=1 status=exported output=1 scene_generation=350 reason=none\nsophia_live_direct_scanout schema=1 status=flipped output=1 scene_generation=350 reason=none\nsophia_live_direct_scanout_geometry schema=2 status=composition_required command=rect output=1 head_width=2560 head_height=1440 layer_x=0 layer_y=0 layer_width=2560 layer_height=1440",
    );
    let error = overlay_verification(&text).unwrap_err();
    assert!(error.contains("while the overlay was up"), "{error}");
}

/// Without a painting refusal the window is satisfiable by a session that
/// simply stopped drawing, which proves nothing about composition.
#[test]
fn an_overlay_that_drew_nothing_is_refused() {
    let text = overlay_log()
        .lines()
        .filter(|line| !line.contains("sophia_live_direct_scanout_geometry"))
        .collect::<Vec<_>>()
        .join("\n");
    let error = overlay_verification(&text).unwrap_err();
    assert!(error.contains("drew nothing"), "{error}");
}

/// `SuccessorComposedRetires`: the displaced direct frame is retired by a
/// composed successor, and a window with no retirement never exercised it.
#[test]
fn an_overlay_window_with_no_retirement_is_refused() {
    let text = overlay_log().replace(
        "sophia_live_session_present schema=2 status=retired transaction=243 surface=2097166 source=2560x1440 target=2560x1440_0_0 clip=2560x1440_0_0 unit_scale=true\n",
        "",
    );
    let error = overlay_verification(&text).unwrap_err();
    assert!(error.contains("no composed successor"), "{error}");
}

/// `ReProveAfterEpisodeChange`: eligibility returns only through a fresh test.
#[test]
fn a_flip_resuming_without_a_fresh_test_is_refused() {
    let text = overlay_log().replace(
        "sophia_live_direct_scanout schema=1 status=test_passed output=1 scene_generation=400 reason=none\n",
        "",
    );
    let error = overlay_verification(&text).unwrap_err();
    assert!(
        error.contains("without a fresh validating commit"),
        "{error}"
    );
}

/// An overlay left up at session end means the withdrawal never happened, so
/// re-eligibility was never asked for.
#[test]
fn an_overlay_that_never_withdrew_is_refused() {
    let text = overlay_log()
        .lines()
        .filter(|line| !line.contains("status=withdrawn"))
        .collect::<Vec<_>>()
        .join("\n");
    let error = overlay_verification(&text).unwrap_err();
    assert!(error.contains("never withdrew"), "{error}");
}

/// Two activations cannot be paired into one window, and the second would be a
/// second episode the brackets silently swallow.
#[test]
fn an_overlay_that_opened_twice_is_refused() {
    let text = overlay_log().replace(
        "sophia_live_direct_scanout_overlay_proof schema=1 status=withdrawn output=0 flips_before=10",
        "sophia_live_direct_scanout_overlay_proof schema=1 status=activated output=1 flips_before=20\nsophia_live_direct_scanout_overlay_proof schema=1 status=withdrawn output=0 flips_before=20",
    );
    let error = overlay_verification(&text).unwrap_err();
    assert!(error.contains("opened twice"), "{error}");
}

/// The gate and the probe share one argument vocabulary, so the flag has to
/// survive alongside the positional arguments rather than displacing one.
#[test]
fn the_overlay_flag_parses_beside_the_positional_arguments() {
    let plain = direct_scanout_gate::Probe::from_arguments(&[]).unwrap();
    assert!(!plain.overlay_proof, "overlay proof is off by default");

    let flagged =
        direct_scanout_gate::Probe::from_arguments(&["--overlay-proof".to_owned()]).unwrap();
    assert!(flagged.overlay_proof);
    assert_eq!(flagged.width, plain.width, "the flag is not a width");
    assert_eq!(flagged.height, plain.height);

    let mixed = direct_scanout_gate::Probe::from_arguments(&[
        "1920".to_owned(),
        "--overlay-proof".to_owned(),
        "1080".to_owned(),
    ])
    .unwrap();
    assert!(mixed.overlay_proof);
    assert_eq!(mixed.width, 1920);
    assert_eq!(mixed.height, 1080, "the flag must not consume a position");
}

/// The gate reads its terminal from the descriptor rather than from `tty(1)`.
///
/// `Command::output` closes the child's stdin, so the subprocess this replaced
/// was asking `tty` about a null descriptor: on a real tty3 it answered "not a
/// tty" and exited 1, and the gate refused before it could start. Reading
/// `/proc/self/fd/0` asks the descriptor we already hold.
#[test]
fn the_gate_terminal_is_read_from_the_descriptor_not_a_subprocess() {
    use std::process::{Command, Stdio};

    // The mechanism that broke: a captured child cannot see our terminal.
    let captured = Command::new("tty").output().unwrap();
    assert!(
        !captured.status.success(),
        "a captured `tty` child can never identify the parent's terminal"
    );

    // The mechanism that replaced it always resolves for this process,
    // whatever it is attached to -- a terminal for the gate, a socket for a
    // test harness. What it must never do is fail the way the subprocess did.
    std::fs::read_link("/proc/self/fd/0")
        .expect("this process's own stdin descriptor always resolves");

    // And the decision the gate actually makes about what it resolved to.
    assert!(direct_scanout_gate::is_gate_terminal(Path::new(
        "/dev/tty3"
    )));
    assert!(!direct_scanout_gate::is_gate_terminal(Path::new(
        "/dev/tty1"
    )));
    let _ = Stdio::null();
}

/// A record decorated by `tracing` -- timestamp, level, module, ANSI colour --
/// ahead of the marker, as a verbose session log actually carries it.
fn decorated(record: &str) -> String {
    format!(
        "\u{1b}[2m2026-08-29T22:46:34.398135Z\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m \u{1b}[2msophia_backend_live::scanout::rendered_scanout::exporter::direct\u{1b}[0m\u{1b}[2m:\u{1b}[0m {record}"
    )
}

/// Episode records are read wherever the marker sits in the line, because the
/// session emits them through `tracing` and a verbose log decorates them.
///
/// Anchoring to the line start saw only bare records, which was none of them:
/// a physical run whose evidence plainly contained the fresh validating
/// commit was refused for lacking one, and `episode_sessions=0` in every
/// earlier gate summary was the same blindness passing vacuously -- the
/// episode-order rules had never actually run against hardware evidence.
#[test]
fn decorated_episode_records_are_read_like_bare_ones() {
    let text = [
        "sophia_live_session schema=16 status=bounded_complete display=:77 runtime_surfaces=0 wm_policy=disabled wm_restarts=0",
        "sophia_live_native_resources schema=12 status=complete direct_scanout_attempts=30 direct_scanout_flips=30 direct_scanout_tests=2 direct_scanout_test_rejections=0 direct_scanout_refusals=0 direct_scanout_unsupported=0 direct_scanout_fallbacks=0",
        "sophia_live_direct_scanout_verdicts schema=2 status=complete eligible=32 layer_count=26 layer_not_active=0 layer_resampled=0 layer_offset=0 layer_not_head_sized=0 layer_clipped=0 layer_not_dma_buf=0 layer_translucent=0 composition_required=2 composed_cursor=0",
        "sophia_live_session_present schema=2 status=retired transaction=242 surface=2097166 source=2560x1440 target=2560x1440_0_0 clip=2560x1440_0_0 unit_scale=true",
        &decorated("sophia_live_direct_scanout schema=1 status=exported output=1 scene_generation=299 reason=none"),
        &decorated("sophia_live_direct_scanout schema=1 status=test_passed output=1 scene_generation=299 reason=none"),
        &decorated("sophia_live_direct_scanout schema=1 status=flipped output=1 scene_generation=299 reason=none"),
        "sophia_live_direct_scanout_overlay_proof schema=1 status=activated output=1 flips_before=10",
        &decorated("sophia_live_direct_scanout_geometry schema=2 status=composition_required command=rect output=1 head_width=2560 head_height=1440 layer_x=0 layer_y=0 layer_width=2560 layer_height=1440"),
        "sophia_live_session_present schema=2 status=retired transaction=243 surface=2097166 source=2560x1440 target=2560x1440_0_0 clip=2560x1440_0_0 unit_scale=true",
        "sophia_live_direct_scanout_overlay_proof schema=1 status=withdrawn output=0 flips_before=10",
        &decorated("sophia_live_direct_scanout schema=1 status=exported output=1 scene_generation=400 reason=none"),
        &decorated("sophia_live_direct_scanout schema=1 status=test_passed output=1 scene_generation=400 reason=none"),
        &decorated("sophia_live_direct_scanout schema=1 status=flipped output=1 scene_generation=400 reason=none"),
        "",
    ]
    .join("\n");

    let report = overlay_verification(&text).expect("decorated records satisfy every rule");
    assert!(
        report
            .iter()
            .any(|line| line.contains("sophia_direct_scanout_overlay schema=1 status=returned")),
        "{report:?}"
    );
    // And the order rules actually ran: a session whose episodes were seen is
    // counted, where the blind reader always reported zero.
    assert!(
        report
            .iter()
            .any(|line| line.contains("episode_sessions=1")),
        "episode records were not seen: {report:?}"
    );
}

/// The rules still bite on decorated records: a decorated flip inside the
/// window is a flip inside the window.
#[test]
fn a_decorated_flip_inside_the_window_is_still_refused() {
    let text = overlay_log().replace(
        "sophia_live_direct_scanout_overlay_proof schema=1 status=withdrawn output=0 flips_before=10",
        &format!(
            "{}\n{}\nsophia_live_direct_scanout_overlay_proof schema=1 status=withdrawn output=0 flips_before=10",
            decorated("sophia_live_direct_scanout schema=1 status=exported output=1 scene_generation=350 reason=none"),
            decorated("sophia_live_direct_scanout schema=1 status=flipped output=1 scene_generation=350 reason=none")
        ),
    );
    let error = overlay_verification(&text).unwrap_err();
    assert!(error.contains("while the overlay was up"), "{error}");
}

/// Attempts with no readable episode records mean the reader is blind.
///
/// This is the rule that was missing. Every attempt emits an `exported`
/// record, so counters without episodes is not a quiet session -- it is a
/// matcher failing to match. `episode_sessions=0` sat in every gate summary
/// from archive 0001 onward while the order rules never ran, and nothing
/// asked why.
#[test]
fn attempts_without_readable_episodes_are_refused() {
    let text = passing_log()
        .lines()
        .filter(|line| !line.contains("sophia_live_direct_scanout schema=1 status="))
        .collect::<Vec<_>>()
        .join("\n");
    let directory = TempDir::new();
    let log = write_log(&directory, &text);
    let error = direct_scanout::verify_logs(&[log]).unwrap_err();
    assert!(error.contains("the reader is not matching"), "{error}");
}

/// A session whose every record is decorated verifies end to end, which is
/// what the promoted archives actually look like.
#[test]
fn a_fully_decorated_session_verifies() {
    let text = passing_log()
        .lines()
        .map(|line| {
            if line.is_empty() {
                line.to_owned()
            } else {
                decorated(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let directory = TempDir::new();
    let log = write_log(&directory, &text);
    let report = direct_scanout::verify_standalone_logs(&[log]).unwrap();
    assert!(
        report
            .iter()
            .any(|line| line.contains("episode_sessions=1")),
        "{report:?}"
    );
}
