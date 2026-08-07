use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

pub const SOPHIA_WM_SOCKET_ENV: &str = "SOPHIA_WM_SOCKET";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyPeerIdentity {
    pub uid: u32,
    pub pid: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyRoleEndpointError {
    PathAlreadyExists,
    Io(String),
    UnauthorizedPeer {
        expected: PolicyPeerIdentity,
        actual: PolicyPeerIdentity,
    },
    PeerAlreadyActive,
    WrongReleasedPeer,
}

impl core::fmt::Display for PolicyRoleEndpointError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for PolicyRoleEndpointError {}

/// Owner-only, session-hosted listener for one exclusive policy role.
pub struct PolicyRoleEndpoint {
    directory: PathBuf,
    socket_path: PathBuf,
    listener: UnixListener,
    expected_peer: PolicyPeerIdentity,
    active_peer: Option<PolicyPeerIdentity>,
}

impl PolicyRoleEndpoint {
    pub fn bind(
        directory: impl AsRef<Path>,
        expected_peer: PolicyPeerIdentity,
    ) -> Result<Self, PolicyRoleEndpointError> {
        let directory = directory.as_ref().to_path_buf();
        match fs::create_dir(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                return Err(PolicyRoleEndpointError::PathAlreadyExists);
            }
            Err(error) => return Err(PolicyRoleEndpointError::Io(error.to_string())),
        }
        if let Err(error) = fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)) {
            let _ = fs::remove_dir(&directory);
            return Err(PolicyRoleEndpointError::Io(error.to_string()));
        }
        let socket_path = directory.join("wm.sock");
        let listener = match UnixListener::bind(&socket_path) {
            Ok(listener) => listener,
            Err(error) => {
                let _ = fs::remove_dir(&directory);
                return Err(PolicyRoleEndpointError::Io(error.to_string()));
            }
        };
        if let Err(error) = fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600)) {
            let _ = fs::remove_file(&socket_path);
            let _ = fs::remove_dir(&directory);
            return Err(PolicyRoleEndpointError::Io(error.to_string()));
        }
        Ok(Self {
            directory,
            socket_path,
            listener,
            expected_peer,
            active_peer: None,
        })
    }

    pub fn accept_expected(&mut self) -> Result<UnixStream, PolicyRoleEndpointError> {
        if self.active_peer.is_some() {
            return Err(PolicyRoleEndpointError::PeerAlreadyActive);
        }
        let (stream, _) = self
            .listener
            .accept()
            .map_err(|error| PolicyRoleEndpointError::Io(error.to_string()))?;
        let credentials = rustix::net::sockopt::socket_peercred(&stream)
            .map_err(|error| PolicyRoleEndpointError::Io(error.to_string()))?;
        let actual = PolicyPeerIdentity {
            uid: credentials.uid.as_raw(),
            pid: credentials.pid.as_raw_pid() as u32,
        };
        if actual != self.expected_peer {
            return Err(PolicyRoleEndpointError::UnauthorizedPeer {
                expected: self.expected_peer,
                actual,
            });
        }
        self.active_peer = Some(actual);
        Ok(stream)
    }

    pub fn release_peer(
        &mut self,
        peer: PolicyPeerIdentity,
    ) -> Result<(), PolicyRoleEndpointError> {
        if self.active_peer != Some(peer) {
            return Err(PolicyRoleEndpointError::WrongReleasedPeer);
        }
        self.active_peer = None;
        Ok(())
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub const fn active_peer(&self) -> Option<PolicyPeerIdentity> {
        self.active_peer
    }
}

impl Drop for PolicyRoleEndpoint {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.socket_path);
        let _ = fs::remove_dir(&self.directory);
    }
}
