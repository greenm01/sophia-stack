use crate::prelude::*;

/// Why a topology probe could or could not reach a conclusion.
#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeTopologyProbeStatus {
    Probed,
    /// No atomic-capable primary card node was usable.
    DeviceUnavailable,
    /// Atomic commits, including validation-only ones, require DRM master. Another
    /// compositor holds it, so nothing could be validated. This is not a rejected
    /// topology and must never be read as one.
    MasterUnavailable,
    /// The selected connector reports no usable mode, so no modeset can be named.
    NoUsableMode,
    /// Property discovery did not find the connector, CRTC, and plane properties an
    /// atomic modeset needs.
    PropertiesUnavailable,
    /// A mode blob could not be created.
    ModeBlobUnavailable,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeTopologyValidationOutcome {
    Accepted,
    /// Carries the kernel's errno. A rejection without one is unrepresentable
    /// deliberately: "rejected" alone cannot separate a malformed request from a
    /// driver refusing a well-formed one, and those two answers send the caller
    /// down different paths.
    Rejected(i32),
    NotAttempted,
}

#[cfg(feature = "libdrm-events")]
impl NativeTopologyValidationOutcome {
    const fn label(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected(_) => "rejected",
            Self::NotAttempted => "not_attempted",
        }
    }

    /// Zero means the kernel reported no errno, which cannot collide with a real
    /// one because errno 0 is success.
    const fn errno(self) -> i32 {
        match self {
            Self::Rejected(errno) => errno,
            Self::Accepted | Self::NotAttempted => 0,
        }
    }
}

/// What one card's current topology admits, and whether a validation-only modeset
/// needs a framebuffer.
///
/// The framebuffer question is driver-dependent and decides how much machinery a
/// topology change needs: if validation requires a framebuffer, then validating a
/// new mode means allocating one at that mode's size before anything can be
/// checked. Answering it by probe rather than by assumption is the point of this
/// report.
#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeTopologyProbeReport {
    pub status: NativeTopologyProbeStatus,
    pub connected_connectors: usize,
    pub modes_on_selected_connector: usize,
    /// A validation-only modeset naming just the connector's CRTC_ID and the
    /// CRTC's MODE_ID and ACTIVE. No plane state at all.
    pub without_plane_state: NativeTopologyValidationOutcome,
    /// The same request plus primary plane state, reusing the CRTC's current
    /// framebuffer so nothing is allocated and nothing changes.
    pub with_current_framebuffer: NativeTopologyValidationOutcome,
    /// Whether the CRTC had a current framebuffer to reuse. Without one the second
    /// probe cannot run, which is itself worth recording.
    pub reused_current_framebuffer: bool,
    /// The mode the probe asked for, in pixels.
    pub mode_size: (u16, u16),
    /// The reused framebuffer's own size, when there was one. A disagreement
    /// between these two sizes explains a second-probe rejection that has nothing
    /// to do with `FB_ID` policy, so reporting one without the other would invite
    /// exactly the wrong conclusion.
    pub framebuffer_size: Option<(u32, u32)>,
}

#[cfg(feature = "libdrm-events")]
impl NativeTopologyProbeReport {
    const fn failed(status: NativeTopologyProbeStatus) -> Self {
        Self {
            status,
            connected_connectors: 0,
            modes_on_selected_connector: 0,
            without_plane_state: NativeTopologyValidationOutcome::NotAttempted,
            with_current_framebuffer: NativeTopologyValidationOutcome::NotAttempted,
            reused_current_framebuffer: false,
            mode_size: (0, 0),
            framebuffer_size: None,
        }
    }

    pub fn reduced_log_line(&self) -> String {
        let (framebuffer_width, framebuffer_height) = self.framebuffer_size.unwrap_or((0, 0));
        format!(
            "sophia_native_topology_probe schema=2 status={:?} connected_connectors={} \
modes={} mode_size={}x{} framebuffer_size={}x{} without_plane_state={} \
without_plane_state_errno={} with_current_framebuffer={} \
with_current_framebuffer_errno={} reused_framebuffer={}",
            self.status,
            self.connected_connectors,
            self.modes_on_selected_connector,
            self.mode_size.0,
            self.mode_size.1,
            framebuffer_width,
            framebuffer_height,
            self.without_plane_state.label(),
            self.without_plane_state.errno(),
            self.with_current_framebuffer.label(),
            self.with_current_framebuffer.errno(),
            self.reused_current_framebuffer
        )
    }

    /// True when the probe reached a conclusion about the framebuffer question.
    pub const fn answered(&self) -> bool {
        matches!(self.status, NativeTopologyProbeStatus::Probed)
            && !matches!(
                self.without_plane_state,
                NativeTopologyValidationOutcome::NotAttempted
            )
    }
}

/// Probes the default device directory.
#[cfg(feature = "libdrm-events")]
pub fn native_topology_probe_report() -> NativeTopologyProbeReport {
    native_topology_probe_report_from_dev_dri(std::path::Path::new("/dev/dri"))
}

/// Probes one card without changing any output state.
///
/// Every commit this issues carries `TEST_ONLY`, so the kernel validates and
/// discards. The only framebuffer involved is one the CRTC is already scanning
/// out, so nothing is allocated either. The probe is safe to run on a live
/// console, but it needs DRM master, which means no other compositor may be
/// running.
#[cfg(feature = "libdrm-events")]
pub fn native_topology_probe_report_from_dev_dri(
    dev_dri: &std::path::Path,
) -> NativeTopologyProbeReport {
    let selection = select_real_atomic_scanout_card_from_dev_dri(dev_dri);
    let (Some(card), Some(selected)) = (selection.card, selection.selection) else {
        return NativeTopologyProbeReport::failed(NativeTopologyProbeStatus::DeviceUnavailable);
    };
    let Some(mode) = selected.mode else {
        return NativeTopologyProbeReport::failed(NativeTopologyProbeStatus::NoUsableMode);
    };

    let connected_connectors = count_connected_connectors(&card);
    let modes_on_selected_connector =
        drm::control::Device::get_connector(&card, selected.connector, false)
            .map_or(0, |connector| connector.modes().len());

    let discovery = discover_native_primary_plane_property_handles(
        &card,
        selected.connector,
        selected.crtc,
        selected.plane,
    );
    let Some(properties) = discovery.properties else {
        return NativeTopologyProbeReport {
            connected_connectors,
            modes_on_selected_connector,
            ..NativeTopologyProbeReport::failed(NativeTopologyProbeStatus::PropertiesUnavailable)
        };
    };

    let Ok(mode_blob) = card.create_mode_blob(mode) else {
        return NativeTopologyProbeReport {
            connected_connectors,
            modes_on_selected_connector,
            ..NativeTopologyProbeReport::failed(NativeTopologyProbeStatus::ModeBlobUnavailable)
        };
    };

    let mut report = NativeTopologyProbeReport {
        status: NativeTopologyProbeStatus::Probed,
        connected_connectors,
        modes_on_selected_connector,
        without_plane_state: NativeTopologyValidationOutcome::NotAttempted,
        with_current_framebuffer: NativeTopologyValidationOutcome::NotAttempted,
        reused_current_framebuffer: false,
        mode_size: mode.size(),
        framebuffer_size: None,
    };

    // Probe one: connector and CRTC state only. If the kernel accepts this, a
    // topology can be validated before any framebuffer exists for it.
    let mut request = drm::control::atomic::AtomicModeReq::new();
    request.add_property(
        selected.connector,
        properties.connector_crtc_id,
        drm::control::property::Value::CRTC(Some(selected.crtc)),
    );
    request.add_property(
        selected.crtc,
        properties.crtc_mode_id,
        drm::control::property::Value::Blob(mode_blob),
    );
    request.add_property(
        selected.crtc,
        properties.crtc_active,
        drm::control::property::Value::Boolean(true),
    );
    match validate(&card, request) {
        Ok(outcome) => report.without_plane_state = outcome,
        Err(status) => {
            let _ = card.destroy_mode_blob(mode_blob);
            return NativeTopologyProbeReport {
                connected_connectors,
                modes_on_selected_connector,
                ..NativeTopologyProbeReport::failed(status)
            };
        }
    }

    // Probe two: the same request plus plane state, reusing whatever framebuffer
    // the CRTC already scans out.
    if let Some(framebuffer) = current_framebuffer(&card, selected.crtc) {
        report.reused_current_framebuffer = true;
        report.framebuffer_size = drm::control::Device::get_framebuffer(&card, framebuffer)
            .ok()
            .map(|info| info.size());
        let head = LibdrmNativeAtomicHead::new(
            selected.into_objects(framebuffer, Some(mode_blob)),
            properties,
        );
        let build = build_native_multi_head_atomic_request(
            &[head],
            LibdrmNativeAtomicCommitRequestScope::Modeset,
        );
        if let Some(request) = build.request {
            let (flags, native) = request.test_only().allow_modeset().into_native();
            // Probe one already reached the kernel, so the master case is settled and
            // a second `MasterUnavailable` is unreachable. Leaving the outcome
            // `NotAttempted` if it somehow occurs beats recording a status that
            // contradicts the probe that just succeeded.
            if let Ok(outcome) = classify(card.submit_atomic_commit(flags, native)) {
                report.with_current_framebuffer = outcome;
            }
        }
    }

    let _ = card.destroy_mode_blob(mode_blob);
    report
}

/// Submits one validation-only commit, separating "not master" from "refused".
#[cfg(feature = "libdrm-events")]
fn validate<D>(
    card: &D,
    request: drm::control::atomic::AtomicModeReq,
) -> Result<NativeTopologyValidationOutcome, NativeTopologyProbeStatus>
where
    D: LibdrmNativeAtomicCommitDevice,
{
    let commit = LibdrmNativeAtomicCommitRequest::modeset(request)
        .test_only()
        .allow_modeset();
    let (flags, native) = commit.into_native();
    classify(card.submit_atomic_commit(flags, native))
}

/// Turns one commit result into an outcome, retaining the errno on a refusal.
///
/// Not-master is an error about the probe rather than about the topology, so it
/// leaves as a status and never as a rejection. Everything else is the kernel
/// judging the request, and the errno is the only thing that says which judgement.
#[cfg(feature = "libdrm-events")]
fn classify(
    result: io::Result<()>,
) -> Result<NativeTopologyValidationOutcome, NativeTopologyProbeStatus> {
    match result {
        Ok(()) => Ok(NativeTopologyValidationOutcome::Accepted),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound
            ) =>
        {
            Err(NativeTopologyProbeStatus::MasterUnavailable)
        }
        Err(error) => Ok(NativeTopologyValidationOutcome::Rejected(
            error.raw_os_error().unwrap_or(0),
        )),
    }
}

#[cfg(feature = "libdrm-events")]
fn count_connected_connectors<D>(card: &D) -> usize
where
    D: drm::control::Device,
{
    card.resource_handles().map_or(0, |handles| {
        handles
            .connectors()
            .iter()
            .filter(|connector| {
                card.get_connector(**connector, false)
                    .is_ok_and(|info| info.state() == drm::control::connector::State::Connected)
            })
            .count()
    })
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
