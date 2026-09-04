use sophia_engine::{
    ShellReservationRefusal, ShellWorkAreaCoordinator, reduce_output_work_areas,
    shell_reservation_band,
};
use sophia_protocol::{
    OutputEdge, OutputId, Rect, ShellV1ReservationEdge, ShellV1WorkAreaReservation,
};

fn two_outputs() -> (Rect, Vec<(OutputId, Rect)>) {
    let left = Rect {
        x: 0,
        y: 0,
        width: 2560,
        height: 1440,
    };
    let right = Rect {
        x: 2560,
        y: 0,
        width: 2560,
        height: 1440,
    };
    let root = Rect {
        x: 0,
        y: 0,
        width: 5120,
        height: 1440,
    };
    (
        root,
        vec![
            (OutputId::from_raw(1), left),
            (OutputId::from_raw(2), right),
        ],
    )
}

fn bottom(thickness_px: u16) -> ShellV1WorkAreaReservation {
    ShellV1WorkAreaReservation {
        edge: ShellV1ReservationEdge::Bottom,
        thickness_px,
    }
}

#[test]
fn an_admitted_bottom_claim_reduces_only_its_output() {
    let (root, outputs) = two_outputs();
    let mut coordinator = ShellWorkAreaCoordinator::new();
    let prepared = coordinator
        .admit(
            1,
            1,
            1,
            OutputId::from_raw(1),
            Some(bottom(28)),
            root,
            &outputs,
        )
        .unwrap();
    assert!(prepared.reservation.is_some());
    // Nothing reduces before the bundle commits.
    assert!(coordinator.active_bands().is_empty());
    assert!(coordinator.commit(1, 1));
    let bands = coordinator.active_bands();
    assert_eq!(bands.len(), 1);
    let areas = reduce_output_work_areas(root, outputs.iter().copied(), &[], &bands);
    assert_eq!(
        areas[0].work,
        Some(Rect {
            x: 0,
            y: 0,
            width: 2560,
            height: 1412,
        })
    );
    // The claim is output-local: the second head keeps its full work area.
    assert_eq!(areas[1].work, Some(outputs[1].1));
}

#[test]
fn a_claim_on_the_second_output_measures_depth_through_the_root() {
    let (root, outputs) = two_outputs();
    let band = shell_reservation_band(bottom(28), root, outputs[1].1).unwrap();
    // Both outputs share the root's bottom edge here, so the depth is the
    // thickness alone; the span keeps the claim away from the first output.
    assert_eq!(band.edge, OutputEdge::Bottom);
    assert_eq!(band.depth, 28);
    assert_eq!((band.span.start, band.span.end), (2560, 5120));
}

#[test]
fn a_shorter_output_claims_through_the_taller_root() {
    // A 1440-tall root with a 1080-tall output whose bottom sits above the
    // root's: the band must reach from the root's bottom edge through the
    // gap to the claimed thickness.
    let root = Rect {
        x: 0,
        y: 0,
        width: 2560,
        height: 1440,
    };
    let output = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let band = shell_reservation_band(bottom(28), root, output).unwrap();
    assert_eq!(band.depth, 360 + 28);
    let areas = reduce_output_work_areas(root, [(OutputId::from_raw(1), output)], &[], &[band]);
    assert_eq!(
        areas[0].work,
        Some(Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1052,
        })
    );
}

#[test]
fn refusals_are_named_for_their_causes() {
    let (root, outputs) = two_outputs();
    let mut coordinator = ShellWorkAreaCoordinator::new();
    assert_eq!(
        coordinator.admit(
            2,
            1,
            1,
            OutputId::from_raw(1),
            Some(bottom(28)),
            root,
            &outputs
        ),
        Err(ShellReservationRefusal::StaleEpoch),
    );
    assert_eq!(
        coordinator.admit(
            1,
            1,
            1,
            OutputId::from_raw(9),
            Some(bottom(28)),
            root,
            &outputs
        ),
        Err(ShellReservationRefusal::UnknownOutput),
    );
    // Wire caps thickness at 512, but a short output can still be exhausted;
    // prove the refusal against a display shorter than the claim.
    let short_root = Rect {
        x: 0,
        y: 0,
        width: 640,
        height: 480,
    };
    let short = vec![(OutputId::from_raw(1), short_root)];
    assert_eq!(
        coordinator.admit(
            1,
            1,
            1,
            OutputId::from_raw(1),
            Some(bottom(480)),
            short_root,
            &short
        ),
        Err(ShellReservationRefusal::ExhaustsOutput),
    );
    assert_eq!(
        ShellReservationRefusal::StaleGeneration.reason(),
        "stale_generation"
    );
}

#[test]
fn a_generation_that_does_not_advance_is_refused() {
    let (root, outputs) = two_outputs();
    let mut coordinator = ShellWorkAreaCoordinator::new();
    coordinator
        .admit(
            1,
            1,
            2,
            OutputId::from_raw(1),
            Some(bottom(28)),
            root,
            &outputs,
        )
        .unwrap();
    assert!(coordinator.commit(1, 2));
    // Not past the presented generation.
    assert_eq!(
        coordinator.admit(
            1,
            1,
            2,
            OutputId::from_raw(1),
            Some(bottom(30)),
            root,
            &outputs
        ),
        Err(ShellReservationRefusal::StaleGeneration),
    );
    coordinator
        .admit(
            1,
            1,
            3,
            OutputId::from_raw(1),
            Some(bottom(30)),
            root,
            &outputs,
        )
        .unwrap();
    // Not past the prepared generation either.
    assert_eq!(
        coordinator.admit(
            1,
            1,
            3,
            OutputId::from_raw(1),
            Some(bottom(32)),
            root,
            &outputs
        ),
        Err(ShellReservationRefusal::StaleGeneration),
    );
}

#[test]
fn commit_requires_the_exact_prepared_identity() {
    let (root, outputs) = two_outputs();
    let mut coordinator = ShellWorkAreaCoordinator::new();
    coordinator
        .admit(
            1,
            1,
            1,
            OutputId::from_raw(1),
            Some(bottom(28)),
            root,
            &outputs,
        )
        .unwrap();
    assert!(!coordinator.commit(1, 2));
    assert!(!coordinator.commit(2, 1));
    assert!(coordinator.presented().is_none());
    assert!(coordinator.commit(1, 1));
    assert!(coordinator.presented().is_some());
}

#[test]
fn disconnect_retains_the_presented_claim_and_burns_the_prepared_one() {
    let (root, outputs) = two_outputs();
    let mut coordinator = ShellWorkAreaCoordinator::new();
    coordinator
        .admit(
            1,
            1,
            1,
            OutputId::from_raw(1),
            Some(bottom(28)),
            root,
            &outputs,
        )
        .unwrap();
    assert!(coordinator.commit(1, 1));
    coordinator
        .admit(
            1,
            1,
            2,
            OutputId::from_raw(1),
            Some(bottom(40)),
            root,
            &outputs,
        )
        .unwrap();
    coordinator.on_disconnect();
    // The in-flight claim died with the connection; the presented one did
    // not, because no coherent bundle has replaced it.
    assert!(!coordinator.commit(1, 2));
    let presented = coordinator.presented().unwrap();
    assert_eq!(presented.candidate_generation, 1);
    assert_eq!(coordinator.active_bands().len(), 1);
}

#[test]
fn a_fresh_epoch_withdraws_through_the_same_bundle_path() {
    let (root, outputs) = two_outputs();
    let mut coordinator = ShellWorkAreaCoordinator::new();
    coordinator
        .admit(
            1,
            1,
            5,
            OutputId::from_raw(1),
            Some(bottom(28)),
            root,
            &outputs,
        )
        .unwrap();
    assert!(coordinator.commit(1, 5));
    coordinator.on_disconnect();
    // The reconnected epoch starts its generations fresh; the presented
    // claim from the dead epoch does not gate them.
    let prepared = coordinator
        .admit(2, 2, 1, OutputId::from_raw(1), None, root, &outputs)
        .unwrap();
    assert!(prepared.reservation.is_none());
    // Withdrawal presents through the same commit path and clears the claim.
    assert!(coordinator.commit(2, 1));
    assert!(coordinator.presented().is_none());
    assert!(coordinator.active_bands().is_empty());
}

#[test]
fn a_rejected_bundle_preserves_the_presented_claim() {
    let (root, outputs) = two_outputs();
    let mut coordinator = ShellWorkAreaCoordinator::new();
    coordinator
        .admit(
            1,
            1,
            1,
            OutputId::from_raw(1),
            Some(bottom(28)),
            root,
            &outputs,
        )
        .unwrap();
    assert!(coordinator.commit(1, 1));
    coordinator
        .admit(
            1,
            1,
            2,
            OutputId::from_raw(1),
            Some(bottom(64)),
            root,
            &outputs,
        )
        .unwrap();
    coordinator.reject_prepared();
    assert!(!coordinator.commit(1, 2));
    assert_eq!(coordinator.presented().unwrap().candidate_generation, 1);
    assert_eq!(coordinator.active_bands()[0].depth, 28);
}

#[test]
fn shell_bands_compose_with_x_side_struts_in_one_reduction() {
    use sophia_protocol::{AxisSpan, OutputReservation, SurfaceId, SurfaceOutputReservations};
    let (root, outputs) = two_outputs();
    // A client-side strut at the top of the first output, plus a shell bar
    // at its bottom: one reduction subtracts both.
    let strut = SurfaceOutputReservations {
        surface: SurfaceId::new(1, 1),
        reservations: vec![OutputReservation {
            edge: OutputEdge::Top,
            depth: 14,
            span: AxisSpan {
                start: 0,
                end: 2560,
            },
        }],
    };
    let shell_band = shell_reservation_band(bottom(28), root, outputs[0].1).unwrap();
    let areas = reduce_output_work_areas(root, outputs.iter().copied(), &[strut], &[shell_band]);
    assert_eq!(
        areas[0].work,
        Some(Rect {
            x: 0,
            y: 14,
            width: 2560,
            height: 1440 - 14 - 28,
        })
    );
    assert_eq!(areas[1].work, Some(outputs[1].1));
}
