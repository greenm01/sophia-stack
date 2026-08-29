use crate::api::*;

#[cfg(feature = "libdrm-events")]
use std::os::fd::OwnedFd;

#[cfg(feature = "libdrm-events")]
use sophia_renderer_live::{
    LiveRendererScanoutBufferDescriptor, LiveRendererScanoutBufferExportDetail,
    LiveRendererScanoutBufferExportStatus,
};

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LiveRenderedScanoutBufferExport<Owner> {
    pub status: LiveRendererScanoutBufferExportStatus,
    pub detail: LiveRendererScanoutBufferExportDetail,
    pub descriptor: Option<LiveRendererScanoutBufferDescriptor>,
    pub owner: Option<Owner>,
}

#[cfg(feature = "libdrm-events")]
impl<Owner> LiveRenderedScanoutBufferExport<Owner> {
    pub fn new(
        status: LiveRendererScanoutBufferExportStatus,
        detail: LiveRendererScanoutBufferExportDetail,
        descriptor: Option<LiveRendererScanoutBufferDescriptor>,
        owner: Option<Owner>,
    ) -> Self {
        match (status, descriptor.is_some() && owner.is_some()) {
            (LiveRendererScanoutBufferExportStatus::Exported, true) => Self {
                status,
                detail,
                descriptor,
                owner,
            },
            (LiveRendererScanoutBufferExportStatus::Exported, false) => Self {
                status: LiveRendererScanoutBufferExportStatus::Degraded,
                detail: LiveRendererScanoutBufferExportDetail::RetainedBufferMissing,
                descriptor: None,
                owner: None,
            },
            (status, _) => Self {
                status,
                detail,
                descriptor: None,
                owner: None,
            },
        }
    }

    pub fn normalized(self) -> Self {
        Self::new(self.status, self.detail, self.descriptor, self.owner)
    }
}

#[cfg(feature = "libdrm-events")]
pub trait LiveRenderedScanoutBufferExporter {
    type Owner;

    fn export_rendered_scanout_buffer(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
    ) -> LiveRenderedScanoutBufferExport<Self::Owner>;

    /// Whether a direct attempt from this exporter still owes the driver a
    /// validating commit before it may reach a screen.
    ///
    /// True on the composition-to-direct edge and after anything that ended an
    /// eligibility episode; false while a run of direct frames continues, so
    /// the steady state costs no extra ioctl. The three hooks below default to
    /// doing nothing so that an exporter with no direct path -- every test
    /// fake, and the hardware-validation exporters -- is unaffected by this
    /// row and cannot accidentally acquire half of it.
    fn direct_scanout_test_required(&self) -> bool {
        false
    }

    /// Record the driver's answer to that validating commit.
    fn record_direct_scanout_test(&mut self, _accepted: bool) {}

    /// The direct buffer reached the driver. Release the composed form kept
    /// against a refusal; never the client's buffer, which is on glass.
    fn commit_direct_scanout(&mut self) {}

    /// The direct attempt did not reach a screen. Re-offer the same content
    /// for composition and report whether anything was re-offered.
    fn fall_back_from_direct(&mut self) -> bool {
        false
    }
}

#[cfg(feature = "libdrm-events")]
pub trait LiveRenderedScanoutBufferPrimeSource {
    /// Whether this owner is a client's own buffer taking the direct path.
    ///
    /// The submit path asks because a direct buffer is the one case where a
    /// refusal must not be terminal: there is a composed form of the same
    /// frame waiting, and the frame is owed that second chance rather than a
    /// failed session.
    fn is_direct_client_buffer(&self) -> bool {
        false
    }

    /// Whether the renderer and KMS device descriptors share one DRM file.
    ///
    /// PRIME-importing a buffer back into the same DRM file can return the
    /// renderer's existing GEM handle. KMS cleanup would then close a handle
    /// still owned by GBM, so shared-file owners must submit their descriptor
    /// directly.
    fn shares_kms_drm_file(&self) -> bool;

    fn export_scanout_dma_buf_fds(&self) -> std::io::Result<Option<LiveRenderedScanoutDmaBufFds>>;
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl LiveRenderedScanoutBufferPrimeSource for sophia_renderer_live::NativeGbmOwnedScanoutBuffer {
    fn shares_kms_drm_file(&self) -> bool {
        true
    }

    fn export_scanout_dma_buf_fds(&self) -> std::io::Result<Option<LiveRenderedScanoutDmaBufFds>> {
        self.export_scanout_dma_buf_fds()
            .map(LiveRenderedScanoutDmaBufFds::from_native_gbm)
            .map(Some)
    }
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl LiveRenderedScanoutBufferPrimeSource for super::NativeGbmRenderedScanoutOwner {
    fn is_direct_client_buffer(&self) -> bool {
        matches!(self, super::NativeGbmRenderedScanoutOwner::Direct(_))
    }

    fn shares_kms_drm_file(&self) -> bool {
        match self {
            // Both compositor-owned buffers come from the renderer's own `dup`
            // of the KMS card file, so re-importing them would hand back a
            // handle GBM still owns.
            super::NativeGbmRenderedScanoutOwner::Inline(_)
            | super::NativeGbmRenderedScanoutOwner::Worker(_) => true,
            // A client's buffer was allocated against the client's device.
            // PRIME import is the only way it becomes a handle this file can
            // hang a framebuffer on, and the handle it produces is ours to
            // close when the bundle retires.
            super::NativeGbmRenderedScanoutOwner::Direct(_) => false,
        }
    }

    fn export_scanout_dma_buf_fds(&self) -> std::io::Result<Option<LiveRenderedScanoutDmaBufFds>> {
        match self {
            super::NativeGbmRenderedScanoutOwner::Inline(owner) => owner
                .export_scanout_dma_buf_fds()
                .map(LiveRenderedScanoutDmaBufFds::from_native_gbm)
                .map(Some),
            super::NativeGbmRenderedScanoutOwner::Worker(owner) => {
                owner.export_scanout_dma_buf_fds().map(|fds| {
                    fds.map(|(plane_count, plane_fds)| LiveRenderedScanoutDmaBufFds {
                        plane_count,
                        plane_fds,
                    })
                })
            }
            super::NativeGbmRenderedScanoutOwner::Direct(buffer) => {
                buffer.try_clone_plane_fds().map(|plane_fds| {
                    Some(LiveRenderedScanoutDmaBufFds {
                        plane_count: buffer.descriptor.plane_count,
                        plane_fds,
                    })
                })
            }
        }
    }
}

#[cfg(feature = "libdrm-events")]
pub struct LiveRenderedScanoutDmaBufFds {
    plane_count: u8,
    plane_fds: [Option<OwnedFd>; 4],
}

#[cfg(feature = "libdrm-events")]
impl LiveRenderedScanoutDmaBufFds {
    #[cfg(feature = "gbm-probe")]
    fn from_native_gbm(fds: sophia_renderer_live::NativeGbmScanoutBufferPlaneFds) -> Self {
        Self {
            plane_count: fds.plane_count(),
            plane_fds: fds.into_plane_fds(),
        }
    }

    pub fn new_for_test(plane_fds: [Option<OwnedFd>; 4], plane_count: u8) -> Self {
        Self {
            plane_count,
            plane_fds,
        }
    }

    pub const fn plane_count(&self) -> u8 {
        self.plane_count
    }

    pub fn into_plane_fds(self) -> [Option<OwnedFd>; 4] {
        self.plane_fds
    }
}
