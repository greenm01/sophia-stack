use std::{
    ffi::c_void,
    os::fd::{AsRawFd, OwnedFd},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

mod import_cache;
mod types;

pub use import_cache::*;
pub use types::*;

use crate::gbm_platform::{
    EGL_PLATFORM_GBM_KHR,
    config::{window_config_attributes, xrgb_window_config_attributes},
};
use crate::gl::{
    GlCpuLayer, PersistentXrgb8888GlPipeline, context_attributes,
    draw_xrgb8888_current_gl_context_with_loader, smoke_current_gl_context_with_loader,
};
use crate::{
    NativeCompositionPixelMetrics, NativeGbmRenderedScanoutContextStatus,
    NativeGbmScanoutBufferExportDetail, NativeGbmScanoutBufferExportStatus,
};

include!("scanout/buffer.rs");
include!("scanout/context.rs");
include!("scanout/export.rs");
include!("scanout/render.rs");
include!("scanout/candidates.rs");
