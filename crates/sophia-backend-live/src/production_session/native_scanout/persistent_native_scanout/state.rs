use sophia_protocol::{OutputId, TransactionId};
use std::collections::BTreeSet;
use std::time::Duration;

pub const LIVE_PRODUCTION_PAGE_FLIP_HARD_STALL: Duration = Duration::from_millis(500);

/// Settles an initialization transaction after its fallible KMS work.
///
/// Once at least one physical head exists, a failed initialization may already
/// have installed scanout resources on an earlier head. The caller must therefore
/// run its ownership rollback before returning the initialization error. Keeping
/// this decision in one helper also makes the error-preservation contract
/// independently testable without opening real DRM devices.
pub fn finish_live_production_native_initialization(
    initialized: Result<(), Box<dyn std::error::Error>>,
    rollback_required: bool,
    rollback: impl FnOnce() -> Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let Err(error) = initialized else {
        return Ok(());
    };
    if !rollback_required {
        return Err(error);
    }
    match rollback() {
        Ok(()) => Err(error),
        Err(rollback) => Err(format!(
            "native output initialization failed: {error}; rollback failed: {rollback}"
        )
        .into()),
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LiveProductionNativeFrameId(u64);

impl LiveProductionNativeFrameId {
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionMirrorGroupBegin {
    Started,
    GenerationInFlight,
    Poisoned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionMirrorHeadTransition {
    Accepted,
    GroupReady,
    Duplicate,
    UnknownHead,
    WrongGeneration,
    NotSubmitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionMirrorGenerationQueueTarget {
    Active,
    Successor,
    ReplaceSuccessor(LiveProductionNativeFrameId),
}

/// Admits one logical generation without exposing per-head queue state.
///
/// An active generation is immutable. While it joins, the group owns one
/// latest-wins successor that replaces an older successor atomically.
pub fn reduce_live_production_mirror_generation_queue_target(
    active: Option<LiveProductionNativeFrameId>,
    successor: Option<LiveProductionNativeFrameId>,
) -> LiveProductionMirrorGenerationQueueTarget {
    match (active, successor) {
        (None, _) => LiveProductionMirrorGenerationQueueTarget::Active,
        (Some(_), None) => LiveProductionMirrorGenerationQueueTarget::Successor,
        (Some(_), Some(successor)) => {
            LiveProductionMirrorGenerationQueueTarget::ReplaceSuccessor(successor)
        }
    }
}

/// Joins independently serviced physical heads back into one logical frame.
///
/// Rendering and KMS ownership remain per connector. The Engine-facing frame is
/// complete only after every connector that began the generation has submitted
/// and flipped it. A second generation cannot enter this coordinator while the
/// first is active, so a fast head cannot race ahead and display a newer logical
/// frame than a slower sibling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveProductionMirrorGroupLifecycle {
    output: OutputId,
    required: BTreeSet<u32>,
    initialized: BTreeSet<u32>,
    active: Option<LiveProductionNativeFrameId>,
    active_progress_at: Option<std::time::Instant>,
    submitted: BTreeSet<u32>,
    flipped: BTreeSet<u32>,
    max_flip_ust_usec: Option<u64>,
    failed: bool,
    completed: Option<LiveProductionNativeFrameId>,
}

impl LiveProductionMirrorGroupLifecycle {
    pub fn new(output: OutputId, connectors: impl IntoIterator<Item = u32>) -> Option<Self> {
        let required = connectors.into_iter().collect::<BTreeSet<_>>();
        (!required.is_empty() && !required.contains(&0)).then_some(Self {
            output,
            required,
            initialized: BTreeSet::new(),
            active: None,
            active_progress_at: None,
            submitted: BTreeSet::new(),
            flipped: BTreeSet::new(),
            max_flip_ust_usec: None,
            failed: false,
            completed: None,
        })
    }

    pub const fn output(&self) -> OutputId {
        self.output
    }

    pub fn connectors(&self) -> impl Iterator<Item = u32> + '_ {
        self.required.iter().copied()
    }

    pub fn initialized(&self) -> bool {
        self.initialized == self.required
    }

    pub fn mark_initialized(&mut self, connector: u32) -> LiveProductionMirrorHeadTransition {
        if !self.required.contains(&connector) {
            return LiveProductionMirrorHeadTransition::UnknownHead;
        }
        if !self.initialized.insert(connector) {
            return LiveProductionMirrorHeadTransition::Duplicate;
        }
        if self.initialized() {
            LiveProductionMirrorHeadTransition::GroupReady
        } else {
            LiveProductionMirrorHeadTransition::Accepted
        }
    }

    pub fn begin(&mut self, frame: LiveProductionNativeFrameId) -> LiveProductionMirrorGroupBegin {
        if self.failed {
            return LiveProductionMirrorGroupBegin::Poisoned;
        }
        if self.active.is_some() {
            return LiveProductionMirrorGroupBegin::GenerationInFlight;
        }
        self.active = Some(frame);
        self.active_progress_at = Some(std::time::Instant::now());
        self.submitted.clear();
        self.flipped.clear();
        self.max_flip_ust_usec = None;
        self.completed = None;
        LiveProductionMirrorGroupBegin::Started
    }

    pub const fn active_frame(&self) -> Option<LiveProductionNativeFrameId> {
        self.active
    }

    /// Age since the reserved generation last made physical progress.
    /// This starts at queue reservation and resets when a renderer worker starts,
    /// a head submits, or a head flips, so a head that never reaches
    /// `submitted_at` cannot evade the group watchdog.
    pub fn active_age(&self) -> Option<Duration> {
        self.active_progress_at.map(|progress| progress.elapsed())
    }

    pub fn active_generation_hard_stalled(&self, hard_stall: Duration) -> bool {
        self.active_age().is_some_and(|age| age >= hard_stall)
    }

    pub fn observe_physical_progress(&mut self, frame: LiveProductionNativeFrameId) -> bool {
        if self.active != Some(frame) {
            return false;
        }
        self.active_progress_at = Some(std::time::Instant::now());
        true
    }

    pub const fn failed(&self) -> bool {
        self.failed
    }

    pub fn awaiting_flips(&self) -> bool {
        self.active.is_some() && self.submitted == self.required
    }

    /// Logical identity retained after every required head has submitted and
    /// until the final physical callback completes the group join.
    pub fn logically_submitted_frame(&self) -> Option<LiveProductionNativeFrameId> {
        self.awaiting_flips().then_some(self.active).flatten()
    }

    /// Whether this connector may consume renderer work for the current turn.
    ///
    /// A physical callback clears the head's KMS submission before its siblings
    /// necessarily finish the logical generation. Membership in `submitted`
    /// therefore remains the generation fence even after that early callback.
    pub fn connector_may_submit(&self, connector: u32) -> bool {
        self.required.contains(&connector)
            && (self.active.is_none() || !self.submitted.contains(&connector))
    }

    /// Whether this connector may submit this exact logical generation.
    ///
    /// The connector fence prevents a fast head from submitting twice, while
    /// the frame fence prevents renderer work queued for the successor from
    /// being relabeled as the still-active generation.
    pub fn connector_may_submit_frame(
        &self,
        connector: u32,
        frame: LiveProductionNativeFrameId,
    ) -> bool {
        self.connector_may_submit(connector) && self.active.is_none_or(|active| active == frame)
    }

    /// Poisons a partially submitted generation.
    ///
    /// Already accepted physical commits still own their resources and must
    /// drain, but no later tick may submit another logical generation or publish
    /// this one as presented.
    pub fn abort(&mut self, frame: LiveProductionNativeFrameId) -> bool {
        if self.active != Some(frame) {
            return false;
        }
        self.failed = true;
        true
    }

    pub fn mark_submitted(
        &mut self,
        connector: u32,
        frame: LiveProductionNativeFrameId,
    ) -> LiveProductionMirrorHeadTransition {
        if !self.required.contains(&connector) {
            return LiveProductionMirrorHeadTransition::UnknownHead;
        }
        if self.active != Some(frame) {
            return LiveProductionMirrorHeadTransition::WrongGeneration;
        }
        if !self.submitted.insert(connector) {
            return LiveProductionMirrorHeadTransition::Duplicate;
        }
        self.active_progress_at = Some(std::time::Instant::now());
        if self.submitted == self.required {
            LiveProductionMirrorHeadTransition::GroupReady
        } else {
            LiveProductionMirrorHeadTransition::Accepted
        }
    }

    pub fn mark_flipped(
        &mut self,
        connector: u32,
        frame: LiveProductionNativeFrameId,
    ) -> LiveProductionMirrorHeadTransition {
        if !self.required.contains(&connector) {
            return LiveProductionMirrorHeadTransition::UnknownHead;
        }
        if self.active != Some(frame) {
            return LiveProductionMirrorHeadTransition::WrongGeneration;
        }
        if !self.submitted.contains(&connector) {
            return LiveProductionMirrorHeadTransition::NotSubmitted;
        }
        if !self.flipped.insert(connector) {
            return LiveProductionMirrorHeadTransition::Duplicate;
        }
        self.active_progress_at = Some(std::time::Instant::now());
        if self.flipped == self.required {
            self.active = None;
            self.active_progress_at = None;
            self.completed = Some(frame);
            LiveProductionMirrorHeadTransition::GroupReady
        } else {
            LiveProductionMirrorHeadTransition::Accepted
        }
    }

    /// Records physical timing for the active generation.
    ///
    /// Logical feedback uses the latest physical UST and the logical generation
    /// as MSC. Kernel sequences belong to individual CRTCs and cannot form one
    /// coherent output-wide sequence.
    pub fn observe_flip_timing(
        &mut self,
        frame: LiveProductionNativeFrameId,
        _serial: u64,
        ust_usec: u64,
    ) -> bool {
        if self.active != Some(frame) {
            return false;
        }
        self.max_flip_ust_usec = Some(
            self.max_flip_ust_usec
                .map_or(ust_usec, |old| old.max(ust_usec)),
        );
        true
    }

    pub const fn flip_timing(&self) -> Option<(u64, u64)> {
        let frame = match self.completed {
            Some(frame) => frame,
            None => match self.active {
                Some(frame) => frame,
                None => return None,
            },
        };
        match self.max_flip_ust_usec {
            Some(ust_usec) => Some((frame.raw(), ust_usec)),
            None => None,
        }
    }

    pub const fn completed_frame(&self) -> Option<LiveProductionNativeFrameId> {
        self.completed
    }

    pub fn take_completed_frame(&mut self) -> Option<LiveProductionNativeFrameId> {
        self.completed.take()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionPageFlipWatchdogStatus {
    Idle,
    Healthy,
    HardStall,
}

pub fn reduce_live_production_page_flip_watchdog(
    submitted_age: Option<Duration>,
    hard_stall: Duration,
) -> LiveProductionPageFlipWatchdogStatus {
    match submitted_age {
        None => LiveProductionPageFlipWatchdogStatus::Idle,
        Some(age) if age >= hard_stall => LiveProductionPageFlipWatchdogStatus::HardStall,
        Some(_) => LiveProductionPageFlipWatchdogStatus::Healthy,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveProductionScanoutContent {
    Cpu {
        frame: LiveProductionNativeFrameId,
        checksum: u64,
    },
    MixedPresent {
        frame: LiveProductionNativeFrameId,
        transaction: TransactionId,
        nonzero_rgb_pixels: usize,
    },
    RetainedMixed {
        frame: LiveProductionNativeFrameId,
        nonzero_rgb_pixels: usize,
    },
}

impl LiveProductionScanoutContent {
    pub const fn frame(self) -> LiveProductionNativeFrameId {
        match self {
            Self::Cpu { frame, .. }
            | Self::MixedPresent { frame, .. }
            | Self::RetainedMixed { frame, .. } => frame,
        }
    }

    pub const fn with_nonzero_rgb_pixels(self, nonzero_rgb_pixels: usize) -> Self {
        match self {
            Self::MixedPresent {
                frame, transaction, ..
            } => Self::MixedPresent {
                frame,
                transaction,
                nonzero_rgb_pixels,
            },
            Self::RetainedMixed { frame, .. } => Self::RetainedMixed {
                frame,
                nonzero_rgb_pixels,
            },
            cpu @ Self::Cpu { .. } => cpu,
        }
    }

    pub const fn cpu_checksum(self) -> Option<u64> {
        match self {
            Self::Cpu { checksum, .. } => Some(checksum),
            Self::MixedPresent { .. } | Self::RetainedMixed { .. } => None,
        }
    }

    pub const fn source_label(self) -> &'static str {
        match self {
            Self::Cpu { .. } => "cpu",
            Self::MixedPresent { .. } => "mixed_present",
            Self::RetainedMixed { .. } => "retained_mixed",
        }
    }

    pub fn same_logical_identity(self, other: Self) -> bool {
        match (self, other) {
            (
                Self::Cpu { frame, checksum },
                Self::Cpu {
                    frame: other_frame,
                    checksum: other_checksum,
                },
            ) => frame == other_frame && checksum == other_checksum,
            (
                Self::MixedPresent {
                    frame, transaction, ..
                },
                Self::MixedPresent {
                    frame: other_frame,
                    transaction: other_transaction,
                    ..
                },
            ) => frame == other_frame && transaction == other_transaction,
            (
                Self::RetainedMixed { frame, .. },
                Self::RetainedMixed {
                    frame: other_frame, ..
                },
            ) => frame == other_frame,
            _ => false,
        }
    }
}

/// Identifies the renderer work that the next exporter poll will complete.
///
/// An in-flight worker owns `rendering_content`; `pending_content` can already
/// contain the next generation and must not be used to identify that result.
pub fn live_production_mirror_head_work_frame(
    worker_in_flight: bool,
    rendering_content: Option<LiveProductionScanoutContent>,
    pending_content: Option<LiveProductionScanoutContent>,
) -> Option<LiveProductionNativeFrameId> {
    if worker_in_flight {
        rendering_content
    } else {
        pending_content
    }
    .map(LiveProductionScanoutContent::frame)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveProductionNativeFrameRetirement {
    pub output: OutputId,
    pub frame: LiveProductionNativeFrameId,
    pub submission: u64,
    pub content: LiveProductionScanoutContent,
    pub ust: u64,
    pub msc: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionCpuFrameQueueStatus {
    Queued,
    /// The named logical output has no head to queue against. Distinct from every
    /// other status here, which describe a head declining the frame: this one says
    /// the frame was addressed to something that is not a screen.
    NoHead,
    BaselineRequired,
    GpuFrameOwned,
    UnchangedPending,
    UnchangedSubmitted,
    UnchangedPresented,
}

pub fn reduce_live_production_cpu_frame_queue(
    pending: Option<LiveProductionScanoutContent>,
    submitted: Option<LiveProductionScanoutContent>,
    presented: Option<LiveProductionScanoutContent>,
    renderer_in_flight: bool,
    callback_observed: bool,
    checksum: u64,
) -> LiveProductionCpuFrameQueueStatus {
    if renderer_in_flight
        || matches!(
            pending,
            Some(
                LiveProductionScanoutContent::MixedPresent { .. }
                    | LiveProductionScanoutContent::RetainedMixed { .. }
            )
        )
        || matches!(
            submitted,
            Some(
                LiveProductionScanoutContent::MixedPresent { .. }
                    | LiveProductionScanoutContent::RetainedMixed { .. }
            )
        )
    {
        LiveProductionCpuFrameQueueStatus::GpuFrameOwned
    } else if matches!(pending, Some(LiveProductionScanoutContent::Cpu {
        checksum: pending_checksum,
        ..
    }) if pending_checksum == checksum)
    {
        LiveProductionCpuFrameQueueStatus::UnchangedPending
    } else if matches!(submitted, Some(LiveProductionScanoutContent::Cpu {
        checksum: submitted_checksum,
        ..
    }) if submitted_checksum == checksum)
    {
        LiveProductionCpuFrameQueueStatus::UnchangedSubmitted
    } else if pending.is_none()
        && submitted.is_none()
        && matches!(presented, Some(LiveProductionScanoutContent::Cpu {
            checksum: presented_checksum,
            ..
        }) if presented_checksum == checksum)
        && !callback_observed
    {
        LiveProductionCpuFrameQueueStatus::BaselineRequired
    } else if pending.is_none()
        && submitted.is_none()
        && matches!(presented, Some(LiveProductionScanoutContent::Cpu {
            checksum: presented_checksum,
            ..
        }) if presented_checksum == checksum)
    {
        LiveProductionCpuFrameQueueStatus::UnchangedPresented
    } else {
        LiveProductionCpuFrameQueueStatus::Queued
    }
}

pub fn live_production_scanout_is_stable_present(
    presented: Option<LiveProductionScanoutContent>,
    submitted: Option<LiveProductionScanoutContent>,
    pending: bool,
    transaction: TransactionId,
) -> bool {
    matches!(
        presented,
        Some(LiveProductionScanoutContent::MixedPresent {
            transaction: presented_transaction,
            nonzero_rgb_pixels,
            ..
        }) if presented_transaction == transaction && nonzero_rgb_pixels > 0
    ) && submitted.is_none()
        && !pending
}
