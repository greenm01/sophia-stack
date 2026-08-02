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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WmReseedPlan {
    pub seed_committed_layout: bool,
    pub replay_manage: Option<SurfaceId>,
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

pub const fn select_wm_reseed_plan(
    pending_admission: Option<SurfaceId>,
    has_committed_layout: bool,
) -> WmReseedPlan {
    WmReseedPlan {
        seed_committed_layout: has_committed_layout,
        replay_manage: pending_admission,
    }
}
