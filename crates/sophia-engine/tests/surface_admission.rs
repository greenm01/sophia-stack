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

#[test]
fn dma_buf_admission_becomes_managed_only_after_exact_visual_retirement() {
    let surface = SurfaceId::new(9, 1);
    let admission = TransactionId::from_raw(44);
    let visual = TransactionId::from_raw(45);
    let intent = request(surface);
    let mut table = SurfaceAdmissionTable::default();

    table.observe_intent(intent);
    assert!(table.begin_control(surface, admission, intent.geometry));
    assert!(table.acknowledge_control(surface, admission));
    assert!(table.begin_retirement(surface, visual));
    assert_eq!(
        table.state(surface),
        SurfacePresentationAdmissionState::AwaitingRetirement {
            admission_transaction: admission,
            visual_transaction: visual,
            geometry: intent.geometry,
        }
    );
    assert!(!table.complete_retirement(surface, TransactionId::from_raw(46)));
    assert_eq!(
        table.state(surface),
        SurfacePresentationAdmissionState::AwaitingRetirement {
            admission_transaction: admission,
            visual_transaction: visual,
            geometry: intent.geometry,
        }
    );
    assert!(table.complete_retirement(surface, visual));
    assert_eq!(
        table.state(surface),
        SurfacePresentationAdmissionState::Managed
    );
}
