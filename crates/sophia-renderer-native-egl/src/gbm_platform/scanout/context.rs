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
}

struct NativeRenderTarget {
    width: u32,
    height: u32,
    egl_context: khronos_egl::Context,
    pipeline: PersistentXrgb8888GlPipeline,
}

impl<T> NativeGbmRenderedScanoutContext<T>
where
    T: std::os::fd::AsFd,
{
    pub fn from_backend_device_result(
        device: std::io::Result<T>,
    ) -> NativeGbmRenderedScanoutContextReport<T> {
        match device {
            Ok(device) => match Self::new(device) {
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

    fn new(device: T) -> Result<Self, NativeGbmRenderedScanoutContextStatus> {
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
        })
    }

    pub const fn persistent_render_stats(&self) -> NativeGbmPersistentRenderStats {
        self.stats
    }

    pub const fn composition_pixel_metrics(&self) -> Option<NativeCompositionPixelMetrics> {
        self.last_composition_pixel_metrics
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

    fn render_one_shot_composition_with_recovery(
        &mut self,
        frame: NativeCompositionFrame<'_>,
        preferred_modifiers: &[u64],
    ) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
        let result = self.render_one_shot_composition(frame, preferred_modifiers);
        if result
            .as_ref()
            .is_err_and(|detail| detail.render_target_retryable())
        {
            self.stats.target_recreations = self.stats.target_recreations.saturating_add(1);
            self.stats.recovery_replacements =
                self.stats.recovery_replacements.saturating_add(1);
            self.render_one_shot_composition(frame, preferred_modifiers)
        } else {
            result
        }
    }

    fn render_one_shot_composition(
        &mut self,
        frame: NativeCompositionFrame<'_>,
        preferred_modifiers: &[u64],
    ) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
        let preferred_modifiers = preferred_modifiers
            .iter()
            .copied()
            .filter(|modifier| *modifier != u64::MAX)
            .collect::<Vec<_>>();
        self.egl
            .bind_api(khronos_egl::OPENGL_API)
            .map_err(|_| NativeGbmScanoutBufferExportDetail::EglBindApiFailed)?;
        let reduced = reduced_gbm_scanout_modifiers(&preferred_modifiers);
        let mut last_detail = NativeGbmScanoutBufferExportDetail::EglConfigUnavailable;
        for candidate in rendered_scanout_candidates(&reduced) {
            let Some(config) = choose_scanout_config_for_format(
                &self.egl,
                self.display,
                candidate.config_attributes,
                candidate.format,
            ) else {
                continue;
            };
            let created = self.create_render_target(RenderTargetSpec {
                width: frame.width,
                height: frame.height,
                config,
                candidate,
            });
            let (mut target, surface, _) = match created {
                Ok(created) => created,
                Err(detail) => {
                    last_detail = preferred_scanout_failure_detail(last_detail, detail);
                    continue;
                }
            };
            self.stats.composition_target_creations =
                self.stats.composition_target_creations.saturating_add(1);
            let capture_pixels = self.composition_pixel_proof_attempts < 3;
            let render_started = Instant::now();
            let rendered = render_native_target_composition(
                &self.egl,
                self.display,
                &mut target,
                surface,
                frame,
                capture_pixels,
            );
            self.stats.max_render = self.stats.max_render.max(render_started.elapsed());
            if capture_pixels {
                self.composition_pixel_proof_attempts =
                    self.composition_pixel_proof_attempts.saturating_add(1);
            }
            self.last_composition_pixel_metrics =
                rendered.as_ref().ok().and_then(|(_, metrics)| *metrics);
            match rendered {
                Ok((buffer, _)) if is_supported_rendered_scanout_candidate_buffer(&buffer) => {
                    self.stats.target_recreations =
                        self.stats.target_recreations.saturating_add(1);
                    self.destroy_native_render_target(target);
                    return Ok(buffer);
                }
                Ok(_) => {
                    self.destroy_native_render_target(target);
                    last_detail = NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor;
                }
                Err(detail) => {
                    self.destroy_native_render_target(target);
                    last_detail = preferred_scanout_failure_detail(last_detail, detail);
                }
            }
        }
        Err(last_detail)
    }

    fn render_one_shot_dmabuf_with_recovery(
        &mut self,
        frame: NativeDmaBufFrame<'_>,
        preferred_modifiers: &[u64],
    ) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
        let result = self.render_one_shot_dmabuf(frame, preferred_modifiers);
        if result
            .as_ref()
            .is_err_and(|detail| detail.render_target_retryable())
        {
            self.stats.target_recreations = self.stats.target_recreations.saturating_add(1);
            self.stats.recovery_replacements =
                self.stats.recovery_replacements.saturating_add(1);
            self.render_one_shot_dmabuf(frame, preferred_modifiers)
        } else {
            result
        }
    }

    fn render_one_shot_dmabuf(
        &mut self,
        frame: NativeDmaBufFrame<'_>,
        preferred_modifiers: &[u64],
    ) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
        let preferred_modifiers = preferred_modifiers
            .iter()
            .copied()
            .filter(|modifier| *modifier != u64::MAX)
            .collect::<Vec<_>>();
        self.egl
            .bind_api(khronos_egl::OPENGL_API)
            .map_err(|_| NativeGbmScanoutBufferExportDetail::EglBindApiFailed)?;
        let reduced = reduced_gbm_scanout_modifiers(&preferred_modifiers);
        let mut last_detail = NativeGbmScanoutBufferExportDetail::EglConfigUnavailable;
        for candidate in rendered_scanout_candidates(&reduced) {
            let Some(config) = choose_scanout_config_for_format(
                &self.egl,
                self.display,
                candidate.config_attributes,
                candidate.format,
            ) else {
                continue;
            };
            let created = self.create_render_target(RenderTargetSpec {
                width: frame.width,
                height: frame.height,
                config,
                candidate,
            });
            let (mut target, surface, _) = match created {
                Ok(created) => created,
                Err(detail) => {
                    last_detail = preferred_scanout_failure_detail(last_detail, detail);
                    continue;
                }
            };
            self.stats.dmabuf_target_creations =
                self.stats.dmabuf_target_creations.saturating_add(1);
            let render_started = Instant::now();
            let rendered = render_native_target_dmabuf(
                &self.egl,
                self.display,
                &mut target,
                surface,
                frame,
            );
            self.stats.max_render = self.stats.max_render.max(render_started.elapsed());
            match rendered {
                Ok(buffer) if is_supported_rendered_scanout_candidate_buffer(&buffer) => {
                    self.stats.target_recreations =
                        self.stats.target_recreations.saturating_add(1);
                    self.destroy_native_render_target(target);
                    return Ok(buffer);
                }
                Ok(_) => {
                    self.destroy_native_render_target(target);
                    last_detail = NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor;
                }
                Err(detail) => {
                    self.destroy_native_render_target(target);
                    last_detail = preferred_scanout_failure_detail(last_detail, detail);
                }
            }
        }
        Err(last_detail)
    }

    fn write_cpu_xrgb8888_scanout_buffer(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
        let mut buffer = self
            .gbm_device
            .create_buffer_object::<()>(
                width,
                height,
                gbm::Format::Xrgb8888,
                gbm::BufferObjectFlags::SCANOUT
                    | gbm::BufferObjectFlags::WRITE
                    | gbm::BufferObjectFlags::LINEAR,
            )
            .map_err(|_| NativeGbmScanoutBufferExportDetail::GbmSurfaceUnavailable)?;
        let source_stride = usize::try_from(width)
            .ok()
            .and_then(|width| width.checked_mul(4))
            .ok_or(NativeGbmScanoutBufferExportDetail::InvalidTarget)?;
        let target_stride = usize::try_from(buffer.stride())
            .map_err(|_| NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor)?;
        if target_stride < source_stride {
            return Err(NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor);
        }
        let upload = if target_stride == source_stride {
            pixels.to_vec()
        } else {
            let mut padded = vec![
                0;
                target_stride
                    .checked_mul(usize::try_from(height).unwrap_or(0))
                    .ok_or(NativeGbmScanoutBufferExportDetail::InvalidTarget)?
            ];
            for row in 0..usize::try_from(height).unwrap_or(0) {
                let source = row * source_stride;
                let target = row * target_stride;
                padded[target..target + source_stride]
                    .copy_from_slice(&pixels[source..source + source_stride]);
            }
            padded
        };
        buffer
            .write(&upload)
            .map_err(|_| NativeGbmScanoutBufferExportDetail::CpuLayerUploadFailed)?;
        trace_native_lifecycle("cpu_frame_direct_written");
        self.stats.frame_uploads = self.stats.frame_uploads.saturating_add(1);
        native_owned_scanout_buffer_from_bo(width, height, buffer, None)
    }

    fn render_one_shot_xrgb8888_with_recovery(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u8],
        preferred_modifiers: &[u64],
    ) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
        let result =
            self.render_one_shot_xrgb8888(width, height, pixels, preferred_modifiers);
        if result
            .as_ref()
            .is_err_and(|detail| detail.render_target_retryable())
        {
            self.stats.target_recreations = self.stats.target_recreations.saturating_add(1);
            self.stats.recovery_replacements =
                self.stats.recovery_replacements.saturating_add(1);
            self.render_one_shot_xrgb8888(width, height, pixels, preferred_modifiers)
        } else {
            result
        }
    }

    fn render_one_shot_xrgb8888(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u8],
        preferred_modifiers: &[u64],
    ) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
        let preferred_modifiers = preferred_modifiers
            .iter()
            .copied()
            .filter(|modifier| *modifier != u64::MAX)
            .collect::<Vec<_>>();
        self.egl
            .bind_api(khronos_egl::OPENGL_API)
            .map_err(|_| NativeGbmScanoutBufferExportDetail::EglBindApiFailed)?;
        let reduced = reduced_gbm_scanout_modifiers(&preferred_modifiers);
        let mut last_detail = NativeGbmScanoutBufferExportDetail::EglConfigUnavailable;
        for candidate in rendered_scanout_candidates(&reduced) {
            let Some(config) = choose_scanout_config_for_format(
                &self.egl,
                self.display,
                candidate.config_attributes,
                candidate.format,
            ) else {
                continue;
            };
            let created = self.create_render_target(RenderTargetSpec {
                width,
                height,
                config,
                candidate,
            });
            let (mut target, surface, _) = match created {
                Ok(created) => created,
                Err(detail) => {
                    last_detail = preferred_scanout_failure_detail(last_detail, detail);
                    continue;
                }
            };
            self.stats.cpu_target_creations =
                self.stats.cpu_target_creations.saturating_add(1);
            let render_started = Instant::now();
            let rendered = render_native_target_frame(
                &self.egl,
                self.display,
                &mut target,
                surface,
                pixels,
            );
            self.stats.max_render = self.stats.max_render.max(render_started.elapsed());
            match rendered {
                Ok(buffer) if is_supported_rendered_scanout_candidate_buffer(&buffer) => {
                    self.stats.frame_uploads = self.stats.frame_uploads.saturating_add(1);
                    self.stats.target_recreations =
                        self.stats.target_recreations.saturating_add(1);
                    self.destroy_native_render_target(target);
                    return Ok(buffer);
                }
                Ok(_) => {
                    self.destroy_native_render_target(target);
                    last_detail = NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor;
                }
                Err(detail) => {
                    self.destroy_native_render_target(target);
                    last_detail = preferred_scanout_failure_detail(last_detail, detail);
                }
            }
        }
        Err(last_detail)
    }

    fn destroy_native_render_target(&self, target: NativeRenderTarget) {
        trace_native_lifecycle("native_render_target_destroy_started");
        let _ = self.egl.make_current(self.display, None, None, None);
        drop(target.pipeline);
        let _ = self.egl.destroy_context(self.display, target.egl_context);
        trace_native_lifecycle("egl_context_destroyed");
    }
}

impl<T> Drop for NativeGbmRenderedScanoutContext<T>
where
    T: std::os::fd::AsFd,
{
    fn drop(&mut self) {
        let _ = self.egl.terminate(self.display);
        trace_native_lifecycle("egl_display_terminated");
    }
}
