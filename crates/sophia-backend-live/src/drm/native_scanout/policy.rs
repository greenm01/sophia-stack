use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneScanoutSubmitPolicy {
    pub allow_modeset: bool,
    pub page_flip_event: bool,
    pub nonblocking: bool,
    pub vrr_enabled: Option<bool>,
    /// Ask the driver whether this exact commit would be accepted, without
    /// performing it.
    ///
    /// Direct scanout hands the plane a client's own buffer, whose format,
    /// modifier, and plane layout the compositor did not choose. Only the
    /// driver can say whether that is scannable here, and the only way to ask
    /// without risking the screen is a commit that changes nothing.
    ///
    /// A test carries no page-flip event: there is no flip to report, and the
    /// kernel refuses the pair outright. The flag owner already makes that
    /// combination unrepresentable, and `page_flip_event` is cleared here so
    /// the two never have to agree twice.
    pub test_only: bool,
}

impl LibdrmNativePrimaryPlaneScanoutSubmitPolicy {
    pub const fn page_flip() -> Self {
        Self {
            allow_modeset: false,
            page_flip_event: true,
            nonblocking: true,
            vrr_enabled: None,
            test_only: false,
        }
    }

    pub const fn modeset() -> Self {
        Self {
            allow_modeset: true,
            page_flip_event: true,
            nonblocking: true,
            vrr_enabled: None,
            test_only: false,
        }
    }

    pub const fn blocking_modeset() -> Self {
        Self {
            allow_modeset: true,
            page_flip_event: false,
            nonblocking: false,
            vrr_enabled: None,
            test_only: false,
        }
    }

    /// The same commit, asked rather than performed.
    pub const fn validating(mut self) -> Self {
        self.test_only = true;
        self.page_flip_event = false;
        self
    }

    pub const fn with_vrr_enabled(mut self, enabled: bool) -> Self {
        self.vrr_enabled = Some(enabled);
        self
    }

    pub const fn expected_request_scope(self) -> LibdrmNativeAtomicCommitRequestScope {
        if self.allow_modeset {
            LibdrmNativeAtomicCommitRequestScope::Modeset
        } else {
            LibdrmNativeAtomicCommitRequestScope::PageFlip
        }
    }
}
