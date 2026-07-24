#[derive(Debug)]
pub struct NativeGbmOwnedScanoutBuffer {
    width: u32,
    height: u32,
    pitch: u32,
    format: u32,
    gem_handle: u32,
    plane_count: u8,
    plane_handles: [u32; 4],
    plane_pitches: [u32; 4],
    plane_offsets: [u32; 4],
    plane_fds: Option<[Option<OwnedFd>; 4]>,
    modifier: Option<u64>,
    // Drop explicitly releases the locked front buffer before its surface.
    _buffer: Option<gbm::BufferObject<()>>,
    _egl_surface: Option<NativeEglSurfaceOwner>,
    _surface: Option<gbm::Surface<()>>,
}

#[derive(Debug)]
struct NativeEglSurfaceOwner {
    destroy_surface: unsafe extern "system" fn(*mut c_void, *mut c_void) -> u32,
    display: khronos_egl::Display,
    surface: khronos_egl::Surface,
}

impl Drop for NativeEglSurfaceOwner {
    fn drop(&mut self) {
        unsafe {
            (self.destroy_surface)(self.display.as_ptr(), self.surface.as_ptr());
        }
    }
}

impl Drop for NativeGbmOwnedScanoutBuffer {
    fn drop(&mut self) {
        trace_native_lifecycle("scanout_owner_drop_started");
        drop(self._buffer.take());
        trace_native_lifecycle("front_buffer_released");
        drop(self._egl_surface.take());
        trace_native_lifecycle("egl_surface_destroyed");
        drop(self._surface.take());
        trace_native_lifecycle("originating_surface_released");
    }
}

impl NativeGbmOwnedScanoutBuffer {
    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn pitch(&self) -> u32 {
        self.pitch
    }

    pub const fn format(&self) -> u32 {
        self.format
    }

    pub const fn gem_handle(&self) -> u32 {
        self.gem_handle
    }

    pub const fn plane_count(&self) -> u8 {
        self.plane_count
    }

    pub const fn plane_handles(&self) -> [u32; 4] {
        self.plane_handles
    }

    pub const fn plane_pitches(&self) -> [u32; 4] {
        self.plane_pitches
    }

    pub const fn plane_offsets(&self) -> [u32; 4] {
        self.plane_offsets
    }

    pub const fn modifier(&self) -> Option<u64> {
        self.modifier
    }

    pub fn export_plane_fds(
        &self,
    ) -> Result<NativeGbmOwnedScanoutBufferPlaneFds, NativeGbmScanoutBufferExportDetail> {
        if self.plane_count == 0 || self.plane_count as usize > self.plane_handles.len() {
            return Err(NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor);
        }

        let Some(retained_plane_fds) = &self.plane_fds else {
            return Err(NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor);
        };

        let mut plane_fds = std::array::from_fn(|_| None);
        let mut index = 0;
        while index < self.plane_count as usize {
            let Some(fd) = &retained_plane_fds[index] else {
                return Err(NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor);
            };
            plane_fds[index] =
                Some(fd.try_clone().map_err(|_error| {
                    NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor
                })?);
            index += 1;
        }

        Ok(NativeGbmOwnedScanoutBufferPlaneFds {
            plane_count: self.plane_count,
            plane_fds,
        })
    }
}

pub struct NativeGbmOwnedScanoutBufferPlaneFds {
    plane_count: u8,
    plane_fds: [Option<OwnedFd>; 4],
}

impl NativeGbmOwnedScanoutBufferPlaneFds {
    pub const fn plane_count(&self) -> u8 {
        self.plane_count
    }

    pub fn into_plane_fds(self) -> [Option<OwnedFd>; 4] {
        self.plane_fds
    }
}
