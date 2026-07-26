use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use sophia_config::{
    ConfigDomain, ConfigIoError, ConfigSourceClass, discover_config_source, read_config_file,
};

fn temporary_directory(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "sophia-config-{name}-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    if path.exists() {
        fs::remove_dir_all(&path).expect("remove stale test directory");
    }
    fs::create_dir_all(&path).expect("create test directory");
    path
}

#[test]
fn explicit_source_wins_without_probing_it() {
    let explicit = Path::new("/does/not/have/to/exist/config.kdl");
    let source = discover_config_source(ConfigDomain::Core, Some(explicit), None);

    assert_eq!(source.class, ConfigSourceClass::Explicit);
    assert_eq!(source.path.as_deref(), Some(explicit));
}

#[test]
fn discovers_xdg_user_source() {
    let root = temporary_directory("discovery");
    let directory = root.join("sophia");
    fs::create_dir(&directory).expect("create Sophia config directory");
    let path = directory.join("wm.kdl");
    fs::write(&path, "schema 2\n").expect("write config");

    let source = discover_config_source(ConfigDomain::Wm, None, Some(&root));

    assert_eq!(source.class, ConfigSourceClass::User);
    assert_eq!(source.path.as_deref(), Some(path.as_path()));
    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn validates_regular_file_mode_and_absolute_path() {
    let root = temporary_directory("mode");
    let path = root.join("config.kdl");
    fs::write(&path, "schema 2\n").expect("write config");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("set safe mode");
    assert_eq!(
        read_config_file(&path).expect("read safe config"),
        b"schema 2\n"
    );

    fs::set_permissions(&path, fs::Permissions::from_mode(0o622)).expect("set unsafe mode");
    assert_eq!(read_config_file(&path), Err(ConfigIoError::UnsafeMode));
    assert_eq!(
        read_config_file(Path::new("relative.kdl")),
        Err(ConfigIoError::InvalidPath)
    );
    fs::remove_dir_all(root).expect("remove test directory");
}
