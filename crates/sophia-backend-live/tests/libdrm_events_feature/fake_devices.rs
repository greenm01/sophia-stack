#[derive(Debug)]
struct FakeNativeAtomicCommitDevice {
    result: io::Result<()>,
}

impl LibdrmNativeAtomicCommitDevice for FakeNativeAtomicCommitDevice {
    fn submit_atomic_commit(
        &self,
        _flags: drm::control::AtomicCommitFlags,
        _request: drm::control::atomic::AtomicModeReq,
    ) -> io::Result<()> {
        self.result
            .as_ref()
            .map(|_| ())
            .map_err(|error| io::Error::new(error.kind(), "synthetic atomic commit failure"))
    }
}

#[derive(Debug)]
struct FakeNativePropertyLookupDevice {
    connector: io::Result<LibdrmNativePropertyHandleSet>,
    crtc: io::Result<LibdrmNativePropertyHandleSet>,
    plane: io::Result<LibdrmNativePropertyHandleSet>,
    connector_value: io::Result<Option<u64>>,
}

impl LibdrmNativePropertyLookupDevice for FakeNativePropertyLookupDevice {
    fn connector_property_handles(
        &self,
        _connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        clone_io_result(&self.connector)
    }

    fn crtc_property_handles(
        &self,
        _crtc: drm::control::crtc::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        clone_io_result(&self.crtc)
    }

    fn plane_property_handles(
        &self,
        _plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        clone_io_result(&self.plane)
    }

    fn connector_property_value(
        &self,
        _connector: drm::control::connector::Handle,
        _property: drm::control::property::Handle,
    ) -> io::Result<Option<u64>> {
        clone_io_result(&self.connector_value)
    }
}

#[derive(Debug)]
struct FakeNativeKmsSelectionDevice {
    connectors: io::Result<Vec<drm::control::connector::Handle>>,
    crtcs: io::Result<Vec<drm::control::crtc::Handle>>,
    planes: io::Result<Vec<drm::control::plane::Handle>>,
    connector_snapshot: io::Result<LibdrmNativeConnectorSnapshot>,
    encoder_snapshot: io::Result<LibdrmNativeEncoderSnapshot>,
    plane_snapshot: io::Result<LibdrmNativePlaneSnapshot>,
    plane_type: io::Result<Option<drm::control::PlaneType>>,
}

impl LibdrmNativeKmsSelectionDevice for FakeNativeKmsSelectionDevice {
    fn connector_handles(&self) -> io::Result<Vec<drm::control::connector::Handle>> {
        clone_io_result(&self.connectors)
    }

    fn crtc_handles(&self) -> io::Result<Vec<drm::control::crtc::Handle>> {
        clone_io_result(&self.crtcs)
    }

    fn connector_snapshot(
        &self,
        _connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativeConnectorSnapshot> {
        clone_io_result(&self.connector_snapshot)
    }

    fn encoder_snapshot(
        &self,
        _encoder: drm::control::encoder::Handle,
    ) -> io::Result<LibdrmNativeEncoderSnapshot> {
        clone_io_result(&self.encoder_snapshot)
    }

    fn plane_handles(&self) -> io::Result<Vec<drm::control::plane::Handle>> {
        clone_io_result(&self.planes)
    }

    fn plane_snapshot(
        &self,
        _plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePlaneSnapshot> {
        clone_io_result(&self.plane_snapshot)
    }

    fn plane_type(
        &self,
        _plane: drm::control::plane::Handle,
    ) -> io::Result<Option<drm::control::PlaneType>> {
        clone_io_result(&self.plane_type)
    }
}

#[derive(Debug)]
struct FakeMultiNativeKmsSelectionDevice {
    connectors: Vec<(
        drm::control::connector::Handle,
        LibdrmNativeConnectorSnapshot,
    )>,
    crtcs: Vec<drm::control::crtc::Handle>,
    encoders: Vec<(drm::control::encoder::Handle, LibdrmNativeEncoderSnapshot)>,
    planes: Vec<(drm::control::plane::Handle, LibdrmNativePlaneSnapshot)>,
}

impl LibdrmNativeKmsSelectionDevice for FakeMultiNativeKmsSelectionDevice {
    fn connector_handles(&self) -> io::Result<Vec<drm::control::connector::Handle>> {
        Ok(self.connectors.iter().map(|(handle, _)| *handle).collect())
    }

    fn crtc_handles(&self) -> io::Result<Vec<drm::control::crtc::Handle>> {
        Ok(self.crtcs.clone())
    }

    fn connector_snapshot(
        &self,
        connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativeConnectorSnapshot> {
        self.connectors
            .iter()
            .find_map(|(handle, snapshot)| (*handle == connector).then_some(snapshot.clone()))
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn encoder_snapshot(
        &self,
        encoder: drm::control::encoder::Handle,
    ) -> io::Result<LibdrmNativeEncoderSnapshot> {
        self.encoders
            .iter()
            .find_map(|(handle, snapshot)| (*handle == encoder).then_some(snapshot.clone()))
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn plane_handles(&self) -> io::Result<Vec<drm::control::plane::Handle>> {
        Ok(self.planes.iter().map(|(handle, _)| *handle).collect())
    }

    fn plane_snapshot(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePlaneSnapshot> {
        self.planes
            .iter()
            .find_map(|(handle, snapshot)| (*handle == plane).then_some(snapshot.clone()))
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn plane_type(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<Option<drm::control::PlaneType>> {
        Ok(self
            .planes
            .iter()
            .any(|(handle, _)| *handle == plane)
            .then_some(drm::control::PlaneType::Primary))
    }
}

#[derive(Debug)]
struct FakeNativePrimaryPlaneResourceDevice {
    mode_blob: io::Result<u64>,
    framebuffer: io::Result<drm::control::framebuffer::Handle>,
    destroy_framebuffer: io::Result<()>,
    destroy_mode_blob: io::Result<()>,
    destroyed_framebuffers: std::cell::Cell<usize>,
    /// PRIME imports and the handles they left open.
    ///
    /// The trait's default refuses to import, which was right while nothing
    /// but the renderer's own buffers reached a plane. A client's buffer takes
    /// the PRIME transport, so a device that cannot import cannot reach the
    /// direct path at all -- and closing is counted too, because a refused
    /// attempt that leaks its imported handles is exactly the failure the
    /// fallback ladder must not have.
    imported_buffers: std::cell::Cell<usize>,
    closed_buffers: std::cell::Cell<usize>,
}

impl LibdrmNativePrimaryPlaneResourceDevice for FakeNativePrimaryPlaneResourceDevice {
    fn create_mode_blob_for_selection(
        &self,
        _selection: sophia_backend_live::LibdrmNativePrimaryPlaneSelection,
    ) -> io::Result<u64> {
        clone_io_result(&self.mode_blob)
    }

    fn create_mode_blob(&self, _mode: drm::control::Mode) -> io::Result<u64> {
        clone_io_result(&self.mode_blob)
    }

    fn add_scanout_framebuffer_with_modifiers<B>(
        &self,
        _buffer: &B,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::PlanarBuffer + ?Sized,
    {
        clone_io_result(&self.framebuffer)
    }

    fn add_scanout_framebuffer_without_modifiers<B>(
        &self,
        _buffer: &B,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::PlanarBuffer + ?Sized,
    {
        clone_io_result(&self.framebuffer)
    }

    fn add_legacy_scanout_framebuffer<B>(
        &self,
        _buffer: &B,
        _depth: u32,
        _bpp: u32,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::Buffer + ?Sized,
    {
        clone_io_result(&self.framebuffer)
    }

    fn destroy_scanout_framebuffer(
        &self,
        _framebuffer: drm::control::framebuffer::Handle,
    ) -> io::Result<()> {
        self.destroyed_framebuffers
            .set(self.destroyed_framebuffers.get().saturating_add(1));
        clone_io_result(&self.destroy_framebuffer)
    }

    fn import_scanout_dma_buf(&self, _fd: BorrowedFd<'_>) -> io::Result<drm::buffer::Handle> {
        self.imported_buffers
            .set(self.imported_buffers.get().saturating_add(1));
        Ok(buffer_handle(77))
    }

    fn close_scanout_buffer(&self, _handle: drm::buffer::Handle) -> io::Result<()> {
        self.closed_buffers
            .set(self.closed_buffers.get().saturating_add(1));
        Ok(())
    }

    fn destroy_mode_blob(&self, _mode_blob: u64) -> io::Result<()> {
        clone_io_result(&self.destroy_mode_blob)
    }
}

#[derive(Debug)]
struct FakeModifierOnlyPrimaryPlaneResourceDevice {
    mode_blob: io::Result<u64>,
    framebuffer_with_modifiers: io::Result<drm::control::framebuffer::Handle>,
    fallback_framebuffer: io::Result<drm::control::framebuffer::Handle>,
    destroy_framebuffer: io::Result<()>,
    destroy_mode_blob: io::Result<()>,
}

impl LibdrmNativePrimaryPlaneResourceDevice for FakeModifierOnlyPrimaryPlaneResourceDevice {
    fn create_mode_blob_for_selection(
        &self,
        _selection: sophia_backend_live::LibdrmNativePrimaryPlaneSelection,
    ) -> io::Result<u64> {
        clone_io_result(&self.mode_blob)
    }

    fn create_mode_blob(&self, _mode: drm::control::Mode) -> io::Result<u64> {
        clone_io_result(&self.mode_blob)
    }

    fn add_scanout_framebuffer_with_modifiers<B>(
        &self,
        _buffer: &B,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::PlanarBuffer + ?Sized,
    {
        clone_io_result(&self.framebuffer_with_modifiers)
    }

    fn add_scanout_framebuffer_without_modifiers<B>(
        &self,
        buffer: &B,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::PlanarBuffer + ?Sized,
    {
        if buffer.modifier().is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "implicit framebuffer registration received a modifier",
            ));
        }
        clone_io_result(&self.fallback_framebuffer)
    }

    fn add_legacy_scanout_framebuffer<B>(
        &self,
        _buffer: &B,
        _depth: u32,
        _bpp: u32,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::Buffer + ?Sized,
    {
        clone_io_result(&self.fallback_framebuffer)
    }

    fn destroy_scanout_framebuffer(
        &self,
        _framebuffer: drm::control::framebuffer::Handle,
    ) -> io::Result<()> {
        clone_io_result(&self.destroy_framebuffer)
    }

    fn destroy_mode_blob(&self, _mode_blob: u64) -> io::Result<()> {
        clone_io_result(&self.destroy_mode_blob)
    }
}

#[derive(Debug)]
struct FakePrimePrimaryPlaneResourceDevice {
    mode_blob: io::Result<u64>,
    framebuffer: io::Result<drm::control::framebuffer::Handle>,
    imported_buffer: io::Result<drm::buffer::Handle>,
    close_buffer: io::Result<()>,
    destroy_framebuffer: io::Result<()>,
    destroy_mode_blob: io::Result<()>,
    expected_framebuffer_buffer: Option<drm::buffer::Handle>,
}

impl LibdrmNativePrimaryPlaneResourceDevice for FakePrimePrimaryPlaneResourceDevice {
    fn create_mode_blob_for_selection(
        &self,
        _selection: sophia_backend_live::LibdrmNativePrimaryPlaneSelection,
    ) -> io::Result<u64> {
        clone_io_result(&self.mode_blob)
    }

    fn create_mode_blob(&self, _mode: drm::control::Mode) -> io::Result<u64> {
        clone_io_result(&self.mode_blob)
    }

    fn add_scanout_framebuffer_with_modifiers<B>(
        &self,
        buffer: &B,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::PlanarBuffer + ?Sized,
    {
        if let Some(expected) = self.expected_framebuffer_buffer {
            assert_eq!(buffer.handles()[0], Some(expected));
        }
        clone_io_result(&self.framebuffer)
    }

    fn add_scanout_framebuffer_without_modifiers<B>(
        &self,
        buffer: &B,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::PlanarBuffer + ?Sized,
    {
        if let Some(expected) = self.expected_framebuffer_buffer {
            assert_eq!(buffer.handles()[0], Some(expected));
        }
        clone_io_result(&self.framebuffer)
    }

    fn add_legacy_scanout_framebuffer<B>(
        &self,
        buffer: &B,
        _depth: u32,
        _bpp: u32,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::Buffer + ?Sized,
    {
        if let Some(expected) = self.expected_framebuffer_buffer {
            assert_eq!(buffer.handle(), expected);
        }
        clone_io_result(&self.framebuffer)
    }

    fn destroy_scanout_framebuffer(
        &self,
        _framebuffer: drm::control::framebuffer::Handle,
    ) -> io::Result<()> {
        clone_io_result(&self.destroy_framebuffer)
    }

    fn import_scanout_dma_buf(&self, _fd: BorrowedFd<'_>) -> io::Result<drm::buffer::Handle> {
        clone_io_result(&self.imported_buffer)
    }

    fn close_scanout_buffer(&self, _handle: drm::buffer::Handle) -> io::Result<()> {
        clone_io_result(&self.close_buffer)
    }

    fn destroy_mode_blob(&self, _mode_blob: u64) -> io::Result<()> {
        clone_io_result(&self.destroy_mode_blob)
    }
}

#[derive(Debug)]
struct FakeNativePrimaryPlaneScanoutDevice {
    selection: FakeNativeKmsSelectionDevice,
    properties: FakeNativePropertyLookupDevice,
    resources: FakeNativePrimaryPlaneResourceDevice,
    submit: io::Result<()>,
    commits: std::cell::Cell<usize>,
    /// Commits that carried `TEST_ONLY`, counted separately.
    ///
    /// A device that only counts commits cannot tell a question from an
    /// answer: a validating commit that quietly lost its flag would change
    /// the screen while every assertion still passed.
    test_only_commits: std::cell::Cell<usize>,
    accept_commits: Option<usize>,
}

impl LibdrmNativeKmsSelectionDevice for FakeNativePrimaryPlaneScanoutDevice {
    fn connector_handles(&self) -> io::Result<Vec<drm::control::connector::Handle>> {
        self.selection.connector_handles()
    }

    fn crtc_handles(&self) -> io::Result<Vec<drm::control::crtc::Handle>> {
        self.selection.crtc_handles()
    }

    fn connector_snapshot(
        &self,
        connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativeConnectorSnapshot> {
        self.selection.connector_snapshot(connector)
    }

    fn encoder_snapshot(
        &self,
        encoder: drm::control::encoder::Handle,
    ) -> io::Result<LibdrmNativeEncoderSnapshot> {
        self.selection.encoder_snapshot(encoder)
    }

    fn plane_handles(&self) -> io::Result<Vec<drm::control::plane::Handle>> {
        self.selection.plane_handles()
    }

    fn plane_snapshot(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePlaneSnapshot> {
        self.selection.plane_snapshot(plane)
    }

    fn plane_type(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<Option<drm::control::PlaneType>> {
        self.selection.plane_type(plane)
    }
}

impl LibdrmNativePropertyLookupDevice for FakeNativePrimaryPlaneScanoutDevice {
    fn connector_property_handles(
        &self,
        connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        self.properties.connector_property_handles(connector)
    }

    fn crtc_property_handles(
        &self,
        crtc: drm::control::crtc::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        self.properties.crtc_property_handles(crtc)
    }

    fn plane_property_handles(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        self.properties.plane_property_handles(plane)
    }
}

impl LibdrmNativePrimaryPlaneResourceDevice for FakeNativePrimaryPlaneScanoutDevice {
    fn create_mode_blob_for_selection(
        &self,
        selection: sophia_backend_live::LibdrmNativePrimaryPlaneSelection,
    ) -> io::Result<u64> {
        self.resources.create_mode_blob_for_selection(selection)
    }

    fn create_mode_blob(&self, mode: drm::control::Mode) -> io::Result<u64> {
        self.resources.create_mode_blob(mode)
    }

    fn add_scanout_framebuffer_with_modifiers<B>(
        &self,
        buffer: &B,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::PlanarBuffer + ?Sized,
    {
        self.resources
            .add_scanout_framebuffer_with_modifiers(buffer)
    }

    fn add_scanout_framebuffer_without_modifiers<B>(
        &self,
        buffer: &B,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::PlanarBuffer + ?Sized,
    {
        self.resources
            .add_scanout_framebuffer_without_modifiers(buffer)
    }

    fn add_legacy_scanout_framebuffer<B>(
        &self,
        buffer: &B,
        depth: u32,
        bpp: u32,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::Buffer + ?Sized,
    {
        self.resources
            .add_legacy_scanout_framebuffer(buffer, depth, bpp)
    }

    fn destroy_scanout_framebuffer(
        &self,
        framebuffer: drm::control::framebuffer::Handle,
    ) -> io::Result<()> {
        self.resources.destroy_scanout_framebuffer(framebuffer)
    }

    // Forwarded rather than left to the trait default, which refuses. A
    // client's buffer reaches this device only through PRIME, so a scanout
    // device that cannot import cannot reach the direct path at all.
    fn import_scanout_dma_buf(&self, fd: BorrowedFd<'_>) -> io::Result<drm::buffer::Handle> {
        self.resources.import_scanout_dma_buf(fd)
    }

    fn close_scanout_buffer(&self, handle: drm::buffer::Handle) -> io::Result<()> {
        self.resources.close_scanout_buffer(handle)
    }

    fn destroy_mode_blob(&self, mode_blob: u64) -> io::Result<()> {
        self.resources.destroy_mode_blob(mode_blob)
    }
}

impl LibdrmNativeAtomicCommitDevice for FakeNativePrimaryPlaneScanoutDevice {
    fn submit_atomic_commit(
        &self,
        flags: drm::control::AtomicCommitFlags,
        _request: drm::control::atomic::AtomicModeReq,
    ) -> io::Result<()> {
        let taken = self.commits.get();
        self.commits.set(taken.saturating_add(1));
        if flags.contains(drm::control::AtomicCommitFlags::TEST_ONLY) {
            self.test_only_commits
                .set(self.test_only_commits.get().saturating_add(1));
        }
        if self.accept_commits.is_some_and(|accept| taken >= accept) {
            return Err(io::Error::other("synthetic head commit refusal"));
        }
        clone_io_result(&self.submit)
    }
}

impl FakeNativePrimaryPlaneScanoutDevice {
    /// Accepts the first `accept` commits and refuses every later one.
    ///
    /// A mirror group submits once per head, so this is how a test reaches the
    /// case that matters: one connector scanning the framebuffer while another
    /// head was refused.
    fn accepting_commits(mut self, accept: usize) -> Self {
        self.accept_commits = Some(accept);
        self
    }

    fn test_only_commits(&self) -> usize {
        self.test_only_commits.get()
    }

    fn commits(&self) -> usize {
        self.commits.get()
    }

    fn destroyed_framebuffers(&self) -> usize {
        self.resources.destroyed_framebuffers.get()
    }

    fn imported_buffers(&self) -> usize {
        self.resources.imported_buffers.get()
    }

    fn closed_buffers(&self) -> usize {
        self.resources.closed_buffers.get()
    }
}
