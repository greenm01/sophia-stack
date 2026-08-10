use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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
    PeerNotAuthorized,
    AcceptTimedOut,
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
    expected_uid: u32,
    expected_pid: Option<u32>,
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
            expected_uid: expected_peer.uid,
            expected_pid: Some(expected_peer.pid),
            active_peer: None,
        })
    }

    pub fn bind_for_supervised_uid(
        directory: impl AsRef<Path>,
        expected_uid: u32,
    ) -> Result<Self, PolicyRoleEndpointError> {
        let placeholder = PolicyPeerIdentity {
            uid: expected_uid,
            pid: u32::MAX,
        };
        let mut endpoint = Self::bind(directory, placeholder)?;
        endpoint.expected_pid = None;
        Ok(endpoint)
    }

    pub fn authorize_supervised_pid(&mut self, pid: u32) -> Result<(), PolicyRoleEndpointError> {
        if pid == 0 || self.active_peer.is_some() {
            return Err(PolicyRoleEndpointError::PeerNotAuthorized);
        }
        self.expected_pid = Some(pid);
        Ok(())
    }

    pub fn accept_expected(&mut self) -> Result<UnixStream, PolicyRoleEndpointError> {
        let expected = self.expected_peer()?;
        let (stream, _) = self
            .listener
            .accept()
            .map_err(|error| PolicyRoleEndpointError::Io(error.to_string()))?;
        self.admit_expected_stream(stream, expected)
    }

    pub fn accept_expected_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<UnixStream, PolicyRoleEndpointError> {
        let expected = self.expected_peer()?;
        self.listener
            .set_nonblocking(true)
            .map_err(|error| PolicyRoleEndpointError::Io(error.to_string()))?;
        let deadline = Instant::now() + timeout;
        let accepted = loop {
            match self.listener.accept() {
                Ok((stream, _)) => break Ok(stream),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    let now = Instant::now();
                    if now >= deadline {
                        break Err(PolicyRoleEndpointError::AcceptTimedOut);
                    }
                    std::thread::sleep(
                        deadline
                            .saturating_duration_since(now)
                            .min(Duration::from_millis(2)),
                    );
                }
                Err(error) => break Err(PolicyRoleEndpointError::Io(error.to_string())),
            }
        };
        self.listener
            .set_nonblocking(false)
            .map_err(|error| PolicyRoleEndpointError::Io(error.to_string()))?;
        self.admit_expected_stream(accepted?, expected)
    }

    fn expected_peer(&self) -> Result<PolicyPeerIdentity, PolicyRoleEndpointError> {
        if self.active_peer.is_some() {
            return Err(PolicyRoleEndpointError::PeerAlreadyActive);
        }
        let pid = self
            .expected_pid
            .ok_or(PolicyRoleEndpointError::PeerNotAuthorized)?;
        Ok(PolicyPeerIdentity {
            uid: self.expected_uid,
            pid,
        })
    }

    fn admit_expected_stream(
        &mut self,
        stream: UnixStream,
        expected: PolicyPeerIdentity,
    ) -> Result<UnixStream, PolicyRoleEndpointError> {
        let credentials = rustix::net::sockopt::socket_peercred(&stream)
            .map_err(|error| PolicyRoleEndpointError::Io(error.to_string()))?;
        let actual = PolicyPeerIdentity {
            uid: credentials.uid.as_raw(),
            pid: credentials.pid.as_raw_pid() as u32,
        };
        if actual != expected {
            return Err(PolicyRoleEndpointError::UnauthorizedPeer { expected, actual });
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
