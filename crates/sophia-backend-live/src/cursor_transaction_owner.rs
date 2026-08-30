//! What a head should do with a cursor that has moved.
//!
//! The kernel serializes atomic commits per CRTC, so a cursor cannot simply
//! be moved when the pointer moves -- it joins the queue the primary already
//! waits in. `CursorPlaneTransactionOwner.tla` settles what happens then, and
//! this is that decision written as a function of the head's state.
//!
//! The interesting case is the one that looks like an optimisation and is
//! not: a cursor-only commit, issued when the CRTC is free and no frame is
//! going out. Without it a cursor waits for the client's next frame, and a
//! client repainting on a cursor blink leaves the pointer frozen for most of
//! a second. TLC refuses the model without it.

use crate::{HardwareCursorPath, LegacyHardwareCursorAdmission, LibdrmNativeCursorPlacement};

/// Where the cursor should be on one head.
///
/// The outer `Option` is whether anything is waiting; the inner one is
/// whether the pointer is on this head at all. A head the pointer left is
/// told to hide, which is a change to commit rather than nothing to say.
pub type PendingCursor = Option<Option<LibdrmNativeCursorPlacement>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CursorCommitPlan {
    /// Nothing to do: nothing pending, or the plane already shows it.
    Idle,
    /// A frame is going out; the cursor rides its commit.
    RideNextPrimary,
    /// The CRTC is free and no frame is waiting: commit the cursor alone.
    CommitCursorOnly,
    /// The CRTC is busy with something not ours to add to. Hold the position.
    Wait,
}

/// Decide what to do with this head's pending cursor.
///
/// Ordered so the cheap answers come first: riding a frame that is going out
/// anyway costs nothing, and a redundant commit costs a commit.
pub fn plan_cursor_commit(
    path: HardwareCursorPath,
    admission: LegacyHardwareCursorAdmission,
    pending: PendingCursor,
    committed: Option<LibdrmNativeCursorPlacement>,
    primary_going_out: bool,
) -> CursorCommitPlan {
    if path != HardwareCursorPath::AtomicPlane {
        return CursorCommitPlan::Idle;
    }
    let Some(placement) = pending else {
        return CursorCommitPlan::Idle;
    };
    // Already showing it. Superseding in place means the pending cell can
    // hold a position the plane reached by some other commit, and paying for
    // a commit to change nothing is exactly the unbounded work the model
    // forbids.
    if placement == committed {
        return CursorCommitPlan::Idle;
    }
    // A frame is going out: the cursor rides it. This is the cheap case, and
    // the reason the submit policy carries a cursor at all.
    if primary_going_out {
        return CursorCommitPlan::RideNextPrimary;
    }
    match admission {
        // The plane is not installed yet, or a flip is in flight. Either way
        // this head cannot commit now -- the position waits rather than being
        // dropped, which is what makes the cursor eventually arrive.
        LegacyHardwareCursorAdmission::DeferredUpdate
        | LegacyHardwareCursorAdmission::DeferredInitialization => CursorCommitPlan::Wait,
        // The CRTC is free and nothing else is going out. This is the commit
        // the model needs: without it a cursor waits on a client that may not
        // draw again for most of a second.
        LegacyHardwareCursorAdmission::Update
        | LegacyHardwareCursorAdmission::InitializeThenUpdate => CursorCommitPlan::CommitCursorOnly,
    }
}
