struct NativeEglScanoutDevice<'a, T: std::os::fd::AsFd> {
    egl: &'a khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    gbm_device: &'a gbm::Device<T>,
}

struct PersistentTargetSpec {
    width: u32,
    height: u32,
    preferred_modifiers: Vec<u64>,
    config: khronos_egl::Config,
    candidate: RenderedScanoutCandidate,
}

struct RenderedFrontBufferSpec<'a> {
    width: u32,
    height: u32,
    config: khronos_egl::Config,
    surface_format: gbm::Format,
    surface_modifiers: &'a [gbm::Modifier],
    surface_usage: gbm::BufferObjectFlags,
}

fn render_gbm_scanout_front_buffer<T: std::os::fd::AsFd>(
    device: T,
    width: u32,
    height: u32,
    preferred_modifiers: &[u64],
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
    use gbm::AsRaw as _;

    let gbm_device = gbm::Device::new(device)
        .map_err(|_error| NativeGbmScanoutBufferExportDetail::GbmDeviceUnavailable)?;
    let egl = unsafe { khronos_egl::DynamicInstance::<khronos_egl::EGL1_5>::load_required() }
        .map_err(|_error| NativeGbmScanoutBufferExportDetail::EglUnavailable)?;

    let native_display = gbm_device.as_raw() as khronos_egl::NativeDisplayType;
    let display = unsafe {
        egl.get_platform_display(
            EGL_PLATFORM_GBM_KHR,
            native_display,
            &[khronos_egl::ATTRIB_NONE],
        )
    }
    .map_err(|_error| NativeGbmScanoutBufferExportDetail::EglDisplayUnavailable)?;

    egl.initialize(display)
        .map_err(|_error| NativeGbmScanoutBufferExportDetail::EglInitializeFailed)?;
    let result = render_initialized_gbm_scanout_front_buffer(
        &egl,
        display,
        &gbm_device,
        width,
        height,
        preferred_modifiers,
        None,
    );
    let _ = egl.terminate(display);
    result
}

fn render_initialized_gbm_scanout_front_buffer<T: std::os::fd::AsFd>(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    gbm_device: &gbm::Device<T>,
    width: u32,
    height: u32,
    preferred_modifiers: &[u64],
    frame: Option<NativeXrgb8888Frame<'_>>,
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
    egl.bind_api(khronos_egl::OPENGL_API)
        .map_err(|_error| NativeGbmScanoutBufferExportDetail::EglBindApiFailed)?;

    let preferred_modifiers = reduced_gbm_scanout_modifiers(preferred_modifiers);
    let mut last_detail = NativeGbmScanoutBufferExportDetail::EglConfigUnavailable;
    for candidate in rendered_scanout_candidates(&preferred_modifiers) {
        let Some(config) = choose_scanout_config_for_format(
            egl,
            display,
            candidate.config_attributes,
            candidate.format,
        ) else {
            continue;
        };

        match render_initialized_gbm_scanout_front_buffer_with_config(
            NativeEglScanoutDevice {
                egl,
                display,
                gbm_device,
            },
            RenderedFrontBufferSpec {
                width,
                height,
                config,
                surface_format: candidate.format,
                surface_modifiers: &candidate.modifiers,
                surface_usage: candidate.usage,
            },
            frame,
        ) {
            Ok(buffer) if is_supported_rendered_scanout_candidate_buffer(&buffer) => {
                return Ok(buffer);
            }
            Ok(_buffer) => {
                last_detail = preferred_scanout_failure_detail(
                    last_detail,
                    NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor,
                );
            }
            Err(detail) => last_detail = preferred_scanout_failure_detail(last_detail, detail),
        }
    }

    Err(last_detail)
}

fn create_persistent_target<T: std::os::fd::AsFd>(
    device: NativeEglScanoutDevice<'_, T>,
    spec: PersistentTargetSpec,
) -> Result<PersistentNativeFrameTarget, NativeGbmScanoutBufferExportDetail> {
    let NativeEglScanoutDevice {
        egl,
        display,
        gbm_device,
    } = device;
    let PersistentTargetSpec {
        width,
        height,
        preferred_modifiers,
        config,
        candidate,
    } = spec;
    use gbm::AsRaw as _;

    let gbm_surface = create_rendered_scanout_surface(
        gbm_device,
        width,
        height,
        candidate.format,
        &candidate.modifiers,
        candidate.usage,
    )?;
    let native_window = gbm_surface.as_raw() as khronos_egl::NativeWindowType;
    let egl_surface = unsafe { egl.create_window_surface(display, config, native_window, None) }
        .map_err(|_| NativeGbmScanoutBufferExportDetail::EglSurfaceUnavailable)?;
    let egl_context = match egl.create_context(display, config, None, &context_attributes()) {
        Ok(context) => context,
        Err(_) => {
            let _ = egl.destroy_surface(display, egl_surface);
            return Err(NativeGbmScanoutBufferExportDetail::EglContextUnavailable);
        }
    };
    if egl
        .make_current(
            display,
            Some(egl_surface),
            Some(egl_surface),
            Some(egl_context),
        )
        .is_err()
    {
        let _ = egl.destroy_context(display, egl_context);
        let _ = egl.destroy_surface(display, egl_surface);
        return Err(NativeGbmScanoutBufferExportDetail::EglMakeCurrentFailed);
    }
    let loader = |name: &str| {
        egl.get_proc_address(name)
            .map_or(ptr::null(), |proc| proc as *const c_void)
    };
    let gl = unsafe { glow::Context::from_loader_function(loader) };
    let pipeline = match unsafe { PersistentXrgb8888GlPipeline::new(gl, width, height) } {
        Ok(pipeline) => pipeline,
        Err(_) => {
            let _ = egl.make_current(display, None, None, None);
            let _ = egl.destroy_context(display, egl_context);
            let _ = egl.destroy_surface(display, egl_surface);
            return Err(NativeGbmScanoutBufferExportDetail::GlSmokeFailed);
        }
    };
    let _ = egl.make_current(display, None, None, None);
    trace_native_lifecycle("persistent_context_created");
    Ok(PersistentNativeFrameTarget {
        width,
        height,
        preferred_modifiers,
        config,
        candidate,
        egl_context,
        egl_surface,
        gbm_surface,
        pipeline,
    })
}

fn render_persistent_target_frame<T: std::os::fd::AsFd>(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    _gbm_device: &gbm::Device<T>,
    target: &mut PersistentNativeFrameTarget,
    pixels: &[u8],
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
    let egl_surface = target.egl_surface;
    if egl
        .make_current(
            display,
            Some(egl_surface),
            Some(egl_surface),
            Some(target.egl_context),
        )
        .is_err()
    {
        let _ = egl.destroy_surface(display, egl_surface);
        return Err(NativeGbmScanoutBufferExportDetail::EglMakeCurrentFailed);
    }
    trace_native_lifecycle("egl_surface_current");
    let result = target
        .pipeline
        .upload(pixels)
        .map_err(|_| NativeGbmScanoutBufferExportDetail::GlSmokeFailed)
        .and_then(|()| {
            trace_native_lifecycle("cpu_frame_uploaded");
            egl.swap_buffers(display, egl_surface)
                .map_err(|_| NativeGbmScanoutBufferExportDetail::EglSwapBuffersFailed)
        })
        .and_then(|()| {
            trace_native_lifecycle("egl_surface_swapped");
            let buffer = unsafe { target.gbm_surface.lock_front_buffer() }
                .map_err(|_| NativeGbmScanoutBufferExportDetail::FrontBufferLockFailed)?;
            trace_native_lifecycle("scanout_front_buffer_locked");
            native_owned_scanout_buffer_from_bo(target.width, target.height, buffer, None)
        });
    let _ = egl.make_current(display, None, None, None);
    result
}

fn retain_egl_surface_until_scanout_release(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    surface: khronos_egl::Surface,
    result: Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail>,
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
    match result {
        Ok(mut buffer) => {
            let Some(destroy_surface) = egl.get_proc_address("eglDestroySurface") else {
                let _ = egl.destroy_surface(display, surface);
                return Err(NativeGbmScanoutBufferExportDetail::EglSurfaceUnavailable);
            };
            buffer._egl_surface = Some(NativeEglSurfaceOwner {
                destroy_surface: unsafe {
                    std::mem::transmute::<
                        extern "system" fn(),
                        unsafe extern "system" fn(*mut c_void, *mut c_void) -> u32,
                    >(destroy_surface)
                },
                display,
                surface,
            });
            Ok(buffer)
        }
        Err(detail) => {
            let _ = egl.destroy_surface(display, surface);
            Err(detail)
        }
    }
}

fn render_persistent_target_dmabuf<T: std::os::fd::AsFd>(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    gbm_device: &gbm::Device<T>,
    target: &mut PersistentNativeFrameTarget,
    frame: NativeDmaBufFrame<'_>,
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
    use gbm::AsRaw as _;

    const EGL_LINUX_DMA_BUF_EXT: khronos_egl::Enum = 0x3270;
    const EGL_WIDTH: khronos_egl::Attrib = 0x3057;
    const EGL_HEIGHT: khronos_egl::Attrib = 0x3056;
    const EGL_LINUX_DRM_FOURCC_EXT: khronos_egl::Attrib = 0x3271;
    const EGL_DMA_BUF_PLANE0_FD_EXT: khronos_egl::Attrib = 0x3272;
    const EGL_DMA_BUF_PLANE0_OFFSET_EXT: khronos_egl::Attrib = 0x3273;
    const EGL_DMA_BUF_PLANE0_PITCH_EXT: khronos_egl::Attrib = 0x3274;
    const EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT: khronos_egl::Attrib = 0x3443;
    const EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT: khronos_egl::Attrib = 0x3444;

    let gbm_surface = create_rendered_scanout_surface(
        gbm_device,
        target.width,
        target.height,
        target.candidate.format,
        &target.candidate.modifiers,
        target.candidate.usage,
    )?;
    let native_window = gbm_surface.as_raw() as khronos_egl::NativeWindowType;
    let egl_surface =
        unsafe { egl.create_window_surface(display, target.config, native_window, None) }
            .map_err(|_| NativeGbmScanoutBufferExportDetail::EglSurfaceUnavailable)?;
    if egl
        .make_current(
            display,
            Some(egl_surface),
            Some(egl_surface),
            Some(target.egl_context),
        )
        .is_err()
    {
        let _ = egl.destroy_surface(display, egl_surface);
        return Err(NativeGbmScanoutBufferExportDetail::EglMakeCurrentFailed);
    }
    trace_dmabuf_lifecycle("egl_surface_current");

    let mut attributes = vec![
        EGL_WIDTH,
        frame.width as khronos_egl::Attrib,
        EGL_HEIGHT,
        frame.height as khronos_egl::Attrib,
        EGL_LINUX_DRM_FOURCC_EXT,
        frame.format as khronos_egl::Attrib,
        EGL_DMA_BUF_PLANE0_FD_EXT,
        frame.fd.as_raw_fd() as khronos_egl::Attrib,
        EGL_DMA_BUF_PLANE0_OFFSET_EXT,
        frame.offset as khronos_egl::Attrib,
        EGL_DMA_BUF_PLANE0_PITCH_EXT,
        frame.stride as khronos_egl::Attrib,
    ];
    if frame.modifier != u64::MAX {
        attributes.extend_from_slice(&[
            EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT,
            (frame.modifier & u64::from(u32::MAX)) as khronos_egl::Attrib,
            EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT,
            (frame.modifier >> 32) as khronos_egl::Attrib,
        ]);
    }
    attributes.push(khronos_egl::ATTRIB_NONE);
    let no_context = unsafe { khronos_egl::Context::from_ptr(khronos_egl::NO_CONTEXT) };
    let no_buffer = unsafe { khronos_egl::ClientBuffer::from_ptr(ptr::null_mut()) };
    let image = egl
        .create_image(
            display,
            no_context,
            EGL_LINUX_DMA_BUF_EXT,
            no_buffer,
            &attributes,
        )
        .map_err(|_| NativeGbmScanoutBufferExportDetail::DmaBufImportFailed);
    let result = match image {
        Ok(image) => {
            trace_dmabuf_lifecycle("egl_image_created");
            let result = egl
                .get_proc_address("glEGLImageTargetTexture2DOES")
                .ok_or(NativeGbmScanoutBufferExportDetail::DmaBufImportFailed)
                .map(|image_target| unsafe {
                    std::mem::transmute::<
                        extern "system" fn(),
                        unsafe extern "system" fn(u32, *const c_void),
                    >(image_target)
                })
                .and_then(|image_target| {
                    unsafe { target.pipeline.draw_egl_image(image_target, image.as_ptr()) }
                        .map_err(|_| NativeGbmScanoutBufferExportDetail::DmaBufImportFailed)
                })
                .and_then(|()| {
                    trace_dmabuf_lifecycle("egl_image_texture_released");
                    trace_dmabuf_lifecycle("egl_image_rendered");
                    egl.swap_buffers(display, egl_surface)
                        .map_err(|_| NativeGbmScanoutBufferExportDetail::EglSwapBuffersFailed)
                })
                .and_then(|()| {
                    trace_dmabuf_lifecycle("egl_surface_swapped");
                    let buffer = unsafe { gbm_surface.lock_front_buffer() }
                        .map_err(|_| NativeGbmScanoutBufferExportDetail::FrontBufferLockFailed)?;
                    trace_dmabuf_lifecycle("scanout_front_buffer_locked");
                    native_owned_scanout_buffer_from_bo(
                        target.width,
                        target.height,
                        buffer,
                        Some(gbm_surface),
                    )
                });
            let image_destroyed = egl.destroy_image(display, image).is_ok();
            if image_destroyed {
                trace_dmabuf_lifecycle("egl_image_destroyed");
            }
            match result {
                Ok(buffer) if image_destroyed => {
                    trace_dmabuf_lifecycle("scanout_owner_returned");
                    Ok(buffer)
                }
                Ok(_) => Err(NativeGbmScanoutBufferExportDetail::DmaBufImportFailed),
                Err(detail) => Err(detail),
            }
        }
        Err(detail) => Err(detail),
    };
    let _ = egl.make_current(display, None, None, None);
    retain_egl_surface_until_scanout_release(egl, display, egl_surface, result)
}

fn render_persistent_target_composition<T: std::os::fd::AsFd>(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    gbm_device: &gbm::Device<T>,
    target: &mut PersistentNativeFrameTarget,
    frame: NativeCompositionFrame<'_>,
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
    use gbm::AsRaw as _;

    let gbm_surface = create_rendered_scanout_surface(
        gbm_device,
        target.width,
        target.height,
        target.candidate.format,
        &target.candidate.modifiers,
        target.candidate.usage,
    )?;
    let native_window = gbm_surface.as_raw() as khronos_egl::NativeWindowType;
    let egl_surface =
        unsafe { egl.create_window_surface(display, target.config, native_window, None) }
            .map_err(|_| NativeGbmScanoutBufferExportDetail::EglSurfaceUnavailable)?;
    if egl
        .make_current(
            display,
            Some(egl_surface),
            Some(egl_surface),
            Some(target.egl_context),
        )
        .is_err()
    {
        let _ = egl.destroy_surface(display, egl_surface);
        return Err(NativeGbmScanoutBufferExportDetail::EglMakeCurrentFailed);
    }

    trace_native_lifecycle("composition_surface_current");
    target.pipeline.begin_composition();
    trace_native_lifecycle("composition_started");
    static PIXEL_TRACE_CLAIMED: AtomicBool = AtomicBool::new(false);
    let trace_pixels = std::env::var_os("SOPHIA_NATIVE_COMPOSITION_PIXEL_TRACE").is_some()
        && PIXEL_TRACE_CLAIMED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok();
    if trace_pixels {
        tracing::info!(
            "sophia_native_composition_pixels schema=1 status=enabled width={} height={} layers={}",
            frame.width,
            frame.height,
            frame.layers.len(),
        );
    }
    let mut draw_result = Ok(());
    for (layer_index, layer) in frame.layers.iter().enumerate() {
        if draw_result.is_err() {
            break;
        }
        draw_result = match layer {
            NativeCompositionLayer::Cpu(layer) => {
                trace_native_lifecycle("composition_cpu_layer_started");
                let result = target
                    .pipeline
                    .draw_cpu_layer(
                        layer.width,
                        layer.height,
                        layer.stride,
                        layer.pixels,
                        layer.target.into(),
                        layer.clip.map(Into::into),
                        layer.alpha,
                        layer.format == 0x3432_5241,
                    )
                    .map_err(|_| NativeGbmScanoutBufferExportDetail::CpuLayerUploadFailed);
                if result.is_ok() {
                    trace_native_lifecycle("composition_cpu_layer_finished");
                    if trace_pixels {
                        trace_composition_pixels(
                            &target.pipeline,
                            "cpu",
                            layer_index,
                            layer.target,
                            layer.format,
                            u64::MAX,
                            layer.stride,
                        );
                    }
                }
                result
            }
            NativeCompositionLayer::DmaBuf(layer) => {
                trace_native_lifecycle("composition_dmabuf_layer_started");
                let result = draw_composition_dmabuf_layer(egl, display, &target.pipeline, *layer);
                if result.is_ok() {
                    trace_native_lifecycle("composition_dmabuf_layer_finished");
                    if trace_pixels {
                        trace_composition_pixels(
                            &target.pipeline,
                            "dmabuf",
                            layer_index,
                            layer.target,
                            layer.frame.format,
                            layer.frame.modifier,
                            layer.frame.planes[0].map_or(0, |plane| plane.stride),
                        );
                    }
                }
                result
            }
        };
    }
    let result = draw_result
        .and_then(|()| {
            target
                .pipeline
                .finish_composition()
                .map_err(|_| NativeGbmScanoutBufferExportDetail::CompositionFinishFailed)
        })
        .and_then(|()| {
            trace_native_lifecycle("composition_finished");
            egl.swap_buffers(display, egl_surface)
                .map_err(|_| NativeGbmScanoutBufferExportDetail::EglSwapBuffersFailed)
        })
        .and_then(|()| {
            trace_native_lifecycle("composition_surface_swapped");
            let buffer = unsafe { gbm_surface.lock_front_buffer() }
                .map_err(|_| NativeGbmScanoutBufferExportDetail::FrontBufferLockFailed)?;
            trace_native_lifecycle("composition_front_buffer_locked");
            native_owned_scanout_buffer_from_bo(
                target.width,
                target.height,
                buffer,
                Some(gbm_surface),
            )
        });
    let _ = egl.make_current(display, None, None, None);
    retain_egl_surface_until_scanout_release(egl, display, egl_surface, result)
}

fn trace_composition_pixels(
    pipeline: &PersistentXrgb8888GlPipeline,
    stage: &str,
    layer: usize,
    target: NativeCompositionRect,
    format: u32,
    modifier: u64,
    stride: u32,
) {
    match pipeline.read_composition_pixels() {
        Ok(metrics) => tracing::info!(
            "sophia_native_composition_pixels schema=1 status=read stage={stage} layer={layer} target={}x{}_{}_{} format={format:#x} modifier={modifier:#x} stride={stride} pixels={} nonzero_rgb_pixels={} alpha_zero_pixels={} alpha_partial_pixels={} alpha_opaque_pixels={} checksum={}",
            target.width,
            target.height,
            target.x,
            target.y,
            metrics.pixels,
            metrics.nonzero_rgb_pixels,
            metrics.alpha_zero_pixels,
            metrics.alpha_partial_pixels,
            metrics.alpha_opaque_pixels,
            metrics.checksum,
        ),
        Err(_) => tracing::warn!(
            "sophia_native_composition_pixels schema=1 status=unavailable stage={stage} layer={layer} target={}x{}_{}_{} format={format:#x} modifier={modifier:#x} stride={stride}",
            target.width,
            target.height,
            target.x,
            target.y,
        ),
    }
}

fn draw_composition_dmabuf_layer(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    pipeline: &PersistentXrgb8888GlPipeline,
    layer: NativeDmaBufCompositionLayer<'_>,
) -> Result<(), NativeGbmScanoutBufferExportDetail> {
    const EGL_LINUX_DMA_BUF_EXT: khronos_egl::Enum = 0x3270;
    const EGL_WIDTH: khronos_egl::Attrib = 0x3057;
    const EGL_HEIGHT: khronos_egl::Attrib = 0x3056;
    const EGL_LINUX_DRM_FOURCC_EXT: khronos_egl::Attrib = 0x3271;
    const PLANE_ATTRIBUTES: [[khronos_egl::Attrib; 5]; 4] = [
        [0x3272, 0x3273, 0x3274, 0x3443, 0x3444],
        [0x3275, 0x3276, 0x3277, 0x3445, 0x3446],
        [0x3278, 0x3279, 0x327A, 0x3447, 0x3448],
        [0x3440, 0x3441, 0x3442, 0x3449, 0x344A],
    ];

    let mut attributes = vec![
        EGL_WIDTH,
        layer.frame.width as khronos_egl::Attrib,
        EGL_HEIGHT,
        layer.frame.height as khronos_egl::Attrib,
        EGL_LINUX_DRM_FOURCC_EXT,
        layer.frame.format as khronos_egl::Attrib,
    ];
    for index in 0..usize::from(layer.frame.plane_count) {
        let plane = layer.frame.planes[index]
            .ok_or(NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor)?;
        let keys = PLANE_ATTRIBUTES[index];
        attributes.extend_from_slice(&[
            keys[0],
            plane.fd.as_raw_fd() as khronos_egl::Attrib,
            keys[1],
            plane.offset as khronos_egl::Attrib,
            keys[2],
            plane.stride as khronos_egl::Attrib,
        ]);
        if layer.frame.modifier != u64::MAX {
            attributes.extend_from_slice(&[
                keys[3],
                (layer.frame.modifier & u64::from(u32::MAX)) as khronos_egl::Attrib,
                keys[4],
                (layer.frame.modifier >> 32) as khronos_egl::Attrib,
            ]);
        }
    }
    attributes.push(khronos_egl::ATTRIB_NONE);
    let no_context = unsafe { khronos_egl::Context::from_ptr(khronos_egl::NO_CONTEXT) };
    let no_buffer = unsafe { khronos_egl::ClientBuffer::from_ptr(ptr::null_mut()) };
    let image = egl
        .create_image(
            display,
            no_context,
            EGL_LINUX_DMA_BUF_EXT,
            no_buffer,
            &attributes,
        )
        .map_err(|_| NativeGbmScanoutBufferExportDetail::DmaBufImageCreateFailed)?;
    let draw =
        egl.get_proc_address("glEGLImageTargetTexture2DOES")
            .ok_or(NativeGbmScanoutBufferExportDetail::DmaBufImageBindFailed)
            .map(|image_target| unsafe {
                std::mem::transmute::<
                    extern "system" fn(),
                    unsafe extern "system" fn(u32, *const c_void),
                >(image_target)
            })
            .and_then(|image_target| {
                unsafe {
                    pipeline.draw_egl_image_layer(
                        image_target,
                        image.as_ptr(),
                        layer.target.into(),
                        layer.clip.map(Into::into),
                        layer.alpha,
                        layer.frame.format == 0x3432_5241,
                    )
                }
                .map_err(|_| NativeGbmScanoutBufferExportDetail::CompositionDrawFailed)
            });
    let destroyed = egl.destroy_image(display, image).is_ok();
    match (draw, destroyed) {
        (Ok(()), true) => Ok(()),
        (Err(detail), _) => Err(detail),
        (Ok(()), false) => Err(NativeGbmScanoutBufferExportDetail::EglImageDestroyFailed),
    }
}

fn trace_dmabuf_lifecycle(stage: &str) {
    if std::env::var_os("SOPHIA_WAYLAND_DMABUF_DIAGNOSTIC").is_some() {
        tracing::info!("sophia_dmabuf_lifecycle schema=1 stage={stage}");
    }
}

fn trace_native_lifecycle(stage: &str) {
    if std::env::var_os("SOPHIA_LIVE_SESSION_DIAGNOSTIC").is_some() {
        tracing::info!("sophia_native_lifecycle schema=1 stage={stage}");
    }
}

fn render_initialized_gbm_scanout_front_buffer_with_config<T: std::os::fd::AsFd>(
    device: NativeEglScanoutDevice<'_, T>,
    spec: RenderedFrontBufferSpec<'_>,
    frame: Option<NativeXrgb8888Frame<'_>>,
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
    let NativeEglScanoutDevice {
        egl,
        display,
        gbm_device,
    } = device;
    let RenderedFrontBufferSpec {
        width,
        height,
        config,
        surface_format,
        surface_modifiers,
        surface_usage,
    } = spec;
    use gbm::AsRaw as _;

    let gbm_surface = create_rendered_scanout_surface(
        gbm_device,
        width,
        height,
        surface_format,
        surface_modifiers,
        surface_usage,
    )?;
    let native_window = gbm_surface.as_raw() as khronos_egl::NativeWindowType;
    let surface = unsafe { egl.create_window_surface(display, config, native_window, None) }
        .map_err(|_error| NativeGbmScanoutBufferExportDetail::EglSurfaceUnavailable)?;
    let context = match egl.create_context(display, config, None, &context_attributes()) {
        Ok(context) => context,
        Err(_error) => {
            let _ = egl.destroy_surface(display, surface);
            return Err(NativeGbmScanoutBufferExportDetail::EglContextUnavailable);
        }
    };

    let result = egl
        .make_current(display, Some(surface), Some(surface), Some(context))
        .map_err(|_error| NativeGbmScanoutBufferExportDetail::EglMakeCurrentFailed)
        .and_then(|()| {
            let loader = |name: &str| {
                egl.get_proc_address(name)
                    .map_or(ptr::null(), |proc| proc as *const c_void)
            };
            match frame {
                Some(frame) => draw_xrgb8888_current_gl_context_with_loader(
                    loader,
                    width,
                    height,
                    frame.stride,
                    frame.pixels,
                ),
                None => smoke_current_gl_context_with_loader(loader),
            }
            .map_err(|_error| NativeGbmScanoutBufferExportDetail::GlSmokeFailed)
        })
        .and_then(|()| {
            egl.swap_buffers(display, surface)
                .map_err(|_error| NativeGbmScanoutBufferExportDetail::EglSwapBuffersFailed)
        })
        .and_then(|()| {
            // `gbm` releases this lock when the returned BufferObject is
            // dropped. The owner retains the surface so the release callback
            // remains valid until KMS scanout has retired the buffer.
            let buffer = unsafe { gbm_surface.lock_front_buffer() }
                .map_err(|_error| NativeGbmScanoutBufferExportDetail::FrontBufferLockFailed)?;
            native_owned_scanout_buffer_from_bo(width, height, buffer, Some(gbm_surface))
        });
    let _ = egl.make_current(display, None, None, None);
    let _ = egl.destroy_context(display, context);
    let _ = egl.destroy_surface(display, surface);

    result
}
