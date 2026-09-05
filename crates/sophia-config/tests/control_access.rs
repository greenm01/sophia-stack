use sophia_config::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn control_access_is_explicit_and_strict() {
    let root = std::env::temp_dir().join(format!("sc-config-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = root.join("desktop.kdl");
    for (mode, expected) in [
        ("", Some(DesktopControlAccess::Disabled)),
        (
            "control \"disabled\";",
            Some(DesktopControlAccess::Disabled),
        ),
        (
            "control \"host-admin\";",
            Some(DesktopControlAccess::HostAdmin),
        ),
        ("control #true;", None),
        ("control \"host\";", None),
    ] {
        std::fs::write(&path, format!("schema 1\nsession {{ {mode} }}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let result = load_prepared_desktop_profile(Some(&path), ConfigGeneration::from_raw(1));
        if let Some(expected) = expected {
            let profile = result.unwrap().profile;
            let candidate =
                prepare_desktop_session_candidate(&profile.candidates[&DesktopAuthority::Session])
                    .unwrap();
            assert_eq!(candidate.control, expected);
        } else {
            assert!(result.is_err());
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}
