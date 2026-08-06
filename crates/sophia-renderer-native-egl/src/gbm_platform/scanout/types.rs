use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::time::Duration;

use super::import_cache::{NativeDmaBufImportCacheStats, NativeRendererImageId};
use crate::gl::GlCompositionRect;

#[derive(Clone, Copy, Debug)]
pub struct NativeDmaBufFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub modifier: u64,
    pub fd: BorrowedFd<'a>,
    pub offset: u32,
    pub stride: u32,
}

impl NativeDmaBufFrame<'_> {
    pub fn is_valid(&self) -> bool {
        const DRM_FORMAT_XRGB8888: u32 = 0x3432_5258;
        const DRM_FORMAT_ARGB8888: u32 = 0x3432_5241;
        self.width > 0
            && self.height > 0
            && matches!(self.format, DRM_FORMAT_XRGB8888 | DRM_FORMAT_ARGB8888)
            && self.stride >= self.width.saturating_mul(4)
            && matches!(self.modifier, 0 | u64::MAX)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NativeDmaBufPlane<'a> {
    pub fd: BorrowedFd<'a>,
    pub offset: u32,
    pub stride: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeMultiPlaneDmaBufFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub modifier: u64,
    pub plane_count: u8,
    pub planes: [Option<NativeDmaBufPlane<'a>>; 4],
}

#[derive(Debug)]
pub struct NativeRendererImageSnapshot {
    // Duplicated plane FDs keep pixels alive without retaining the old DRM
    // device, EGL display, or GBM context across a renderer generation change.
    pub(super) image_id: NativeRendererImageId,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) format: u32,
    pub(super) modifier: u64,
    pub(super) plane_count: u8,
    pub(super) planes: [Option<NativeOwnedDmaBufPlane>; 4],
}

#[derive(Debug)]
pub struct NativeOwnedDmaBufPlane {
    pub(super) fd: OwnedFd,
    pub(super) offset: u32,
    pub(super) stride: u32,
}

impl NativeRendererImageSnapshot {
    pub const fn image_id(&self) -> NativeRendererImageId {
        self.image_id
    }

    pub(super) fn as_frame(&self) -> NativeMultiPlaneDmaBufFrame<'_> {
        NativeMultiPlaneDmaBufFrame {
            width: self.width,
            height: self.height,
            format: self.format,
            modifier: self.modifier,
            plane_count: self.plane_count,
            planes: std::array::from_fn(|index| {
                self.planes[index].as_ref().map(|plane| NativeDmaBufPlane {
                    fd: plane.fd.as_fd(),
                    offset: plane.offset,
                    stride: plane.stride,
                })
            }),
        }
    }
}

impl NativeMultiPlaneDmaBufFrame<'_> {
    pub fn is_valid(&self) -> bool {
        const DRM_FORMAT_XRGB8888: u32 = 0x3432_5258;
        const DRM_FORMAT_ARGB8888: u32 = 0x3432_5241;
        self.width > 0
            && self.height > 0
            && matches!(self.format, DRM_FORMAT_XRGB8888 | DRM_FORMAT_ARGB8888)
            && self.plane_count > 0
            && usize::from(self.plane_count) <= self.planes.len()
            && self.planes[..usize::from(self.plane_count)]
                .iter()
                .all(Option::is_some)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeCompositionRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl From<NativeCompositionRect> for GlCompositionRect {
    fn from(rect: NativeCompositionRect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NativeCpuCompositionLayer<'a> {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: u32,
    pub pixels: &'a [u8],
    pub target: NativeCompositionRect,
    pub clip: Option<NativeCompositionRect>,
    pub alpha: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeDmaBufCompositionLayer<'a> {
    pub image_id: NativeRendererImageId,
    pub frame: NativeMultiPlaneDmaBufFrame<'a>,
    pub target: NativeCompositionRect,
    pub clip: Option<NativeCompositionRect>,
    pub alpha: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeRendererImageCompositionLayer {
    pub image_id: NativeRendererImageId,
    pub target: NativeCompositionRect,
    pub clip: Option<NativeCompositionRect>,
    pub alpha: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeSolidCompositionLayer {
    pub target: NativeCompositionRect,
    pub color: [u8; 3],
}

#[derive(Clone, Copy, Debug)]
pub enum NativeCompositionLayer<'a> {
    Cpu(NativeCpuCompositionLayer<'a>),
    DmaBuf(NativeDmaBufCompositionLayer<'a>),
    RendererImage(NativeRendererImageCompositionLayer),
    Solid(NativeSolidCompositionLayer),
}

#[derive(Clone, Copy, Debug)]
pub struct NativeCompositionFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub layers: &'a [NativeCompositionLayer<'a>],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeGbmPersistentRenderStats {
    pub target_creations: usize,
    pub target_recreations: usize,
    pub gl_pipeline_creations: usize,
    pub frame_surface_creations: usize,
    pub cpu_target_creations: usize,
    pub dmabuf_target_creations: usize,
    pub composition_target_creations: usize,
    pub composition_target_reuses: usize,
    pub generation_replacements: usize,
    pub recovery_replacements: usize,
    pub frame_uploads: usize,
    pub snapshot_captures: usize,
    pub snapshot_promotions: usize,
    pub snapshot_rollbacks: usize,
    pub snapshot_evictions: usize,
    pub snapshot_live_entries: usize,
    pub snapshot_live_bytes: u64,
    pub import_cache: NativeDmaBufImportCacheStats,
    pub max_target_create: Duration,
    pub max_frame_surface_create: Duration,
    pub max_render: Duration,
    pub max_upload: Duration,
}
