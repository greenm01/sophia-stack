#![cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]

//! The rules the cursor transaction owner follows, and why each exists.

use sophia_backend_live::{
    CursorCommitPlan, HardwareCursorPath, LegacyHardwareCursorAdmission,
    LibdrmNativeCursorPlacement, plan_cursor_commit,
};

fn placement(x: i32) -> LibdrmNativeCursorPlacement {
    LibdrmNativeCursorPlacement {
        framebuffer: drm::control::from_u32(7).unwrap(),
        x,
        y: 100,
        width: 64,
        height: 64,
    }
}

fn plan(
    admission: LegacyHardwareCursorAdmission,
    pending: Option<Option<LibdrmNativeCursorPlacement>>,
    committed: Option<LibdrmNativeCursorPlacement>,
    primary_going_out: bool,
) -> CursorCommitPlan {
    plan_cursor_commit(
        HardwareCursorPath::AtomicPlane,
        admission,
        pending,
        committed,
        primary_going_out,
    )
}

/// A session on the legacy ioctl is not this owner's business.
#[test]
fn the_legacy_path_is_left_alone() {
    assert_eq!(
        plan_cursor_commit(
            HardwareCursorPath::LegacyIoctl,
            LegacyHardwareCursorAdmission::Update,
            Some(Some(placement(10))),
            None,
            false,
        ),
        CursorCommitPlan::Idle
    );
}

/// The cheap case: a frame is going out anyway, so the cursor rides it. This
/// is why the submit policy carries a cursor at all.
#[test]
fn a_cursor_rides_a_frame_that_is_going_out() {
    assert_eq!(
        plan(
            LegacyHardwareCursorAdmission::Update,
            Some(Some(placement(10))),
            None,
            true,
        ),
        CursorCommitPlan::RideNextPrimary
    );
    // Even while a flip is in flight: the frame being prepared is the one the
    // cursor joins, and its commit has not been issued yet.
    assert_eq!(
        plan(
            LegacyHardwareCursorAdmission::DeferredUpdate,
            Some(Some(placement(10))),
            None,
            true,
        ),
        CursorCommitPlan::RideNextPrimary
    );
}

/// The case the model exists for. Nothing is going out and the CRTC is free,
/// so the cursor commits alone -- a client repainting on a cursor blink would
/// otherwise leave the pointer frozen for most of a second.
#[test]
fn an_idle_crtc_takes_a_cursor_only_commit() {
    assert_eq!(
        plan(
            LegacyHardwareCursorAdmission::Update,
            Some(Some(placement(10))),
            None,
            false,
        ),
        CursorCommitPlan::CommitCursorOnly
    );
}

/// A busy CRTC holds the position rather than dropping it. Waiting is what
/// makes the cursor eventually arrive; dropping is what makes it stutter.
#[test]
fn a_busy_crtc_holds_the_position() {
    for admission in [
        LegacyHardwareCursorAdmission::DeferredUpdate,
        LegacyHardwareCursorAdmission::DeferredInitialization,
    ] {
        assert_eq!(
            plan(admission, Some(Some(placement(10))), None, false),
            CursorCommitPlan::Wait,
            "{admission:?} must hold the position"
        );
    }
}

/// Superseding in place means the cell can hold what the plane already shows.
/// Committing to change nothing is the unbounded work the model forbids.
#[test]
fn a_position_already_on_the_plane_costs_no_commit() {
    assert_eq!(
        plan(
            LegacyHardwareCursorAdmission::Update,
            Some(Some(placement(10))),
            Some(placement(10)),
            false,
        ),
        CursorCommitPlan::Idle
    );
    assert_eq!(
        plan(
            LegacyHardwareCursorAdmission::Update,
            Some(Some(placement(11))),
            Some(placement(10)),
            false,
        ),
        CursorCommitPlan::CommitCursorOnly,
        "a different position is still worth a commit"
    );
}

/// Hiding is a change, not an absence. A head the pointer left has something
/// to say, and saying nothing is how a cursor ends up showing on two
/// monitors at once.
#[test]
fn a_head_the_pointer_left_still_commits() {
    assert_eq!(
        plan(
            LegacyHardwareCursorAdmission::Update,
            Some(None),
            Some(placement(10)),
            false,
        ),
        CursorCommitPlan::CommitCursorOnly
    );
    // And once hidden, it stays quiet.
    assert_eq!(
        plan(
            LegacyHardwareCursorAdmission::Update,
            Some(None),
            None,
            false,
        ),
        CursorCommitPlan::Idle
    );
}

/// Nothing pending is nothing to do, whatever the CRTC is doing.
#[test]
fn nothing_pending_is_idle() {
    for going_out in [false, true] {
        assert_eq!(
            plan(
                LegacyHardwareCursorAdmission::Update,
                None,
                Some(placement(10)),
                going_out,
            ),
            CursorCommitPlan::Idle
        );
    }
}
