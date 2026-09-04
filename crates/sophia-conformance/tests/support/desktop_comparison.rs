#![cfg(test)]
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[test]
fn comparison_candidate_must_have_a_clean_worktree() {
    assert!(require_clean_worktree("").is_ok());
    assert!(require_clean_worktree(" M crates/sophia/src/lib.rs").is_err());
}

#[test]
fn tool_version_matching_requires_a_complete_version_token() {
    assert!(version_output_matches(
        "kitty 0.48.2 created by Kovid Goyal",
        "0.48.2"
    ));
    assert!(version_output_matches("Mozilla Firefox 155.0", "155"));
    assert!(version_output_matches(
        "niri 26.04 (unknown commit)",
        "26.04"
    ));
    assert!(!version_output_matches("Mozilla Firefox 154.0", "155"));
    assert!(!version_output_matches("Mozilla Firefox 1155.0", "155"));
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
    prepared_run_for_lane(root, "interactive", schedule())
}

fn prepared_run_for_lane(
    root: &Path,
    lane: &str,
    scheduled: Vec<ScheduledSample>,
) -> (PathBuf, String) {
    let repo = repo();
    let run = root.join("run");
    fs::create_dir(&run).unwrap();
    fs::create_dir(run.join("samples")).unwrap();
    let candidate = git_output(&repo, &["rev-parse", "HEAD"]).unwrap();
    let mut manifest = format!(
        "desktop_comparison_manifest schema=4 status=prepared diagnostic_only=true acquisition=terminal_free_visible lane={lane} optional_soak=separate source_commit={candidate}\n"
    );
    for config in CONFIGS {
        manifest.push_str(&format!(
            "desktop_comparison_input schema=2 path={config} sha256={}\n",
            digest_file(&repo.join(config)).unwrap()
        ));
    }
    fs::write(run.join("manifest.kdl"), manifest).unwrap();
    let encoded = scheduled
        .iter()
        .map(|item| format!(
            "desktop_comparison_schedule schema=2 order={} stack={} workload={} repetition={} backend=native\n",
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
    assert!(verified[0].contains("samples=36"));
    assert!(verified[0].contains("relative_performance_gate=false"));
    let report = report(&repo, &run).unwrap();
    assert_eq!(report.len(), 13);
    assert!(
        report
            .iter()
            .skip(1)
            .all(|line| line.ends_with("verdict=none"))
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn optional_soak_is_a_separate_nonblocking_sophia_lane() {
    assert_eq!(schedule().len(), 36);
    assert!(schedule().iter().all(|item| item.workload != "soak-2h"));

    let soak = optional_soak_schedule();
    assert_eq!(soak.len(), 1);
    assert_eq!(soak[0].stack, "sophia");
    assert_eq!(soak[0].workload, "soak-2h");

    let root = temporary_root("optional-soak");
    let (run, candidate) = prepared_run_for_lane(&root, "optional-soak", soak.clone());
    let incoming = root.join("incoming-soak.log");
    fs::write(&incoming, sample_record(&soak[0], &candidate)).unwrap();
    run_sample(&repo(), &run, &incoming).unwrap();
    let verified = verify(&repo(), &run).unwrap();
    assert!(verified[0].contains("samples=1"));

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

fn raw_capture_attempt(root: &Path, item: &ScheduledSample, candidate: &str) -> PathBuf {
    let attempt = root.join("raw-attempt");
    if attempt.exists() {
        fs::remove_dir_all(&attempt).unwrap();
    }
    fs::create_dir(&attempt).unwrap();
    let version = match item.stack.as_str() {
        "sophia" => candidate,
        "xlibre-xmonad" => XLIBRE_COMMIT,
        "niri" => NIRI_VERSION,
        _ => unreachable!(),
    };
    fs::write(
        attempt.join("attempt.kdl"),
        format!(
            "desktop_comparison_attempt schema=2 status=measured order={} stack={} workload={} repetition={} backend=native stack_version={} topology={} kitty={} firefox={} duration_msec=60000 controller_outside_supervisor=true visibility_samples=60 crashes=0 sample_loss=0 teardown=clean\n",
            item.order,
            item.stack,
            item.workload,
            item.repetition,
            version,
            TOPOLOGY,
            KITTY_VERSION,
            FIREFOX_VERSION,
        ),
    )
    .unwrap();
    let mut visibility = String::from(
        "desktop_comparison_visibility schema=1 phase=baseline seq=0 monotonic_usec=0 owned_toplevels=0 visible_dp1=0 foreign_toplevels=0 focused_visible_dp1=false\n\
         desktop_comparison_visibility schema=1 phase=settled seq=0 monotonic_usec=0 owned_toplevels=1 visible_dp1=1 foreign_toplevels=0 focused_visible_dp1=true\n",
    );
    for seq in 1..=60u64 {
        visibility.push_str(&format!(
            "desktop_comparison_visibility schema=1 phase=sample seq={seq} monotonic_usec={} owned_toplevels=1 visible_dp1=1 foreign_toplevels=0 focused_visible_dp1=true\n",
            seq * 1_000_000,
        ));
    }
    fs::write(attempt.join("visibility.log"), visibility).unwrap();
    let resources = (1..=60u64)
        .map(|seq| {
            format!(
                "desktop_comparison_resource schema=1 seq={seq} monotonic_usec={} processes=4 pss_kib=200000 rss_kib=220000 anonymous_kib=120000 private_dirty_kib=80000 cpu_msec={} minor_faults={} major_faults=0 threads=12 fds=80\n",
                seq * 1_000_000,
                seq * 10,
                seq * 2,
            )
        })
        .collect::<String>();
    fs::write(attempt.join("resources.log"), resources).unwrap();
    let frames = (1..=121u64)
        .map(|seq| {
            format!(
                "desktop_comparison_kernel_frame schema=1 seq={seq} crtc=41 ust_usec={}\n",
                1_000_000 + (seq - 1) * 16_667,
            )
        })
        .collect::<String>();
    fs::write(attempt.join("kernel-frames.log"), frames).unwrap();
    fs::write(
        attempt.join("workload.log"),
        "desktop_comparison_workload schema=1 status=complete launch_usec=100000 settle_usec=200000 resize_samples=0 resize_p50_usec=0 resize_p95_usec=0 resize_p99_usec=0 resize_max_usec=0\n",
    )
    .unwrap();
    fs::write(
        attempt.join("native.log"),
        "desktop_comparison_native_timing schema=1 availability=available source=x-present samples=120\n",
    )
    .unwrap();
    attempt
}

#[test]
fn raw_capture_replays_from_complete_monotonic_populations() {
    let root = temporary_root("raw-replay");
    let (run, candidate) = prepared_run(&root);
    let item = schedule().remove(0);
    let attempt = raw_capture_attempt(&root, &item, &candidate);
    let replay = replay_attempt(&run, &attempt).unwrap();
    assert_eq!(replay.order, 1);
    assert!(
        replay
            .sample_record
            .starts_with("desktop_comparison_sample schema=3 status=complete order=1 ")
    );
    assert!(replay.sample_record.contains("resource_samples=60"));
    assert!(replay.sample_record.contains("frame_samples=120"));
    assert!(replay.sample_record.contains("frame_p95_usec=16667"));
    assert!(replay.sample_record.contains("native_timing=available"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn raw_capture_rejects_truncation_and_nonmonotonic_kernel_time() {
    let root = temporary_root("raw-mutations");
    let (run, candidate) = prepared_run(&root);
    let item = schedule().remove(0);
    let attempt = raw_capture_attempt(&root, &item, &candidate);
    fs::write(
        attempt.join("resources.log"),
        "desktop_comparison_resource schema=1 seq=1 monotonic_usec=1 processes=1 pss_kib=1 rss_kib=1 anonymous_kib=1 private_dirty_kib=1 cpu_msec=0 minor_faults=0 major_faults=0 threads=1 fds=1\n",
    )
    .unwrap();
    assert!(
        replay_attempt(&run, &attempt)
            .unwrap_err()
            .contains("truncated")
    );

    let attempt = raw_capture_attempt(&root, &item, &candidate);
    let resources = fs::read_to_string(attempt.join("resources.log")).unwrap();
    fs::write(
        attempt.join("resources.log"),
        resources.replace(
            "seq=30 monotonic_usec=30000000",
            "seq=30 monotonic_usec=30800000",
        ),
    )
    .unwrap();
    assert!(
        replay_attempt(&run, &attempt)
            .unwrap_err()
            .contains("cadence drifted")
    );

    let attempt = raw_capture_attempt(&root, &item, &candidate);
    let frames = fs::read_to_string(attempt.join("kernel-frames.log")).unwrap();
    fs::write(
        attempt.join("kernel-frames.log"),
        frames.replace(
            "seq=3 crtc=41 ust_usec=1033334",
            "seq=3 crtc=41 ust_usec=1010000",
        ),
    )
    .unwrap();
    assert!(
        replay_attempt(&run, &attempt)
            .unwrap_err()
            .contains("strictly monotonic")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn raw_capture_rejects_missing_focus_and_foreign_windows() {
    let root = temporary_root("visibility-mutations");
    let (run, candidate) = prepared_run(&root);
    let item = schedule().remove(0);
    let attempt = raw_capture_attempt(&root, &item, &candidate);
    let visibility_path = attempt.join("visibility.log");
    let visibility = fs::read_to_string(&visibility_path).unwrap();
    fs::write(
        &visibility_path,
        visibility.replacen(
            "phase=sample seq=30 monotonic_usec=30000000 owned_toplevels=1 visible_dp1=1 foreign_toplevels=0 focused_visible_dp1=true",
            "phase=sample seq=30 monotonic_usec=30000000 owned_toplevels=1 visible_dp1=1 foreign_toplevels=0 focused_visible_dp1=false",
            1,
        ),
    )
    .unwrap();
    assert!(
        replay_attempt(&run, &attempt)
            .unwrap_err()
            .contains("lacks focused workload ownership")
    );

    let attempt = raw_capture_attempt(&root, &item, &candidate);
    let visibility_path = attempt.join("visibility.log");
    let visibility = fs::read_to_string(&visibility_path).unwrap();
    fs::write(
        &visibility_path,
        visibility.replacen(
            "phase=sample seq=30 monotonic_usec=30000000 owned_toplevels=1 visible_dp1=1 foreign_toplevels=0",
            "phase=sample seq=30 monotonic_usec=30000000 owned_toplevels=1 visible_dp1=1 foreign_toplevels=1",
            1,
        ),
    )
    .unwrap();
    assert!(
        replay_attempt(&run, &attempt)
            .unwrap_err()
            .contains("foreign application toplevel")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_manifest_is_preserved_but_not_admitted() {
    let root = temporary_root("legacy-manifest");
    let (run, _) = prepared_run(&root);
    let manifest_path = run.join("manifest.kdl");
    let manifest = fs::read_to_string(&manifest_path).unwrap().replacen(
        "schema=4 status=prepared diagnostic_only=true acquisition=terminal_free_visible lane=interactive optional_soak=separate",
        "schema=3 status=prepared diagnostic_only=true acquisition=terminal_free_visible",
        1,
    );
    fs::write(manifest_path, manifest).unwrap();
    let error = status(&repo(), &run).unwrap_err();
    assert!(error.contains("predates the terminal-free visibility and optional-soak contract"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn status_refuses_a_nonprefix_schedule() {
    let root = temporary_root("status-order");
    let (run, candidate) = prepared_run(&root);
    let repo = repo();
    let item = schedule().remove(1);
    let incoming = root.join("incoming-second.log");
    fs::write(&incoming, sample_record(&item, &candidate)).unwrap();
    run_sample(&repo, &run, &incoming).unwrap();
    assert!(
        status(&repo, &run)
            .unwrap_err()
            .contains("contiguous schedule prefix")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn bind_archives_raw_evidence_and_rejects_later_tampering() {
    let root = temporary_root("raw-bind");
    let (run, candidate) = prepared_run(&root);
    let repo = repo();
    let manifest_path = run.join("manifest.kdl");
    let manifest = fs::read_to_string(&manifest_path).unwrap().replacen(
        "diagnostic_only=true",
        "diagnostic_only=true raw_capture_required=true",
        1,
    );
    fs::write(&manifest_path, manifest).unwrap();
    fs::create_dir(run.join("attempts")).unwrap();
    rewrite_checksums(&run, &[]).unwrap();

    let item = schedule().remove(0);
    let attempt = raw_capture_attempt(&root, &item, &candidate);
    let records = bind_attempt(&repo, &run, &attempt).unwrap();
    assert!(records[0].contains("desktop_comparison_bind schema=2 status=complete"));
    assert!(status(&repo, &run).unwrap()[0].contains("next_order=2"));

    let archived = run.join("attempts").join(format!(
        "{:02}-{}-{}-{}",
        item.order, item.stack, item.workload, item.repetition
    ));
    let extra = archived.join("unowned.log");
    fs::write(&extra, "unowned\n").unwrap();
    assert!(
        status(&repo, &run)
            .unwrap_err()
            .contains("unowned or non-file")
    );
    fs::remove_file(extra).unwrap();
    fs::write(archived.join("resources.log"), "tampered\n").unwrap();
    assert!(
        status(&repo, &run)
            .unwrap_err()
            .contains("attempt checksum mismatch")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn local_capture_parsers_reject_malformed_kernel_time() {
    capture_owner::parse_proc_stat(
        "123 (name with ) marker) S 10 0 0 0 0 0 0 11 0 3 0 7 5 0 0 0 0 4 0 99 0 0",
    )
    .unwrap();

    let root = temporary_root("kernel-normalize");
    let raw = root.join("trace.raw");
    let normalized = root.join("kernel-frames.log");
    fs::write(
        &raw,
        "worker 10.000000: drm_vblank_event_delivered: crtc=41, seq=1\n\
         worker 10.016667: drm_vblank_event_delivered: crtc=42, seq=9\n\
         worker 10.016668: drm_vblank_event_delivered: crtc=41, seq=2\n\
         worker 10.033335: drm_vblank_event_delivered: crtc=41, seq=3\n",
    )
    .unwrap();
    capture_owner::normalize_kernel_trace(&raw, &normalized, 41).unwrap();
    let evidence = fs::read_to_string(&normalized).unwrap();
    assert_eq!(evidence.lines().count(), 3);
    assert!(evidence.contains("seq=3 crtc=41 ust_usec=10033335"));

    fs::write(&raw, "CPU: 0 [LOST 4 EVENTS]\n").unwrap();
    let lost = root.join("lost.log");
    assert!(
        capture_owner::normalize_kernel_trace(&raw, &lost, 41)
            .unwrap_err()
            .contains("lost events")
    );

    fs::write(
        &raw,
        "worker 10.000000: drm_vblank_event_delivered: crtc=41\n\
         worker 9.000000: drm_vblank_event_delivered: crtc=41\n\
         worker 11.000000: drm_vblank_event_delivered: crtc=41\n",
    )
    .unwrap();
    let second = root.join("invalid.log");
    assert!(
        capture_owner::normalize_kernel_trace(&raw, &second, 41)
            .unwrap_err()
            .contains("strictly monotonic")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn partial_capture_and_unpinned_reference_install_fail_closed() {
    let root = temporary_root("partial-capture");
    let (run, _) = prepared_run(&root);
    let repo = repo();
    let incoming = run.join("incoming");
    fs::create_dir(&incoming).unwrap();
    fs::create_dir(incoming.join("row.partial")).unwrap();
    assert!(
        status(&repo, &run)
            .unwrap_err()
            .contains("partial comparison capture")
    );

    let relative = Path::new("relative-prefix");
    assert!(
        install_reference(&repo, &repo, relative)
            .unwrap_err()
            .contains("must be absolute")
    );
    let prefix = root.join("reference");
    assert!(
        install_reference(&repo, &repo, &prefix)
            .unwrap_err()
            .contains("must be pinned")
    );
    assert!(!prefix.exists());
    fs::remove_dir_all(root).unwrap();
}
