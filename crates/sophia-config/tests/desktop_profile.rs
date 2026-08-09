use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sophia_config::{
    ConfigGeneration, ConfigIoError, DESKTOP_PROFILE_MAX_BYTES, DesktopAuthority,
    DesktopProfileError, discover_desktop_profile_source, load_desktop_profile,
    stage_desktop_profile,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

fn temporary_directory(name: &str) -> PathBuf {
    let serial = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "sophia-desktop-profile-{name}-{}-{serial}",
        std::process::id()
    ));
    fs::create_dir(&path).expect("create test directory");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
        .expect("make test directory private");
    path
}

fn write_profile(path: &Path, source: &str) {
    fs::write(path, source).expect("write profile");
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("make profile private");
}

#[test]
fn compiled_profile_partitions_every_authority_deterministically() {
    let first = load_desktop_profile(None, ConfigGeneration::INITIAL).unwrap();
    let second = load_desktop_profile(None, ConfigGeneration::INITIAL).unwrap();

    assert_eq!(first.digest, second.digest);
    assert_eq!(first.sources, vec![PathBuf::from("<compiled>")]);
    assert_eq!(first.candidates.len(), DesktopAuthority::ALL.len());
    for authority in DesktopAuthority::ALL {
        let candidate = first.candidates.get(&authority).unwrap();
        assert_eq!(candidate.authority, authority);
        assert_eq!(candidate.generation, first.generation);
        assert_eq!(candidate.digest, first.digest);
        assert!(
            candidate
                .values
                .iter()
                .all(|value| value.key.starts_with(authority.name()))
        );
    }
}

#[test]
fn includes_expand_in_place_with_per_value_provenance() {
    let root = temporary_directory("include");
    let main = root.join("config.kdl");
    let policy = root.join("policy.kdl");
    write_profile(
        &main,
        "schema 1\ninclude \"policy.kdl\"\nsession { terminal \"foot\"; }\n",
    );
    write_profile(
        &policy,
        "policy { layout \"scroller\"; view-count 9; outer-gap 4; inner-gap 2; }\n",
    );

    let profile = load_desktop_profile(Some(&main), ConfigGeneration::INITIAL).unwrap();
    assert_eq!(profile.sources, vec![main.clone(), policy.clone()]);
    let policy_candidate = profile.candidates.get(&DesktopAuthority::Policy).unwrap();
    assert_eq!(policy_candidate.values.len(), 4);
    assert!(
        policy_candidate
            .values
            .iter()
            .all(|value| value.provenance.path == policy)
    );
    assert_eq!(
        profile
            .candidates
            .get(&DesktopAuthority::Session)
            .unwrap()
            .values[0]
            .provenance
            .path,
        main
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_cycles_duplicates_unsupported_and_reserved_controls() {
    let root = temporary_directory("rejections");
    let main = root.join("config.kdl");
    let child = root.join("child.kdl");
    write_profile(&main, "schema 1\ninclude \"child.kdl\"\n");
    write_profile(&child, "include \"config.kdl\"\n");
    assert!(matches!(
        load_desktop_profile(Some(&main), ConfigGeneration::INITIAL),
        Err(DesktopProfileError::Schema(message)) if message.contains("cycle")
    ));

    for source in [
        "schema 1\npolicy { outer-gap 1; outer-gap 2; }\n",
        "schema 1\npolicy { magic 1; }\n",
        "schema 1\npolicy { max-surfaces 2; }\n",
        "schema 1\nshell { enabled #true; }\n",
        "schema 1\npolicy { view-count 10; }\n",
        "schema 1\npolicy { outer-gap -1; }\n",
        "schema 1\npolicy { inner-gap \"wide\"; }\n",
    ] {
        write_profile(&main, source);
        assert!(matches!(
            load_desktop_profile(Some(&main), ConfigGeneration::INITIAL),
            Err(DesktopProfileError::Schema(_))
        ));
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_unsafe_sources_symlinks_depth_and_aggregate_size() {
    let root = temporary_directory("safety");
    let main = root.join("config.kdl");
    write_profile(&main, "schema 1\npolicy { layout \"scroller\"; }\n");
    fs::set_permissions(&main, fs::Permissions::from_mode(0o622)).unwrap();
    assert_eq!(
        load_desktop_profile(Some(&main), ConfigGeneration::INITIAL),
        Err(DesktopProfileError::Io(ConfigIoError::UnsafeMode))
    );
    fs::set_permissions(&main, fs::Permissions::from_mode(0o600)).unwrap();
    let link = root.join("linked.kdl");
    symlink(&main, &link).unwrap();
    assert_eq!(
        load_desktop_profile(Some(&link), ConfigGeneration::INITIAL),
        Err(DesktopProfileError::Io(ConfigIoError::NotRegularFile))
    );

    for index in 0..=11 {
        let next = if index == 11 {
            "policy { layout \"scroller\"; }\n".to_owned()
        } else {
            format!("include \"depth-{}.kdl\"\n", index + 1)
        };
        write_profile(&root.join(format!("depth-{index}.kdl")), &next);
    }
    assert!(matches!(
        load_desktop_profile(
            Some(&root.join("depth-0.kdl")),
            ConfigGeneration::INITIAL
        ),
        Err(DesktopProfileError::Limit(message)) if message.contains("depth")
    ));

    let large = root.join("large.kdl");
    let mut large_source = "policy { layout \"scroller\"; }\n".to_owned();
    large_source.push_str(&" ".repeat(DESKTOP_PROFILE_MAX_BYTES - large_source.len()));
    write_profile(&large, &large_source);
    write_profile(&main, "schema 1\ninclude \"large.kdl\"\n");
    assert!(matches!(
        load_desktop_profile(Some(&main), ConfigGeneration::INITIAL),
        Err(DesktopProfileError::Limit(message)) if message.contains("aggregate")
    ));

    let mut includes = "schema 1\n".to_owned();
    for index in 0..64 {
        let child = root.join(format!("part-{index}.kdl"));
        write_profile(&child, "session {}\n");
        includes.push_str(&format!("include \"part-{index}.kdl\"\n"));
    }
    write_profile(&main, &includes);
    assert!(matches!(
        load_desktop_profile(Some(&main), ConfigGeneration::INITIAL),
        Err(DesktopProfileError::Limit(message)) if message.contains("64 files")
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stages_isolated_owner_only_authority_fragments() {
    let root = temporary_directory("stage");
    let profile = load_desktop_profile(None, ConfigGeneration::INITIAL).unwrap();
    let fragments = stage_desktop_profile(&profile, &root).unwrap();

    for authority in DesktopAuthority::ALL {
        let path = fragments.path(authority);
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let source = fs::read_to_string(path).unwrap();
        assert!(source.contains(&format!("profile-generation {}", profile.generation.raw())));
        assert!(source.contains(&format!("profile-digest \"{}\"", profile.digest)));
        for other in DesktopAuthority::ALL {
            if other != authority {
                assert!(
                    !source
                        .lines()
                        .any(|line| line == format!("{} {{", other.name()))
                );
            }
        }
    }
    drop(fragments);
    assert!(fs::read_dir(&root).unwrap().next().is_none());
    fs::remove_dir(root).unwrap();
}

#[test]
fn desktop_profile_discovery_prefers_explicit_then_xdg() {
    let root = temporary_directory("discovery");
    let user = root.join("hagia/config.kdl");
    fs::create_dir(user.parent().unwrap()).unwrap();
    write_profile(&user, "schema 1\n");
    let explicit = Path::new("/explicit/profile.kdl");

    assert_eq!(
        discover_desktop_profile_source(Some(explicit), Some(&root)).as_deref(),
        Some(explicit)
    );
    assert_eq!(
        discover_desktop_profile_source(None, Some(&root)).as_deref(),
        Some(user.as_path())
    );
    fs::remove_dir_all(root).unwrap();
}
