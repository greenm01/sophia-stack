use super::*;

pub(super) struct LiveXAdmissionPolicy {
    pub(super) registry: Arc<Mutex<NamespaceRegistry>>,
    pub(super) namespace: NamespaceId,
    pub(super) session_user_id: u32,
}

impl XServerFrontendAdmissionPolicy for LiveXAdmissionPolicy {
    fn admit(
        &self,
        request: XServerFrontendAdmissionRequest,
    ) -> Result<ClientAdmissionContext, XServerFrontendAdmissionError> {
        let peer = request
            .peer_credentials
            .ok_or(XServerFrontendAdmissionError::Denied)?;
        if peer.user_id != self.session_user_id {
            return Err(XServerFrontendAdmissionError::Denied);
        }
        self.registry
            .lock()
            .map_err(|_| XServerFrontendAdmissionError::Unavailable)?
            .admit(self.namespace, request.setup_authentication)
            .map_err(|_| XServerFrontendAdmissionError::Unavailable)
    }

    fn revoke(&self, context: ClientAdmissionContext) -> Result<(), XServerFrontendAdmissionError> {
        if context.namespace.id != self.namespace {
            return Err(XServerFrontendAdmissionError::Unavailable);
        }
        self.registry
            .lock()
            .map_err(|_| XServerFrontendAdmissionError::Unavailable)?
            .revoke_admission(context.client_id)
            .map(|_| ())
            .map_err(|_| XServerFrontendAdmissionError::Unavailable)
    }
}

/// Originates buffers for pixmaps a client did not allocate.
///
/// Holds the same device the render node is handed from, so a buffer the
/// authority originates and one the client imports come from one device and can
/// be composited by the same path.
#[cfg(feature = "atomic-scanout-live")]
pub(super) struct LiveXPixmapAllocator {
    pub(super) device: std::fs::File,
}

#[cfg(feature = "atomic-scanout-live")]
impl XServerFrontendPixmapAllocator for LiveXPixmapAllocator {
    fn allocate_pixmap_buffer(
        &self,
        request: XServerFrontendPixmapAllocation,
    ) -> Result<XServerFrontendAllocatedPixmap, XServerFrontendPixmapAllocationError> {
        let allocation = sophia_backend_live::allocate_shared_buffer(
            &self.device,
            request.handle,
            request.size,
            request.depth,
        )
        .map_err(|error| match error {
            sophia_backend_live::LiveSharedBufferError::UnsupportedTarget => {
                XServerFrontendPixmapAllocationError::UnsupportedTarget
            }
            sophia_backend_live::LiveSharedBufferError::DeviceRejected
            | sophia_backend_live::LiveSharedBufferError::ExportFailed => {
                XServerFrontendPixmapAllocationError::AllocationFailed
            }
        })?;
        Ok(XServerFrontendAllocatedPixmap {
            descriptor: allocation.descriptor,
            plane_fds: allocation.plane_fds,
        })
    }
}

pub(super) struct LiveXRenderDeviceProvider {
    pub(super) device: std::fs::File,
}

impl XServerFrontendRenderDeviceProvider for LiveXRenderDeviceProvider {
    fn open_render_device_fd(
        &self,
    ) -> Result<std::os::fd::OwnedFd, XServerFrontendRenderDeviceError> {
        use std::os::fd::AsRawFd as _;

        let proc_path = format!("/proc/self/fd/{}", self.device.as_raw_fd());
        let selected_node = std::fs::read_link(&proc_path)
            .map_err(|_| XServerFrontendRenderDeviceError::Unavailable)?;
        let selected_name = selected_node
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(XServerFrontendRenderDeviceError::Unavailable)?;

        let render_node = if selected_name.starts_with("renderD") {
            selected_node
        } else {
            let selected_device =
                std::fs::canonicalize(format!("/sys/class/drm/{selected_name}/device"))
                    .map_err(|_| XServerFrontendRenderDeviceError::Unavailable)?;
            std::fs::read_dir("/sys/class/drm")
                .map_err(|_| XServerFrontendRenderDeviceError::Unavailable)?
                .filter_map(Result::ok)
                .take(64)
                .find_map(|entry| {
                    let name = entry.file_name();
                    let name = name.to_str()?;
                    if !name.starts_with("renderD") {
                        return None;
                    }
                    let device = std::fs::canonicalize(entry.path().join("device")).ok()?;
                    (device == selected_device).then(|| std::path::Path::new("/dev/dri").join(name))
                })
                .ok_or(XServerFrontendRenderDeviceError::Unavailable)?
        };

        // A fresh render-node open gives each DRI3 client its own DRM file
        // description and withholds the compositor's primary/KMS node.
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(render_node)
            .map(std::os::fd::OwnedFd::from)
            .map_err(|_| XServerFrontendRenderDeviceError::OpenFailed)
    }
}
