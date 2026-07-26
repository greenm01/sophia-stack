use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use sophia_engine::{WmSocketIncoming, WmSocketTransport, WmSocketTransportConfig};
use sophia_protocol::{
    OutputId, TransactionId, WM_API_VERSION, WmActionActivation, WmActionId, WmOutputWorkspace,
    WmPolicyAck, WmPolicyAckOutcome, WmRequestKind, WmRequestPacket, WmSessionDescriptor,
    WorkspaceId,
};

#[test]
fn socket_server_emits_and_acknowledges_hot_reloaded_policy() {
    let directory = unique_test_directory();
    std::fs::create_dir(&directory).unwrap();
    let socket = directory.join("wm.sock");
    let config = directory.join("wm.kdl");
    write_private_config(&config, wm_config(2));
    let child = Command::new(sophia_wm_demo_binary())
        .arg("serve-socket")
        .arg(format!("--socket={}", socket.display()))
        .arg(format!("--wm-config={}", config.display()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let mut guard = ServerGuard { child, directory };
    wait_for_socket(&socket, &mut guard.child);

    let stream = UnixStream::connect(&socket).unwrap();
    let mut transport = WmSocketTransport::new(
        stream,
        WmSocketTransportConfig {
            response_timeout: Duration::from_millis(500),
        },
    );
    let workspace = WorkspaceId::from_raw(1);
    let registry = transport
        .negotiate(&WmSessionDescriptor {
            api_version: WM_API_VERSION,
            workspaces: vec![workspace],
            active_workspaces: vec![WmOutputWorkspace {
                output: sophia_protocol::OutputId::from_raw(1),
                workspace,
            }],
            session_actions: Vec::new(),
        })
        .unwrap();
    assert_eq!(registry.policy_generation(), 1);
    assert_eq!(registry.chrome().thickness, 2);

    let candidate = guard.directory.join("wm.next");
    write_private_config(&candidate, wm_config(5));
    std::fs::rename(candidate, &config).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let update = loop {
        if let Some(update) = transport
            .poll_policy_update(Duration::from_millis(50))
            .unwrap()
        {
            break update;
        }
        assert!(
            Instant::now() < deadline,
            "WM server did not emit a policy update"
        );
    };

    assert_eq!(update.generation, 2);
    assert_eq!(update.chrome.thickness, 5);
    let request = WmRequestPacket {
        transaction: TransactionId::from_raw(7),
        kind: WmRequestKind::ActionActivated(WmActionActivation {
            action: WmActionId::from_raw(1),
            output: OutputId::from_raw(1),
            workspace,
            focused_surface: None,
            nodes: Vec::new(),
        }),
    };
    transport.send_request(&request).unwrap();
    transport
        .acknowledge_policy_update(WmPolicyAck {
            generation: update.generation,
            outcome: WmPolicyAckOutcome::Applied,
        })
        .unwrap();
    assert!(matches!(
        transport
            .poll_incoming(Duration::from_millis(500))
            .unwrap(),
        Some(WmSocketIncoming::Response(response))
            if response.transaction == request.transaction
    ));
}

fn wm_config(chrome_thickness: u16) -> String {
    format!(
        r##"/- kdl-version 2
schema 1

policy timeout-ms=300
workspace 1
layout "columns"
action "focus-next" id=1 behavior="focus-next"
binding action=1 keycode=57 modifiers="super"
chrome enabled=#true thickness={chrome_thickness} color="#70b7ff"
"##
    )
}

fn write_private_config(path: &Path, contents: String) {
    std::fs::write(path, contents).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

fn wait_for_socket(path: &Path, child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if path.exists() {
            return;
        }
        assert!(
            child.try_wait().unwrap().is_none(),
            "WM server exited before creating its socket"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("WM server did not create its socket");
}

fn unique_test_directory() -> PathBuf {
    let nonce = SystemTime::UNIX_EPOCH.elapsed().unwrap().as_nanos();
    std::env::temp_dir().join(format!(
        "sophia-wm-policy-reload-{}-{nonce}",
        std::process::id()
    ))
}

fn sophia_wm_demo_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_sophia-wm-demo") {
        return path.into();
    }
    std::env::current_exe()
        .expect("integration test executable path should be available")
        .parent()
        .and_then(Path::parent)
        .expect("integration test executable should live under target/debug/deps")
        .join("sophia-wm-demo")
}

struct ServerGuard {
    child: Child,
    directory: PathBuf,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
