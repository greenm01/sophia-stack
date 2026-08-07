#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};

use sophia_runtime::{
    PolicyPeerIdentity, PolicyRoleEndpoint, PolicyRoleEndpointError, SOPHIA_WM_SOCKET_ENV,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

#[test]
fn endpoint_is_owner_only_and_admits_only_the_supervised_peer() {
    let directory = unique_directory("admission");
    let peer = current_peer();
    let mut endpoint = PolicyRoleEndpoint::bind(&directory, peer).unwrap();
    assert_eq!(SOPHIA_WM_SOCKET_ENV, "SOPHIA_WM_SOCKET");
    assert_eq!(
        fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(endpoint.socket_path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );

    let client = UnixStream::connect(endpoint.socket_path()).unwrap();
    let accepted = endpoint.accept_expected().unwrap();
    assert_eq!(endpoint.active_peer(), Some(peer));
    assert_eq!(
        endpoint.accept_expected().unwrap_err(),
        PolicyRoleEndpointError::PeerAlreadyActive
    );
    drop((client, accepted));
    endpoint.release_peer(peer).unwrap();
    drop(endpoint);
    assert!(!directory.exists());
}

#[test]
fn credential_mismatch_fails_without_claiming_the_role() {
    let directory = unique_directory("credentials");
    let mut expected = current_peer();
    expected.pid = expected.pid.saturating_add(1);
    let mut endpoint = PolicyRoleEndpoint::bind(&directory, expected).unwrap();
    let _client = UnixStream::connect(endpoint.socket_path()).unwrap();

    assert!(matches!(
        endpoint.accept_expected(),
        Err(PolicyRoleEndpointError::UnauthorizedPeer { expected: denied, actual })
            if denied == expected && actual == current_peer()
    ));
    assert_eq!(endpoint.active_peer(), None);
}

#[test]
fn endpoint_never_reuses_an_existing_directory() {
    let directory = unique_directory("existing");
    fs::create_dir(&directory).unwrap();
    assert!(matches!(
        PolicyRoleEndpoint::bind(&directory, current_peer()),
        Err(PolicyRoleEndpointError::PathAlreadyExists)
    ));
    fs::remove_dir(directory).unwrap();
}

fn current_peer() -> PolicyPeerIdentity {
    PolicyPeerIdentity {
        uid: rustix::process::geteuid().as_raw(),
        pid: std::process::id(),
    }
}

fn unique_directory(label: &str) -> std::path::PathBuf {
    let serial = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "sophia-policy-{label}-{}-{serial}",
        std::process::id()
    ))
}
