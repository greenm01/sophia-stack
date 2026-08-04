use sophia_cli::session_shutdown::{
    SessionLogoutDrainDecision, SessionLogoutDrainState, session_logout_drain_decision,
};

fn drained_logout() -> SessionLogoutDrainState {
    SessionLogoutDrainState {
        requested: true,
        pending_input_deliveries: 0,
        pending_key_release_barriers: 0,
        pending_controls: 0,
        pending_wm_update: false,
    }
}

#[test]
fn logout_waits_for_the_committed_wm_update_to_enter_engine() {
    let mut state = drained_logout();
    state.pending_wm_update = true;
    assert_eq!(
        session_logout_drain_decision(state),
        SessionLogoutDrainDecision::Draining
    );

    state.pending_wm_update = false;
    assert_eq!(
        session_logout_drain_decision(state),
        SessionLogoutDrainDecision::Complete
    );
}

#[test]
fn logout_waits_for_every_delivery_boundary() {
    for state in [
        SessionLogoutDrainState {
            pending_input_deliveries: 1,
            ..drained_logout()
        },
        SessionLogoutDrainState {
            pending_key_release_barriers: 1,
            ..drained_logout()
        },
        SessionLogoutDrainState {
            pending_controls: 1,
            ..drained_logout()
        },
    ] {
        assert_eq!(
            session_logout_drain_decision(state),
            SessionLogoutDrainDecision::Draining
        );
    }
}

#[test]
fn idle_session_does_not_exit_without_logout() {
    assert_eq!(
        session_logout_drain_decision(SessionLogoutDrainState {
            requested: false,
            ..drained_logout()
        }),
        SessionLogoutDrainDecision::Running
    );
}
