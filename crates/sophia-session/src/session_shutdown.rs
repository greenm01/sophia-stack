#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionLogoutDrainState {
    pub requested: bool,
    pub pending_input_deliveries: usize,
    pub pending_key_release_barriers: usize,
    pub pending_controls: usize,
    pub pending_wm_update: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionLogoutDrainDecision {
    Running,
    Draining,
    Complete,
}

pub fn session_logout_drain_decision(state: SessionLogoutDrainState) -> SessionLogoutDrainDecision {
    if !state.requested {
        return SessionLogoutDrainDecision::Running;
    }
    if state.pending_input_deliveries != 0
        || state.pending_key_release_barriers != 0
        || state.pending_controls != 0
        || state.pending_wm_update
    {
        return SessionLogoutDrainDecision::Draining;
    }
    SessionLogoutDrainDecision::Complete
}
