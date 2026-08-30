//! Live compositor backend boundary.
//!
//! This crate is where real kernel-facing dependencies belong. The current
//! implementation deliberately stays on deterministic engine traits: sysfs-style
//! DRM/KMS discovery and static input descriptors. Future libdrm/libinput code
//! can replace these adapters without changing Sophia Engine, WM IPC, or
//! protocol authority packets.
//!
//! Keep this file as the crate boundary. Backend code belongs in domain modules:
//! input capture, DRM/KMS discovery, scanout, runtime assembly, session loop,
//! startup probing, and hardware validation.

mod api;
mod dependency;
mod direct_scanout_cost;
mod drm;
mod hardware_validation;
mod input;
mod prelude;
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
mod presentation;
mod production_cpu_cycle;
mod production_intake;
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
mod production_output_runtime;
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
mod production_present_scheduler;
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
mod production_renderer_image_handoff;
mod production_session;
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
mod production_visual_runtime;
mod runtime;
mod scanout;
#[cfg(feature = "seat-control")]
mod seat;
mod session_loop;
mod startup;

pub use api::*;
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
pub use direct_scanout_cost::*;
pub use drm::{
    LiveDrmSysfsDiscovery, LiveDrmSysfsDiscoveryConfig, LiveSysfsConnectorRecord,
    SysfsDrmKmsOutputBackend, discover_native_connector_records,
};
#[cfg(feature = "drm-hotplug")]
pub use drm::{LiveDrmTopologyMonitor, LiveDrmTopologyMonitorStats, LiveDrmTopologyRescanNotice};
pub use presentation::*;
pub use production_cpu_cycle::*;
pub use production_intake::*;
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
pub use production_output_runtime::*;
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
pub use production_present_scheduler::*;
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
pub use production_renderer_image_handoff::*;
pub use production_session::*;
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
pub use production_visual_runtime::*;
#[cfg(feature = "seat-control")]
pub use seat::*;
pub use sophia_renderer_live::LivePresentationDisconnectReport;
#[cfg(feature = "gbm-probe")]
pub use sophia_renderer_live::allocate_shared_buffer;
pub use sophia_renderer_live::{LiveSharedBufferAllocation, LiveSharedBufferError};
