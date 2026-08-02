use super::*;

#[test]
fn firefox_m10_kitty_proof_requires_peer_selection_checkpoints() {
    let mut proof = FirefoxM10KittyProof::default();
    let expected = [
        (193, "a", "before"),
        (194, "b", "before"),
        (202, "b", "clipboard_peer"),
        (203, "b", "primary_peer"),
        (211, "a", "after_normal_close"),
        (212, "b", "after_normal_close"),
        (229, "a", "after_forced_close"),
        (230, "b", "after_forced_close"),
    ];

    for (index, (title_bytes, terminal, checkpoint)) in expected.iter().enumerate() {
        assert_eq!(
            proof.observe("_NET_WM_NAME", *title_bytes),
            Some((*terminal, *checkpoint)),
        );
        assert_eq!(proof.completed(), index + 1);
        assert_eq!(proof.observe("_NET_WM_NAME", *title_bytes), None);
    }
    assert!(!proof.complete(3, 4));
    assert!(!proof.complete(4, 3));
    assert!(proof.complete(4, 4));

    let mut lifecycle = FirefoxM10KittyProof::default();
    for title_bytes in [193, 194, 211, 212, 229, 230] {
        assert!(lifecycle.observe("_NET_WM_NAME", title_bytes).is_some());
    }
    assert!(lifecycle.lifecycle_complete());
    assert!(!lifecycle.complete(4, 4));
}

#[test]
fn focused_selection_kitty_proof_requires_all_three_checkpoints() {
    let mut proof = FirefoxM10SelectionKittyProof::default();
    for (title_bytes, checkpoint) in [
        (241, "before"),
        (242, "clipboard_peer"),
        (243, "primary_peer"),
    ] {
        assert_eq!(proof.observe("_NET_WM_NAME", title_bytes), Some(checkpoint));
    }
    assert_eq!(proof.completed(), 3);
    assert!(proof.complete());
}

#[test]
fn firefox_physical_slices_are_mutually_exclusive() {
    let base = [
        "--session-mode=normal".to_owned(),
        "--session-app=firefox=/usr/bin/firefox".to_owned(),
        "--session-action-app=firefox=firefox".to_owned(),
    ];
    for proof in [
        "--firefox-m10-selection-proof",
        "--firefox-m10-lifecycle-proof",
    ] {
        let mut arguments = base.to_vec();
        arguments.push(proof.to_owned());
        let config = PersistentXtermSessionConfig::from_args(&arguments).unwrap();
        assert!(config.firefox_proof_requested());
        assert!(!config.firefox_full_proof_requested());
    }

    let mut conflicting = base.to_vec();
    conflicting.extend([
        "--firefox-m10-selection-proof".to_owned(),
        "--firefox-m10-lifecycle-proof".to_owned(),
    ]);
    assert!(
        PersistentXtermSessionConfig::from_args(&conflicting)
            .unwrap_err()
            .to_string()
            .contains("select only one Firefox proof mode")
    );
}

#[test]
fn live_x_session_profiles_are_explicit_and_fail_closed() {
    let classic = PersistentXtermSessionConfig::from_args(&[]).unwrap();
    assert_eq!(classic.namespace_profile, NamespaceProfile::ClassicShared);
    assert_eq!(classic.namespace_capabilities, NamespaceCapabilities::NONE);

    let confined =
        PersistentXtermSessionConfig::from_args(&["--namespace-profile=confined".to_owned()])
            .unwrap();
    assert_eq!(confined.namespace_profile, NamespaceProfile::Confined);
    assert_eq!(confined.namespace_capabilities, NamespaceCapabilities::NONE);

    assert!(
        PersistentXtermSessionConfig::from_args(&["--namespace-profile=unknown".to_owned()])
            .unwrap_err()
            .to_string()
            .contains("expected classic or confined")
    );
}

#[test]
fn normal_session_application_registry_is_bounded_and_explicit() {
    let config = PersistentXtermSessionConfig::from_args(&[
        "--session-mode=normal".to_owned(),
        "--session-app=terminal=/usr/bin/xterm".to_owned(),
        "--session-app-arg=terminal=-cm".to_owned(),
        "--session-start=terminal".to_owned(),
        "--session-action-app=terminal=terminal".to_owned(),
    ])
    .unwrap();
    assert!(config.normal_session);
    assert_eq!(config.applications.startup, ["terminal"]);
    assert_eq!(
        config
            .application_for_action(WmSessionAction::LaunchApplication {
                application: super::super::TERMINAL_APPLICATION_ID,
            })
            .unwrap()
            .arguments,
        ["-cm"]
    );

    let blank = PersistentXtermSessionConfig::from_args(&[
        "--session-mode=normal".to_owned(),
        "--session-app=terminal=/usr/bin/kitty".to_owned(),
        "--session-action-app=terminal=terminal".to_owned(),
    ])
    .unwrap();
    assert!(blank.applications.startup.is_empty());
    assert!(
        blank
            .application_for_action(WmSessionAction::LaunchApplication {
                application: super::super::TERMINAL_APPLICATION_ID,
            })
            .is_some()
    );

    let dual_terminal = PersistentXtermSessionConfig::from_args(&[
        "--session-mode=normal".to_owned(),
        "--session-app=terminal=/usr/bin/kitty".to_owned(),
        "--session-app=terminal-secondary=/usr/bin/kitty".to_owned(),
        "--session-start=terminal".to_owned(),
        "--session-start=terminal-secondary".to_owned(),
        "--session-action-app=terminal=terminal".to_owned(),
    ])
    .unwrap();
    assert_eq!(
        dual_terminal.applications.startup,
        ["terminal", "terminal-secondary"]
    );
    assert!(!dual_terminal.secondary_terminal);

    for args in [
        vec![
            "--session-mode=normal".to_owned(),
            "--session-app=terminal=xterm".to_owned(),
            "--session-start=terminal".to_owned(),
        ],
        vec![
            "--session-mode=normal".to_owned(),
            "--session-app=terminal=/usr/bin/xterm".to_owned(),
            "--session-start=missing".to_owned(),
        ],
        vec![
            "--session-app=terminal=/usr/bin/xterm".to_owned(),
            "--session-start=terminal".to_owned(),
        ],
        vec![
            "--session-mode=normal".to_owned(),
            "--session-app=terminal=/usr/bin/xterm".to_owned(),
            "--session-app=terminal=/usr/bin/xterm".to_owned(),
            "--session-start=terminal".to_owned(),
        ],
    ] {
        assert!(PersistentXtermSessionConfig::from_args(&args).is_err());
    }
}

#[test]
fn normal_session_rejects_proof_only_options() {
    let result = PersistentXtermSessionConfig::from_args(&[
        "--session-mode=normal".to_owned(),
        "--session-app=terminal=/usr/bin/xterm".to_owned(),
        "--session-start=terminal".to_owned(),
        "--proof".to_owned(),
    ]);
    assert!(result.is_err());
}

#[test]
fn kitty_only_session_can_exit_with_its_single_startup_app() {
    let config = PersistentXtermSessionConfig::from_args(&[
        "--session-mode=normal".to_owned(),
        "--session-app=terminal=/usr/bin/kitty".to_owned(),
        "--session-start=terminal".to_owned(),
        "--exit-when-startup-exits".to_owned(),
    ])
    .unwrap();
    assert!(config.exit_when_startup_exits);

    for args in [
        vec!["--exit-when-startup-exits".to_owned()],
        vec![
            "--session-mode=normal".to_owned(),
            "--session-app=terminal=/usr/bin/kitty".to_owned(),
            "--session-action-app=terminal=terminal".to_owned(),
            "--exit-when-startup-exits".to_owned(),
        ],
    ] {
        assert!(PersistentXtermSessionConfig::from_args(&args).is_err());
    }
}

#[test]
fn startup_readiness_timeout_is_bounded_and_requires_a_startup_app() {
    let config = PersistentXtermSessionConfig::from_args(&[
        "--session-mode=normal".to_owned(),
        "--session-app=terminal=/usr/bin/kitty".to_owned(),
        "--session-start=terminal".to_owned(),
        "--startup-ready-timeout-ms=8000".to_owned(),
    ])
    .unwrap();
    assert_eq!(
        config.startup_ready_timeout,
        Some(Duration::from_millis(8_000))
    );

    for args in [
        vec!["--startup-ready-timeout-ms=8000".to_owned()],
        vec![
            "--session-mode=normal".to_owned(),
            "--session-app=terminal=/usr/bin/kitty".to_owned(),
            "--session-action-app=terminal=terminal".to_owned(),
            "--startup-ready-timeout-ms=8000".to_owned(),
        ],
        vec![
            "--session-mode=normal".to_owned(),
            "--session-app=terminal=/usr/bin/kitty".to_owned(),
            "--session-start=terminal".to_owned(),
            "--startup-ready-timeout-ms=99".to_owned(),
        ],
    ] {
        assert!(PersistentXtermSessionConfig::from_args(&args).is_err());
    }
}

#[test]
fn application_admission_outlives_the_longest_wm_transaction() {
    assert!(
        SESSION_APP_ADMISSION_TIMEOUT_MSEC > u64::from(SESSION_WM_TRANSACTION_TIMEOUT_MAX_MSEC)
    );
}

#[test]
fn production_input_seat_and_explicit_paths_are_distinct_modes() {
    let seat = PersistentXtermSessionConfig::from_args(&[
        "--input-seat=seat0".to_owned(),
        "--max-ticks=1".to_owned(),
    ])
    .unwrap();
    assert_eq!(seat.input_seat.as_deref(), Some("seat0"));
    assert!(seat.input_devices.is_empty());

    assert!(
        PersistentXtermSessionConfig::from_args(&[
            "--input-seat=seat0".to_owned(),
            "--input-devices=/dev/input/event0".to_owned(),
        ])
        .is_err()
    );
    assert!(
        PersistentXtermSessionConfig::from_args(&["--input-seat=../../seat0".to_owned()]).is_err()
    );
}

#[test]
fn live_x_output_injection_is_bounded_and_explicit() {
    let config = PersistentXtermSessionConfig::from_args(&[
        "--inject-output-size=1600x900".to_owned(),
        "--inject-surface-resize=960x640".to_owned(),
    ])
    .unwrap();
    assert_eq!(
        config.inject_output_size,
        Some(Size {
            width: 1600,
            height: 900
        })
    );
    assert_eq!(
        config.inject_surface_resize,
        Some(Size {
            width: 960,
            height: 640
        })
    );
    assert!(
        PersistentXtermSessionConfig::from_args(&["--inject-output-size=0x900".to_owned(),])
            .is_err()
    );
    assert!(
        PersistentXtermSessionConfig::from_args(&["--inject-output-size=wide".to_owned(),])
            .is_err()
    );
}

#[test]
fn live_x_application_client_contract_is_bounded_and_exclusive() {
    let config = PersistentXtermSessionConfig::from_args(&[
        "--client=zenity".to_owned(),
        "--client-arg=--entry".to_owned(),
        "--expect-client-stdout=sophia\n".to_owned(),
        "--require-client-normal-exit".to_owned(),
        "--expect-physical-text=sophia".to_owned(),
        "--expect-physical-pointer".to_owned(),
        "--input-devices=/dev/input/event0,/dev/input/event1".to_owned(),
        "--max-runtime-ms=30000".to_owned(),
    ])
    .unwrap();
    assert_eq!(config.client.as_deref(), Some("zenity"));
    assert_eq!(config.client_args, ["--entry"]);
    assert_eq!(config.expect_client_stdout.as_deref(), Some("sophia\n"));
    assert!(config.require_client_normal_exit);

    assert!(
        PersistentXtermSessionConfig::from_args(&[
            "--client=zenity".to_owned(),
            "--terminal=xterm".to_owned(),
        ])
        .is_err()
    );
    assert!(
        PersistentXtermSessionConfig::from_args(&["--client-arg=--entry".to_owned(),]).is_err()
    );
}

#[test]
fn live_xauthority_file_is_owner_only_valid_and_removed_on_drop() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn field<'a>(record: &'a [u8], offset: &mut usize) -> &'a [u8] {
        let len = usize::from(u16::from_be_bytes([record[*offset], record[*offset + 1]]));
        *offset += 2;
        let value = &record[*offset..*offset + len];
        *offset += len;
        value
    }

    let directory = std::env::temp_dir().join(format!(
        "sophia-live-xauthority-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let (authority, cookie) = LiveXAuthorityFile::create_in(&directory, 77).unwrap();
    let path = authority.path().to_owned();
    let metadata = std::fs::metadata(&path).unwrap();
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);

    let record = std::fs::read(&path).unwrap();
    assert_eq!(u16::from_be_bytes([record[0], record[1]]), 256);
    let mut offset = 2;
    assert_eq!(
        field(&record, &mut offset),
        rustix::system::uname().nodename().to_bytes()
    );
    assert_eq!(field(&record, &mut offset), b"77");
    assert_eq!(field(&record, &mut offset), b"MIT-MAGIC-COOKIE-1");
    assert_eq!(field(&record, &mut offset), cookie);
    assert_eq!(offset, record.len());

    drop(authority);
    assert!(!path.exists());
    std::fs::remove_dir(directory).unwrap();
}
