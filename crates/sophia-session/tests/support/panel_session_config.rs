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
