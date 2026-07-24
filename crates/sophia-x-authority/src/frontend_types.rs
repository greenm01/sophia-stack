use std::os::fd::OwnedFd;

use sophia_protocol::{ClientAdmissionContext, ClientAuthenticationMethod};

/// Monotonically assigned identity for one live X11 client connection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct XServerFrontendClientId(pub(crate) u64);

impl XServerFrontendClientId {
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Eq, PartialEq, Default)]
pub enum XServerFrontendSetupAuthorization {
    #[default]
    UnauthenticatedLocal,
    MitMagicCookie([u8; 16]),
}

impl core::fmt::Debug for XServerFrontendSetupAuthorization {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnauthenticatedLocal => formatter.write_str("UnauthenticatedLocal"),
            Self::MitMagicCookie(_) => formatter.write_str("MitMagicCookie([redacted])"),
        }
    }
}

impl XServerFrontendSetupAuthorization {
    pub(crate) fn permits(&self, request: &crate::XSetupRequest) -> bool {
        match self {
            Self::UnauthenticatedLocal => true,
            Self::MitMagicCookie(expected) => {
                request.authorization_protocol_name == b"MIT-MAGIC-COOKIE-1"
                    && authorization_data_eq(&request.authorization_data, expected)
            }
        }
    }

    pub(crate) const fn authentication_method(&self) -> ClientAuthenticationMethod {
        match self {
            Self::UnauthenticatedLocal => ClientAuthenticationMethod::TrustedLocal,
            Self::MitMagicCookie(_) => ClientAuthenticationMethod::MitMagicCookie1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct XServerFrontendPeerCredentials {
    pub process_id: u32,
    pub user_id: u32,
    pub group_id: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XServerFrontendAdmissionRequest {
    pub peer_credentials: Option<XServerFrontendPeerCredentials>,
    pub setup_authentication: ClientAuthenticationMethod,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XServerFrontendAdmissionError {
    Denied,
    Unavailable,
}

impl core::fmt::Display for XServerFrontendAdmissionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Denied => formatter.write_str("X11 client admission denied"),
            Self::Unavailable => formatter.write_str("X11 client admission unavailable"),
        }
    }
}

impl std::error::Error for XServerFrontendAdmissionError {}

pub trait XServerFrontendAdmissionPolicy: Send + Sync + 'static {
    fn admit(
        &self,
        request: XServerFrontendAdmissionRequest,
    ) -> Result<ClientAdmissionContext, XServerFrontendAdmissionError>;

    fn revoke(&self, context: ClientAdmissionContext) -> Result<(), XServerFrontendAdmissionError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XServerFrontendRenderDeviceError {
    Unavailable,
    OpenFailed,
}

impl core::fmt::Display for XServerFrontendRenderDeviceError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("X11 render device is unavailable"),
            Self::OpenFailed => formatter.write_str("X11 render device open failed"),
        }
    }
}

impl std::error::Error for XServerFrontendRenderDeviceError {}

pub trait XServerFrontendRenderDeviceProvider: Send + Sync + 'static {
    fn open_render_device_fd(&self) -> Result<OwnedFd, XServerFrontendRenderDeviceError>;
}

fn authorization_data_eq(actual: &[u8], expected: &[u8]) -> bool {
    if actual.len() != expected.len() {
        return false;
    }
    actual
        .iter()
        .zip(expected)
        .fold(0u8, |difference, (actual, expected)| {
            difference | (actual ^ expected)
        })
        == 0
}
