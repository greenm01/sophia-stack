use sophia_cli::wm_recovery::{WmReseedRequest, select_wm_reseed_request};
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
