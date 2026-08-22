//! Shared runtime conventions for Sophia processes.
//!
//! Libraries emit structured diagnostics through `tracing`; binaries decide
//! when and how to install a subscriber.

#[cfg(target_os = "linux")]
mod broker_transport;
mod error;
mod output_ipc;
#[cfg(target_os = "linux")]
mod output_service;
#[cfg(target_os = "linux")]
mod output_transport;
mod policy_ipc;
mod policy_profile_handoff;
#[cfg(target_os = "linux")]
mod policy_socket;
#[cfg(target_os = "linux")]
mod policy_transport;
mod session;
mod supervisor;
mod tracing;

mod prelude {
    pub(crate) use core::fmt;
    pub(crate) use std::ffi::OsString;
    pub(crate) use std::process::{Child, Command};
    pub(crate) use std::time::Duration;

    pub(crate) use sophia_protocol::{
        BrokerHealthState, BrokerKind, SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN, TransactionOutcome,
    };
    pub(crate) use tracing_subscriber::EnvFilter;

    pub(crate) use crate::SupervisedProcessKind;
    pub(crate) use crate::{SophiaErrorExt, SophiaErrorKind};
}

#[cfg(target_os = "linux")]
pub use broker_transport::*;
pub use error::*;
pub use output_ipc::*;
#[cfg(target_os = "linux")]
pub use output_service::*;
#[cfg(target_os = "linux")]
pub use output_transport::*;
pub use policy_ipc::*;
pub use policy_profile_handoff::*;
#[cfg(target_os = "linux")]
pub use policy_socket::*;
#[cfg(target_os = "linux")]
pub use policy_transport::*;
pub use session::*;
pub use supervisor::*;
pub use tracing::*;
