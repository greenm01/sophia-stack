use crate::prelude::*;

#[derive(Debug)]
pub struct LibdrmNativePrimaryPlaneScanoutSubmitResult {
    pub status: LibdrmNativePrimaryPlaneScanoutSubmitStatus,
    pub selection: LibdrmNativePrimaryPlaneSelectionStatus,
    pub scanout_buffer: LiveRendererScanoutBufferStatus,
    pub buffer_format: Option<LibdrmNativeScanoutBufferFormatDetail>,
    pub buffer_modifier: Option<LibdrmNativeScanoutBufferModifierDetail>,
    pub buffer_planes: Option<LibdrmNativeScanoutBufferPlaneDetail>,
    pub properties: Option<LibdrmNativePrimaryPlanePropertyDiscoveryStatus>,
    pub format_table: Option<LibdrmNativePrimaryPlaneFormatTableStatus>,
    pub resources: Option<LibdrmNativePrimaryPlaneResourceCreateStatus>,
    pub framebuffer: Option<LibdrmNativePrimaryPlaneFramebufferCreateDetail>,
    pub request: Option<LibdrmNativeAtomicRequestBuildStatus>,
    pub request_scope: Option<LibdrmNativeAtomicCommitRequestScope>,
    pub commit_flags: Option<LibdrmNativeAtomicCommitFlagsReport>,
    pub submit: Option<LibdrmNativeAtomicCommitSubmitStatus>,
    pub submission: Option<LibdrmNativePrimaryPlaneScanoutSubmission>,
    pub cleanup: Option<LibdrmNativePrimaryPlaneResourceCleanup>,
    /// How many of the group's heads accepted the commit.
    ///
    /// One for an ordinary output. For a group it is the fact the singular fields
    /// above cannot carry: they describe one commit, and a group is several.
    pub heads_committed: usize,
}

impl LibdrmNativePrimaryPlaneScanoutSubmitResult {
    pub(crate) const fn new(
        status: LibdrmNativePrimaryPlaneScanoutSubmitStatus,
        selection: LibdrmNativePrimaryPlaneSelectionStatus,
        scanout_buffer: LiveRendererScanoutBufferStatus,
    ) -> Self {
        Self {
            status,
            selection,
            scanout_buffer,
            buffer_format: None,
            buffer_modifier: None,
            buffer_planes: None,
            properties: None,
            format_table: None,
            resources: None,
            framebuffer: None,
            request: None,
            request_scope: None,
            commit_flags: None,
            submit: None,
            submission: None,
            cleanup: None,
            heads_committed: 0,
        }
    }

    pub(crate) fn from_descriptor(
        status: LibdrmNativePrimaryPlaneScanoutSubmitStatus,
        selection: LibdrmNativePrimaryPlaneSelectionStatus,
        scanout_buffer: LiveRendererScanoutBufferStatus,
        descriptor: LiveRendererScanoutBufferDescriptor,
    ) -> Self {
        let mut report = Self::new(status, selection, scanout_buffer);
        report.buffer_format = Some(LibdrmNativeScanoutBufferFormatDetail::from_descriptor(
            descriptor,
        ));
        report.buffer_modifier = Some(LibdrmNativeScanoutBufferModifierDetail::from_descriptor(
            descriptor,
        ));
        report.buffer_planes = Some(LibdrmNativeScanoutBufferPlaneDetail::from_descriptor(
            descriptor,
        ));
        report
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeScanoutBufferFormatDetail {
    Xrgb8888,
    Argb8888,
    Unsupported,
}

impl LibdrmNativeScanoutBufferFormatDetail {
    pub(crate) const fn from_descriptor(descriptor: LiveRendererScanoutBufferDescriptor) -> Self {
        match descriptor.format {
            LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888 => Self::Xrgb8888,
            LIVE_RENDERER_SCANOUT_FORMAT_ARGB8888 => Self::Argb8888,
            _ => Self::Unsupported,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeScanoutBufferModifierDetail {
    Implicit,
    Linear,
    NonLinear,
    Invalid,
}

impl LibdrmNativeScanoutBufferModifierDetail {
    pub(crate) fn from_descriptor(descriptor: LiveRendererScanoutBufferDescriptor) -> Self {
        let Some(modifier) = descriptor.modifier.map(drm::buffer::DrmModifier::from) else {
            return Self::Implicit;
        };

        match modifier {
            drm::buffer::DrmModifier::Invalid => Self::Invalid,
            drm::buffer::DrmModifier::Linear => Self::Linear,
            _ => Self::NonLinear,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeScanoutBufferPlaneDetail {
    Single,
    Multiple,
    Invalid,
}

impl LibdrmNativeScanoutBufferPlaneDetail {
    pub(crate) const fn from_descriptor(descriptor: LiveRendererScanoutBufferDescriptor) -> Self {
        if descriptor.plane_count == 1 {
            Self::Single
        } else if descriptor.plane_count > 1
            && descriptor.plane_count as usize <= LIVE_RENDERER_SCANOUT_MAX_PLANES
        {
            Self::Multiple
        } else {
            Self::Invalid
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneFormatTableStatus {
    Present,
    Missing,
}

impl LibdrmNativePrimaryPlaneFormatTableStatus {
    pub(crate) const fn from_property_handles(
        properties: LibdrmNativePrimaryPlanePropertyHandles,
    ) -> Self {
        if properties.plane_in_formats().is_some() {
            Self::Present
        } else {
            Self::Missing
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneScanoutSubmitStatus {
    SubmittedWaitingForPageFlip,
    /// Some heads of a mirror group took the frame and at least one did not.
    ///
    /// Distinct from `AtomicSubmitFailed`, and the distinction is load-bearing:
    /// a connector is scanning the framebuffer, so the submission owns it and it
    /// must not be destroyed. The candidate still fails closed -- a group that
    /// shows the frame on some of its screens is not a presented group.
    PartiallySubmitted,
    KmsTargetUnavailable,
    ScanoutBufferUnavailable,
    PropertyDiscoveryUnavailable,
    ResourceCreationUnavailable,
    AtomicRequestBuildFailed,
    AtomicSubmitFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneScanoutRetireResult {
    pub status: LibdrmNativePrimaryPlaneScanoutRetireStatus,
    pub destroy: Option<LibdrmNativePrimaryPlaneResourceDestroyStatus>,
    pub submission: Option<LibdrmNativePrimaryPlaneScanoutSubmission>,
    pub cleanup: Option<LibdrmNativePrimaryPlaneResourceCleanup>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneScanoutRetireStatus {
    RetiredAfterPageFlip,
    WaitingForAcceptedPageFlip,
    ResourceRetireFailed,
}
