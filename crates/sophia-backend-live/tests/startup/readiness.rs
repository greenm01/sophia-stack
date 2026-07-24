#[test]
fn scanout_readiness_reports_ready_without_exposing_kms_identity() {
    let root = ready_drm_sysfs_fixture("scanout-ready");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));

    assert_eq!(
        report.scanout_readiness_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Ready,
        }),
        LiveScanoutReadinessReport {
            status: LiveScanoutReadinessStatus::Ready,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn kms_scanout_target_reports_ready_size_without_kms_identity() {
    let root = ready_drm_sysfs_fixture("kms-scanout-target-ready");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));

    assert_eq!(
        report.kms_scanout_target_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Ready,
        }),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::Ready,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scanout_readiness_reports_missing_output_before_renderer_status() {
    let root = drm_sysfs_fixture("scanout-no-output");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));

    assert_eq!(
        report.scanout_readiness_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Ready,
        }),
        LiveScanoutReadinessReport {
            status: LiveScanoutReadinessStatus::OutputUnavailable,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn kms_scanout_target_reports_missing_output_without_kms_identity() {
    let root = drm_sysfs_fixture("kms-scanout-target-no-output");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));

    assert_eq!(
        report.kms_scanout_target_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Ready,
        }),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::OutputUnavailable,
            size: None,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scanout_readiness_collapses_unavailable_presentation_without_native_details() {
    let root = ready_drm_sysfs_fixture("scanout-presentation-unavailable");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));

    assert_eq!(
        report.scanout_readiness_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Unavailable,
        }),
        LiveScanoutReadinessReport {
            status: LiveScanoutReadinessStatus::PresentationUnavailable,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn kms_scanout_target_collapses_presentation_without_native_details() {
    let root = ready_drm_sysfs_fixture("kms-scanout-target-presentation");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));

    assert_eq!(
        report.kms_scanout_target_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Unavailable,
        }),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::PresentationUnavailable,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );
    assert_eq!(
        report.kms_scanout_target_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Degraded,
        }),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::Degraded,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scanout_readiness_collapses_degraded_presentation_without_native_details() {
    let root = ready_drm_sysfs_fixture("scanout-degraded");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));

    assert_eq!(
        report.scanout_readiness_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Degraded,
        }),
        LiveScanoutReadinessReport {
            status: LiveScanoutReadinessStatus::Degraded,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn page_flip_event_projects_scanout_readiness_without_kms_identity() {
    assert_eq!(
        LivePageFlipEvent::from_scanout_status(LiveScanoutReadinessStatus::Ready),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Ready,
            frame_serial: None,
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_scanout_status(LiveScanoutReadinessStatus::OutputUnavailable),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::OutputUnavailable,
            frame_serial: None,
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_scanout_status(LiveScanoutReadinessStatus::PresentationUnavailable,),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::PresentationUnavailable,
            frame_serial: None,
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_scanout_status(LiveScanoutReadinessStatus::Degraded),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Degraded,
            frame_serial: None,
        }
    );
}

#[test]
fn page_flip_event_projects_kms_scanout_target_without_kms_identity() {
    assert_eq!(
        LivePageFlipEvent::from_kms_scanout_target_status(LiveKmsScanoutTargetStatus::Ready),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Ready,
            frame_serial: None,
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_kms_scanout_target_status(
            LiveKmsScanoutTargetStatus::FrameTargetUnavailable,
        ),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::FrameTargetUnavailable,
            frame_serial: None,
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_kms_scanout_target_status(
            LiveKmsScanoutTargetStatus::InvalidFrameTarget,
        ),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::InvalidFrameTarget,
            frame_serial: None,
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_kms_scanout_target_status(
            LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch,
        ),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::FrameTargetSizeMismatch,
            frame_serial: None,
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_kms_scanout_target_status(LiveKmsScanoutTargetStatus::Degraded),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Degraded,
            frame_serial: None,
        }
    );
}

#[test]
fn page_flip_event_drops_output_transaction_and_surface_identity() {
    assert_eq!(
        LivePageFlipEvent::from_commit_outcome(&PageFlipCommitOutcome::WaitingForOutput {
            expected: OutputId::from_raw(4),
            actual: OutputId::from_raw(9),
            transaction: TransactionId::from_raw(55),
        }),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::WaitingForOutput,
            frame_serial: None,
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_commit_outcome(
            &PageFlipCommitOutcome::WaitingForTransactionReadiness {
                transaction: TransactionId::from_raw(56),
                pending_surfaces: vec![sophia_protocol::SurfaceId::new(77, 1)],
            },
        ),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::WaitingForTransactionReadiness,
            frame_serial: None,
        }
    );
}

#[test]
fn page_flip_event_preserves_only_frame_serial_for_terminal_outcomes() {
    assert_eq!(
        LivePageFlipEvent::from_commit_outcome(&PageFlipCommitOutcome::Committed {
            frame_serial: 91,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(57),
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![sophia_protocol::SurfaceId::new(88, 1)],
            },
        }),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(91),
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_commit_outcome(&PageFlipCommitOutcome::Rejected {
            frame_serial: 92,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(58),
                outcome: TransactionOutcome::RejectedInvalidSurface,
                applied_surfaces: vec![sophia_protocol::SurfaceId::new(89, 1)],
            },
        }),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Rejected,
            frame_serial: Some(92),
        }
    );
}

#[test]
fn atomic_scanout_commit_report_reduces_page_flip_outcomes() {
    assert_eq!(
        LiveAtomicScanoutCommitReport::from_page_flip_outcome(&PageFlipCommitOutcome::Committed {
            frame_serial: 91,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(57),
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![sophia_protocol::SurfaceId::new(88, 1)],
            },
        }),
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::Committed,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(91),
            },
        }
    );
    assert_eq!(
        LiveAtomicScanoutCommitReport::from_page_flip_outcome(&PageFlipCommitOutcome::Rejected {
            frame_serial: 92,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(58),
                outcome: TransactionOutcome::RejectedInvalidSurface,
                applied_surfaces: vec![sophia_protocol::SurfaceId::new(89, 1)],
            },
        }),
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::Rejected,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Rejected,
                frame_serial: Some(92),
            },
        }
    );
    assert_eq!(
        LiveAtomicScanoutCommitReport::from_page_flip_outcome(&PageFlipCommitOutcome::Rejected {
            frame_serial: 93,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(60),
                outcome: TransactionOutcome::TimedOut,
                applied_surfaces: Vec::new(),
            },
        }),
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::TimedOut,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Rejected,
                frame_serial: Some(93),
            },
        }
    );
    assert_eq!(
        LiveAtomicScanoutCommitReport::from_page_flip_outcome(
            &PageFlipCommitOutcome::WaitingForTransactionReadiness {
                transaction: TransactionId::from_raw(59),
                pending_surfaces: vec![sophia_protocol::SurfaceId::new(90, 1)],
            },
        ),
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::WaitingForTransactionReadiness,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::WaitingForTransactionReadiness,
                frame_serial: None,
            },
        }
    );
}

#[test]
fn atomic_scanout_commit_report_rejects_mismatched_page_flip_frame_serial() {
    let callback = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(90),
        },
    };

    assert_eq!(
        LiveAtomicScanoutCommitReport::from_page_flip_callback_and_outcome(
            &callback,
            &PageFlipCommitOutcome::Committed {
                frame_serial: 91,
                commit: TransactionCommit {
                    transaction: TransactionId::from_raw(57),
                    outcome: TransactionOutcome::Committed,
                    applied_surfaces: vec![sophia_protocol::SurfaceId::new(88, 1)],
                },
            },
        ),
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::Rejected,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Rejected,
                frame_serial: Some(90),
            },
        }
    );
}

#[test]
fn fake_atomic_scanout_committer_counts_only_committed_outcomes() {
    let mut committer = FakeAtomicScanoutCommitter::default();

    let committed = committer.commit_atomic_scanout(&PageFlipCommitOutcome::Committed {
        frame_serial: 91,
        commit: TransactionCommit {
            transaction: TransactionId::from_raw(57),
            outcome: TransactionOutcome::Committed,
            applied_surfaces: vec![sophia_protocol::SurfaceId::new(88, 1)],
        },
    });
    assert_eq!(committed.status, LiveAtomicScanoutCommitStatus::Committed);
    assert_eq!(committer.committed_count(), 1);

    let waiting =
        committer.commit_atomic_scanout(&PageFlipCommitOutcome::WaitingForTransactionReadiness {
            transaction: TransactionId::from_raw(59),
            pending_surfaces: vec![sophia_protocol::SurfaceId::new(90, 1)],
        });
    assert_eq!(
        waiting.status,
        LiveAtomicScanoutCommitStatus::WaitingForTransactionReadiness
    );
    assert_eq!(committer.committed_count(), 1);

    let rejected = committer.commit_atomic_scanout(&PageFlipCommitOutcome::Rejected {
        frame_serial: 92,
        commit: TransactionCommit {
            transaction: TransactionId::from_raw(58),
            outcome: TransactionOutcome::RejectedInvalidSurface,
            applied_surfaces: vec![sophia_protocol::SurfaceId::new(89, 1)],
        },
    });
    assert_eq!(rejected.status, LiveAtomicScanoutCommitStatus::Rejected);
    assert_eq!(committer.committed_count(), 1);

    let timed_out = committer.commit_atomic_scanout(&PageFlipCommitOutcome::Rejected {
        frame_serial: 93,
        commit: TransactionCommit {
            transaction: TransactionId::from_raw(60),
            outcome: TransactionOutcome::TimedOut,
            applied_surfaces: Vec::new(),
        },
    });
    assert_eq!(timed_out.status, LiveAtomicScanoutCommitStatus::TimedOut);
    assert_eq!(committer.committed_count(), 1);
}

