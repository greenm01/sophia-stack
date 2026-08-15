use sophia_backend_live::{SysfsDrmKmsOutputBackend, discover_native_connector_records};
use sophia_engine::{DrmKmsMode, HeadlessOutput, OutputDiscoveryBackend, RenderHeadId};
use sophia_protocol::{OutputId, Size};
use std::fs;
use std::path::{Path, PathBuf};

fn drm_sysfs_fixture(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "sophia-live-drm-sysfs-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn write_fixture_file(root: &Path, name: &str, contents: &str) {
    fs::write(root.join(name), contents).unwrap();
}

#[test]
fn sysfs_discovery_finds_connected_connector_records() {
    let root = drm_sysfs_fixture("connected");
    let connector = root.join("card0-HDMI-A-1");
    fs::create_dir_all(&connector).unwrap();
    write_fixture_file(&connector, "status", "connected\n");
    write_fixture_file(&connector, "modes", "1920x1080\n1280x720\n");
    write_fixture_file(&connector, "connector_id", "42\n");
    write_fixture_file(&connector, "crtc_id", "99\n");
    write_fixture_file(&connector, "scale", "2\n");

    let records = discover_native_connector_records(&root).unwrap();

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].connector_name, "card0-HDMI-A-1");
    assert_eq!(records[0].connector_id, 42);
    assert_eq!(records[0].crtc_id, 99);
    assert_eq!(records[0].mode, DrmKmsMode::new(1920, 1080, 60_000));
    assert_eq!(records[0].scale, 2);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sysfs_discovery_ignores_disconnected_or_modeless_connectors() {
    let root = drm_sysfs_fixture("filtered");
    let disconnected = root.join("card0-DP-1");
    let modeless = root.join("card0-HDMI-A-1");
    let connected = root.join("card0-eDP-1");
    fs::create_dir_all(&disconnected).unwrap();
    fs::create_dir_all(&modeless).unwrap();
    fs::create_dir_all(&connected).unwrap();
    write_fixture_file(&disconnected, "status", "disconnected\n");
    write_fixture_file(&disconnected, "modes", "3840x2160\n");
    write_fixture_file(&modeless, "status", "connected\n");
    write_fixture_file(&modeless, "modes", "\n");
    write_fixture_file(&connected, "status", "connected\n");
    write_fixture_file(&connected, "modes", "2560x1440\n");

    let records = discover_native_connector_records(&root).unwrap();

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].connector_name, "card0-eDP-1");
    assert_eq!(records[0].crtc_id, 0);
    assert_eq!(records[0].mode, DrmKmsMode::new(2560, 1440, 60_000));
    assert_eq!(records[0].scale, 1);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sysfs_output_backend_mints_one_head_per_connected_connector() {
    let root = drm_sysfs_fixture("backend");
    for (name, mode) in [("card0-DP-1", "1920x1080\n"), ("card0-DP-2", "2560x1440\n")] {
        let connector = root.join(name);
        fs::create_dir_all(&connector).unwrap();
        write_fixture_file(&connector, "status", "connected\n");
        write_fixture_file(&connector, "modes", mode);
    }

    let registry = SysfsDrmKmsOutputBackend::new(&root)
        .discover_outputs()
        .unwrap();

    // One logical output and one minted head per connector; the registry
    // carries no connector or CRTC identity.
    assert_eq!(registry.output_count(), 2);
    assert_eq!(registry.head_count(), 2);
    let heads: Vec<_> = registry.heads().collect();
    assert!(heads.iter().all(|target| target.head.is_valid()));
    assert_ne!(heads[0].head, heads[1].head);
    assert_eq!(
        registry.logical_output(OutputId::from_raw(1)),
        Some(HeadlessOutput {
            id: OutputId::from_raw(1),
            size: Size {
                width: 1920,
                height: 1080,
            },
            scale: 1,
        })
    );
    assert!(registry.head(RenderHeadId::INVALID).is_none());

    fs::remove_dir_all(root).unwrap();
}
