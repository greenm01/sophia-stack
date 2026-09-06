use super::*;

#[test]
fn headless_panel_session_explicitly_disables_physical_input() {
    let config = PersistentXtermSessionConfig::from_args(&[
        "--no-config".to_owned(),
        "--no-input".to_owned(),
    ])
    .unwrap();
    assert!(config.input_seat.is_none());
    assert!(config.input_devices.is_empty());
    for override_arg in ["--input-seat=seat0", "--input-devices=/dev/input/event0"] {
        assert!(
            PersistentXtermSessionConfig::from_args(&[
                "--no-config".to_owned(),
                "--no-input".to_owned(),
                override_arg.to_owned(),
            ])
            .is_err()
        );
    }
}

#[test]
fn runtime_profiles_remain_readable_after_public_policy_activation() {
    let mut config = PersistentXtermSessionConfig::from_args(&[
        "--no-config".to_owned(),
        "--no-input".to_owned(),
    ])
    .unwrap();
    let key = sophia_config::DesktopProfileActivationKey::from(&config.desktop_profile);
    let input = config.input_profile.current().clone();
    let output = config.output_profile.current().clone();
    *config.input_profile.slot_mut() =
        sophia_config::activate_desktop_profile_candidate_slot(config.input_profile.slot(), key)
            .unwrap();
    *config.output_profile.slot_mut() =
        sophia_config::activate_desktop_profile_candidate_slot(config.output_profile.slot(), key)
            .unwrap();
    assert_eq!(config.input_profile.current(), &input);
    assert_eq!(config.output_profile.current(), &output);
    // Repreparing for reload must still expose the newly staged payload.
    config.output_profile = PreparedOutputProfile::new(output.clone()).unwrap();
    assert_eq!(config.output_profile.current(), &output);
}

#[test]
fn desktop_startup_list_beats_launcher_default_but_explicit_cli_still_wins() {
    use std::os::unix::fs::PermissionsExt;
    let directory =
        std::env::temp_dir().join(format!("sophia-startup-selection-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let core = directory.join("core.kdl");
    let desktop = directory.join("desktop.kdl");
    std::fs::write(&core, "schema 2\n").unwrap();
    std::fs::write(
        &desktop,
        "schema 1\nshell { enabled #false; }\nsession { startup \"terminal\" \"panel\"; }\n",
    )
    .unwrap();
    for path in [&core, &desktop] {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let mut args = vec![
        format!("--config={}", core.display()),
        format!("--desktop-profile={}", desktop.display()),
        "--no-input".to_owned(),
        "--session-mode=normal".to_owned(),
        "--session-app=terminal=/usr/bin/xterm".to_owned(),
        "--session-app=panel=/usr/bin/true".to_owned(),
        "--session-action-app=terminal=terminal".to_owned(),
        "--session-start-default=terminal".to_owned(),
    ];
    let read = |args: &[String]| {
        PersistentXtermSessionConfig::from_args(args)
            .unwrap()
            .applications
            .startup
    };
    assert_eq!(read(&args), ["terminal", "panel"]);
    args.push("--session-start=panel".to_owned());
    assert_eq!(read(&args), ["panel"]);
    args.pop();
    std::fs::write(&desktop, "schema 1\nshell { enabled #false; }\n").unwrap();
    assert_eq!(read(&args), ["terminal"]);
    std::fs::write(&core, "schema 2\nsession { application \"panel\" id=4 executable=\"/usr/bin/true\" {}; startup 4; }\n").unwrap();
    args.retain(|arg| arg != "--session-app=panel=/usr/bin/true");
    assert_eq!(read(&args), ["panel"]);
    std::fs::write(
        &desktop,
        "schema 1\nshell { enabled #false; }\nsession { startup \"missing\"; }\n",
    )
    .unwrap();
    assert!(PersistentXtermSessionConfig::from_args(&args).is_err());
    std::fs::write(
        &desktop,
        "schema 1\nshell { enabled #false; }\nsession { startup \"terminal\" \"xterm\"; }\n",
    )
    .unwrap();
    assert!(PersistentXtermSessionConfig::from_args(&args).is_err());
    std::fs::remove_dir_all(directory).unwrap();
}
