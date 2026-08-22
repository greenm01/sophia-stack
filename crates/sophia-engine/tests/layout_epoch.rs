use sophia_engine::{
    LayoutEpochCoordinator, SurfaceAdmissionState, SurfaceChromeStyle, SurfaceVisualEvidence,
    SurfaceVisualExtentDisposition, apply_surface_chrome_clearance, classify_surface_visual_extent,
    content_surface_geometry, outer_surface_geometry,
};
use sophia_protocol::{
    BufferSource, LayoutTransaction, Rect, Size, SurfaceConstraints, SurfaceId, SurfacePlacement,
    SurfaceSizeRequest, SurfaceTransactionKey, TransactionId, Transform,
};

fn size(width: i32, height: i32) -> Size {
    Size { width, height }
}

fn dma_candidate(surface: SurfaceId, transaction: TransactionId) -> SurfaceTransactionKey {
    SurfaceTransactionKey {
        transaction,
        surface,
        target_buffer: BufferSource::DmaBuf {
            handle: transaction.raw(),
        },
    }
}

fn cpu_candidate(surface: SurfaceId, transaction: TransactionId) -> SurfaceTransactionKey {
    SurfaceTransactionKey {
        transaction,
        surface,
        target_buffer: BufferSource::CpuBuffer {
            handle: transaction.raw(),
        },
    }
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
        coordinator.declared_constraints(surface),
        SurfaceConstraints {
            min_size: None,
            max_size: None,
        }
    );
    assert_eq!(
        coordinator.effective_constraints(surface),
        SurfaceConstraints {
            min_size: Some(size(500, 500)),
            max_size: Some(size(500, 500)),
        }
    );
    assert!(!coordinator.surface_resizable(surface));
    assert!(coordinator.surface_declared_resizable(surface));
    assert_eq!(
        coordinator.admission(surface),
        SurfaceAdmissionState::PendingLayout
    );
}

#[test]
fn admission_recovery_can_use_complete_uncommitted_authority_pixels() {
    let surface = SurfaceId::new(18, 1);
    let safe = size(500, 500);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.record_safe_observation(
        dma_candidate(surface, TransactionId::from_raw(17)),
        safe,
        SurfaceVisualEvidence::PresentedBuffer,
    );

    let configures = coordinator
        .begin_recovery([(surface, size(1276, 1422))], [surface])
        .unwrap();

    assert_eq!(coordinator.committed_size(surface), None);
    assert_eq!(coordinator.safe_size(surface), Some(safe));
    assert_eq!(configures[0].size, safe);
}

#[test]
fn presented_buffer_outlives_later_backing_snapshot_during_admission() {
    let surface = SurfaceId::new(19, 1);
    let present = TransactionId::from_raw(20);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.set_admission(surface, SurfaceAdmissionState::Unmanaged);
    coordinator.record_safe_observation(
        dma_candidate(surface, present),
        size(500, 500),
        SurfaceVisualEvidence::PresentedBuffer,
    );
    coordinator.record_safe_observation(
        cpu_candidate(surface, TransactionId::from_raw(21)),
        size(1276, 1422),
        SurfaceVisualEvidence::BackingSnapshot,
    );

    assert_eq!(
        coordinator.safe_observation(surface),
        Some(sophia_engine::SafeSurfaceObservation {
            candidate: Some(dma_candidate(surface, present)),
            extent: size(500, 500),
            evidence: SurfaceVisualEvidence::PresentedBuffer,
            sequence: 1,
        })
    );
}

#[test]
fn presented_buffer_replaces_earlier_backing_snapshot_during_admission() {
    let surface = SurfaceId::new(20, 1);
    let present = TransactionId::from_raw(22);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.record_safe_observation(
        cpu_candidate(surface, TransactionId::from_raw(21)),
        size(1276, 1422),
        SurfaceVisualEvidence::BackingSnapshot,
    );
    coordinator.record_safe_observation(
        dma_candidate(surface, present),
        size(500, 500),
        SurfaceVisualEvidence::PresentedBuffer,
    );

    let observation = coordinator.safe_observation(surface).unwrap();
    assert_eq!(observation.candidate, Some(dma_candidate(surface, present)));
    assert_eq!(observation.extent, size(500, 500));
    assert_eq!(observation.evidence, SurfaceVisualEvidence::PresentedBuffer);
}

#[test]
fn managed_surface_accepts_the_latest_complete_observation() {
    let surface = SurfaceId::new(21, 1);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.record_safe_observation(
        dma_candidate(surface, TransactionId::from_raw(23)),
        size(500, 500),
        SurfaceVisualEvidence::PresentedBuffer,
    );
    coordinator.set_admission(surface, SurfaceAdmissionState::Managed);
    coordinator.record_safe_observation(
        cpu_candidate(surface, TransactionId::from_raw(24)),
        size(640, 480),
        SurfaceVisualEvidence::BackingSnapshot,
    );

    assert_eq!(coordinator.safe_size(surface), Some(size(640, 480)));
}

#[test]
fn presented_surface_requires_presented_evidence_for_later_resizes() {
    let surface = SurfaceId::new(22, 1);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.set_admission(surface, SurfaceAdmissionState::Managed);
    coordinator.record_safe_observation(
        dma_candidate(surface, TransactionId::from_raw(25)),
        size(1280, 1040),
        SurfaceVisualEvidence::PresentedBuffer,
    );
    coordinator.record_safe_observation(
        cpu_candidate(surface, TransactionId::from_raw(26)),
        size(1276, 1422),
        SurfaceVisualEvidence::BackingSnapshot,
    );

    assert_eq!(coordinator.safe_size(surface), Some(size(1276, 1422)));
    assert_eq!(
        coordinator.required_visual_evidence(surface),
        SurfaceVisualEvidence::PresentedBuffer
    );
    assert!(!coordinator.resize_evidence_allowed(surface, SurfaceVisualEvidence::BackingSnapshot));
    assert!(coordinator.resize_evidence_allowed(surface, SurfaceVisualEvidence::PresentedBuffer));
}

#[test]
fn removing_surface_clears_its_visual_evidence_requirement() {
    let surface = SurfaceId::new(23, 1);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.record_safe_observation(
        dma_candidate(surface, TransactionId::from_raw(27)),
        size(800, 600),
        SurfaceVisualEvidence::PresentedBuffer,
    );

    coordinator.remove(surface);

    assert_eq!(
        coordinator.required_visual_evidence(surface),
        SurfaceVisualEvidence::BackingSnapshot
    );
    assert!(coordinator.resize_evidence_allowed(surface, SurfaceVisualEvidence::BackingSnapshot));
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

    assert_eq!(coordinator.recovery_extent_count(), 1);
    assert_eq!(coordinator.declared_constraints(surface), declared);
    assert!(coordinator.surface_declared_resizable(surface));
    assert!(coordinator.clear_recovery_extent(surface));
    assert_eq!(coordinator.recovery_extent_count(), 0);
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

#[test]
fn fixed_content_extent_is_reconciled_before_client_configure() {
    let terminal = SurfaceId::new(14, 1);
    let fixed = SurfaceId::new(15, 1);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.record_committed(terminal, size(2556, 1422));
    coordinator.record_committed(fixed, size(500, 500));
    coordinator.set_declared_constraints(
        fixed,
        SurfaceConstraints {
            min_size: Some(size(500, 500)),
            max_size: Some(size(500, 500)),
        },
    );
    let transaction = layout_transaction([
        (
            terminal,
            Rect {
                x: 2,
                y: 16,
                width: 1276,
                height: 1422,
            },
        ),
        (
            fixed,
            Rect {
                x: 1282,
                y: 16,
                width: 1276,
                height: 1422,
            },
        ),
    ]);

    let reconciled = coordinator
        .reconcile_transaction(
            &transaction,
            Rect {
                x: 0,
                y: 14,
                width: 2560,
                height: 1426,
            },
        )
        .unwrap();

    assert_eq!(reconciled.adjusted_surfaces, vec![fixed]);
    assert_eq!(
        reconciled
            .transaction
            .render_positions
            .iter()
            .find(|placement| placement.surface == fixed)
            .unwrap()
            .geometry,
        Rect {
            x: 1282,
            y: 16,
            width: 500,
            height: 500,
        }
    );
    assert_eq!(
        reconciled
            .transaction
            .requested_sizes
            .iter()
            .find(|request| request.surface == fixed)
            .unwrap()
            .size,
        size(500, 500),
    );
}

#[test]
fn minimum_extent_is_repositioned_inside_output_bounds() {
    let surface = SurfaceId::new(16, 1);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.set_declared_constraints(
        surface,
        SurfaceConstraints {
            min_size: Some(size(640, 480)),
            max_size: None,
        },
    );
    let transaction = layout_transaction([(
        surface,
        Rect {
            x: 900,
            y: 700,
            width: 100,
            height: 100,
        },
    )]);

    let reconciled = coordinator
        .reconcile_transaction(
            &transaction,
            Rect {
                x: 0,
                y: 0,
                width: 1280,
                height: 720,
            },
        )
        .unwrap();

    assert_eq!(
        reconciled.transaction.render_positions[0].geometry,
        Rect {
            x: 640,
            y: 240,
            width: 640,
            height: 480,
        }
    );
}

#[test]
fn constraint_larger_than_output_is_rejected() {
    let surface = SurfaceId::new(17, 1);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.set_declared_constraints(
        surface,
        SurfaceConstraints {
            min_size: Some(size(1920, 1080)),
            max_size: None,
        },
    );
    let transaction = layout_transaction([(
        surface,
        Rect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        },
    )]);

    assert!(
        coordinator
            .reconcile_transaction(
                &transaction,
                Rect {
                    x: 0,
                    y: 0,
                    width: 1280,
                    height: 720,
                },
            )
            .is_err()
    );
}

fn layout_transaction<const N: usize>(placements: [(SurfaceId, Rect); N]) -> LayoutTransaction {
    LayoutTransaction {
        transaction: TransactionId::from_raw(90),
        requested_sizes: placements
            .iter()
            .map(|(surface, geometry)| SurfaceSizeRequest {
                surface: *surface,
                size: Size {
                    width: geometry.width,
                    height: geometry.height,
                },
            })
            .collect(),
        focus: None,
        render_positions: placements
            .into_iter()
            .enumerate()
            .map(|(z_index, (surface, geometry))| SurfacePlacement {
                surface,
                geometry,
                z_index: i32::try_from(z_index).unwrap(),
                crop: None,
                transform: Transform::IDENTITY,
            })
            .collect(),
        timeout_msec: 500,
    }
}

#[test]
fn admission_surface_retains_a_standing_target_instead_of_welding_to_its_launch_size() {
    // Reproduces the Firefox M10 weld: a first-launch client presents its own
    // default extent (1280x1040) before it can honor the blind-WM tile
    // (1276x1422), so the launch epoch times out. Recovery must fence the
    // surface for visibility WITHOUT rolling it back to its own size or
    // rejecting the WM target, and the owner records a standing obligation that
    // drives it to the tile once the temporary extent clears.
    let surface = SurfaceId::new(9, 1);
    let launch = size(1280, 1040);
    let target = size(1276, 1422);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.record_committed(surface, launch);
    coordinator.set_admission(surface, SurfaceAdmissionState::Unmanaged);

    // After the fix, expire_pending passes an admission surface only as a fixed
    // (fenced) surface, never as a rolled-back request. That must not reject the
    // WM target or emit a configure back at the client's own launch size.
    let configures = coordinator
        .begin_recovery(std::iter::empty::<(SurfaceId, Size)>(), [surface])
        .unwrap();
    assert!(
        configures.is_empty(),
        "a fenced admission surface must not receive a rollback configure"
    );
    assert!(
        coordinator.request_allowed(surface, target),
        "the blind-WM target must never be marked rejected"
    );
    assert_eq!(coordinator.recovery_extent(surface), Some(launch));

    // The owner records the standing obligation toward the WM tile.
    coordinator.set_pending_target(surface, target);
    assert_eq!(coordinator.pending_target(surface), Some(target));

    // Once the launch buffer retires the temporary extent clears and the
    // surface is free to resize, but the obligation is still outstanding.
    assert!(coordinator.clear_recovery_extent(surface));
    assert!(coordinator.surface_resizable(surface));
    assert_eq!(coordinator.pending_target(surface), Some(target));

    // Committing the exact target discharges the obligation.
    coordinator.record_committed(surface, target);
    assert_eq!(coordinator.pending_target(surface), None);
}

#[test]
fn pending_target_matching_committed_size_is_not_an_obligation() {
    let surface = SurfaceId::new(10, 1);
    let extent = size(800, 600);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.record_committed(surface, extent);
    // Nothing to drive when the surface already sits at the target.
    coordinator.set_pending_target(surface, extent);
    assert_eq!(coordinator.pending_target(surface), None);
    // Removing a managed surface drops any obligation with it.
    coordinator.set_pending_target(surface, size(1000, 700));
    assert_eq!(coordinator.pending_target(surface), Some(size(1000, 700)));
    coordinator.remove(surface);
    assert_eq!(coordinator.pending_target(surface), None);
}

#[test]
fn retained_extent_updates_remain_presentable_while_a_standing_target_is_pending() {
    let surface = SurfaceId::new(24, 1);
    let recovery = size(300, 300);
    let target = size(1276, 709);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.set_recovery_extent(surface, recovery);
    coordinator.set_pending_target(surface, target);

    for _ in 0..3 {
        assert_eq!(
            classify_surface_visual_extent(
                Some(recovery),
                coordinator.pending_target(surface),
                coordinator.recovery_extent(surface),
            ),
            SurfaceVisualExtentDisposition::RetainedRecovery,
        );
    }
    assert_eq!(
        classify_surface_visual_extent(
            Some(target),
            coordinator.pending_target(surface),
            coordinator.recovery_extent(surface),
        ),
        SurfaceVisualExtentDisposition::Expected,
    );
    assert_eq!(
        classify_surface_visual_extent(
            Some(size(640, 480)),
            coordinator.pending_target(surface),
            coordinator.recovery_extent(surface),
        ),
        SurfaceVisualExtentDisposition::Mismatch,
    );
    assert_eq!(coordinator.recovery_extent(surface), Some(recovery));
    assert_eq!(coordinator.pending_target(surface), Some(target));

    assert!(coordinator.clear_recovery_extent(surface));
    assert_eq!(
        classify_surface_visual_extent(
            Some(recovery),
            coordinator.pending_target(surface),
            coordinator.recovery_extent(surface),
        ),
        SurfaceVisualExtentDisposition::Mismatch,
    );
    assert_eq!(coordinator.pending_target(surface), Some(target));
}

/// A recovery extent yields to the client's own constraints on an output it
/// cannot fit.
///
/// The extent pins a surface to exactly the pixels it has already produced, so
/// admission can show real content before the blind WM drives final geometry.
/// It is a courtesy, not something the client asked for. An output-topology
/// change can leave it larger than the output the surface now lands on -- a
/// mixed mirror-plus-extended commit put a 1280x1440 extent onto a 1920x1080
/// output -- and because the pin is both minimum and maximum, no proposal
/// could satisfy it and the session ended.
#[test]
fn an_oversized_recovery_extent_yields_to_declared_constraints() {
    let surface = SurfaceId::new(80, 1);
    let extent = size(1280, 1440);
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.record_committed(surface, extent);
    coordinator.set_recovery_extent(surface, extent);

    let placement = |bounds: Rect| layout_transaction([(surface, bounds)]);

    // An output that can hold the extent still honours the pin exactly.
    let roomy = Rect {
        x: 0,
        y: 0,
        width: 2560,
        height: 1440,
    };
    let reconciled = coordinator
        .reconcile_transaction(&placement(roomy), roomy)
        .expect("an extent that fits is satisfiable");
    assert_eq!(
        reconciled
            .transaction
            .render_positions
            .iter()
            .find(|position| position.surface == surface)
            .expect("the surface keeps its placement")
            .geometry
            .height,
        1440
    );

    // One that cannot must fall back rather than fail the session.
    let narrow = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let reconciled = coordinator
        .reconcile_transaction(&placement(narrow), narrow)
        .expect("a courtesy extent yields rather than ending the session");
    assert!(
        reconciled
            .transaction
            .render_positions
            .iter()
            .find(|position| position.surface == surface)
            .expect("the surface keeps its placement")
            .geometry
            .height
            <= 1080
    );
}

#[test]
fn recovery_extent_is_reconciled_within_chrome_content_bounds() {
    let surface = SurfaceId::new(81, 1);
    let outer_bounds = Rect {
        x: 0,
        y: 0,
        width: 2560,
        height: 1440,
    };
    let chrome = SurfaceChromeStyle::default();
    let content_bounds = content_surface_geometry(outer_bounds, chrome).unwrap();
    let outer_transaction = layout_transaction([(surface, outer_bounds)]);
    let content_transaction = apply_surface_chrome_clearance(&outer_transaction, chrome).unwrap();
    let mut coordinator = LayoutEpochCoordinator::default();
    coordinator.set_recovery_extent(surface, size(2558, 1438));

    let reconciled = coordinator
        .reconcile_transaction(&content_transaction, content_bounds)
        .expect("a recovery extent outside content bounds must yield");
    let content_geometry = reconciled.transaction.render_positions[0].geometry;

    assert_eq!(content_geometry, content_bounds);
    assert_eq!(
        outer_surface_geometry(content_geometry, chrome).unwrap(),
        outer_bounds
    );
}
