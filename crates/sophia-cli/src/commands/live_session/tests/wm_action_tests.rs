use crate::commands::live_session::{LivePhysicalWmActionDisposition, LiveWmRequestAdmission};

#[test]
fn repeated_physical_wm_action_is_a_nonfatal_coalesced_disposition() {
    assert_eq!(
        LivePhysicalWmActionDisposition::from(LiveWmRequestAdmission::Duplicate),
        LivePhysicalWmActionDisposition::Coalesced
    );
}
