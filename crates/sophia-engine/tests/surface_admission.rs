use sophia_engine::{SurfaceAdmissionTable, SurfacePresentationAdmissionState};
use sophia_protocol::{
    Rect, SurfaceConstraints, SurfaceId, SurfacePresentationIntent, SurfacePresentationIntentKind,
    SurfacePresentationRole, TransactionId,
};

fn request(surface: SurfaceId) -> SurfacePresentationIntent {
    SurfacePresentationIntent {
        surface,
        kind: SurfacePresentationIntentKind::Request,
        role: SurfacePresentationRole::PolicyManaged,
        geometry: Rect {
            x: 10,
            y: 20,
            width: 500,
            height: 500,
        },
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        generation: 1,
    }
}

#[test]
fn admission_progresses_only_through_the_exact_control_transaction() {
    let surface = SurfaceId::new(7, 1);
    let transaction = TransactionId::from_raw(42);
    let geometry = request(surface).geometry;
    let mut table = SurfaceAdmissionTable::default();

    assert!(table.observe_intent(request(surface)));
    assert_eq!(
        table.state(surface),
        SurfacePresentationAdmissionState::PolicyPending
    );
    assert!(table.begin_control(surface, transaction, geometry));
    assert!(!table.mark_managed(surface));
    assert!(!table.acknowledge_control(surface, TransactionId::from_raw(41)));
    assert!(table.acknowledge_control(surface, transaction));
    assert!(table.mark_managed(surface));
    assert_eq!(
        table.state(surface),
        SurfacePresentationAdmissionState::Managed
    );
}

#[test]
fn withdrawal_cancels_pending_facts_and_late_acknowledgements() {
    let surface = SurfaceId::new(8, 1);
    let transaction = TransactionId::from_raw(43);
    let mut table = SurfaceAdmissionTable::default();
    let intent = request(surface);

    table.observe_intent(intent);
    table.begin_control(surface, transaction, intent.geometry);
    assert!(table.observe_intent(SurfacePresentationIntent {
        kind: SurfacePresentationIntentKind::Withdraw,
        ..intent
    }));

    assert_eq!(
        table.state(surface),
        SurfacePresentationAdmissionState::Inactive
    );
    assert_eq!(table.facts(surface), None);
    assert!(!table.acknowledge_control(surface, transaction));
}
