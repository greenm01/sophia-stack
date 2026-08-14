use crate::prelude::*;

pub const LIVE_RENDERED_OUTPUT_CAPACITY: usize = 16;

pub struct LiveRenderedOutputState {
    pub(crate) output: OutputId,
    pub(crate) output_size: Option<Size>,
    pub(crate) scanout_readiness: LiveScanoutReadinessReport,
    pub(crate) kms_scanout_target: LiveKmsScanoutTargetReport,
    pub(crate) gbm_egl_frame_target: Option<LiveGbmEglFrameTargetRecord>,
    pub(crate) gbm_egl_frame_target_lifecycle: Option<LiveGbmEglFrameTargetLifecycleReport>,
    pub(crate) gbm_egl_frame_target_allocation: Option<LiveGbmEglFrameTargetAllocationReport>,
    pub(crate) page_flip_event: LivePageFlipEvent,
    pub(crate) page_flip_callback_intake: LivePageFlipCallbackIntake,

    pub(crate) vrr_decision: OutputVrrDecision,
    pub(crate) vrr_property_request: Option<bool>,
    /// Every connector backing this logical output, in head order.
    ///
    /// One entry is the ordinary desktop; several is a mirror group. Empty means
    /// no head has been configured, which is the headless path -- there is no
    /// hardware to wait for and nothing to be joint about.
    ///
    /// This is the single source of truth for the group. The connector set that
    /// retirement waits on is derived from it rather than stored beside it, so the
    /// heads that were submitted and the heads that are awaited cannot disagree.
    #[cfg(feature = "libdrm-events")]
    pub(crate) native_selections: Vec<LibdrmNativePrimaryPlaneSelection>,
    #[cfg(feature = "libdrm-events")]
    pub(crate) rendered_primary_plane_scanout_submission:
        Option<BoxedRenderedPrimaryPlaneScanoutSubmission>,
    #[cfg(feature = "libdrm-events")]
    pub(crate) rendered_primary_plane_displayed_submission:
        Option<BoxedRenderedPrimaryPlaneScanoutSubmission>,
    #[cfg(feature = "libdrm-events")]
    pub(crate) retain_rendered_primary_plane_displayed_submission: bool,
    #[cfg(feature = "libdrm-events")]
    pub(crate) rendered_primary_plane_scanout_cleanup:
        Option<BoxedRenderedPrimaryPlaneScanoutCleanup>,
    #[cfg(feature = "libdrm-events")]
    pub(crate) rendered_primary_plane_runtime_scanout_state: Option<RuntimeScanoutState>,
    #[cfg(feature = "libdrm-events")]
    pub(crate) rendered_primary_plane_scanout_in_flight_ticks: u64,
    #[cfg(feature = "libdrm-events")]
    pub(crate) pending_runtime_scanout_states: VecDeque<RuntimeScanoutState>,
}

impl LiveRenderedOutputState {
    pub fn ready(output: HeadlessOutput) -> Self {
        let presentation = LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Ready,
        };
        let scanout_readiness =
            LiveScanoutReadinessReport::from_output_and_presentation(true, presentation);
        let frame_target = LiveGbmEglFrameTargetRecord::new(output.size);
        let kms_scanout_target = LiveKmsScanoutTargetReport::from_output_target_and_presentation(
            Some(output.size),
            Some(frame_target),
            presentation,
        );
        Self {
            output: output.id,
            output_size: Some(output.size),
            scanout_readiness,
            kms_scanout_target,
            gbm_egl_frame_target: Some(frame_target),
            gbm_egl_frame_target_lifecycle: Some(LiveGbmEglFrameTargetLifecycleReport::created(
                frame_target,
            )),
            gbm_egl_frame_target_allocation: None,
            page_flip_event: LivePageFlipEvent::from_kms_scanout_target_status(
                kms_scanout_target.status,
            ),
            page_flip_callback_intake: LivePageFlipCallbackIntake::new(output.id),
            vrr_decision: OutputVrrDecision::DisabledByPolicy,
            vrr_property_request: None,
            #[cfg(feature = "libdrm-events")]
            native_selections: Vec::new(),
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_submission: None,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_displayed_submission: None,
            #[cfg(feature = "libdrm-events")]
            retain_rendered_primary_plane_displayed_submission: false,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_cleanup: None,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_runtime_scanout_state: None,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_in_flight_ticks: 0,
            #[cfg(feature = "libdrm-events")]
            pending_runtime_scanout_states: VecDeque::new(),
        }
    }

    pub const fn output(&self) -> OutputId {
        self.output
    }

    pub const fn output_size(&self) -> Option<Size> {
        self.output_size
    }

    pub const fn vrr_decision(&self) -> OutputVrrDecision {
        self.vrr_decision
    }

    #[cfg(feature = "libdrm-events")]
    pub fn in_flight(&self) -> bool {
        self.rendered_primary_plane_scanout_submission.is_some()
    }

    #[cfg(feature = "libdrm-events")]
    pub fn cleanup_pending(&self) -> bool {
        self.rendered_primary_plane_scanout_cleanup.is_some()
    }

    /// The head this output is addressed through, which is its first.
    #[cfg(feature = "libdrm-events")]
    pub fn native_selection(&self) -> Option<LibdrmNativePrimaryPlaneSelection> {
        self.native_selections.first().copied()
    }

    /// Every head of this output beyond the first.
    #[cfg(feature = "libdrm-events")]
    pub fn native_peer_selections(&self) -> &[LibdrmNativePrimaryPlaneSelection] {
        self.native_selections.get(1..).unwrap_or_default()
    }

    /// The connectors backing this output, derived from its selections.
    #[cfg(feature = "libdrm-events")]
    pub fn head_connectors(&self) -> impl Iterator<Item = u32> + '_ {
        self.native_selections
            .iter()
            .map(|selection| selection.connector_id())
    }

    /// Whether a mirror group still has a head that has not flipped this submission.
    ///
    /// Derived rather than counted: the intake already records the newest serial
    /// per head, and the submission already records the serial it was submitted
    /// after, so "this head has flipped since we submitted" is a comparison rather
    /// than a second tally that could disagree with the first.
    ///
    /// Only a group can be waiting. With one head there is nothing joint about
    /// retirement and the single-head rules decide it, so this deliberately cannot
    /// change what an unmirrored desktop does.
    #[cfg(feature = "libdrm-events")]
    pub fn group_awaiting_flip(&self, submitted_after_page_flip_serial: Option<u64>) -> bool {
        self.native_selections.len() > 1
            && self.head_connectors().any(|connector| {
                match (
                    self.page_flip_callback_intake.head_frame_serial(connector),
                    submitted_after_page_flip_serial,
                ) {
                    (None, _) => true,
                    (Some(serial), Some(baseline)) => serial <= baseline,
                    (Some(_), None) => false,
                }
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRenderedOutputTableUpdate {
    Inserted,
    Replaced,
    CapacityExceeded,
}

#[derive(Default)]
pub struct LiveRenderedOutputTable {
    pub(crate) outputs: BTreeMap<OutputId, LiveRenderedOutputState>,
}

impl LiveRenderedOutputTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, state: LiveRenderedOutputState) -> LiveRenderedOutputTableUpdate {
        let output = state.output;
        if let std::collections::btree_map::Entry::Occupied(mut e) = self.outputs.entry(output) {
            e.insert(state);
            return LiveRenderedOutputTableUpdate::Replaced;
        }
        if self.outputs.len() >= LIVE_RENDERED_OUTPUT_CAPACITY {
            return LiveRenderedOutputTableUpdate::CapacityExceeded;
        }
        self.outputs.insert(output, state);
        LiveRenderedOutputTableUpdate::Inserted
    }

    pub fn get(&self, output: OutputId) -> Option<&LiveRenderedOutputState> {
        self.outputs.get(&output)
    }

    pub(crate) fn get_mut(&mut self, output: OutputId) -> Option<&mut LiveRenderedOutputState> {
        self.outputs.get_mut(&output)
    }

    pub fn outputs(&self) -> impl Iterator<Item = &LiveRenderedOutputState> {
        self.outputs.values()
    }

    pub fn len(&self) -> usize {
        self.outputs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.outputs.is_empty()
    }

    pub fn remove(&mut self, output: OutputId) -> Option<LiveRenderedOutputState> {
        self.outputs.remove(&output)
    }
}
