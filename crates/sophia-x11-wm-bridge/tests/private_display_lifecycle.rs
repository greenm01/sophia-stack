use std::{env, io::Write, os::unix::net::UnixStream, path::PathBuf, thread, time::Duration};

use sophia_protocol::Rect;
use sophia_x11_wm_bridge::{LegacyWmLaunchSpec, LegacyX11WmBridgeRuntime};

#[test]
fn private_display_fixture_process() {
    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        return;
    };
    let is_bridge_child = home
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("sophia-x11-wm-bridge-"));
    if !is_bridge_child {
        return;
    }

    let display = env::var("DISPLAY")
        .unwrap()
        .strip_prefix(':')
        .unwrap()
        .parse::<u16>()
        .unwrap();
    let path = format!("/tmp/.X11-unix/X{display}");
    let mut stream = UnixStream::connect(path).unwrap();
    stream
        .write_all(&[b'l', 0, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0])
        .unwrap();
    thread::sleep(Duration::from_secs(30));
}

#[test]
fn accepted_private_displays_are_unlinked_and_remain_exclusively_leased() {
    let executable = env::current_exe().unwrap();
    let launch = || {
        LegacyWmLaunchSpec::new(&executable)
            .arg("--exact")
            .arg("private_display_fixture_process")
            .arg("--nocapture")
    };
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 1280,
        height: 720,
    };

    let first = LegacyX11WmBridgeRuntime::start_with_root(launch(), bounds).unwrap();
    let first_display = first.private_display();
    assert!(!PathBuf::from(format!("/tmp/.X11-unix/X{first_display}")).exists());

    let second = LegacyX11WmBridgeRuntime::start_with_root(launch(), bounds).unwrap();
    let second_display = second.private_display();
    assert_ne!(first_display, second_display);
    assert!(!PathBuf::from(format!("/tmp/.X11-unix/X{second_display}")).exists());
}
