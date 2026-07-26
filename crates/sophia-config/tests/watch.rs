use std::fs;
use std::sync::mpsc::TryRecvError;
use std::thread;
use std::time::{Duration, Instant};

use sophia_config::{ConfigWatchEvent, ConfigWatcher};

#[test]
fn watches_parent_directory_across_atomic_replacement() {
    let root = std::env::temp_dir().join(format!("sophia-config-watch-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("remove stale watch test directory");
    }
    fs::create_dir(&root).expect("create watch test directory");
    let path = root.join("config.kdl");
    fs::write(&path, "schema 2\n").expect("write initial config");
    let watcher = ConfigWatcher::spawn(&path).expect("start watcher");

    let replacement = root.join(".config.kdl.next");
    fs::write(&replacement, "schema 2\ndiagnostics verbose=#true\n").expect("write replacement");
    fs::rename(&replacement, &path).expect("atomically replace config");

    let deadline = Instant::now() + Duration::from_secs(3);
    let event = loop {
        match watcher.try_recv() {
            Ok(event) => break event,
            Err(TryRecvError::Empty) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => panic!("watcher failed before event: {error}"),
        }
    };

    assert!(matches!(
        event,
        ConfigWatchEvent::Changed | ConfigWatchEvent::Overflow
    ));
    drop(watcher);
    fs::remove_dir_all(root).expect("remove watch test directory");
}
