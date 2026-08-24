use sophia_cli::wm_recovery::{
    WmAdmissionSelection, WmReseedAdmissionCandidate, WmReseedPlan, select_wm_admission,
    select_wm_reseed_plan,
};
use sophia_protocol::SurfaceId;

#[test]
fn rejected_manage_is_replayed_after_committed_layout_reseed() {
    let firefox = SurfaceId::new(3, 1);

    assert_eq!(
        select_wm_reseed_plan(Some(firefox), true),
        WmReseedPlan {
            seed_committed_layout: true,
            replay_manage: Some(firefox),
        }
    );
    assert_eq!(
        select_wm_reseed_plan(Some(firefox), false),
        WmReseedPlan {
            seed_committed_layout: false,
            replay_manage: Some(firefox),
        }
    );
}

#[test]
fn committed_layout_is_reseeded_when_no_admission_is_pending() {
    assert_eq!(
        select_wm_reseed_plan(None, true),
        WmReseedPlan {
            seed_committed_layout: true,
            replay_manage: None,
        }
    );
    assert_eq!(select_wm_reseed_plan(None, false), WmReseedPlan::default());
}

#[test]
fn restart_replay_selection_is_independent_of_ordinary_rollback_scheduling() {
    let withdrawn = SurfaceId::new(2, 1);
    let firefox = SurfaceId::new(3, 1);
    let candidates = [
        WmReseedAdmissionCandidate {
            surface: withdrawn,
            known: false,
            retries: 0,
            settled: false,
        },
        WmReseedAdmissionCandidate {
            surface: firefox,
            known: true,
            retries: 1,
            settled: false,
        },
    ];

    assert_eq!(
        select_wm_admission(candidates, true, WmAdmissionSelection::Ordinary),
        None
    );
    assert_eq!(
        select_wm_admission(candidates, true, WmAdmissionSelection::ReseedReplay),
        Some(firefox)
    );
    assert_eq!(
        select_wm_admission(
            [WmReseedAdmissionCandidate {
                surface: firefox,
                known: true,
                retries: 2,
                settled: false,
            }],
            true,
            WmAdmissionSelection::ReseedReplay,
        ),
        None
    );
}

/// A settled surface is one policy already answered by placing nothing. An
/// ordinary turn stops offering it; a restarted window manager has answered
/// nothing yet and must still be asked.
#[test]
fn a_settled_surface_is_withheld_from_ordinary_turns_and_replayed_after_a_restart() {
    let firefox = SurfaceId::new(3, 1);
    let settled = [WmReseedAdmissionCandidate {
        surface: firefox,
        known: true,
        retries: 0,
        settled: true,
    }];

    assert_eq!(
        select_wm_admission(settled, false, WmAdmissionSelection::Ordinary),
        None
    );
    assert_eq!(
        select_wm_admission(settled, false, WmAdmissionSelection::ReseedReplay),
        Some(firefox)
    );

    // Settlement is orthogonal to the withdrawal counter: an unsettled surface
    // is still offered, and a replay still refuses a surface past its retries.
    assert_eq!(
        select_wm_admission(
            [WmReseedAdmissionCandidate {
                surface: firefox,
                known: true,
                retries: 0,
                settled: false,
            }],
            false,
            WmAdmissionSelection::Ordinary,
        ),
        Some(firefox)
    );
    assert_eq!(
        select_wm_admission(
            [WmReseedAdmissionCandidate {
                surface: firefox,
                known: true,
                retries: 2,
                settled: true,
            }],
            false,
            WmAdmissionSelection::ReseedReplay,
        ),
        None
    );
}
