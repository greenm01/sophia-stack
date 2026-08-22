#[derive(Debug)]
pub struct NativeGbmOwnedScanoutBufferExportReport {
    pub status: NativeGbmScanoutBufferExportStatus,
    pub detail: NativeGbmScanoutBufferExportDetail,
    pub buffer: Option<NativeGbmOwnedScanoutBuffer>,
}

pub struct NativeGbmRenderedScanoutContext<T: std::os::fd::AsFd> {
    egl: khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    gbm_device: gbm::Device<T>,
    stats: NativeGbmPersistentRenderStats,
    last_composition_pixel_metrics: Option<NativeCompositionPixelMetrics>,
    composition_pixel_proof_attempts: usize,
    composition_target: Option<PersistentCompositionTarget>,
    import_cache_capacity: usize,
    renderer_images: std::collections::BTreeMap<NativeRendererImageId, NativeRendererImage>,
    renderer_image_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeRendererImageState {
    Staged,
    Promoted,
}

struct NativeRendererImage {
    buffer: NativeGbmOwnedScanoutBuffer,
    state: NativeRendererImageState,
    bytes: u64,
}

struct NativeRenderTarget {
    width: u32,
    height: u32,
    egl_context: khronos_egl::Context,
    pipeline: PersistentXrgb8888GlPipeline,
}

struct PersistentCompositionTarget {
    target: NativeRenderTarget,
    surface: std::sync::Arc<NativeFrameSurface>,
    import_cache: NativeDmaBufImportCache,
}

pub const DEFAULT_NATIVE_DMA_BUF_IMPORT_CACHE_CAPACITY: usize = 256;
pub const DEFAULT_NATIVE_RENDERER_IMAGE_CAPACITY: usize = 256;
pub const DEFAULT_NATIVE_RENDERER_IMAGE_BYTE_BUDGET: u64 = 512 * 1024 * 1024;

impl<T> NativeGbmRenderedScanoutContext<T>
where
    T: std::os::fd::AsFd,
{
    pub fn from_backend_device_result(
        device: std::io::Result<T>,
    ) -> NativeGbmRenderedScanoutContextReport<T> {
        match device {
            Ok(device) => match Self::new(device, DEFAULT_NATIVE_DMA_BUF_IMPORT_CACHE_CAPACITY) {
                Ok(context) => NativeGbmRenderedScanoutContextReport {
                    status: NativeGbmRenderedScanoutContextStatus::Ready,
                    context: Some(context),
                },
                Err(status) => NativeGbmRenderedScanoutContextReport {
                    status,
                    context: None,
                },
            },
            Err(_error) => NativeGbmRenderedScanoutContextReport {
                status: NativeGbmRenderedScanoutContextStatus::Unavailable,
                context: None,
            },
        }
    }

    pub fn from_backend_device_result_with_import_cache_capacity(
        device: std::io::Result<T>,
        import_cache_capacity: usize,
    ) -> NativeGbmRenderedScanoutContextReport<T> {
        match device {
            Ok(device) => match Self::new(device, import_cache_capacity) {
                Ok(context) => NativeGbmRenderedScanoutContextReport {
                    status: NativeGbmRenderedScanoutContextStatus::Ready,
                    context: Some(context),
                },
                Err(status) => NativeGbmRenderedScanoutContextReport {
                    status,
                    context: None,
                },
            },
            Err(_error) => NativeGbmRenderedScanoutContextReport {
                status: NativeGbmRenderedScanoutContextStatus::Unavailable,
                context: None,
            },
        }
    }

    fn new(
        device: T,
        import_cache_capacity: usize,
    ) -> Result<Self, NativeGbmRenderedScanoutContextStatus> {
        use gbm::AsRaw as _;

        let gbm_device = gbm::Device::new(device)
            .map_err(|_error| NativeGbmRenderedScanoutContextStatus::Unavailable)?;
        let egl = unsafe { khronos_egl::DynamicInstance::<khronos_egl::EGL1_5>::load_required() }
            .map_err(|_error| NativeGbmRenderedScanoutContextStatus::Unavailable)?;
        let native_display = gbm_device.as_raw() as khronos_egl::NativeDisplayType;
        let display = unsafe {
            egl.get_platform_display(
                EGL_PLATFORM_GBM_KHR,
                native_display,
                &[khronos_egl::ATTRIB_NONE],
            )
        }
        .map_err(|_error| NativeGbmRenderedScanoutContextStatus::Unavailable)?;

        egl.initialize(display)
            .map_err(|_error| NativeGbmRenderedScanoutContextStatus::Degraded)?;

        Ok(Self {
            egl,
            display,
            gbm_device,
            stats: NativeGbmPersistentRenderStats::default(),
            last_composition_pixel_metrics: None,
            composition_pixel_proof_attempts: 0,
            composition_target: None,
            import_cache_capacity,
            renderer_images: std::collections::BTreeMap::new(),
            renderer_image_bytes: 0,
        })
    }

    pub fn persistent_render_stats(&self) -> NativeGbmPersistentRenderStats {
        let mut stats = self.stats;
        if let Some(persistent) = self.composition_target.as_ref() {
            stats.sampling = stats
                .sampling
                .saturating_add(persistent.target.pipeline.sampling_stats());
        }
        stats
    }

    pub const fn composition_pixel_metrics(&self) -> Option<NativeCompositionPixelMetrics> {
        self.last_composition_pixel_metrics
    }

    pub fn evict_renderer_image(
        &mut self,
        image_id: NativeRendererImageId,
    ) -> Result<bool, NativeGbmScanoutBufferExportDetail> {
        let mut evicted_import = false;
        if let Some(persistent) = self.composition_target.as_mut() {
            let surface = persistent.surface.egl_surface();
            self.egl
                .make_current(
                    self.display,
                    Some(surface),
                    Some(surface),
                    Some(persistent.target.egl_context),
                )
                .map_err(|_| NativeGbmScanoutBufferExportDetail::EglMakeCurrentFailed)?;
            let result = persistent.import_cache.evict(
                &self.egl,
                self.display,
                &persistent.target.pipeline,
                image_id,
            );
            self.stats.import_cache = persistent.import_cache.stats();
            let _ = self.egl.make_current(self.display, None, None, None);
            if result.is_err()
                && let Some(persistent) = self.composition_target.take()
            {
                self.stats.target_recreations = self.stats.target_recreations.saturating_add(1);
                self.stats.recovery_replacements =
                    self.stats.recovery_replacements.saturating_add(1);
                self.destroy_persistent_composition_target(persistent);
                evicted_import = true;
            } else {
                evicted_import = result?;
            }
        }
        // Drop the EGL import before its compositor-owned GBM backing store.
        // This ordering keeps cache recovery from observing a dead DMA-BUF.
        let evicted_image = self.renderer_images.remove(&image_id);
        if let Some(image) = evicted_image {
            self.renderer_image_bytes = self.renderer_image_bytes.saturating_sub(image.bytes);
            self.stats.snapshot_evictions = self.stats.snapshot_evictions.saturating_add(1);
            self.update_renderer_image_stats();
            return Ok(true);
        }
        Ok(evicted_import)
    }

    pub fn clear_renderer_images(
        &mut self,
    ) -> Result<usize, NativeGbmScanoutBufferExportDetail> {
        let mut cleared_imports = 0;
        if let Some(persistent) = self.composition_target.as_mut() {
            let surface = persistent.surface.egl_surface();
            self.egl
                .make_current(
                    self.display,
                    Some(surface),
                    Some(surface),
                    Some(persistent.target.egl_context),
                )
                .map_err(|_| NativeGbmScanoutBufferExportDetail::EglMakeCurrentFailed)?;
            let live_entries = persistent.import_cache.stats().live_entries;
            let result = persistent.import_cache.clear(
                &self.egl,
                self.display,
                &persistent.target.pipeline,
            );
            self.stats.import_cache = persistent.import_cache.stats();
            let _ = self.egl.make_current(self.display, None, None, None);
            if result.is_err()
                && let Some(persistent) = self.composition_target.take()
            {
                self.stats.target_recreations = self.stats.target_recreations.saturating_add(1);
                self.stats.recovery_replacements =
                    self.stats.recovery_replacements.saturating_add(1);
                self.destroy_persistent_composition_target(persistent);
                cleared_imports = live_entries;
            } else {
                cleared_imports = result?;
            }
        }
        let cleared_images = self.renderer_images.len();
        self.renderer_images.clear();
        self.renderer_image_bytes = 0;
        self.stats.snapshot_evictions = self
            .stats
            .snapshot_evictions
            .saturating_add(cleared_images);
        self.update_renderer_image_stats();
        Ok(cleared_imports.max(cleared_images))
    }

    pub fn promote_renderer_image(
        &mut self,
        image_id: NativeRendererImageId,
    ) -> Result<bool, NativeGbmScanoutBufferExportDetail> {
        let Some(image) = self.renderer_images.get_mut(&image_id) else {
            return Ok(false);
        };
        if image.state == NativeRendererImageState::Promoted {
            return Ok(false);
        }
        image.state = NativeRendererImageState::Promoted;
        self.stats.snapshot_promotions = self.stats.snapshot_promotions.saturating_add(1);
        Ok(true)
    }

    pub fn export_promoted_renderer_image(
        &self,
        image_id: NativeRendererImageId,
    ) -> Result<Option<NativeRendererImageSnapshot>, NativeGbmScanoutBufferExportDetail> {
        let Some(image) = self.renderer_images.get(&image_id) else {
            return Ok(None);
        };
        if image.state != NativeRendererImageState::Promoted {
            return Ok(None);
        }
        let mut plane_fds = image.buffer.export_plane_fds()?.into_plane_fds();
        let pitches = image.buffer.plane_pitches();
        let offsets = image.buffer.plane_offsets();
        let planes = std::array::from_fn(|index| {
            plane_fds[index]
                .take()
                .map(|fd| NativeOwnedDmaBufPlane {
                    fd,
                    offset: offsets[index],
                    stride: pitches[index],
                })
        });
        if planes[..usize::from(image.buffer.plane_count())]
            .iter()
            .any(Option::is_none)
        {
            return Err(NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor);
        }
        Ok(Some(NativeRendererImageSnapshot {
            image_id,
            width: image.buffer.width(),
            height: image.buffer.height(),
            format: image.buffer.format(),
            modifier: image.buffer.modifier().unwrap_or(u64::MAX),
            plane_count: image.buffer.plane_count(),
            planes,
        }))
    }

    pub fn restore_promoted_renderer_image(
        &mut self,
        snapshot: NativeRendererImageSnapshot,
    ) -> Result<bool, NativeGbmScanoutBufferExportDetail> {
        let image_id = snapshot.image_id();
        if !self.capture_renderer_image(image_id, snapshot.as_frame())? {
            return Ok(false);
        }
        if !self.promote_renderer_image(image_id)? {
            let _ = self.rollback_renderer_image(image_id);
            return Err(NativeGbmScanoutBufferExportDetail::InvalidRendererImageId);
        }
        Ok(true)
    }

    pub fn rollback_renderer_image(
        &mut self,
        image_id: NativeRendererImageId,
    ) -> Result<bool, NativeGbmScanoutBufferExportDetail> {
        if self
            .renderer_images
            .get(&image_id)
            .is_none_or(|image| image.state != NativeRendererImageState::Staged)
        {
            return Ok(false);
        }
        self.stats.snapshot_rollbacks = self.stats.snapshot_rollbacks.saturating_add(1);
        self.evict_renderer_image(image_id)
    }

    pub fn export_rendered_owned_scanout_buffer(
        &self,
        width: u32,
        height: u32,
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        self.export_rendered_owned_scanout_buffer_with_modifiers(width, height, &[])
    }

    pub fn export_rendered_owned_scanout_buffer_with_modifiers(
        &self,
        width: u32,
        height: u32,
        preferred_modifiers: &[u64],
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if width == 0 || height == 0 {
            return NativeGbmOwnedScanoutBufferExportReport {
                status: NativeGbmScanoutBufferExportStatus::InvalidTarget,
                detail: NativeGbmScanoutBufferExportDetail::InvalidTarget,
                buffer: None,
            };
        }

        match render_initialized_gbm_scanout_front_buffer(
            &self.egl,
            self.display,
            &self.gbm_device,
            width,
            height,
            preferred_modifiers,
            None,
        ) {
            Ok(buffer) => exported_scanout_buffer_report(buffer),
            Err(detail) => failed_scanout_buffer_report(detail),
        }
    }

    pub fn export_xrgb8888_owned_scanout_buffer_with_modifiers(
        &mut self,
        width: u32,
        height: u32,
        stride: u32,
        pixels: &[u8],
        preferred_modifiers: &[u64],
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if width == 0 || height == 0 {
            return NativeGbmOwnedScanoutBufferExportReport {
                status: NativeGbmScanoutBufferExportStatus::InvalidTarget,
                detail: NativeGbmScanoutBufferExportDetail::InvalidTarget,
                buffer: None,
            };
        }

        let expected_stride = width.saturating_mul(4);
        let expected_len = usize::try_from(expected_stride)
            .ok()
            .and_then(|stride| stride.checked_mul(usize::try_from(height).ok()?));
        if stride != expected_stride || expected_len != Some(pixels.len()) {
            return NativeGbmOwnedScanoutBufferExportReport {
                status: NativeGbmScanoutBufferExportStatus::InvalidTarget,
                detail: NativeGbmScanoutBufferExportDetail::InvalidTarget,
                buffer: None,
            };
        }
        let started = Instant::now();
        let result = self
            .write_cpu_xrgb8888_scanout_buffer(width, height, pixels)
            .or_else(|_| {
                self.render_one_shot_xrgb8888_with_recovery(
                    width,
                    height,
                    pixels,
                    preferred_modifiers,
                )
            });
        self.stats.max_upload = self.stats.max_upload.max(started.elapsed());
        match result {
            Ok(buffer) => exported_scanout_buffer_report(buffer),
            Err(detail) => failed_scanout_buffer_report(detail),
        }
    }

    pub fn rewrite_xrgb8888_owned_scanout_buffer_damage(
        &mut self,
        buffer: &mut NativeGbmOwnedScanoutBuffer,
        pixels: &[u8],
        damage: &[NativeCompositionRect],
    ) -> Result<(), NativeGbmScanoutBufferExportDetail> {
        let started = Instant::now();
        let result = buffer.rewrite_xrgb8888_damage(pixels, damage);
        self.stats.max_upload = self.stats.max_upload.max(started.elapsed());
        if result.is_ok() {
            self.stats.frame_uploads = self.stats.frame_uploads.saturating_add(1);
        }
        result
    }

    pub fn export_dmabuf_owned_scanout_buffer_with_modifiers(
        &mut self,
        frame: NativeDmaBufFrame<'_>,
        preferred_modifiers: &[u64],
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if !frame.is_valid() {
            return NativeGbmOwnedScanoutBufferExportReport {
                status: NativeGbmScanoutBufferExportStatus::InvalidTarget,
                detail: NativeGbmScanoutBufferExportDetail::InvalidTarget,
                buffer: None,
            };
        }
        let result = self.render_one_shot_dmabuf_with_recovery(frame, preferred_modifiers);
        match result {
            Ok(buffer) => exported_scanout_buffer_report(buffer),
            Err(detail) => failed_scanout_buffer_report(detail),
        }
    }

    pub fn export_composed_owned_scanout_buffer_with_modifiers(
        &mut self,
        frame: NativeCompositionFrame<'_>,
        preferred_modifiers: &[u64],
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if frame.width == 0
            || frame.height == 0
            || frame.layers.iter().any(|layer| match layer {
                NativeCompositionLayer::Cpu(layer) => {
                    layer.width == 0
                        || layer.height == 0
                        || !matches!(layer.format, 0x3432_5258 | 0x3432_5241)
                        || layer.target.width <= 0
                        || layer.target.height <= 0
                        || !layer.alpha.is_finite()
                }
                NativeCompositionLayer::DmaBuf(layer) => {
                    !layer.frame.is_valid()
                        || layer.target.width <= 0
                        || layer.target.height <= 0
                        || !layer.alpha.is_finite()
                }
                NativeCompositionLayer::RendererImage(layer) => {
                    !layer.image_id.is_valid()
                        || layer.target.width <= 0
                        || layer.target.height <= 0
                        || !layer.alpha.is_finite()
                        || !self.renderer_images.contains_key(&layer.image_id)
                }
                NativeCompositionLayer::Solid(layer) => {
                    layer.target.width <= 0 || layer.target.height <= 0
                }
            })
        {
            return NativeGbmOwnedScanoutBufferExportReport {
                status: NativeGbmScanoutBufferExportStatus::InvalidTarget,
                detail: NativeGbmScanoutBufferExportDetail::InvalidTarget,
                buffer: None,
            };
        }
        match self.render_one_shot_composition_with_recovery(frame, preferred_modifiers) {
            Ok(buffer) => exported_scanout_buffer_report(buffer),
            Err(detail) => failed_scanout_buffer_report(detail),
        }
    }

    pub fn capture_renderer_image(
        &mut self,
        image_id: NativeRendererImageId,
        frame: NativeMultiPlaneDmaBufFrame<'_>,
    ) -> Result<bool, NativeGbmScanoutBufferExportDetail> {
        if !image_id.is_valid() || !frame.is_valid() {
            return Err(NativeGbmScanoutBufferExportDetail::InvalidTarget);
        }
        if self.renderer_images.contains_key(&image_id) {
            return Ok(false);
        }
        let estimated_bytes = u64::from(frame.width)
            .checked_mul(u64::from(frame.height))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(NativeGbmScanoutBufferExportDetail::InvalidTarget)?;
        if self.renderer_images.len() >= DEFAULT_NATIVE_RENDERER_IMAGE_CAPACITY
            || self.renderer_image_bytes.saturating_add(estimated_bytes)
                > DEFAULT_NATIVE_RENDERER_IMAGE_BYTE_BUDGET
        {
            return Err(NativeGbmScanoutBufferExportDetail::RendererImageStoreFull);
        }
        let buffer = self.render_renderer_image_snapshot(image_id, frame)?;
        if buffer.format() != frame.format {
            return Err(NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor);
        }
        let bytes = u64::from(buffer.pitch())
            .checked_mul(u64::from(buffer.height()))
            .ok_or(NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor)?;
        if self.renderer_image_bytes.saturating_add(bytes)
            > DEFAULT_NATIVE_RENDERER_IMAGE_BYTE_BUDGET
        {
            return Err(NativeGbmScanoutBufferExportDetail::RendererImageStoreFull);
        }
        self.renderer_images.insert(
            image_id,
            NativeRendererImage {
                buffer,
                state: NativeRendererImageState::Staged,
                bytes,
            },
        );
        self.renderer_image_bytes = self.renderer_image_bytes.saturating_add(bytes);
        self.stats.snapshot_captures = self.stats.snapshot_captures.saturating_add(1);
        self.update_renderer_image_stats();
        Ok(true)
    }

    fn render_renderer_image_snapshot(
        &mut self,
        image_id: NativeRendererImageId,
        source: NativeMultiPlaneDmaBufFrame<'_>,
    ) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
        let format = match source.format {
            0x3432_5258 => gbm::Format::Xrgb8888,
            0x3432_5241 => gbm::Format::Argb8888,
            _ => return Err(NativeGbmScanoutBufferExportDetail::InvalidTarget),
        };
        self.egl
            .bind_api(khronos_egl::OPENGL_API)
            .map_err(|_| NativeGbmScanoutBufferExportDetail::EglBindApiFailed)?;
        let layer = NativeCompositionLayer::DmaBuf(NativeDmaBufCompositionLayer {
            image_id,
            frame: source,
            target: NativeCompositionRect {
                x: 0,
                y: 0,
                width: i32::try_from(source.width).unwrap_or(i32::MAX),
                height: i32::try_from(source.height).unwrap_or(i32::MAX),
            },
            clip: None,
            alpha: 1.0,
            sampling: crate::NativeCompositionSampling::ExactNearest,
        });
        let layers = [layer];
        let frame = NativeCompositionFrame {
            width: source.width,
            height: source.height,
            layers: &layers,
            trace: None,
        };
        let mut last_detail = NativeGbmScanoutBufferExportDetail::EglConfigUnavailable;
        for candidate in rendered_scanout_candidates(&[])
            .into_iter()
            .filter(|candidate| candidate.format == format)
        {
            let Some(config) = choose_scanout_config_for_format(
                &self.egl,
                self.display,
                candidate.config_attributes,
                candidate.format,
            ) else {
                continue;
            };
            let (mut target, surface, _) = match self.create_render_target(RenderTargetSpec {
                width: source.width,
                height: source.height,
                config,
                candidate,
            }) {
                Ok(created) => created,
                Err(detail) => {
                    last_detail = preferred_scanout_failure_detail(last_detail, detail);
                    continue;
                }
            };
            self.stats.dmabuf_target_creations =
                self.stats.dmabuf_target_creations.saturating_add(1);
            let mut import_cache = NativeDmaBufImportCache::with_capacity_and_stats(
                1,
                NativeDmaBufImportCacheStats::default(),
            );
            let empty_images = std::collections::BTreeMap::new();
            let rendered = render_native_target_composition(
                &self.egl,
                self.display,
                &mut target,
                surface.clone(),
                &mut import_cache,
                &empty_images,
                frame,
                false,
                true,
            );
            let persistent = PersistentCompositionTarget {
                target,
                surface,
                import_cache,
            };
            match rendered {
                Ok((buffer, _)) if is_supported_rendered_scanout_candidate_buffer(&buffer) => {
                    self.destroy_renderer_image_capture_target(persistent);
                    return Ok(buffer);
                }
                Ok(_) => {
                    last_detail = NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor;
                }
                Err(detail) => {
                    last_detail = preferred_scanout_failure_detail(last_detail, detail);
                }
            }
            self.destroy_renderer_image_capture_target(persistent);
        }
        Err(last_detail)
    }

    fn update_renderer_image_stats(&mut self) {
        self.stats.snapshot_live_entries = self.renderer_images.len();
        self.stats.snapshot_live_bytes = self.renderer_image_bytes;
    }

    fn destroy_renderer_image_capture_target(
        &mut self,
        target: PersistentCompositionTarget,
    ) {
        // Capture uses a one-entry temporary import cache for the client
        // source. Do not merge it into the persistent output-import ledger.
        let output_import_stats = self.stats.import_cache;
        self.destroy_persistent_composition_target(target);
        self.stats.import_cache = output_import_stats;
    }

    fn create_render_target(
        &mut self,
        spec: RenderTargetSpec,
    ) -> Result<
        (
            NativeRenderTarget,
            std::sync::Arc<NativeFrameSurface>,
            std::time::Duration,
        ),
        NativeGbmScanoutBufferExportDetail,
    > {
        let started = Instant::now();
        let created = create_native_render_target(
            NativeEglScanoutDevice {
                egl: &self.egl,
                display: self.display,
                gbm_device: &self.gbm_device,
            },
            spec,
        );
        self.stats.max_target_create = self.stats.max_target_create.max(started.elapsed());
        if let Ok((_, _, surface_create_duration)) = &created {
            self.stats.target_creations = self.stats.target_creations.saturating_add(1);
            self.stats.gl_pipeline_creations =
                self.stats.gl_pipeline_creations.saturating_add(1);
            self.stats.frame_surface_creations =
                self.stats.frame_surface_creations.saturating_add(1);
            self.stats.max_frame_surface_create = self
                .stats
                .max_frame_surface_create
                .max(*surface_create_duration);
        }
        created
    }

}
include!("context/render_once.rs");
impl<T> NativeGbmRenderedScanoutContext<T>
where
    T: std::os::fd::AsFd,
{
    fn destroy_native_render_target(&self, target: NativeRenderTarget) {
        trace_native_lifecycle("native_render_target_destroy_started");
        let _ = self.egl.make_current(self.display, None, None, None);
        drop(target.pipeline);
        let _ = self.egl.destroy_context(self.display, target.egl_context);
        trace_native_lifecycle("egl_context_destroyed");
    }

    fn destroy_persistent_composition_target(
        &mut self,
        mut persistent: PersistentCompositionTarget,
    ) {
        let surface = persistent.surface.egl_surface();
        if self
            .egl
            .make_current(
                self.display,
                Some(surface),
                Some(surface),
                Some(persistent.target.egl_context),
            )
            .is_ok()
        {
            let _ = persistent.import_cache.clear(
                &self.egl,
                self.display,
                &persistent.target.pipeline,
            );
            self.stats.import_cache = persistent.import_cache.stats();
        } else {
            persistent.import_cache.abandon(&self.egl, self.display);
            self.stats.import_cache = persistent.import_cache.stats();
        }
        self.stats.sampling = self
            .stats
            .sampling
            .saturating_add(persistent.target.pipeline.sampling_stats());
        self.destroy_native_render_target(persistent.target);
    }
}

impl<T> Drop for NativeGbmRenderedScanoutContext<T>
where
    T: std::os::fd::AsFd,
{
    fn drop(&mut self) {
        if let Some(persistent) = self.composition_target.take() {
            self.destroy_persistent_composition_target(persistent);
        }
        let _ = self.egl.terminate(self.display);
        trace_native_lifecycle("egl_display_terminated");
    }
}
