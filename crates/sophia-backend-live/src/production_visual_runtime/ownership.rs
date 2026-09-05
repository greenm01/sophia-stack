//! Who owns a frame retirement.
//!
//! The kernel retires what it scanned out, not what the scheduler currently
//! names, so a frame can retire after a later present has displaced it. That
//! is ordinary; a retirement naming a frame this session never submitted is
//! not, and telling the two apart is the whole job here.
//!
//! `PresentMixedOwnership` in `validation/tla` states the rule, and its
//! negative control is the version that judges by the current frame alone.

use super::*;

/// Everything that disagreed when a retirement failed the ownership check.
///
/// The check used to end the session with a sentence naming nothing, so the
/// only way to tell which transition produced it was to read the source and
/// infer from surrounding log lines. These are the facts that separate the
/// candidates, carried to the message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveProductionNativeOwnershipMismatch {
    pub retired_frame: LiveProductionNativeFrameId,
    pub content_frame: LiveProductionNativeFrameId,
    pub content_transaction: Option<TransactionId>,
    pub submitted_frame: Option<LiveProductionNativeFrameId>,
    pub in_flight_frame: Option<LiveProductionNativeFrameId>,
    pub in_flight_transaction: Option<TransactionId>,
}

impl LiveProductionNativeOwnershipMismatch {
    /// Which shape of disagreement this is.
    ///
    /// A scheduler holding nothing, a scheduler holding a later present, and a
    /// frame reserved but never submitted are three different bugs. Reporting
    /// them with one sentence is what made this expensive to diagnose.
    pub fn kind(self) -> &'static str {
        if self.content_frame != self.retired_frame {
            "content_names_another_frame"
        } else if self.in_flight_frame == Some(self.retired_frame) {
            "reserved_but_not_submitted"
        } else if self.submitted_frame.is_none() && self.in_flight_frame.is_none() {
            "no_present_in_flight"
        } else {
            "superseded_by_later_present"
        }
    }
}

impl core::fmt::Display for LiveProductionNativeOwnershipMismatch {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            formatter,
            "DMA Present retired on a native frame with different ownership: \
kind={} retired_frame={} content_frame={} content_transaction={:?} \
submitted_frame={:?} in_flight_frame={:?} in_flight_transaction={:?}",
            self.kind(),
            self.retired_frame.raw(),
            self.content_frame.raw(),
            self.content_transaction.map(TransactionId::raw),
            self.submitted_frame.map(LiveProductionNativeFrameId::raw),
            self.in_flight_frame.map(LiveProductionNativeFrameId::raw),
            self.in_flight_transaction.map(TransactionId::raw),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionNativeRetirementOwner {
    IndependentFrame,
    SubmittedDmaPresent,
    /// A frame this session submitted, displaced by a later present before the
    /// kernel retired it. Ordinary: the kernel retires what it scanned out,
    /// not what the scheduler now names. It settles without advancing present
    /// feedback, because a superseded frame is not the frame on glass.
    SupersededDmaPresent,
    InvalidDmaOwnership,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionNativeSubmissionOwner {
    IndependentFrame,
    SubmittedDmaPresent,
    InvalidDmaOwnership,
}

/// Classifies a physical submission before any Present state advances.
///
/// Ordinary compositor frames and DMA-BUF Presents share the same native
/// scheduler. The content tag and the output-scoped Present reservation must
/// therefore agree; the mere fact that KMS accepted a frame does not make it a
/// Present submission.
pub fn reduce_live_production_native_submission_owner(
    submitted_content: LiveProductionScanoutContent,
    expected_present: Option<(LiveProductionNativeFrameId, TransactionId)>,
) -> LiveProductionNativeSubmissionOwner {
    match (submitted_content, expected_present) {
        (
            LiveProductionScanoutContent::MixedPresent {
                frame, transaction, ..
            },
            Some((expected_frame, expected_transaction)),
        ) if frame == expected_frame && transaction == expected_transaction => {
            LiveProductionNativeSubmissionOwner::SubmittedDmaPresent
        }
        (LiveProductionScanoutContent::MixedPresent { .. }, _) | (_, Some(_)) => {
            LiveProductionNativeSubmissionOwner::InvalidDmaOwnership
        }
        (_, None) => LiveProductionNativeSubmissionOwner::IndependentFrame,
    }
}

impl LiveProductionVisualRuntime {
    /// Assembles the mismatch record from the retirement and what the
    /// scheduler currently holds for that output.
    pub(super) fn ownership_mismatch(
        &self,
        output: OutputId,
        retirement: LiveProductionNativeFrameRetirement,
    ) -> LiveProductionNativeOwnershipMismatch {
        LiveProductionNativeOwnershipMismatch {
            retired_frame: retirement.frame,
            content_frame: retirement.content.frame(),
            content_transaction: match retirement.content {
                LiveProductionScanoutContent::MixedPresent { transaction, .. } => Some(transaction),
                _ => None,
            },
            submitted_frame: self.present_scheduler.submitted_frame(output),
            in_flight_frame: self.present_scheduler.in_flight_frame(output),
            in_flight_transaction: self.present_scheduler.in_flight_transaction(),
        }
    }
}

pub fn reduce_live_production_native_retirement_owner(
    retired_frame: LiveProductionNativeFrameId,
    retired_content: LiveProductionScanoutContent,
    submitted_dma_frame: Option<LiveProductionNativeFrameId>,
    // Whether this session ever gave the kernel this frame for this output.
    // Ownership is that question; `submitted_dma_frame` answers only which
    // frame is current.
    session_submitted: bool,
) -> LiveProductionNativeRetirementOwner {
    if retired_content.frame() != retired_frame {
        return LiveProductionNativeRetirementOwner::InvalidDmaOwnership;
    }
    match (retired_content, submitted_dma_frame) {
        (LiveProductionScanoutContent::MixedPresent { .. }, Some(submitted))
            if submitted == retired_frame =>
        {
            LiveProductionNativeRetirementOwner::SubmittedDmaPresent
        }
        (LiveProductionScanoutContent::MixedPresent { .. }, _) if session_submitted => {
            LiveProductionNativeRetirementOwner::SupersededDmaPresent
        }
        (LiveProductionScanoutContent::MixedPresent { .. }, _) => {
            LiveProductionNativeRetirementOwner::InvalidDmaOwnership
        }
        (_, _) => LiveProductionNativeRetirementOwner::IndependentFrame,
    }
}
