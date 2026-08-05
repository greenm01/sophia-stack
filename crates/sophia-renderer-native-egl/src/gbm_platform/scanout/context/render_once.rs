impl<T> NativeGbmRenderedScanoutContext<T>
where
    T: std::os::fd::AsFd,
{
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
        if self
            .composition_target
            .as_ref()
            .is_some_and(|persistent| {
                persistent.target.width != frame.width
                    || persistent.target.height != frame.height
            })
            && let Some(persistent) = self.composition_target.take()
        {
            self.stats.target_recreations = self.stats.target_recreations.saturating_add(1);
            self.destroy_persistent_composition_target(persistent);
        }
        if let Some(persistent) = self.composition_target.as_mut() {
            let capture_pixels = self.composition_pixel_proof_attempts < 3;
            let render_started = Instant::now();
            let rendered = render_native_target_composition(
                &self.egl,
                self.display,
                &mut persistent.target,
                persistent.surface.clone(),
                &mut persistent.import_cache,
                &self.renderer_images,
                frame,
                capture_pixels,
                false,
            );
            self.stats.import_cache = persistent.import_cache.stats();
            self.stats.max_render = self.stats.max_render.max(render_started.elapsed());
            let pixel_metrics = rendered.as_ref().ok().and_then(|(_, metrics)| *metrics);
            if capture_pixels {
                self.composition_pixel_proof_attempts =
                    self.composition_pixel_proof_attempts.saturating_add(1);
                if let Some(metrics) = pixel_metrics {
                    self.last_composition_pixel_metrics = Some(metrics);
                    if metrics.nonzero_rgb_pixels > 0 {
                        self.composition_pixel_proof_attempts = 3;
                    }
                }
            }
            match rendered {
                Ok((buffer, _)) if is_supported_rendered_scanout_candidate_buffer(&buffer) => {
                    self.stats.composition_target_reuses =
                        self.stats.composition_target_reuses.saturating_add(1);
                    return Ok(buffer);
                }
                Ok(_) => {
                    last_detail = NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor;
                }
                Err(detail) if detail.import_cache_rejection() => {
                    return Err(detail);
                }
                Err(detail) => {
                    last_detail = preferred_scanout_failure_detail(last_detail, detail);
                }
            }
            let persistent = self
                .composition_target
                .take()
                .expect("persistent composition target checked above");
            self.stats.target_recreations = self.stats.target_recreations.saturating_add(1);
            self.destroy_persistent_composition_target(persistent);
        }
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
            let mut import_cache = NativeDmaBufImportCache::with_capacity_and_stats(
                self.import_cache_capacity,
                self.stats.import_cache,
            );
            let rendered = render_native_target_composition(
                &self.egl,
                self.display,
                &mut target,
                surface.clone(),
                &mut import_cache,
                &self.renderer_images,
                frame,
                capture_pixels,
                false,
            );
            self.stats.import_cache = import_cache.stats();
            self.stats.max_render = self.stats.max_render.max(render_started.elapsed());
            let pixel_metrics = rendered.as_ref().ok().and_then(|(_, metrics)| *metrics);
            if capture_pixels {
                self.composition_pixel_proof_attempts =
                    self.composition_pixel_proof_attempts.saturating_add(1);
                if let Some(metrics) = pixel_metrics {
                    self.last_composition_pixel_metrics = Some(metrics);
                    if metrics.nonzero_rgb_pixels > 0 {
                        self.composition_pixel_proof_attempts = 3;
                    }
                }
            }
            match rendered {
                Ok((buffer, _)) if is_supported_rendered_scanout_candidate_buffer(&buffer) => {
                    self.composition_target = Some(PersistentCompositionTarget {
                        target,
                        surface,
                        import_cache,
                    });
                    return Ok(buffer);
                }
                Ok(_) => {
                    self.destroy_persistent_composition_target(PersistentCompositionTarget {
                        target,
                        surface,
                        import_cache,
                    });
                    last_detail = NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor;
                }
                Err(detail) => {
                    self.destroy_persistent_composition_target(PersistentCompositionTarget {
                        target,
                        surface,
                        import_cache,
                    });
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
            std::borrow::Cow::Borrowed(pixels)
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
            std::borrow::Cow::Owned(padded)
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

}
