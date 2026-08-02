use sophia_protocol::SurfaceId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WmReseedAdmissionCandidate {
    pub surface: SurfaceId,
    pub known: bool,
    pub retries: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WmAdmissionSelection {
    Ordinary,
    ReseedReplay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WmReseedRequest {
    ReplayManage(SurfaceId),
    Relayout,
}

pub fn select_wm_admission(
    candidates: impl IntoIterator<Item = WmReseedAdmissionCandidate>,
    rollback_active: bool,
    selection: WmAdmissionSelection,
) -> Option<SurfaceId> {
    if rollback_active && selection == WmAdmissionSelection::Ordinary {
        return None;
    }
    candidates
        .into_iter()
        .find(|candidate| candidate.known && candidate.retries <= 1)
        .map(|candidate| candidate.surface)
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
