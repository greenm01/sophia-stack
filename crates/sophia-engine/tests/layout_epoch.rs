use sophia_engine::{LayoutEpochCoordinator, SurfaceAdmissionState};
use sophia_protocol::{Size, SurfaceConstraints, SurfaceId};

fn size(width: i32, height: i32) -> Size {
    Size { width, height }
}

#[test]
fn timed_out_admission_replans_at_the_safe_content_extent() {
    let surface = SurfaceId::new(7, 1);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.record_committed(surface, size(500, 500));
    coordinator.set_admission(surface, SurfaceAdmissionState::Unmanaged);

    let configures = coordinator
        .begin_recovery([(surface, size(1278, 1422))], [surface])
        .unwrap();

    assert_eq!(configures[0].size, size(500, 500));
    assert_eq!(coordinator.recovery_extent(surface), Some(size(500, 500)));
    assert_eq!(
        coordinator.effective_constraints(surface),
        SurfaceConstraints {
            min_size: Some(size(500, 500)),
            max_size: Some(size(500, 500)),
        }
    );
    assert!(!coordinator.surface_resizable(surface));
    assert_eq!(
        coordinator.admission(surface),
        SurfaceAdmissionState::PendingLayout
    );
}

#[test]
fn recovery_preserves_declared_constraints_and_can_be_cleared() {
    let surface = SurfaceId::new(8, 1);
    let declared = SurfaceConstraints {
        min_size: Some(size(320, 200)),
        max_size: None,
    };
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.set_declared_constraints(surface, declared);
    coordinator.record_committed(surface, size(640, 480));
    coordinator
        .begin_recovery([(surface, size(900, 700))], [surface])
        .unwrap();

    assert!(coordinator.clear_recovery_extent(surface));
    assert_eq!(coordinator.effective_constraints(surface), declared);
    assert!(coordinator.surface_resizable(surface));
}

#[test]
fn abandoned_pixels_remain_fenced_until_safe_content_returns() {
    let surface = SurfaceId::new(9, 1);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.record_committed(surface, size(640, 480));
    coordinator
        .begin_recovery([(surface, size(1000, 800))], [surface])
        .unwrap();

    assert!(!coordinator.accept_observation(surface, size(1000, 800)));
    assert!(coordinator.rollback_pending(surface));
    assert!(coordinator.accept_observation(surface, size(640, 480)));
    assert!(!coordinator.rollback_pending(surface));
}

#[test]
fn configure_ack_unblocks_replan_when_safe_pixels_are_already_retained() {
    let surface = SurfaceId::new(11, 1);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.record_committed(surface, size(500, 500));
    let recovery = coordinator
        .begin_recovery([(surface, size(900, 700))], [surface])
        .unwrap();

    assert!(coordinator.acknowledge_recovery_configure(recovery[0].transaction, surface));
    assert!(!coordinator.rollback_pending(surface));
    assert!(!coordinator.acknowledge_recovery_configure(recovery[0].transaction, surface));
}

#[test]
fn removal_cancels_recovery_and_admission_state() {
    let surface = SurfaceId::new(10, 1);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.record_committed(surface, size(400, 300));
    coordinator
        .begin_recovery([(surface, size(800, 600))], [surface])
        .unwrap();

    coordinator.remove(surface);

    assert_eq!(coordinator.committed_size(surface), None);
    assert_eq!(coordinator.recovery_extent(surface), None);
    assert_eq!(
        coordinator.admission(surface),
        SurfaceAdmissionState::Unmanaged
    );
    assert!(coordinator.rollback_surfaces().next().is_none());
}

#[test]
fn invalid_recovery_request_does_not_partially_publish_constraints() {
    let safe = SurfaceId::new(12, 1);
    let missing = SurfaceId::new(13, 1);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.record_committed(safe, size(500, 500));

    assert!(
        coordinator
            .begin_recovery([(missing, size(900, 700))], [safe])
            .is_err()
    );
    assert_eq!(coordinator.recovery_extent(safe), None);
    assert!(coordinator.rollback_surfaces().next().is_none());
}
