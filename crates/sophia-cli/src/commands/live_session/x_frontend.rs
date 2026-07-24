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
