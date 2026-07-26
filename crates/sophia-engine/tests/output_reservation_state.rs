use sophia_engine::SurfaceOutputReservationState;
use sophia_protocol::{
    AxisSpan, OutputEdge, OutputReservation, SurfaceId, SurfaceOutputReservations,
    SurfacePresentationRole,
};

#[test]
fn reservation_becomes_active_only_for_mapped_client_positioned_surface() {
    let surface = SurfaceId::new(1, 1);
    let snapshot = top_reservation(surface, 28);
    let mut state = SurfaceOutputReservationState::default();

    assert!(!state.observe_reservations(snapshot.clone()));
    assert!(state.active_reservations().is_empty());
    assert!(!state.observe_presentation(surface, SurfacePresentationRole::PolicyManaged, true));
    assert!(state.active_reservations().is_empty());
    assert!(state.observe_presentation(surface, SurfacePresentationRole::ClientPositioned, true));
    assert_eq!(state.active_reservations(), vec![snapshot]);
}

#[test]
fn replacement_unmap_remap_and_clear_report_effective_changes() {
    let surface = SurfaceId::new(2, 1);
    let mut state = SurfaceOutputReservationState::default();
    assert!(!state.observe_presentation(surface, SurfacePresentationRole::ClientPositioned, true));
    assert!(state.observe_reservations(top_reservation(surface, 28)));
    assert!(!state.observe_reservations(top_reservation(surface, 28)));
    assert!(state.observe_reservations(top_reservation(surface, 32)));

    assert!(state.observe_presentation(surface, SurfacePresentationRole::ClientPositioned, false));
    assert!(state.active_reservations().is_empty());
    assert!(state.observe_presentation(surface, SurfacePresentationRole::ClientPositioned, true));
    assert!(state.observe_reservations(SurfaceOutputReservations {
        surface,
        reservations: Vec::new(),
    }));
    assert!(state.active_reservations().is_empty());
}

#[test]
fn removal_clears_active_and_dormant_reservations() {
    let active = SurfaceId::new(3, 1);
    let dormant = SurfaceId::new(4, 1);
    let mut state = SurfaceOutputReservationState::default();
    state.observe_reservations(top_reservation(active, 28));
    state.observe_reservations(top_reservation(dormant, 30));
    state.observe_presentation(active, SurfacePresentationRole::ClientPositioned, true);

    assert!(state.remove_surface(active));
    assert!(!state.remove_surface(dormant));
    assert!(state.is_empty());
}

#[test]
fn invalid_snapshot_preserves_previous_valid_reservation() {
    let surface = SurfaceId::new(5, 1);
    let mut state = SurfaceOutputReservationState::default();
    state.observe_presentation(surface, SurfacePresentationRole::ClientPositioned, true);
    let valid = top_reservation(surface, 28);
    assert!(state.observe_reservations(valid.clone()));
    assert!(!state.observe_reservations(SurfaceOutputReservations {
        surface,
        reservations: vec![
            reservation(OutputEdge::Top, 30),
            reservation(OutputEdge::Top, 40),
        ],
    }));
    assert_eq!(state.active_reservations(), vec![valid]);
}

fn top_reservation(surface: SurfaceId, depth: i32) -> SurfaceOutputReservations {
    SurfaceOutputReservations {
        surface,
        reservations: vec![reservation(OutputEdge::Top, depth)],
    }
}

const fn reservation(edge: OutputEdge, depth: i32) -> OutputReservation {
    OutputReservation {
        edge,
        depth,
        span: AxisSpan {
            start: 0,
            end: 1920,
        },
    }
}
