use sophia_cli::wm_recovery::{
    WmAdmissionSelection, WmReseedAdmissionCandidate, WmReseedRequest, select_wm_admission,
    select_wm_reseed_request,
};
use sophia_protocol::SurfaceId;

#[test]
fn rejected_manage_is_replayed_before_committed_layout_reseed() {
    let firefox = SurfaceId::new(3, 1);

    assert_eq!(
        select_wm_reseed_request(Some(firefox), true),
        Some(WmReseedRequest::ReplayManage(firefox))
    );
    assert_eq!(
        select_wm_reseed_request(Some(firefox), false),
        Some(WmReseedRequest::ReplayManage(firefox))
    );
}

#[test]
fn committed_layout_is_reseeded_when_no_admission_is_pending() {
    assert_eq!(
        select_wm_reseed_request(None, true),
        Some(WmReseedRequest::Relayout)
    );
    assert_eq!(select_wm_reseed_request(None, false), None);
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
        },
        WmReseedAdmissionCandidate {
            surface: firefox,
            known: true,
            retries: 1,
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
            }],
            true,
            WmAdmissionSelection::ReseedReplay,
        ),
        None
    );
}
