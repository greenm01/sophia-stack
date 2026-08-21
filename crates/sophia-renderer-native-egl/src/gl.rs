use glow::HasContext;
use std::{
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[cfg(feature = "gbm-platform")]
mod sampling_program;
#[cfg(feature = "gbm-platform")]
mod shaders;

#[cfg(feature = "gbm-platform")]
use sampling_program::{CompositionProgram, composition_draw_plan, sampling_evidence_index};
#[cfg(feature = "gbm-platform")]
use shaders::{
    FRAGMENT_SHADER, SHARP_RECONSTRUCTION_FRAGMENT_SHADER, VERTEX_SHADER, compile_program,
    compile_shader,
};

use crate::NativeEglDrawSmokeStatus;
#[cfg(feature = "gbm-platform")]
use crate::{
    NativeCompositionAlphaMode, NativeCompositionPixelMetrics, NativeCompositionSampling,
    NativeCompositionSamplingStats, NativeGbmScanoutBufferExportDetail,
    native_composition_gl_read_y, native_composition_pixel_metrics, native_composition_sampling,
};
#[cfg(feature = "gbm-platform")]
use std::cell::Cell;

#[cfg(feature = "gbm-platform")]
const FULLSCREEN_QUAD_VERTICES: [f32; 16] = [
    -1.0, -1.0, 0.0, 1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0,
];

#[cfg(feature = "gbm-platform")]
pub(crate) struct PersistentXrgb8888GlPipeline {
    gl: glow::Context,
    program: glow::NativeProgram,
    reconstruction_program: Option<glow::NativeProgram>,
    texture: glow::NativeTexture,
    cpu_layer_texture: glow::NativeTexture,
    cpu_layer_texture_width: Cell<u32>,
    cpu_layer_texture_height: Cell<u32>,
    vertex_buffer: glow::NativeBuffer,
    sampling_evidence: Cell<u16>,
    exact_nearest_draws: Cell<usize>,
    sharp_downscale_draws: Cell<usize>,
    sharp_upscale_draws: Cell<usize>,
    linear_fallback_draws: Cell<usize>,
    width: u32,
    height: u32,
}

#[cfg(feature = "gbm-platform")]
#[derive(Clone, Copy, Debug)]
pub(crate) struct GlCompositionRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[cfg(feature = "gbm-platform")]
pub(crate) struct GlCpuLayer<'a> {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixels: &'a [u8],
    pub alpha: f32,
    pub alpha_mode: NativeCompositionAlphaMode,
}

#[cfg(feature = "gbm-platform")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeCpuTextureUpload {
    Reallocate,
    Update,
}

#[cfg(feature = "gbm-platform")]
pub const fn native_cpu_texture_upload(
    allocated_width: u32,
    allocated_height: u32,
    layer_width: u32,
    layer_height: u32,
) -> NativeCpuTextureUpload {
    if allocated_width == layer_width && allocated_height == layer_height {
        NativeCpuTextureUpload::Update
    } else {
        NativeCpuTextureUpload::Reallocate
    }
}

#[cfg(feature = "gbm-platform")]
impl PersistentXrgb8888GlPipeline {
    pub(crate) unsafe fn new(
        gl: glow::Context,
        width: u32,
        height: u32,
    ) -> Result<Self, NativeEglDrawSmokeStatus> {
        let result = unsafe { Self::new_inner(gl, width, height) };
        result.map_err(|_| NativeEglDrawSmokeStatus::GlUnavailable)
    }

    unsafe fn new_inner(gl: glow::Context, width: u32, height: u32) -> Result<Self, String> {
        let program = unsafe { compile_program(&gl, FRAGMENT_SHADER)? };
        let reconstruction_program = match unsafe {
            compile_program(&gl, SHARP_RECONSTRUCTION_FRAGMENT_SHADER)
        } {
            Ok(program) => Some(program),
            Err(error) => {
                tracing::warn!(
                    "sophia_native_composition_sampling schema=2 status=unavailable mode=sharp_reconstruction reason=shader_compile error={error}"
                );
                None
            }
        };
        let texture = unsafe { gl.create_texture()? };
        let cpu_layer_texture = unsafe { gl.create_texture()? };
        let vertex_buffer = unsafe { gl.create_buffer()? };
        let vertex_bytes = unsafe {
            std::slice::from_raw_parts(
                FULLSCREEN_QUAD_VERTICES.as_ptr().cast::<u8>(),
                FULLSCREEN_QUAD_VERTICES.len() * std::mem::size_of::<f32>(),
            )
        };
        unsafe {
            gl.viewport(0, 0, width as i32, height as i32);
            gl.disable(glow::BLEND);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                width as i32,
                height as i32,
                0,
                glow::BGRA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.bind_texture(glow::TEXTURE_2D, Some(cpu_layer_texture));
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                width as i32,
                height as i32,
                0,
                glow::BGRA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, vertex_bytes, glow::STATIC_DRAW);
        }
        Ok(Self {
            gl,
            program,
            reconstruction_program,
            texture,
            cpu_layer_texture,
            cpu_layer_texture_width: Cell::new(width),
            cpu_layer_texture_height: Cell::new(height),
            vertex_buffer,
            sampling_evidence: Cell::new(0),
            exact_nearest_draws: Cell::new(0),
            sharp_downscale_draws: Cell::new(0),
            sharp_upscale_draws: Cell::new(0),
            linear_fallback_draws: Cell::new(0),
            width,
            height,
        })
    }

    pub(crate) fn upload(&self, pixels: &[u8]) -> Result<(), NativeEglDrawSmokeStatus> {
        let expected = usize::try_from(self.width)
            .ok()
            .and_then(|width| width.checked_mul(4))
            .and_then(|stride| stride.checked_mul(usize::try_from(self.height).ok()?))
            .ok_or(NativeEglDrawSmokeStatus::GlUnavailable)?;
        if pixels.len() != expected {
            return Err(NativeEglDrawSmokeStatus::GlUnavailable);
        }
        unsafe {
            self.gl
                .viewport(0, 0, self.width as i32, self.height as i32);
            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            self.gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                0,
                0,
                self.width as i32,
                self.height as i32,
                glow::BGRA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(pixels)),
            );
        }
        self.draw_fullscreen_bound_texture()
    }

    fn draw_fullscreen_bound_texture(&self) -> Result<(), NativeEglDrawSmokeStatus> {
        let vertex_bytes = unsafe {
            std::slice::from_raw_parts(
                FULLSCREEN_QUAD_VERTICES.as_ptr().cast::<u8>(),
                FULLSCREEN_QUAD_VERTICES.len() * std::mem::size_of::<f32>(),
            )
        };
        unsafe {
            self.gl.disable(glow::BLEND);
            self.gl.disable(glow::SCISSOR_TEST);
            self.gl
                .bind_buffer(glow::ARRAY_BUFFER, Some(self.vertex_buffer));
            self.gl
                .buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, 0, vertex_bytes);
            self.gl.use_program(Some(self.program));
            self.gl.uniform_1_i32(
                self.gl.get_uniform_location(self.program, "frame").as_ref(),
                0,
            );
            self.gl.uniform_1_f32(
                self.gl
                    .get_uniform_location(self.program, "opacity")
                    .as_ref(),
                1.0,
            );
            self.gl.enable_vertex_attrib_array(0);
            self.gl
                .vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);
            self.gl.enable_vertex_attrib_array(1);
            self.gl
                .vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);
            self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
            if self.gl.get_error() != glow::NO_ERROR {
                return Err(NativeEglDrawSmokeStatus::GlUnavailable);
            }
        }
        Ok(())
    }

    pub(crate) fn begin_composition(&self) {
        self.begin_composition_with_clear_alpha(1.0);
    }

    pub(crate) fn begin_composition_with_clear_alpha(&self, alpha: f32) {
        unsafe {
            self.gl
                .viewport(0, 0, self.width as i32, self.height as i32);
            self.gl.disable(glow::BLEND);
            self.gl.disable(glow::SCISSOR_TEST);
            self.gl.clear_color(0.0, 0.0, 0.0, alpha);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }
    }

    pub(crate) fn draw_cpu_layer(
        &self,
        layer: GlCpuLayer<'_>,
        target: GlCompositionRect,
        clip: Option<GlCompositionRect>,
    ) -> Result<(), NativeEglDrawSmokeStatus> {
        let GlCpuLayer {
            width,
            height,
            stride,
            pixels,
            alpha,
            alpha_mode,
        } = layer;
        let expected_stride = width
            .checked_mul(4)
            .ok_or(NativeEglDrawSmokeStatus::GlUnavailable)?;
        let expected = usize::try_from(stride)
            .ok()
            .and_then(|stride| stride.checked_mul(usize::try_from(height).ok()?))
            .ok_or(NativeEglDrawSmokeStatus::GlUnavailable)?;
        if width == 0 || height == 0 || stride != expected_stride || pixels.len() != expected {
            return Err(NativeEglDrawSmokeStatus::GlUnavailable);
        }
        let upload = native_cpu_texture_upload(
            self.cpu_layer_texture_width.get(),
            self.cpu_layer_texture_height.get(),
            width,
            height,
        );
        // Per draw, not once at creation: filter state lives on the texture object
        // and this one texture is reused for layers at different scales. The
        // filter comes from the same resolution that picks the program, so a
        // reconstructed draw is guaranteed the unfiltered texels its kernel
        // gathers rather than a hardware blend of them.
        let sampling = sampling_for_rect((width, height), target);
        let filter = self.draw_plan(sampling).texture_filter;
        unsafe {
            self.gl.active_texture(glow::TEXTURE0);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.cpu_layer_texture));
            self.gl
                .tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, filter as i32);
            self.gl
                .tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, filter as i32);
            match upload {
                NativeCpuTextureUpload::Update => self.gl.tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    0,
                    0,
                    width as i32,
                    height as i32,
                    glow::BGRA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(pixels)),
                ),
                NativeCpuTextureUpload::Reallocate => self.gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA as i32,
                    width as i32,
                    height as i32,
                    0,
                    glow::BGRA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(pixels)),
                ),
            }
            if upload == NativeCpuTextureUpload::Reallocate {
                self.cpu_layer_texture_width.set(width);
                self.cpu_layer_texture_height.set(height);
            }
        }
        self.draw_bound_texture(target, clip, alpha, alpha_mode, (width, height), sampling)
    }

    pub(crate) fn draw_solid_layer(
        &self,
        target: GlCompositionRect,
        color: [u8; 3],
    ) -> Result<(), NativeEglDrawSmokeStatus> {
        let left = i64::from(target.x).max(0);
        let top = i64::from(target.y).max(0);
        let right = i64::from(target.x)
            .saturating_add(i64::from(target.width))
            .min(i64::from(self.width));
        let bottom = i64::from(target.y)
            .saturating_add(i64::from(target.height))
            .min(i64::from(self.height));
        if left >= right || top >= bottom {
            return Ok(());
        }
        let x = i32::try_from(left).map_err(|_| NativeEglDrawSmokeStatus::GlUnavailable)?;
        let y = i32::try_from(i64::from(self.height).saturating_sub(bottom))
            .map_err(|_| NativeEglDrawSmokeStatus::GlUnavailable)?;
        let width = i32::try_from(right.saturating_sub(left))
            .map_err(|_| NativeEglDrawSmokeStatus::GlUnavailable)?;
        let height = i32::try_from(bottom.saturating_sub(top))
            .map_err(|_| NativeEglDrawSmokeStatus::GlUnavailable)?;
        unsafe {
            self.gl.disable(glow::BLEND);
            self.gl.enable(glow::SCISSOR_TEST);
            self.gl.scissor(x, y, width, height);
            self.gl.clear_color(
                f32::from(color[0]) / 255.0,
                f32::from(color[1]) / 255.0,
                f32::from(color[2]) / 255.0,
                1.0,
            );
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            if self.gl.get_error() != glow::NO_ERROR {
                return Err(NativeEglDrawSmokeStatus::GlUnavailable);
            }
        }
        Ok(())
    }

    pub(crate) unsafe fn draw_egl_image(
        &self,
        image_target: unsafe extern "system" fn(u32, *const c_void),
        image: *const c_void,
    ) -> Result<(), NativeEglDrawSmokeStatus> {
        let image_texture = unsafe {
            self.gl
                .create_texture()
                .map_err(|_| NativeEglDrawSmokeStatus::GlUnavailable)?
        };
        unsafe {
            self.gl
                .viewport(0, 0, self.width as i32, self.height as i32);
            self.gl.active_texture(glow::TEXTURE0);
            // An EGLImage must not be rebound into the persistent texture used
            // by the CPU upload path. Keep the imported sibling local to this
            // frame, finish its draw, and delete the texture before the caller
            // destroys the EGLImage.
            self.gl.bind_texture(glow::TEXTURE_2D, Some(image_texture));
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            image_target(glow::TEXTURE_2D, image);
        }
        self.begin_composition();
        let draw = self.draw_bound_texture(
            GlCompositionRect {
                x: 0,
                y: 0,
                width: self.width as i32,
                height: self.height as i32,
            },
            None,
            1.0,
            NativeCompositionAlphaMode::Opaque,
            (self.width, self.height),
            NativeCompositionSampling::ExactNearest,
        );
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, None);
            self.gl.delete_texture(image_texture);
        }
        draw?;
        self.validate_composition()
    }

    pub(crate) unsafe fn create_egl_image_texture(
        &self,
        egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
        image: *const c_void,
    ) -> Result<glow::NativeTexture, NativeGbmScanoutBufferExportDetail> {
        let image_target = egl
            .get_proc_address("glEGLImageTargetTexture2DOES")
            .ok_or(NativeGbmScanoutBufferExportDetail::DmaBufImageBindFailed)
            .map(|image_target| unsafe {
                std::mem::transmute::<
                    extern "system" fn(),
                    unsafe extern "system" fn(u32, *const c_void),
                >(image_target)
            })?;
        let texture = unsafe {
            self.gl
                .create_texture()
                .map_err(|_| NativeGbmScanoutBufferExportDetail::DmaBufImageBindFailed)?
        };
        unsafe {
            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            for (parameter, value) in [
                (glow::TEXTURE_MIN_FILTER, glow::NEAREST),
                (glow::TEXTURE_MAG_FILTER, glow::NEAREST),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                self.gl
                    .tex_parameter_i32(glow::TEXTURE_2D, parameter, value as i32);
            }
            image_target(glow::TEXTURE_2D, image);
            if self.gl.get_error() != glow::NO_ERROR {
                self.gl.delete_texture(texture);
                return Err(NativeGbmScanoutBufferExportDetail::DmaBufImageBindFailed);
            }
        }
        Ok(texture)
    }

    pub(crate) fn draw_texture_layer(
        &self,
        texture: glow::NativeTexture,
        source: (u32, u32),
        target: GlCompositionRect,
        clip: Option<GlCompositionRect>,
        alpha: f32,
        alpha_mode: NativeCompositionAlphaMode,
    ) -> Result<(), NativeEglDrawSmokeStatus> {
        // Same reasoning as the CPU layer: the filter and the program are one
        // decision, so the shader never samples texels the hardware already
        // blended in gamma-encoded space.
        let sampling = sampling_for_rect(source, target);
        let filter = self.draw_plan(sampling).texture_filter;
        unsafe {
            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            self.gl
                .tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, filter as i32);
            self.gl
                .tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, filter as i32);
        }
        self.draw_bound_texture(target, clip, alpha, alpha_mode, source, sampling)
    }

    pub(crate) unsafe fn delete_texture(&self, texture: glow::NativeTexture) {
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, None);
            self.gl.delete_texture(texture);
        }
    }

    pub(crate) fn validate_composition(&self) -> Result<(), NativeEglDrawSmokeStatus> {
        unsafe {
            self.gl.disable(glow::BLEND);
            self.gl.disable(glow::SCISSOR_TEST);
            if self.gl.get_error() != glow::NO_ERROR {
                return Err(NativeEglDrawSmokeStatus::GlUnavailable);
            }
        }
        Ok(())
    }

    /// How this pipeline will serve a requested sampling, given what compiled.
    fn draw_plan(
        &self,
        requested: NativeCompositionSampling,
    ) -> sampling_program::CompositionDrawPlan {
        composition_draw_plan(requested, self.reconstruction_program.is_some())
    }

    pub(crate) fn sampling_stats(&self) -> NativeCompositionSamplingStats {
        NativeCompositionSamplingStats {
            exact_nearest_draws: self.exact_nearest_draws.get(),
            sharp_downscale_draws: self.sharp_downscale_draws.get(),
            sharp_upscale_draws: self.sharp_upscale_draws.get(),
            linear_fallback_draws: self.linear_fallback_draws.get(),
        }
    }

    pub(crate) fn read_composition_pixels(
        &self,
    ) -> Result<NativeCompositionPixelMetrics, NativeEglDrawSmokeStatus> {
        self.read_composition_region_pixels(GlCompositionRect {
            x: 0,
            y: 0,
            width: self.width as i32,
            height: self.height as i32,
        })
    }

    pub(crate) fn read_composition_region_pixels(
        &self,
        region: GlCompositionRect,
    ) -> Result<NativeCompositionPixelMetrics, NativeEglDrawSmokeStatus> {
        let width = u32::try_from(region.width)
            .ok()
            .filter(|width| *width > 0)
            .ok_or(NativeEglDrawSmokeStatus::GlUnavailable)?;
        let height = u32::try_from(region.height)
            .ok()
            .filter(|height| *height > 0)
            .ok_or(NativeEglDrawSmokeStatus::GlUnavailable)?;
        u32::try_from(region.x)
            .ok()
            .and_then(|x| x.checked_add(width))
            .filter(|right| *right <= self.width)
            .ok_or(NativeEglDrawSmokeStatus::GlUnavailable)?;
        let top = u32::try_from(region.y)
            .ok()
            .ok_or(NativeEglDrawSmokeStatus::GlUnavailable)?;
        let read_y = native_composition_gl_read_y(self.height, top, height)
            .ok_or(NativeEglDrawSmokeStatus::GlUnavailable)?;
        let byte_len = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(NativeEglDrawSmokeStatus::GlUnavailable)?;
        let mut rgba = vec![0; byte_len];
        unsafe {
            self.gl.read_pixels(
                region.x,
                read_y as i32,
                width as i32,
                height as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut rgba)),
            );
            if self.gl.get_error() != glow::NO_ERROR {
                return Err(NativeEglDrawSmokeStatus::GlUnavailable);
            }
        }
        Ok(native_composition_pixel_metrics(&rgba))
    }

    fn draw_bound_texture(
        &self,
        target: GlCompositionRect,
        clip: Option<GlCompositionRect>,
        alpha: f32,
        alpha_mode: NativeCompositionAlphaMode,
        source: (u32, u32),
        requested_sampling: NativeCompositionSampling,
    ) -> Result<(), NativeEglDrawSmokeStatus> {
        if target.width <= 0 || target.height <= 0 || !alpha.is_finite() || alpha <= 0.0 {
            return Ok(());
        }
        let left = -1.0 + 2.0 * target.x as f32 / self.width as f32;
        let right = -1.0 + 2.0 * (target.x + target.width) as f32 / self.width as f32;
        let top = 1.0 - 2.0 * target.y as f32 / self.height as f32;
        let bottom = 1.0 - 2.0 * (target.y + target.height) as f32 / self.height as f32;
        let vertices: [f32; 16] = [
            left, bottom, 0.0, 1.0, right, bottom, 1.0, 1.0, left, top, 0.0, 0.0, right, top, 1.0,
            0.0,
        ];
        let vertex_bytes = unsafe {
            std::slice::from_raw_parts(
                vertices.as_ptr().cast::<u8>(),
                vertices.len() * std::mem::size_of::<f32>(),
            )
        };
        let plan = self.draw_plan(requested_sampling);
        let program = match plan.program {
            CompositionProgram::Direct => self.program,
            // `draw_plan` only names the reconstruction program when it exists,
            // so the fallback here is unreachable rather than a second policy.
            CompositionProgram::Reconstruction => {
                self.reconstruction_program.unwrap_or(self.program)
            }
        };
        let effective_sampling = plan.effective;
        self.record_sampling(
            requested_sampling,
            effective_sampling,
            plan.status,
            source,
            target,
            alpha_mode,
        );
        unsafe {
            if alpha_mode.is_premultiplied() || alpha < 1.0 {
                self.gl.enable(glow::BLEND);
                self.gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
            } else {
                self.gl.disable(glow::BLEND);
            }
            if let Some(clip) = clip {
                self.gl.enable(glow::SCISSOR_TEST);
                self.gl.scissor(
                    clip.x,
                    self.height as i32 - clip.y - clip.height,
                    clip.width.max(0),
                    clip.height.max(0),
                );
            } else {
                self.gl.disable(glow::SCISSOR_TEST);
            }
            self.gl
                .bind_buffer(glow::ARRAY_BUFFER, Some(self.vertex_buffer));
            self.gl
                .buffer_data_u8_slice(glow::ARRAY_BUFFER, vertex_bytes, glow::STREAM_DRAW);
            self.gl.use_program(Some(program));
            self.gl
                .uniform_1_i32(self.gl.get_uniform_location(program, "frame").as_ref(), 0);
            self.gl.uniform_1_f32(
                self.gl.get_uniform_location(program, "opacity").as_ref(),
                alpha.clamp(0.0, 1.0),
            );
            self.gl.uniform_1_f32(
                self.gl
                    .get_uniform_location(program, "source_is_opaque")
                    .as_ref(),
                if alpha_mode == NativeCompositionAlphaMode::Opaque {
                    1.0
                } else {
                    0.0
                },
            );
            // Both directions now, not the reduction alone: the kernel derives
            // every tap coordinate from this, so an enlargement left without it
            // would gather from wherever the uniform happened to be.
            if plan.program == CompositionProgram::Reconstruction {
                self.gl.uniform_2_f32(
                    self.gl
                        .get_uniform_location(program, "source_size")
                        .as_ref(),
                    source.0 as f32,
                    source.1 as f32,
                );
            }
            self.gl.enable_vertex_attrib_array(0);
            self.gl
                .vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);
            self.gl.enable_vertex_attrib_array(1);
            self.gl
                .vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);
            self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
            if self.gl.get_error() != glow::NO_ERROR {
                return Err(NativeEglDrawSmokeStatus::GlUnavailable);
            }
        }
        Ok(())
    }

    fn record_sampling(
        &self,
        requested: NativeCompositionSampling,
        effective: NativeCompositionSampling,
        status: &'static str,
        source: (u32, u32),
        target: GlCompositionRect,
        alpha_mode: NativeCompositionAlphaMode,
    ) {
        // One counter per outcome, incremented once. The fallback used to be
        // tallied twice -- as an upscale and as a fallback -- which left the
        // upscale count meaning either of two things.
        let counter = match effective {
            NativeCompositionSampling::ExactNearest => &self.exact_nearest_draws,
            NativeCompositionSampling::SharpDownscale => &self.sharp_downscale_draws,
            NativeCompositionSampling::SharpUpscale => &self.sharp_upscale_draws,
            NativeCompositionSampling::LinearFallback => &self.linear_fallback_draws,
        };
        counter.set(counter.get().saturating_add(1));
        let evidence_index = sampling_evidence_index(
            requested,
            status == "fallback",
            alpha_mode == NativeCompositionAlphaMode::Premultiplied,
        );
        let bit = 1u16 << evidence_index;
        if self.sampling_evidence.get() & bit != 0 {
            return;
        }
        self.sampling_evidence
            .set(self.sampling_evidence.get() | bit);
        tracing::info!(
            "sophia_native_composition_sampling schema=2 status={status} requested={} effective={} alpha_mode={} source={}x{} target={}x{} output={}x{}",
            requested.reduced_name(),
            effective.reduced_name(),
            alpha_mode.reduced_name(),
            source.0,
            source.1,
            target.width,
            target.height,
            self.width,
            self.height,
        );
    }
}

#[cfg(feature = "gbm-platform")]
fn sampling_for_rect(source: (u32, u32), target: GlCompositionRect) -> NativeCompositionSampling {
    native_composition_sampling(
        source,
        (
            u32::try_from(target.width).unwrap_or(0),
            u32::try_from(target.height).unwrap_or(0),
        ),
    )
}

pub(crate) fn smoke_current_gl_context_with_loader<F>(
    mut loader: F,
) -> Result<(), NativeEglDrawSmokeStatus>
where
    F: FnMut(&str) -> *const c_void,
{
    let result = catch_unwind(AssertUnwindSafe(|| {
        // GL function pointers are loaded only after the EGL context is current
        // and never escape this adapter.
        let gl = unsafe { glow::Context::from_loader_function(|name| loader(name)) };

        unsafe {
            gl.clear_color(0.02, 0.03, 0.05, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.finish();
            gl.get_error()
        }
    }))
    .map_err(|_panic| NativeEglDrawSmokeStatus::GlUnavailable)?;

    if result == glow::NO_ERROR {
        Ok(())
    } else {
        Err(NativeEglDrawSmokeStatus::GlUnavailable)
    }
}

#[cfg(feature = "gbm-platform")]
pub(crate) fn draw_xrgb8888_current_gl_context_with_loader<F>(
    mut loader: F,
    width: u32,
    height: u32,
    stride: u32,
    pixels: &[u8],
) -> Result<(), NativeEglDrawSmokeStatus>
where
    F: FnMut(&str) -> *const c_void,
{
    let expected_stride = width
        .checked_mul(4)
        .ok_or(NativeEglDrawSmokeStatus::GlUnavailable)?;
    let expected_len = usize::try_from(expected_stride)
        .ok()
        .and_then(|stride| stride.checked_mul(usize::try_from(height).ok()?))
        .ok_or(NativeEglDrawSmokeStatus::GlUnavailable)?;
    if width == 0 || height == 0 || stride != expected_stride || pixels.len() != expected_len {
        return Err(NativeEglDrawSmokeStatus::GlUnavailable);
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let gl = unsafe { glow::Context::from_loader_function(|name| loader(name)) };
        unsafe { draw_xrgb8888_frame(&gl, width, height, pixels) }
    }))
    .map_err(|_panic| NativeEglDrawSmokeStatus::GlUnavailable)?;

    result.map_err(|_error| NativeEglDrawSmokeStatus::GlUnavailable)
}

#[cfg(feature = "gbm-platform")]
unsafe fn draw_xrgb8888_frame(
    gl: &glow::Context,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> Result<(), String> {
    let vertex_shader = unsafe { compile_shader(gl, glow::VERTEX_SHADER, VERTEX_SHADER)? };
    let fragment_shader =
        match unsafe { compile_shader(gl, glow::FRAGMENT_SHADER, FRAGMENT_SHADER) } {
            Ok(shader) => shader,
            Err(error) => {
                unsafe { gl.delete_shader(vertex_shader) };
                return Err(error);
            }
        };
    let program = unsafe { gl.create_program()? };
    unsafe {
        gl.attach_shader(program, vertex_shader);
        gl.attach_shader(program, fragment_shader);
        gl.bind_attrib_location(program, 0, "position");
        gl.bind_attrib_location(program, 1, "texture_coordinate");
        gl.link_program(program);
    }
    if !unsafe { gl.get_program_link_status(program) } {
        let error = unsafe { gl.get_program_info_log(program) };
        unsafe {
            gl.delete_program(program);
            gl.delete_shader(vertex_shader);
            gl.delete_shader(fragment_shader);
        }
        return Err(error);
    }

    let texture = unsafe { gl.create_texture()? };
    let vertex_buffer = unsafe { gl.create_buffer()? };
    let vertices: [f32; 16] = [
        -1.0, -1.0, 0.0, 1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0,
    ];
    let vertex_bytes = unsafe {
        std::slice::from_raw_parts(
            vertices.as_ptr().cast::<u8>(),
            vertices.len() * std::mem::size_of::<f32>(),
        )
    };

    unsafe {
        gl.viewport(0, 0, width as i32, height as i32);
        gl.disable(glow::BLEND);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::NEAREST as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::NEAREST as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as i32,
            width as i32,
            height as i32,
            0,
            glow::BGRA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(pixels)),
        );
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, vertex_bytes, glow::STATIC_DRAW);
        gl.use_program(Some(program));
        gl.uniform_1_i32(gl.get_uniform_location(program, "frame").as_ref(), 0);
        gl.uniform_1_f32(gl.get_uniform_location(program, "opacity").as_ref(), 1.0);
        gl.uniform_1_f32(
            gl.get_uniform_location(program, "source_is_opaque")
                .as_ref(),
            1.0,
        );
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);
        gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
    }
    let gl_error = unsafe { gl.get_error() };
    unsafe {
        gl.disable_vertex_attrib_array(0);
        gl.disable_vertex_attrib_array(1);
        gl.bind_buffer(glow::ARRAY_BUFFER, None);
        gl.bind_texture(glow::TEXTURE_2D, None);
        gl.use_program(None);
        gl.delete_buffer(vertex_buffer);
        gl.delete_texture(texture);
        gl.delete_program(program);
        gl.delete_shader(vertex_shader);
        gl.delete_shader(fragment_shader);
    }
    if gl_error == glow::NO_ERROR {
        Ok(())
    } else {
        Err(format!(
            "OpenGL frame upload failed with error {gl_error:#x}"
        ))
    }
}

pub(crate) fn context_attributes() -> [khronos_egl::Int; 1] {
    [khronos_egl::NONE]
}
