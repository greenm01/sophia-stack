//! Whether one framebuffer can drive two connectors at once.
//!
//! Mirroring is one logical output backed by N connectors scanning out one
//! buffer. Every design above this rests on that being possible, and it is a
//! driver property rather than a kernel guarantee: primary planes can carry
//! per-CRTC constraints on address, pitch, tiling, and modifier. Assuming it
//! works and discovering otherwise costs a rewrite of buffer ownership, so the
//! question is asked of the hardware first.
//!
//! Every commit here carries `TEST_ONLY`, so the kernel validates and discards
//! and no output changes. Buffers are allocated -- a shared framebuffer cannot be
//! tested without one -- but they are dumb buffers created and destroyed inside
//! this call, never shown.
//!
//! The probe runs a control beside the real question. A shared-framebuffer
//! rejection means nothing on its own: the driver might be refusing the sharing,
//! or refusing to drive two CRTCs in one commit at all. Running the same request
//! with a framebuffer each separates those two answers, and they send the design
//! somewhere completely different.

use crate::prelude::*;

/// Why a mirror probe could or could not reach a conclusion.
#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeMirrorProbeStatus {
    Probed,
    /// No atomic-capable primary card node was usable.
    DeviceUnavailable,
    /// Fewer than two connected connectors on one card. Mirroring needs two heads
    /// on one device, and this machine cannot present the question.
    SingleHead,
    /// Two connectors exist but share no mode. Mirroring is same-mode only,
    /// because no plane scaling exists on this path, so there is nothing to ask.
    NoCommonMode,
    /// Atomic commits, including validation-only ones, require DRM master.
    /// Another compositor holds it. This is not a refusal of mirroring and must
    /// never be read as one.
    MasterUnavailable,
    /// Property discovery did not find what an atomic modeset needs on both heads.
    PropertiesUnavailable,
    /// A mode blob could not be created.
    ModeBlobUnavailable,
    /// A dumb buffer or framebuffer could not be allocated at the common mode.
    FramebufferUnavailable,
}

/// What the driver said about one two-head request.
#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeMirrorValidation {
    Accepted,
    /// Carries the kernel's errno, because "rejected" alone cannot separate a
    /// malformed request from a driver refusing a well-formed one.
    Rejected(i32),
    NotAttempted,
}

#[cfg(feature = "libdrm-events")]
impl NativeMirrorValidation {
    const fn label(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected(_) => "rejected",
            Self::NotAttempted => "not_attempted",
        }
    }

    /// Zero means no errno was reported, which cannot collide with a real one
    /// because errno 0 is success.
    const fn errno(self) -> i32 {
        match self {
            Self::Rejected(errno) => errno,
            Self::Accepted | Self::NotAttempted => 0,
        }
    }
}

/// What one card admits about driving two connectors from one buffer.
#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeMirrorProbeReport {
    pub status: NativeMirrorProbeStatus,
    pub connected_connectors: usize,
    /// The two connectors the probe used.
    pub connector_ids: (u32, u32),
    /// The highest mode both connectors can present.
    pub common_mode: (u16, u16),
    pub common_mode_refresh_millihz: u32,
    /// Two heads, one framebuffer. The question.
    pub shared_framebuffer: NativeMirrorValidation,
    /// Two heads, a framebuffer each. The control that says whether a shared
    /// rejection was about sharing or about two heads.
    pub separate_framebuffers: NativeMirrorValidation,
    /// Whether one mode blob served both CRTCs. Sharing it is what the group
    /// submit would do, so a driver that refuses is worth knowing about before
    /// the submit is written rather than after.
    pub shared_mode_blob: bool,
}

#[cfg(feature = "libdrm-events")]
impl NativeMirrorProbeReport {
    const fn failed(status: NativeMirrorProbeStatus, connected_connectors: usize) -> Self {
        Self {
            status,
            connected_connectors,
            connector_ids: (0, 0),
            common_mode: (0, 0),
            common_mode_refresh_millihz: 0,
            shared_framebuffer: NativeMirrorValidation::NotAttempted,
            separate_framebuffers: NativeMirrorValidation::NotAttempted,
            shared_mode_blob: false,
        }
    }

    pub fn reduced_log_line(&self) -> String {
        format!(
            "sophia_native_mirror_probe schema=1 status={:?} connected_connectors={} \
connectors={},{} common_mode={}x{}@{} shared_framebuffer={} shared_framebuffer_errno={} \
separate_framebuffers={} separate_framebuffers_errno={} shared_mode_blob={}",
            self.status,
            self.connected_connectors,
            self.connector_ids.0,
            self.connector_ids.1,
            self.common_mode.0,
            self.common_mode.1,
            self.common_mode_refresh_millihz,
            self.shared_framebuffer.label(),
            self.shared_framebuffer.errno(),
            self.separate_framebuffers.label(),
            self.separate_framebuffers.errno(),
            self.shared_mode_blob,
        )
    }

    /// True when the probe reached a conclusion about the sharing question.
    pub const fn answered(&self) -> bool {
        matches!(self.status, NativeMirrorProbeStatus::Probed)
            && !matches!(
                self.shared_framebuffer,
                NativeMirrorValidation::NotAttempted
            )
    }
}

/// How many page-flip events a two-CRTC commit produced, per CRTC.
///
/// The retirement design waits for every head of a group to flip. If a commit
/// naming two CRTCs delivers one event rather than one per CRTC, a group would
/// wait forever, so this is measured rather than assumed -- the same lesson as
/// `TEST_ONLY | PAGE_FLIP_EVENT`, which the kernel rejects outright and which no
/// amount of reasoning about the flags had predicted.
#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeMirrorPageFlipReport {
    pub attempted: bool,
    /// Why not, when the flip could not be attempted at all.
    pub skipped: Option<NativeMirrorPageFlipSkip>,
    pub commit: NativeMirrorValidation,
    /// What the request builder said. A commit that was never submitted has a
    /// reason, and swallowing it leaves "not_attempted" looking like a refusal by
    /// the driver when it was a refusal by our own validation.
    pub build: LibdrmNativeMultiHeadRequestBuildStatus,
    pub events_on_first_crtc: usize,
    pub events_on_second_crtc: usize,
    /// Events that decoded to neither CRTC. Nonzero means the reader saw traffic
    /// the probe cannot attribute, which makes the counts above untrustworthy
    /// rather than merely incomplete.
    pub unattributed_events: usize,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeMirrorPageFlipSkip {
    /// The caller did not ask. A real commit reaches the screen, so it is opt-in.
    NotRequested,
    /// A CRTC is not currently scanning out, so there is nothing to re-flip. The
    /// probe deliberately will not modeset to create the condition: that would
    /// change what is on the display to answer a question about events.
    CrtcInactive,
    /// The two CRTCs already share one framebuffer but run different modes.
    ///
    /// Worth stating plainly, because it is evidence rather than an obstacle: the
    /// kernel console is driving two connectors from a single buffer right now, on
    /// this hardware, which is a stronger demonstration that sharing works than a
    /// validation-only commit can give. What it is not is a mirror group -- a group
    /// is same-mode, and these two are not -- so re-presenting the current state
    /// cannot answer a question about how a group completes. Reshaping the planes
    /// to make it one would change what is on the display, which this probe will
    /// not do.
    SharedFramebufferSizeMismatch,
}

#[cfg(feature = "libdrm-events")]
impl NativeMirrorPageFlipReport {
    const fn skipped(reason: NativeMirrorPageFlipSkip) -> Self {
        Self {
            attempted: false,
            skipped: Some(reason),
            commit: NativeMirrorValidation::NotAttempted,
            build: LibdrmNativeMultiHeadRequestBuildStatus::NoHeads,
            events_on_first_crtc: 0,
            events_on_second_crtc: 0,
            unattributed_events: 0,
        }
    }

    fn label(&self) -> &'static str {
        match self.skipped {
            None => "attempted",
            Some(NativeMirrorPageFlipSkip::NotRequested) => "not_requested",
            Some(NativeMirrorPageFlipSkip::CrtcInactive) => "crtc_inactive",
            Some(NativeMirrorPageFlipSkip::SharedFramebufferSizeMismatch) => {
                "shared_framebuffer_size_mismatch"
            }
        }
    }

    pub fn reduced_log_line(&self) -> String {
        format!(
            "sophia_native_mirror_page_flip schema=1 phase={} build={:?} commit={} \
commit_errno={} events_first_crtc={} events_second_crtc={} unattributed_events={}",
            self.label(),
            self.build,
            self.commit.label(),
            self.commit.errno(),
            self.events_on_first_crtc,
            self.events_on_second_crtc,
            self.unattributed_events,
        )
    }

    /// True when both CRTCs reported exactly the one flip that was submitted.
    ///
    /// Exactly one each, not at least one: a second event on either head would
    /// mean the commit produced more completions than flips, and joint retirement
    /// counts events.
    pub const fn one_event_per_crtc(&self) -> bool {
        self.attempted
            && matches!(self.commit, NativeMirrorValidation::Accepted)
            && self.events_on_first_crtc == 1
            && self.events_on_second_crtc == 1
            && self.unattributed_events == 0
    }
}

/// Probes the default device directory, validation only.
#[cfg(feature = "libdrm-events")]
pub fn native_mirror_probe_report() -> NativeMirrorProbeReport {
    native_mirror_probe_report_from_dev_dri(std::path::Path::new("/dev/dri"))
}

/// Asks how many events a two-CRTC page flip delivers.
///
/// This one commits for real, which is why it is a separate entry point rather
/// than another phase of the probe above. It is still as close to a no-op as the
/// question allows: each CRTC is flipped to the framebuffer it is *already*
/// scanning out, at its current mode, with no `ALLOW_MODESET`. Nothing on screen
/// changes; the kernel simply re-presents what is there and reports completion.
///
/// It cannot be answered with `TEST_ONLY`, because the kernel rejects that
/// together with `PAGE_FLIP_EVENT` before looking at anything else -- and the
/// event behaviour is precisely what is being asked.
#[cfg(feature = "libdrm-events")]
pub fn native_mirror_page_flip_report() -> NativeMirrorPageFlipReport {
    native_mirror_page_flip_report_from_dev_dri(std::path::Path::new("/dev/dri"))
}

#[cfg(feature = "libdrm-events")]
pub fn native_mirror_page_flip_report_from_dev_dri(
    dev_dri: &std::path::Path,
) -> NativeMirrorPageFlipReport {
    use crate::LibdrmNativePageFlipReader as _;

    let selection = select_real_atomic_scanout_cards_from_dev_dri(dev_dri);
    let Some(card) = selection.cards.into_iter().next() else {
        return NativeMirrorPageFlipReport::skipped(NativeMirrorPageFlipSkip::CrtcInactive);
    };
    let targets = select_native_primary_plane_targets(&card.card);
    if targets.selections.len() < 2 {
        return NativeMirrorPageFlipReport::skipped(NativeMirrorPageFlipSkip::CrtcInactive);
    }
    let (first, second) = (targets.selections[0], targets.selections[1]);

    // Each head keeps its own current framebuffer. Sharing one here would change
    // what a screen displays, and this phase is asking about events, not sharing.
    let (Some(first_framebuffer), Some(second_framebuffer)) = (
        current_framebuffer(&card.card, first.crtc_handle()),
        current_framebuffer(&card.card, second.crtc_handle()),
    ) else {
        return NativeMirrorPageFlipReport::skipped(NativeMirrorPageFlipSkip::CrtcInactive);
    };

    let (Some(first_properties), Some(second_properties)) = (
        discover_native_primary_plane_property_handles(
            &card.card,
            first.connector_handle(),
            first.crtc_handle(),
            first.plane_handle(),
        )
        .properties,
        discover_native_primary_plane_property_handles(
            &card.card,
            second.connector_handle(),
            second.crtc_handle(),
            second.plane_handle(),
        )
        .properties,
    ) else {
        return NativeMirrorPageFlipReport::skipped(NativeMirrorPageFlipSkip::CrtcInactive);
    };

    let mut report = NativeMirrorPageFlipReport {
        attempted: true,
        skipped: None,
        commit: NativeMirrorValidation::NotAttempted,
        build: LibdrmNativeMultiHeadRequestBuildStatus::NoHeads,
        events_on_first_crtc: 0,
        events_on_second_crtc: 0,
        unattributed_events: 0,
    };

    let heads = [
        page_flip_head(first, first_properties, first_framebuffer),
        page_flip_head(second, second_properties, second_framebuffer),
    ];
    let build = build_native_multi_head_atomic_request(
        &heads,
        LibdrmNativeAtomicCommitRequestScope::PageFlip,
    );
    report.build = build.status;
    let Some(request) = build.request else {
        if build.status == LibdrmNativeMultiHeadRequestBuildStatus::MismatchedMirrorSize {
            report.attempted = false;
            report.skipped = Some(NativeMirrorPageFlipSkip::SharedFramebufferSizeMismatch);
        }
        return report;
    };

    let (Some(first_slot), Some(second_slot)) = (
        LibdrmNativeOutputSlot::new(1),
        LibdrmNativeOutputSlot::new(2),
    ) else {
        return report;
    };
    let Ok(reader_card) = card.card.try_clone() else {
        return report;
    };
    let mut reader = NativeLibdrmPageFlipEventReader::new(reader_card)
        .with_crtc_routes([first.crtc_route(first_slot), second.crtc_route(second_slot)]);

    // Blocking, so the commit has landed before events are drained. Nonblocking
    // would make an empty first read indistinguishable from "no event ever".
    let (flags, native) = request.blocking().into_native();
    match classify(card.card.submit_atomic_commit(flags, native)) {
        Ok(outcome) => report.commit = outcome,
        Err(_) => return report,
    }
    if report.commit != NativeMirrorValidation::Accepted {
        return report;
    }

    // A refresh at the slowest mode anyone ships is well under this. Reading in
    // slices rather than once keeps a single early empty read from ending the
    // count before the second CRTC has reported.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
    while std::time::Instant::now() < deadline {
        for callback in reader.read_ready_page_flip_callbacks(8).callbacks {
            if callback.output_slot == first_slot {
                report.events_on_first_crtc = report.events_on_first_crtc.saturating_add(1);
            } else if callback.output_slot == second_slot {
                report.events_on_second_crtc = report.events_on_second_crtc.saturating_add(1);
            } else {
                report.unattributed_events = report.unattributed_events.saturating_add(1);
            }
        }
        if report.events_on_first_crtc > 0 && report.events_on_second_crtc > 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    report
}

#[cfg(feature = "libdrm-events")]
fn page_flip_head(
    selection: LibdrmNativePrimaryPlaneSelection,
    properties: LibdrmNativePrimaryPlanePropertyHandles,
    framebuffer: drm::control::framebuffer::Handle,
) -> LibdrmNativeAtomicHead {
    LibdrmNativeAtomicHead::new(selection.into_objects(framebuffer, None), properties)
}

#[cfg(feature = "libdrm-events")]
fn current_framebuffer<D>(
    card: &D,
    crtc: drm::control::crtc::Handle,
) -> Option<drm::control::framebuffer::Handle>
where
    D: drm::control::Device,
{
    card.get_crtc(crtc).ok().and_then(|info| info.framebuffer())
}

/// Asks one card whether two connectors can scan out one framebuffer.
///
/// Validation-only throughout: every commit carries `TEST_ONLY | ALLOW_MODESET`,
/// so nothing reaches a screen. Safe to run on a live console, but it needs DRM
/// master, which means no other compositor may be running.
#[cfg(feature = "libdrm-events")]
pub fn native_mirror_probe_report_from_dev_dri(
    dev_dri: &std::path::Path,
) -> NativeMirrorProbeReport {
    let selection = select_real_atomic_scanout_cards_from_dev_dri(dev_dri);
    let Some(card) = selection.cards.into_iter().next() else {
        return NativeMirrorProbeReport::failed(NativeMirrorProbeStatus::DeviceUnavailable, 0);
    };
    let targets = select_native_primary_plane_targets(&card.card);
    let connected = targets.connected_connectors;
    if targets.selections.len() < 2 {
        return NativeMirrorProbeReport::failed(NativeMirrorProbeStatus::SingleHead, connected);
    }
    let (first, second) = (targets.selections[0], targets.selections[1]);

    let Some(mode) = highest_common_mode(&card.card, first, second) else {
        return NativeMirrorProbeReport::failed(NativeMirrorProbeStatus::NoCommonMode, connected);
    };
    let (width, height) = mode.size();

    let mut report = NativeMirrorProbeReport {
        status: NativeMirrorProbeStatus::Probed,
        connected_connectors: connected,
        connector_ids: (first.connector_id(), second.connector_id()),
        common_mode: (width, height),
        common_mode_refresh_millihz: refresh_millihz(&mode),
        shared_framebuffer: NativeMirrorValidation::NotAttempted,
        separate_framebuffers: NativeMirrorValidation::NotAttempted,
        shared_mode_blob: false,
    };

    let (Some(first_properties), Some(second_properties)) = (
        discover_native_primary_plane_property_handles(
            &card.card,
            first.connector_handle(),
            first.crtc_handle(),
            first.plane_handle(),
        )
        .properties,
        discover_native_primary_plane_property_handles(
            &card.card,
            second.connector_handle(),
            second.crtc_handle(),
            second.plane_handle(),
        )
        .properties,
    ) else {
        return NativeMirrorProbeReport {
            status: NativeMirrorProbeStatus::PropertiesUnavailable,
            ..report
        };
    };

    let Ok(mode_blob) = card.card.create_mode_blob(mode) else {
        return NativeMirrorProbeReport {
            status: NativeMirrorProbeStatus::ModeBlobUnavailable,
            ..report
        };
    };
    // One blob for both CRTCs, which is what a group submit would do. A driver
    // that refuses shows up as a rejection here rather than as a puzzle later.
    report.shared_mode_blob = true;

    let size = Size {
        width: i32::from(width),
        height: i32::from(height),
    };
    let Some(shared) = ProbeFramebuffer::allocate(&card.card, width, height) else {
        let _ = card.card.destroy_mode_blob(mode_blob);
        return NativeMirrorProbeReport {
            status: NativeMirrorProbeStatus::FramebufferUnavailable,
            ..report
        };
    };

    // The question: both heads naming the same framebuffer handle.
    match validate_two_heads(
        &card.card,
        [
            head(first, first_properties, shared.framebuffer, mode_blob, size),
            head(
                second,
                second_properties,
                shared.framebuffer,
                mode_blob,
                size,
            ),
        ],
    ) {
        Ok(outcome) => report.shared_framebuffer = outcome,
        Err(status) => {
            shared.release(&card.card);
            let _ = card.card.destroy_mode_blob(mode_blob);
            return NativeMirrorProbeReport { status, ..report };
        }
    }

    // The control: the same two heads with a framebuffer each. This is the shape
    // that already works today, so a rejection here says the driver refuses two
    // CRTCs in one commit and the shared result above says nothing about sharing.
    if let Some(second_framebuffer) = ProbeFramebuffer::allocate(&card.card, width, height) {
        if let Ok(outcome) = validate_two_heads(
            &card.card,
            [
                head(first, first_properties, shared.framebuffer, mode_blob, size),
                head(
                    second,
                    second_properties,
                    second_framebuffer.framebuffer,
                    mode_blob,
                    size,
                ),
            ],
        ) {
            report.separate_framebuffers = outcome;
        }
        second_framebuffer.release(&card.card);
    }

    shared.release(&card.card);
    let _ = card.card.destroy_mode_blob(mode_blob);
    report
}

#[cfg(feature = "libdrm-events")]
fn head(
    selection: LibdrmNativePrimaryPlaneSelection,
    properties: LibdrmNativePrimaryPlanePropertyHandles,
    framebuffer: drm::control::framebuffer::Handle,
    mode_blob: u64,
    size: Size,
) -> LibdrmNativeAtomicHead {
    LibdrmNativeAtomicHead::new(
        LibdrmNativePrimaryPlaneObjects::new(
            selection.connector_handle(),
            selection.crtc_handle(),
            selection.plane_handle(),
            framebuffer,
            mode_blob,
            size,
        ),
        properties,
    )
}

/// Submits one validation-only two-head modeset.
#[cfg(feature = "libdrm-events")]
fn validate_two_heads<D>(
    card: &D,
    heads: [LibdrmNativeAtomicHead; 2],
) -> Result<NativeMirrorValidation, NativeMirrorProbeStatus>
where
    D: LibdrmNativeAtomicCommitDevice,
{
    let build = build_native_multi_head_atomic_request(
        &heads,
        LibdrmNativeAtomicCommitRequestScope::Modeset,
    );
    let Some(request) = build.request else {
        return Ok(NativeMirrorValidation::NotAttempted);
    };
    let (flags, native) = request.test_only().allow_modeset().into_native();
    classify(card.submit_atomic_commit(flags, native))
}

/// Not-master is an error about the probe rather than about mirroring, so it
/// leaves as a status and never as a rejection.
#[cfg(feature = "libdrm-events")]
fn classify(
    result: std::io::Result<()>,
) -> Result<NativeMirrorValidation, NativeMirrorProbeStatus> {
    match result {
        Ok(()) => Ok(NativeMirrorValidation::Accepted),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::NotFound
            ) =>
        {
            Err(NativeMirrorProbeStatus::MasterUnavailable)
        }
        Err(error) => Ok(NativeMirrorValidation::Rejected(
            error.raw_os_error().unwrap_or(0),
        )),
    }
}

/// The largest mode both connectors can present, by pixel count then refresh.
///
/// Largest rather than either connector's preferred, because the two preferred
/// modes routinely differ and a group has to agree on one. Same-mode is the whole
/// constraint: no plane scaling exists on this path.
#[cfg(feature = "libdrm-events")]
fn highest_common_mode<D>(
    card: &D,
    first: LibdrmNativePrimaryPlaneSelection,
    second: LibdrmNativePrimaryPlaneSelection,
) -> Option<drm::control::Mode>
where
    D: drm::control::Device,
{
    let second_modes = card
        .get_connector(second.connector_handle(), false)
        .ok()?
        .modes()
        .to_vec();
    card.get_connector(first.connector_handle(), false)
        .ok()?
        .modes()
        .iter()
        .copied()
        .filter(|mode| {
            second_modes
                .iter()
                .any(|candidate| candidate.size() == mode.size())
        })
        .max_by_key(|mode| {
            let (width, height) = mode.size();
            (u32::from(width) * u32::from(height), refresh_millihz(mode))
        })
}

/// The same reading of refresh the capability table uses, so the probe's evidence
/// and the running system cannot describe one mode two ways.
#[cfg(feature = "libdrm-events")]
fn refresh_millihz(mode: &drm::control::Mode) -> u32 {
    mode.vrefresh().saturating_mul(1_000)
}

/// A dumb buffer and its framebuffer, alive only for the probe.
#[cfg(feature = "libdrm-events")]
struct ProbeFramebuffer {
    buffer: drm::control::dumbbuffer::DumbBuffer,
    framebuffer: drm::control::framebuffer::Handle,
}

#[cfg(feature = "libdrm-events")]
impl ProbeFramebuffer {
    fn allocate<D>(card: &D, width: u16, height: u16) -> Option<Self>
    where
        D: drm::control::Device,
    {
        let buffer = card
            .create_dumb_buffer(
                (u32::from(width), u32::from(height)),
                drm::buffer::DrmFourcc::Xrgb8888,
                32,
            )
            .ok()?;
        match card.add_framebuffer(&buffer, 24, 32) {
            Ok(framebuffer) => Some(Self {
                buffer,
                framebuffer,
            }),
            Err(_) => {
                let _ = card.destroy_dumb_buffer(buffer);
                None
            }
        }
    }

    fn release<D>(self, card: &D)
    where
        D: drm::control::Device,
    {
        let _ = card.destroy_framebuffer(self.framebuffer);
        let _ = card.destroy_dumb_buffer(self.buffer);
    }
}
