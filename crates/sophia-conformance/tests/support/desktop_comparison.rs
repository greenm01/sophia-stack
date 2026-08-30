#![cfg(test)]
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[test]
fn comparison_candidate_must_have_a_clean_worktree() {
    assert!(require_clean_worktree("").is_ok());
    assert!(require_clean_worktree(" M crates/sophia/src/lib.rs").is_err());
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("conformance crate is in the workspace")
        .to_path_buf()
}

fn temporary_root(label: &str) -> PathBuf {
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "sophia-desktop-comparison-{label}-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&root).expect("temporary root is unique");
    root
}

fn prepared_run(root: &Path) -> (PathBuf, String) {
    let repo = repo();
    let run = root.join("run");
    fs::create_dir(&run).unwrap();
    fs::create_dir(run.join("samples")).unwrap();
    let candidate = git_output(&repo, &["rev-parse", "HEAD"]).unwrap();
    let mut manifest = format!(
        "desktop_comparison_manifest schema=1 status=prepared diagnostic_only=true source_commit={candidate}\n"
    );
    for config in CONFIGS {
        manifest.push_str(&format!(
            "desktop_comparison_input schema=1 path={config} sha256={}\n",
            digest_file(&repo.join(config)).unwrap()
        ));
    }
    fs::write(run.join("manifest.kdl"), manifest).unwrap();
    let encoded = schedule()
        .iter()
        .map(|item| format!(
            "desktop_comparison_schedule schema=1 order={} stack={} workload={} repetition={} backend=native\n",
            item.order, item.stack, item.workload, item.repetition
        ))
        .collect::<String>();
    fs::write(run.join("schedule.kdl"), encoded).unwrap();
    rewrite_checksums(&run, &[]).unwrap();
    (run, candidate)
}

fn sample_record(item: &ScheduledSample, candidate: &str) -> String {
    let version = match item.stack.as_str() {
        "sophia" => candidate,
        "xlibre-xmonad" => XLIBRE_COMMIT,
        "niri" => NIRI_VERSION,
        _ => unreachable!(),
    };
    let duration = match item.workload.as_str() {
        "kitty-60s" => 60_000,
        "soak-2h" => 7_200_000,
        _ => 10_000,
    };
    format!(
        "desktop_comparison_sample schema=1 status=complete order={} stack={} workload={} repetition={} backend=native stack_version={} topology={} kitty={} firefox={} duration_msec={} processes=4 pss_peak_kib=200000 rss_peak_kib=220000 cpu_msec=1000 threads_peak=12 fds_peak=80 launch_msec=100 settle_msec=200 resize_msec=50 frame_samples=300 frame_mean_usec=16667 crashes=0 sample_loss=0\n",
        item.order,
        item.stack,
        item.workload,
        item.repetition,
        version,
        TOPOLOGY,
        KITTY_VERSION,
        FIREFOX_VERSION,
        duration,
    )
}

#[test]
fn complete_rotated_matrix_verifies_and_reports_without_a_relative_verdict() {
    let root = temporary_root("complete");
    let (run, candidate) = prepared_run(&root);
    let repo = repo();
    for item in schedule() {
        let incoming = root.join(format!("incoming-{}.log", item.order));
        fs::write(&incoming, sample_record(&item, &candidate)).unwrap();
        run_sample(&repo, &run, &incoming).unwrap();
    }

    let verified = verify(&repo, &run).unwrap();
    assert!(verified[0].contains("samples=39"));
    assert!(verified[0].contains("relative_performance_gate=false"));
    let report = report(&repo, &run).unwrap();
    assert_eq!(report.len(), 16);
    assert!(
        report
            .iter()
            .skip(1)
            .all(|line| line.ends_with("verdict=none"))
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn incomplete_matrix_and_raw_tampering_fail_closed() {
    let root = temporary_root("incomplete");
    let (run, candidate) = prepared_run(&root);
    let repo = repo();
    let first = schedule().remove(0);
    let incoming = root.join("incoming.log");
    fs::write(&incoming, sample_record(&first, &candidate)).unwrap();
    run_sample(&repo, &run, &incoming).unwrap();
    assert!(verify(&repo, &run).unwrap_err().contains("incomplete"));

    let bound = run
        .join("samples")
        .join(&first.stack)
        .join(format!("{}-{}.log", first.workload, first.repetition));
    fs::write(&bound, "tampered\n").unwrap();
    assert!(verify_checksums(&run).unwrap_err().contains("mismatch"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn identity_crash_and_sample_loss_are_acceptance_failures() {
    let root = temporary_root("invalid");
    let (run, candidate) = prepared_run(&root);
    let repo = repo();
    let item = schedule().remove(0);
    for (name, from, to) in [
        ("backend", "backend=native", "backend=compatibility"),
        ("crash", "crashes=0", "crashes=1"),
        ("loss", "sample_loss=0", "sample_loss=1"),
    ] {
        let incoming = root.join(format!("{name}.log"));
        fs::write(
            &incoming,
            sample_record(&item, &candidate).replace(from, to),
        )
        .unwrap();
        assert!(run_sample(&repo, &run, &incoming).is_err());
    }

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn duplicate_sample_fields_fail_closed() {
    let root = temporary_root("duplicate-field");
    let (run, candidate) = prepared_run(&root);
    let repo = repo();
    let item = schedule().remove(0);
    let incoming = root.join("duplicate.log");
    let record = sample_record(&item, &candidate).replace("crashes=0", "crashes=1 crashes=0");
    fs::write(&incoming, record).unwrap();
    let error = run_sample(&repo, &run, &incoming).unwrap_err();
    assert!(error.contains("repeats field crashes"));

    fs::remove_dir_all(root).unwrap();
}
