use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use crate::{
    LiveCpuBufferSourceRef, LiveGbmEglFrameTargetRecord, LiveRendererScanoutBufferDescriptor,
    LiveRendererScanoutBufferExportDetail, LiveRendererScanoutBufferExportStatus,
    LiveRendererScanoutBufferPlanes, Size,
};
use sophia_engine::{CompositorRgb8, HeadSamplingClass, RenderHeadId};
use sophia_protocol::OutputId;
use sophia_protocol::{DRM_FORMAT_ARGB8888, DRM_FORMAT_XRGB8888, Rect, Transform};

mod renderer_images;
pub use renderer_images::{LiveRendererImageId, LiveRendererImageSnapshot};

#[derive(Debug)]
pub struct NativeGbmOwnedScanoutBuffer {
    descriptor: LiveRendererScanoutBufferDescriptor,
    _buffer: sophia_renderer_native_egl::NativeGbmOwnedScanoutBuffer,
}

impl NativeGbmOwnedScanoutBuffer {
    pub const fn descriptor(&self) -> LiveRendererScanoutBufferDescriptor {
        self.descriptor
    }

    pub fn export_scanout_dma_buf_fds(&self) -> std::io::Result<NativeGbmScanoutBufferPlaneFds> {
        self._buffer
            .export_plane_fds()
            .map(NativeGbmScanoutBufferPlaneFds::new)
            .map_err(|_error| std::io::Error::other("GBM scanout DMA-BUF export failed"))
    }
}

pub struct NativeGbmScanoutBufferPlaneFds {
    inner: sophia_renderer_native_egl::NativeGbmOwnedScanoutBufferPlaneFds,
}

impl NativeGbmScanoutBufferPlaneFds {
    fn new(inner: sophia_renderer_native_egl::NativeGbmOwnedScanoutBufferPlaneFds) -> Self {
        Self { inner }
    }

    pub const fn plane_count(&self) -> u8 {
        self.inner.plane_count()
    }

    pub fn into_plane_fds(self) -> [Option<OwnedFd>; 4] {
        self.inner.into_plane_fds()
    }
}

#[derive(Debug)]
pub struct NativeGbmOwnedScanoutBufferExportReport {
    pub status: LiveRendererScanoutBufferExportStatus,
    pub detail: LiveRendererScanoutBufferExportDetail,
    pub buffer: Option<NativeGbmOwnedScanoutBuffer>,
}

impl NativeGbmOwnedScanoutBufferExportReport {
    pub fn new(
        status: LiveRendererScanoutBufferExportStatus,
        detail: LiveRendererScanoutBufferExportDetail,
        buffer: Option<NativeGbmOwnedScanoutBuffer>,
    ) -> Self {
        match status {
            LiveRendererScanoutBufferExportStatus::Exported => Self {
                status: if buffer.is_some() {
                    LiveRendererScanoutBufferExportStatus::Exported
                } else {
                    LiveRendererScanoutBufferExportStatus::Degraded
                },
                detail: if buffer.is_some() {
                    detail
                } else {
                    LiveRendererScanoutBufferExportDetail::RetainedBufferMissing
                },
                buffer,
            },
            LiveRendererScanoutBufferExportStatus::Pending => Self {
                status,
                detail,
                buffer: None,
            },
            status => Self {
                status,
                detail,
                buffer: None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGbmRenderedScanoutContextStatus {
    Ready,
    Unavailable,
    Degraded,
}

pub struct NativeGbmRenderedScanoutContext<T: AsFd> {
    inner: sophia_renderer_native_egl::NativeGbmRenderedScanoutContext<T>,
}

#[derive(Debug)]
pub struct LiveOwnedDmaBufFrame {
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub modifier: u64,
    pub fd: OwnedFd,
    pub offset: u32,
    pub stride: u32,
}

#[derive(Debug)]
pub struct LiveOwnedDmaBufPlane {
    pub fd: OwnedFd,
    pub offset: u32,
    pub stride: u32,
}

#[derive(Debug)]
pub struct LiveOwnedMultiPlaneDmaBufFrame {
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub modifier: u64,
    pub plane_count: u8,
    pub planes: [Option<LiveOwnedDmaBufPlane>; 4],
}

impl LiveOwnedMultiPlaneDmaBufFrame {
    pub fn try_clone(&self) -> std::io::Result<Self> {
        let mut planes = std::array::from_fn(|_| None);
        for (target, source) in planes.iter_mut().zip(&self.planes) {
            if let Some(source) = source {
                *target = Some(LiveOwnedDmaBufPlane {
                    fd: source.fd.try_clone()?,
                    offset: source.offset,
                    stride: source.stride,
                });
            }
        }
        Ok(Self {
            width: self.width,
            height: self.height,
            format: self.format,
            modifier: self.modifier,
            plane_count: self.plane_count,
            planes,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LiveCompositionPlacement {
    pub target: Rect,
    pub clip: Option<Rect>,
    pub transform: Transform,
    pub alpha: f32,
    /// Sampling required by the realized source extent, not merely the plan.
    pub sampling: HeadSamplingClass,
}

#[derive(Clone, Copy, Debug)]
pub enum LiveMixedCompositionLayer<'a> {
    Cpu {
        buffer: LiveCpuBufferSourceRef<'a>,
        placement: LiveCompositionPlacement,
    },
    DmaBuf {
        image_id: LiveRendererImageId,
        frame: &'a LiveOwnedMultiPlaneDmaBufFrame,
        placement: LiveCompositionPlacement,
    },
    RendererImage {
        image_id: LiveRendererImageId,
        placement: LiveCompositionPlacement,
    },
    Solid {
        geometry: Rect,
        color: CompositorRgb8,
    },
}

#[derive(Debug)]
pub enum LiveOwnedMixedCompositionLayer {
    Cpu {
        buffer: crate::LiveSharedCpuBufferSource,
        placement: LiveCompositionPlacement,
    },
    DmaBuf {
        image_id: LiveRendererImageId,
        frame: LiveOwnedMultiPlaneDmaBufFrame,
        placement: LiveCompositionPlacement,
    },
    RendererImage {
        image_id: LiveRendererImageId,
        size: Size,
        format: u32,
        placement: LiveCompositionPlacement,
    },
    Solid {
        geometry: Rect,
        color: CompositorRgb8,
    },
}

#[derive(Debug, Default)]
pub struct LiveOwnedMixedCompositionFrame {
    pub layers: Vec<LiveOwnedMixedCompositionLayer>,
    pub output_damage_snapshot: Option<sophia_engine::OutputFrameDamageSnapshot>,
    pub trace: Option<LiveCompositionTrace>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveCompositionTrace {
    pub output: OutputId,
    pub head: RenderHeadId,
    pub scene_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveMixedCompositionError {
    InvalidOutput,
    InvalidLayer,
    UnsupportedTransform,
    Renderer(LiveRendererScanoutBufferExportDetail),
}

#[derive(Clone, Copy, Debug)]
pub struct LiveDmaBufFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub modifier: u64,
    pub fd: BorrowedFd<'a>,
    pub offset: u32,
    pub stride: u32,
}

impl LiveOwnedDmaBufFrame {
    pub fn as_frame(&self) -> LiveDmaBufFrame<'_> {
        LiveDmaBufFrame {
            width: self.width,
            height: self.height,
            format: self.format,
            modifier: self.modifier,
            fd: self.fd.as_fd(),
            offset: self.offset,
            stride: self.stride,
        }
    }

    pub fn try_clone(&self) -> std::io::Result<Self> {
        Ok(Self {
            width: self.width,
            height: self.height,
            format: self.format,
            modifier: self.modifier,
            fd: self.fd.try_clone()?,
            offset: self.offset,
            stride: self.stride,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LiveNativePersistentRenderStats {
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
    pub import_cache: LiveNativeDmaBufImportCacheStats,
    pub exact_nearest_draws: usize,
    pub sharp_downscale_draws: usize,
    pub sharp_upscale_draws: usize,
    pub linear_fallback_draws: usize,
    pub max_target_create: std::time::Duration,
    pub max_frame_surface_create: std::time::Duration,
    pub max_render: std::time::Duration,
    pub max_upload: std::time::Duration,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LiveNativeDmaBufImportCacheStats {
    pub imports: usize,
    pub hits: usize,
    pub evictions: usize,
    pub live_entries: usize,
    pub descriptor_mismatches: usize,
    pub capacity_rejections: usize,
}

impl<T> NativeGbmRenderedScanoutContext<T>
where
    T: AsFd,
{
    pub fn persistent_render_stats(&self) -> LiveNativePersistentRenderStats {
        let stats = self.inner.persistent_render_stats();
        LiveNativePersistentRenderStats {
            target_creations: stats.target_creations,
            target_recreations: stats.target_recreations,
            gl_pipeline_creations: stats.gl_pipeline_creations,
            frame_surface_creations: stats.frame_surface_creations,
            cpu_target_creations: stats.cpu_target_creations,
            dmabuf_target_creations: stats.dmabuf_target_creations,
            composition_target_creations: stats.composition_target_creations,
            composition_target_reuses: stats.composition_target_reuses,
            generation_replacements: stats.generation_replacements,
            recovery_replacements: stats.recovery_replacements,
            frame_uploads: stats.frame_uploads,
            snapshot_captures: stats.snapshot_captures,
            snapshot_promotions: stats.snapshot_promotions,
            snapshot_rollbacks: stats.snapshot_rollbacks,
            snapshot_evictions: stats.snapshot_evictions,
            snapshot_live_entries: stats.snapshot_live_entries,
            snapshot_live_bytes: stats.snapshot_live_bytes,
            import_cache: LiveNativeDmaBufImportCacheStats {
                imports: stats.import_cache.imports,
                hits: stats.import_cache.hits,
                evictions: stats.import_cache.evictions,
                live_entries: stats.import_cache.live_entries,
                descriptor_mismatches: stats.import_cache.descriptor_mismatches,
                capacity_rejections: stats.import_cache.capacity_rejections,
            },
            exact_nearest_draws: stats.sampling.exact_nearest_draws,
            sharp_downscale_draws: stats.sampling.sharp_downscale_draws,
            sharp_upscale_draws: stats.sampling.sharp_upscale_draws,
            linear_fallback_draws: stats.sampling.linear_fallback_draws,
            max_target_create: stats.max_target_create,
            max_frame_surface_create: stats.max_frame_surface_create,
            max_render: stats.max_render,
            max_upload: stats.max_upload,
        }
    }

    pub fn composition_nonzero_rgb_pixels(&self) -> usize {
        self.inner
            .composition_pixel_metrics()
            .map_or(0, |metrics| metrics.nonzero_rgb_pixels)
    }

    pub fn from_backend_device_result(
        device: std::io::Result<T>,
    ) -> NativeGbmRenderedScanoutContextReport<T> {
        let report = sophia_renderer_native_egl::NativeGbmRenderedScanoutContext::
            from_backend_device_result_with_import_cache_capacity(
                device,
                crate::LIVE_PRESENTATION_REGISTRY_CAPACITY,
            );
        NativeGbmRenderedScanoutContextReport {
            status: match report.status {
                sophia_renderer_native_egl::NativeGbmRenderedScanoutContextStatus::Ready => {
                    NativeGbmRenderedScanoutContextStatus::Ready
                }
                sophia_renderer_native_egl::NativeGbmRenderedScanoutContextStatus::Unavailable => {
                    NativeGbmRenderedScanoutContextStatus::Unavailable
                }
                sophia_renderer_native_egl::NativeGbmRenderedScanoutContextStatus::Degraded => {
                    NativeGbmRenderedScanoutContextStatus::Degraded
                }
            },
            context: report
                .context
                .map(|inner| NativeGbmRenderedScanoutContext { inner }),
        }
    }

    pub fn evict_renderer_image(
        &mut self,
        image_id: LiveRendererImageId,
    ) -> Result<bool, LiveRendererScanoutBufferExportDetail> {
        self.inner
            .evict_renderer_image(sophia_renderer_native_egl::NativeRendererImageId::from_raw(
                image_id.raw(),
            ))
            .map_err(reduced_native_owned_scanout_buffer_export_detail)
    }

    pub fn promote_renderer_image(
        &mut self,
        image_id: LiveRendererImageId,
    ) -> Result<bool, LiveRendererScanoutBufferExportDetail> {
        self.inner
            .promote_renderer_image(sophia_renderer_native_egl::NativeRendererImageId::from_raw(
                image_id.raw(),
            ))
            .map_err(reduced_native_owned_scanout_buffer_export_detail)
    }

    pub fn export_promoted_renderer_image(
        &self,
        image_id: LiveRendererImageId,
    ) -> Result<Option<LiveRendererImageSnapshot>, LiveRendererScanoutBufferExportDetail> {
        self.inner
            .export_promoted_renderer_image(
                sophia_renderer_native_egl::NativeRendererImageId::from_raw(image_id.raw()),
            )
            .map(|snapshot| snapshot.map(|inner| LiveRendererImageSnapshot { image_id, inner }))
            .map_err(reduced_native_owned_scanout_buffer_export_detail)
    }

    pub fn restore_promoted_renderer_image(
        &mut self,
        snapshot: LiveRendererImageSnapshot,
    ) -> Result<bool, LiveRendererScanoutBufferExportDetail> {
        self.inner
            .restore_promoted_renderer_image(snapshot.inner)
            .map_err(reduced_native_owned_scanout_buffer_export_detail)
    }

    pub fn rollback_renderer_image(
        &mut self,
        image_id: LiveRendererImageId,
    ) -> Result<bool, LiveRendererScanoutBufferExportDetail> {
        self.inner
            .rollback_renderer_image(sophia_renderer_native_egl::NativeRendererImageId::from_raw(
                image_id.raw(),
            ))
            .map_err(reduced_native_owned_scanout_buffer_export_detail)
    }

    pub fn clear_renderer_images(
        &mut self,
    ) -> Result<usize, LiveRendererScanoutBufferExportDetail> {
        self.inner
            .clear_renderer_images()
            .map_err(reduced_native_owned_scanout_buffer_export_detail)
    }

    pub fn export_rendered_owned_scanout_buffer(
        &self,
        target: LiveGbmEglFrameTargetRecord,
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        self.export_rendered_owned_scanout_buffer_with_modifiers(target, &[])
    }

    pub fn export_rendered_owned_scanout_buffer_with_modifiers(
        &self,
        target: LiveGbmEglFrameTargetRecord,
        preferred_modifiers: &[u64],
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if !target.is_valid_scanout_target() {
            return NativeGbmOwnedScanoutBufferExportReport::new(
                LiveRendererScanoutBufferExportStatus::InvalidTarget,
                LiveRendererScanoutBufferExportDetail::InvalidTarget,
                None,
            );
        }

        reduced_native_owned_scanout_buffer_export_report(
            self.inner
                .export_rendered_owned_scanout_buffer_with_modifiers(
                    target.size.width as u32,
                    target.size.height as u32,
                    preferred_modifiers,
                ),
        )
    }

    pub fn export_xrgb8888_owned_scanout_buffer_with_modifiers(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
        frame: &crate::LiveCpuComposedFrame,
        preferred_modifiers: &[u64],
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if !target.is_valid_scanout_target()
            || frame.size != target.size
            || frame.format != crate::LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888
        {
            return NativeGbmOwnedScanoutBufferExportReport::new(
                LiveRendererScanoutBufferExportStatus::InvalidTarget,
                LiveRendererScanoutBufferExportDetail::InvalidTarget,
                None,
            );
        }

        reduced_native_owned_scanout_buffer_export_report(
            self.inner
                .export_xrgb8888_owned_scanout_buffer_with_modifiers(
                    target.size.width as u32,
                    target.size.height as u32,
                    frame.stride,
                    &frame.bytes,
                    preferred_modifiers,
                ),
        )
    }

    pub fn rewrite_xrgb8888_owned_scanout_buffer_damage(
        &mut self,
        buffer: &mut NativeGbmOwnedScanoutBuffer,
        frame: &crate::LiveCpuComposedFrame,
        damage: &[Rect],
    ) -> Result<(), LiveRendererScanoutBufferExportDetail> {
        if buffer.descriptor.size != frame.size
            || buffer.descriptor.format != crate::LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888
        {
            return Err(LiveRendererScanoutBufferExportDetail::InvalidTarget);
        }
        let damage = damage
            .iter()
            .map(|rect| sophia_renderer_native_egl::NativeCompositionRect {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
            })
            .collect::<Vec<_>>();
        self.inner
            .rewrite_xrgb8888_owned_scanout_buffer_damage(
                &mut buffer._buffer,
                &frame.bytes,
                &damage,
            )
            .map_err(reduced_native_owned_scanout_buffer_export_detail)
    }

    pub fn export_dmabuf_owned_scanout_buffer_with_modifiers(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
        frame: LiveDmaBufFrame<'_>,
        preferred_modifiers: &[u64],
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if !target.is_valid_scanout_target()
            || target.size.width != i32::try_from(frame.width).unwrap_or(i32::MAX)
            || target.size.height != i32::try_from(frame.height).unwrap_or(i32::MAX)
        {
            return NativeGbmOwnedScanoutBufferExportReport::new(
                LiveRendererScanoutBufferExportStatus::InvalidTarget,
                LiveRendererScanoutBufferExportDetail::InvalidTarget,
                None,
            );
        }
        reduced_native_owned_scanout_buffer_export_report(
            self.inner
                .export_dmabuf_owned_scanout_buffer_with_modifiers(
                    sophia_renderer_native_egl::NativeDmaBufFrame {
                        width: frame.width,
                        height: frame.height,
                        format: frame.format,
                        modifier: frame.modifier,
                        fd: frame.fd,
                        offset: frame.offset,
                        stride: frame.stride,
                    },
                    preferred_modifiers,
                ),
        )
    }

    pub fn export_mixed_owned_scanout_buffer_with_modifiers(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
        layers: &[LiveMixedCompositionLayer<'_>],
        preferred_modifiers: &[u64],
    ) -> Result<NativeGbmOwnedScanoutBufferExportReport, LiveMixedCompositionError> {
        self.export_mixed_owned_scanout_buffer_with_modifiers_and_trace(
            target,
            layers,
            preferred_modifiers,
            None,
        )
    }

    fn export_mixed_owned_scanout_buffer_with_modifiers_and_trace(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
        layers: &[LiveMixedCompositionLayer<'_>],
        preferred_modifiers: &[u64],
        trace: Option<LiveCompositionTrace>,
    ) -> Result<NativeGbmOwnedScanoutBufferExportReport, LiveMixedCompositionError> {
        if !target.is_valid_scanout_target() {
            return Err(LiveMixedCompositionError::InvalidOutput);
        }
        let native_layers = layers
            .iter()
            .map(|layer| match layer {
                LiveMixedCompositionLayer::Cpu { buffer, placement } => {
                    validate_placement(*placement)?;
                    if buffer.size.width <= 0
                        || buffer.size.height <= 0
                        || !matches!(buffer.format, DRM_FORMAT_XRGB8888 | DRM_FORMAT_ARGB8888)
                    {
                        return Err(LiveMixedCompositionError::InvalidLayer);
                    }
                    Ok(sophia_renderer_native_egl::NativeCompositionLayer::Cpu(
                        sophia_renderer_native_egl::NativeCpuCompositionLayer {
                            width: buffer.size.width as u32,
                            height: buffer.size.height as u32,
                            stride: buffer.stride,
                            format: buffer.format,
                            pixels: buffer.bytes,
                            target: native_rect(placement.target),
                            clip: placement.clip.map(native_rect),
                            alpha: placement.alpha,
                            sampling: native_sampling(placement.sampling),
                        },
                    ))
                }
                LiveMixedCompositionLayer::DmaBuf {
                    image_id,
                    frame,
                    placement,
                } => {
                    validate_placement(*placement)?;
                    if !image_id.is_valid()
                        || frame.width == 0
                        || frame.height == 0
                        || frame.plane_count == 0
                        || usize::from(frame.plane_count) > frame.planes.len()
                    {
                        return Err(LiveMixedCompositionError::InvalidLayer);
                    }
                    let planes = std::array::from_fn(|index| {
                        frame.planes[index].as_ref().map(|plane| {
                            sophia_renderer_native_egl::NativeDmaBufPlane {
                                fd: plane.fd.as_fd(),
                                offset: plane.offset,
                                stride: plane.stride,
                            }
                        })
                    });
                    Ok(sophia_renderer_native_egl::NativeCompositionLayer::DmaBuf(
                        sophia_renderer_native_egl::NativeDmaBufCompositionLayer {
                            image_id: sophia_renderer_native_egl::NativeRendererImageId::from_raw(
                                image_id.raw(),
                            ),
                            frame: sophia_renderer_native_egl::NativeMultiPlaneDmaBufFrame {
                                width: frame.width,
                                height: frame.height,
                                format: frame.format,
                                modifier: frame.modifier,
                                plane_count: frame.plane_count,
                                planes,
                            },
                            target: native_rect(placement.target),
                            clip: placement.clip.map(native_rect),
                            alpha: placement.alpha,
                            sampling: native_sampling(placement.sampling),
                        },
                    ))
                }
                LiveMixedCompositionLayer::RendererImage {
                    image_id,
                    placement,
                } => {
                    validate_placement(*placement)?;
                    if !image_id.is_valid() {
                        return Err(LiveMixedCompositionError::InvalidLayer);
                    }
                    Ok(
                        sophia_renderer_native_egl::NativeCompositionLayer::RendererImage(
                            sophia_renderer_native_egl::NativeRendererImageCompositionLayer {
                                image_id:
                                    sophia_renderer_native_egl::NativeRendererImageId::from_raw(
                                        image_id.raw(),
                                    ),
                                target: native_rect(placement.target),
                                clip: placement.clip.map(native_rect),
                                alpha: placement.alpha,
                                sampling: native_sampling(placement.sampling),
                            },
                        ),
                    )
                }
                LiveMixedCompositionLayer::Solid { geometry, color } => {
                    if geometry.is_empty() {
                        return Err(LiveMixedCompositionError::InvalidLayer);
                    }
                    Ok(sophia_renderer_native_egl::NativeCompositionLayer::Solid(
                        sophia_renderer_native_egl::NativeSolidCompositionLayer {
                            target: native_rect(*geometry),
                            color: [color.red, color.green, color.blue],
                        },
                    ))
                }
            })
            .collect::<Result<Vec<_>, LiveMixedCompositionError>>()?;
        Ok(reduced_native_owned_scanout_buffer_export_report(
            self.inner
                .export_composed_owned_scanout_buffer_with_modifiers(
                    sophia_renderer_native_egl::NativeCompositionFrame {
                        width: target.size.width as u32,
                        height: target.size.height as u32,
                        layers: &native_layers,
                        trace: trace.map(|trace| {
                            sophia_renderer_native_egl::NativeCompositionTrace {
                                output: trace.output.raw(),
                                head: trace.head.raw(),
                                scene_generation: trace.scene_generation,
                            }
                        }),
                    },
                    preferred_modifiers,
                ),
        ))
    }

    pub fn export_owned_mixed_frame_with_modifiers(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
        frame: &LiveOwnedMixedCompositionFrame,
        preferred_modifiers: &[u64],
    ) -> Result<NativeGbmOwnedScanoutBufferExportReport, LiveMixedCompositionError> {
        // Capture client DMA-BUFs before assembling the output frame. Retained
        // scene state below then refers only to compositor-owned images.
        for layer in &frame.layers {
            let LiveOwnedMixedCompositionLayer::DmaBuf {
                image_id, frame, ..
            } = layer
            else {
                continue;
            };
            let planes = std::array::from_fn(|index| {
                frame.planes[index].as_ref().map(|plane| {
                    sophia_renderer_native_egl::NativeDmaBufPlane {
                        fd: plane.fd.as_fd(),
                        offset: plane.offset,
                        stride: plane.stride,
                    }
                })
            });
            self.inner
                .capture_renderer_image(
                    sophia_renderer_native_egl::NativeRendererImageId::from_raw(image_id.raw()),
                    sophia_renderer_native_egl::NativeMultiPlaneDmaBufFrame {
                        width: frame.width,
                        height: frame.height,
                        format: frame.format,
                        modifier: frame.modifier,
                        plane_count: frame.plane_count,
                        planes,
                    },
                )
                .map_err(|detail| {
                    LiveMixedCompositionError::Renderer(
                        reduced_native_owned_scanout_buffer_export_detail(detail),
                    )
                })?;
        }
        let layers = frame
            .layers
            .iter()
            .map(|layer| match layer {
                LiveOwnedMixedCompositionLayer::Cpu { buffer, placement } => {
                    LiveMixedCompositionLayer::Cpu {
                        buffer: LiveCpuBufferSourceRef {
                            handle: buffer.handle,
                            size: buffer.size,
                            stride: buffer.stride,
                            format: buffer.format,
                            generation: buffer.generation,
                            bytes: buffer.bytes.as_slice(),
                        },
                        placement: *placement,
                    }
                }
                LiveOwnedMixedCompositionLayer::DmaBuf {
                    image_id,
                    placement,
                    ..
                } => LiveMixedCompositionLayer::RendererImage {
                    image_id: *image_id,
                    placement: *placement,
                },
                LiveOwnedMixedCompositionLayer::RendererImage {
                    image_id,
                    placement,
                    ..
                } => LiveMixedCompositionLayer::RendererImage {
                    image_id: *image_id,
                    placement: *placement,
                },
                LiveOwnedMixedCompositionLayer::Solid { geometry, color } => {
                    LiveMixedCompositionLayer::Solid {
                        geometry: *geometry,
                        color: *color,
                    }
                }
            })
            .collect::<Vec<_>>();
        self.export_mixed_owned_scanout_buffer_with_modifiers_and_trace(
            target,
            &layers,
            preferred_modifiers,
            frame.trace,
        )
    }

    pub fn export_rendered_owned_scanout_buffer_with_modifiers_from_backend_device_result<
        Device: AsFd,
    >(
        device: std::io::Result<Device>,
        target: LiveGbmEglFrameTargetRecord,
        preferred_modifiers: &[u64],
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if !target.is_valid_scanout_target() {
            return NativeGbmOwnedScanoutBufferExportReport::new(
                LiveRendererScanoutBufferExportStatus::InvalidTarget,
                LiveRendererScanoutBufferExportDetail::InvalidTarget,
                None,
            );
        }

        reduced_native_owned_scanout_buffer_export_report(
            sophia_renderer_native_egl::export_rendered_gbm_scanout_buffer_with_modifiers_from_backend_device_result(
                device,
                target.size.width as u32,
                target.size.height as u32,
                preferred_modifiers,
            ),
        )
    }
}

fn validate_placement(
    placement: LiveCompositionPlacement,
) -> Result<(), LiveMixedCompositionError> {
    if placement.transform != Transform::IDENTITY {
        return Err(LiveMixedCompositionError::UnsupportedTransform);
    }
    if placement.target.is_empty() || !placement.alpha.is_finite() {
        return Err(LiveMixedCompositionError::InvalidLayer);
    }
    Ok(())
}

const fn native_rect(rect: Rect) -> sophia_renderer_native_egl::NativeCompositionRect {
    sophia_renderer_native_egl::NativeCompositionRect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

const fn native_sampling(
    sampling: HeadSamplingClass,
) -> sophia_renderer_native_egl::NativeCompositionSampling {
    match sampling {
        HeadSamplingClass::Exact => {
            sophia_renderer_native_egl::NativeCompositionSampling::ExactNearest
        }
        HeadSamplingClass::Downsampled => {
            sophia_renderer_native_egl::NativeCompositionSampling::SharpDownscale
        }
        HeadSamplingClass::Upsampled => {
            sophia_renderer_native_egl::NativeCompositionSampling::SharpUpscale
        }
        HeadSamplingClass::Mixed => {
            sophia_renderer_native_egl::NativeCompositionSampling::SharpMixed
        }
    }
}

pub struct NativeGbmRenderedScanoutContextReport<T: AsFd> {
    pub status: NativeGbmRenderedScanoutContextStatus,
    pub context: Option<NativeGbmRenderedScanoutContext<T>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeGbmScanoutBufferExporter;

impl NativeGbmScanoutBufferExporter {
    pub fn export_owned_scanout_buffer_from_backend_device_result<T: AsFd>(
        device: std::io::Result<T>,
        target: LiveGbmEglFrameTargetRecord,
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        export_native_owned_scanout_buffer_from_backend_device_result(
            device,
            target,
            sophia_renderer_native_egl::export_gbm_scanout_buffer_from_backend_device_result,
        )
    }

    pub fn export_rendered_owned_scanout_buffer_from_backend_device_result<T: AsFd>(
        device: std::io::Result<T>,
        target: LiveGbmEglFrameTargetRecord,
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        export_native_owned_scanout_buffer_from_backend_device_result(
            device,
            target,
            sophia_renderer_native_egl::export_rendered_gbm_scanout_buffer_from_backend_device_result,
        )
    }

    pub fn export_direct_cpu_owned_scanout_buffer_from_backend_device_result<T: AsFd>(
        device: std::io::Result<T>,
        target: LiveGbmEglFrameTargetRecord,
        frame: &crate::LiveCpuComposedFrame,
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if !target.is_valid_scanout_target()
            || frame.size != target.size
            || frame.format != crate::LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888
        {
            return NativeGbmOwnedScanoutBufferExportReport::new(
                LiveRendererScanoutBufferExportStatus::InvalidTarget,
                LiveRendererScanoutBufferExportDetail::InvalidTarget,
                None,
            );
        }
        reduced_native_owned_scanout_buffer_export_report(
            sophia_renderer_native_egl::export_direct_cpu_xrgb8888_gbm_scanout_buffer_from_backend_device_result(
                device,
                target.size.width as u32,
                target.size.height as u32,
                frame.stride,
                &frame.bytes,
            ),
        )
    }
}

fn export_native_owned_scanout_buffer_from_backend_device_result<T, F>(
    device: std::io::Result<T>,
    target: LiveGbmEglFrameTargetRecord,
    export: F,
) -> NativeGbmOwnedScanoutBufferExportReport
where
    T: AsFd,
    F: FnOnce(
        std::io::Result<T>,
        u32,
        u32,
    ) -> sophia_renderer_native_egl::NativeGbmOwnedScanoutBufferExportReport,
{
    if !target.is_valid_scanout_target() {
        return NativeGbmOwnedScanoutBufferExportReport::new(
            LiveRendererScanoutBufferExportStatus::InvalidTarget,
            LiveRendererScanoutBufferExportDetail::InvalidTarget,
            None,
        );
    }

    let report = export(device, target.size.width as u32, target.size.height as u32);
    reduced_native_owned_scanout_buffer_export_report(report)
}

fn reduced_native_owned_scanout_buffer_export_report(
    report: sophia_renderer_native_egl::NativeGbmOwnedScanoutBufferExportReport,
) -> NativeGbmOwnedScanoutBufferExportReport {
    let status = match report.status {
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportStatus::Exported => {
            LiveRendererScanoutBufferExportStatus::Exported
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportStatus::InvalidTarget => {
            LiveRendererScanoutBufferExportStatus::InvalidTarget
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportStatus::Unavailable => {
            LiveRendererScanoutBufferExportStatus::Unavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportStatus::Degraded => {
            LiveRendererScanoutBufferExportStatus::Degraded
        }
    };

    let buffer = report.buffer.and_then(|buffer| {
        let descriptor = LiveRendererScanoutBufferDescriptor::new_with_planes(
            Size {
                width: buffer.width() as i32,
                height: buffer.height() as i32,
            },
            buffer.pitch(),
            buffer.format(),
            buffer.gem_handle(),
            LiveRendererScanoutBufferPlanes {
                count: buffer.plane_count(),
                handles: buffer.plane_handles(),
                pitches: buffer.plane_pitches(),
                offsets: buffer.plane_offsets(),
                modifier: buffer.modifier(),
            },
        );
        descriptor
            .is_valid_scanout_buffer()
            .then_some(NativeGbmOwnedScanoutBuffer {
                descriptor,
                _buffer: buffer,
            })
    });
    NativeGbmOwnedScanoutBufferExportReport::new(
        status,
        reduced_native_owned_scanout_buffer_export_detail(report.detail),
        buffer,
    )
}

fn reduced_native_owned_scanout_buffer_export_detail(
    detail: sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail,
) -> LiveRendererScanoutBufferExportDetail {
    match detail {
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::Exported => {
            LiveRendererScanoutBufferExportDetail::Exported
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::InvalidTarget => {
            LiveRendererScanoutBufferExportDetail::InvalidTarget
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::BackendDeviceUnavailable => {
            LiveRendererScanoutBufferExportDetail::BackendDeviceUnavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::GbmDeviceUnavailable => {
            LiveRendererScanoutBufferExportDetail::GbmDeviceUnavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglUnavailable => {
            LiveRendererScanoutBufferExportDetail::EglUnavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglDisplayUnavailable => {
            LiveRendererScanoutBufferExportDetail::EglDisplayUnavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglInitializeFailed => {
            LiveRendererScanoutBufferExportDetail::EglInitializeFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglBindApiFailed => {
            LiveRendererScanoutBufferExportDetail::EglBindApiFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglConfigUnavailable => {
            LiveRendererScanoutBufferExportDetail::EglConfigUnavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::GbmSurfaceUnavailable => {
            LiveRendererScanoutBufferExportDetail::GbmSurfaceUnavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglSurfaceUnavailable => {
            LiveRendererScanoutBufferExportDetail::EglSurfaceUnavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglContextUnavailable => {
            LiveRendererScanoutBufferExportDetail::EglContextUnavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglMakeCurrentFailed => {
            LiveRendererScanoutBufferExportDetail::EglMakeCurrentFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::GlSmokeFailed => {
            LiveRendererScanoutBufferExportDetail::GlSmokeFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::CpuLayerUploadFailed => {
            LiveRendererScanoutBufferExportDetail::CpuLayerUploadFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::DmaBufImageCreateFailed => {
            LiveRendererScanoutBufferExportDetail::DmaBufImageCreateFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::DmaBufImageBindFailed => {
            LiveRendererScanoutBufferExportDetail::DmaBufImageBindFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::CompositionDrawFailed => {
            LiveRendererScanoutBufferExportDetail::CompositionDrawFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::CompositionFinishFailed => {
            LiveRendererScanoutBufferExportDetail::CompositionFinishFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglImageDestroyFailed => {
            LiveRendererScanoutBufferExportDetail::EglImageDestroyFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::DmaBufImportFailed => {
            LiveRendererScanoutBufferExportDetail::DmaBufImportFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglSwapBuffersFailed => {
            LiveRendererScanoutBufferExportDetail::EglSwapBuffersFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::FrontBufferLockFailed => {
            LiveRendererScanoutBufferExportDetail::FrontBufferLockFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor => {
            LiveRendererScanoutBufferExportDetail::InvalidBufferDescriptor
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::InvalidRendererImageId => {
            LiveRendererScanoutBufferExportDetail::InvalidRendererImageId
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::DmaBufDescriptorMismatch => {
            LiveRendererScanoutBufferExportDetail::DmaBufDescriptorMismatch
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::DmaBufImportCacheFull => {
            LiveRendererScanoutBufferExportDetail::DmaBufImportCacheFull
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::RendererImageStoreFull => {
            LiveRendererScanoutBufferExportDetail::RendererImageStoreFull
        }
    }
}
