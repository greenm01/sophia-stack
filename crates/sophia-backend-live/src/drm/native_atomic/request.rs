/// Cloneable so one built request can be asked about and then performed.
///
/// A validating commit and the flip that follows it must describe the exact
/// same framebuffer, or the driver's answer was about something else. Cloning
/// the request is what makes that literal rather than approximate: the
/// alternative is building a second framebuffer and trusting that identical
/// inputs produce an identically scannable one.
#[derive(Clone, Debug)]
pub struct LibdrmNativeAtomicCommitRequest {
    request: drm::control::atomic::AtomicModeReq,
    scope: LibdrmNativeAtomicCommitRequestScope,
    page_flip_event: bool,
    nonblocking: bool,
    allow_modeset: bool,
    test_only: bool,
}

impl LibdrmNativeAtomicCommitRequest {
    pub const fn new(request: drm::control::atomic::AtomicModeReq) -> Self {
        Self {
            request,
            scope: LibdrmNativeAtomicCommitRequestScope::PageFlip,
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: false,
            test_only: false,
        }
    }

    pub const fn modeset(request: drm::control::atomic::AtomicModeReq) -> Self {
        Self {
            request,
            scope: LibdrmNativeAtomicCommitRequestScope::Modeset,
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: false,
            test_only: false,
        }
    }

    pub const fn without_page_flip_event(mut self) -> Self {
        self.page_flip_event = false;
        self
    }

    pub const fn blocking(mut self) -> Self {
        self.nonblocking = false;
        self
    }

    pub const fn allow_modeset(mut self) -> Self {
        self.allow_modeset = true;
        self
    }

    pub const fn test_only(mut self) -> Self {
        self.test_only = true;
        self
    }

    /// A validation-only commit never reaches scanout, so it never completes with
    /// an event. The kernel does not merely ignore the combination: it rejects
    /// `TEST_ONLY` together with `PAGE_FLIP_EVENT` with `EINVAL` before inspecting
    /// a single property, so leaving both set makes every validation look like a
    /// refused topology. Deriving the flag instead of storing it keeps that
    /// unrepresentable whatever order a caller sets things in.
    const fn effective_page_flip_event(&self) -> bool {
        self.page_flip_event && !self.test_only
    }

    pub const fn reduced_flags(&self) -> LibdrmNativeAtomicCommitFlagsReport {
        LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: self.effective_page_flip_event(),
            nonblocking: self.nonblocking,
            allow_modeset: self.allow_modeset,
            test_only: self.test_only,
        }
    }

    pub const fn reduced_scope(&self) -> LibdrmNativeAtomicCommitRequestScope {
        self.scope
    }

    pub(crate) fn into_native(
        self,
    ) -> (
        drm::control::AtomicCommitFlags,
        drm::control::atomic::AtomicModeReq,
    ) {
        let mut flags = drm::control::AtomicCommitFlags::empty();
        if self.effective_page_flip_event() {
            flags |= drm::control::AtomicCommitFlags::PAGE_FLIP_EVENT;
        }
        if self.nonblocking {
            flags |= drm::control::AtomicCommitFlags::NONBLOCK;
        }
        if self.allow_modeset {
            flags |= drm::control::AtomicCommitFlags::ALLOW_MODESET;
        }
        if self.test_only {
            flags |= drm::control::AtomicCommitFlags::TEST_ONLY;
        }
        (flags, self.request)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeAtomicCommitRequestScope {
    PageFlip,
    Modeset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeAtomicCommitFlagsReport {
    pub page_flip_event: bool,
    pub nonblocking: bool,
    pub allow_modeset: bool,
    pub test_only: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeAtomicCommitSubmitReport {
    pub status: LibdrmNativeAtomicCommitSubmitStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeAtomicCommitSubmitStatus {
    Submitted,
    WouldBlock,
    Rejected,
}
