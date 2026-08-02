use sophia_protocol::SurfaceId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WmReseedRequest {
    ReplayManage(SurfaceId),
    Relayout,
}

pub const fn select_wm_reseed_request(
    pending_admission: Option<SurfaceId>,
    has_committed_layout: bool,
) -> Option<WmReseedRequest> {
    match pending_admission {
        Some(surface) => Some(WmReseedRequest::ReplayManage(surface)),
        None if has_committed_layout => Some(WmReseedRequest::Relayout),
        None => None,
    }
}
