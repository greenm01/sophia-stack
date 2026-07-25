use sophia_cli::session_startup::{
    SessionStartupEvent, SessionStartupReadiness, SessionStartupUpdate, reduce_session_startup,
};
use sophia_protocol::SurfaceId;

#[test]
fn startup_proof_is_pinned_and_monotonic() {
    let startup = SurfaceId::new(1, 1);
    let later = SurfaceId::new(2, 1);
    let mut state = SessionStartupReadiness::default();

    reduce_session_startup(&mut state, SessionStartupEvent::PinSurface(startup));
    reduce_session_startup(&mut state, SessionStartupEvent::PinSurface(later));
    reduce_session_startup(&mut state, SessionStartupEvent::ClientFocusApplied(later));
    reduce_session_startup(&mut state, SessionStartupEvent::VisualDetail(later));
    reduce_session_startup(&mut state, SessionStartupEvent::StablePresented(later));
    reduce_session_startup(&mut state, SessionStartupEvent::OutputsPresented);
    assert!(!state.ready);

    reduce_session_startup(&mut state, SessionStartupEvent::ClientFocusApplied(startup));
    reduce_session_startup(&mut state, SessionStartupEvent::VisualDetail(startup));
    assert_eq!(
        reduce_session_startup(&mut state, SessionStartupEvent::StablePresented(startup)),
        SessionStartupUpdate::Ready
    );
    assert_eq!(state.surface, Some(startup));
    assert!(state.ready);
    assert_eq!(
        reduce_session_startup(&mut state, SessionStartupEvent::NativeRecovered),
        SessionStartupUpdate::AlreadyReady
    );
    assert!(state.ready);
}

#[test]
fn recovery_invalidates_only_unfinished_presentation_evidence() {
    let startup = SurfaceId::new(1, 1);
    let mut state = SessionStartupReadiness::default();

    for event in [
        SessionStartupEvent::PinSurface(startup),
        SessionStartupEvent::ClientFocusApplied(startup),
        SessionStartupEvent::VisualDetail(startup),
        SessionStartupEvent::StablePresented(startup),
    ] {
        reduce_session_startup(&mut state, event);
    }
    reduce_session_startup(&mut state, SessionStartupEvent::NativeRecovered);

    assert_eq!(state.surface, Some(startup));
    assert!(state.client_focus_applied);
    assert!(state.visual_detail);
    assert!(!state.stable_presented);
    assert!(!state.outputs_presented);
    assert!(!state.ready);
}
