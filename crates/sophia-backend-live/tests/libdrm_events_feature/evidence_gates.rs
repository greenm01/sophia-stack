#[test]
fn runtime_rendered_scanout_failure_evidence_has_stable_reduced_line() {
    let report = LiveRuntimeRenderedScanoutEvidenceFailureReport::new(
        LiveRuntimeRenderedScanoutEvidenceFailureStatus::RetireTimedOut,
        true,
        false,
    );

    assert_eq!(
        report.reduced_log_line(),
        "sophia_runtime_rendered_scanout_failure schema=1 status=RetireTimedOut submit_seen=true retire_seen=false"
    );
}

#[test]
fn live_session_composition_smoke_rejects_empty_authority_batches() {
    let report = run_live_session_composition_smoke(Vec::new());

    assert_eq!(
        report.status,
        LiveSessionCompositionSmokeStatus::NoAuthorityBatches
    );
    assert_eq!(
        report.reduced_log_line(),
        "sophia_live_session_composition schema=2 status=NoAuthorityBatches authority_batches_input=0 authority_batches_drained=0 authority_transactions_committed=0 authority_surfaces_applied=0 rendered_scanout_submit=none rendered_scanout_retire=none rendered_scanout_cleanup=none runtime_scanout_state=none rendered_scanout_in_flight=false cleanup_pending=false"
    );
}

#[test]
fn live_session_composition_smoke_commits_authority_batch_and_submits_rendered_scanout() {
    let transaction_id = TransactionId::from_raw(90);
    let report = run_live_session_composition_smoke(vec![AuthorityTransactionIntake::new(
        transaction_id,
        vec![live_session_composition_transaction(transaction_id)],
    )]);

    assert_eq!(report.status, LiveSessionCompositionSmokeStatus::Passed);
    assert_eq!(report.authority_batches_input, 1);
    assert_eq!(report.authority_batches_drained, 1);
    assert_eq!(report.authority_transactions_committed, 1);
    assert_eq!(report.authority_surfaces_applied, 1);
    assert_eq!(
        report.rendered_scanout_submit,
        Some(LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip)
    );
    assert_eq!(
        report.rendered_scanout_retire,
        Some(LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip)
    );
    assert_eq!(
        report.rendered_scanout_cleanup,
        Some(LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::NoCleanupPending)
    );
    assert_eq!(
        report.runtime_scanout_state,
        Some(RuntimeScanoutState::Retired)
    );
    assert!(!report.rendered_scanout_in_flight);
    assert!(!report.cleanup_pending);
    assert_eq!(
        report.reduced_log_line(),
        "sophia_live_session_composition schema=2 status=Passed authority_batches_input=1 authority_batches_drained=1 authority_transactions_committed=1 authority_surfaces_applied=1 rendered_scanout_submit=SubmittedWaitingForPageFlip rendered_scanout_retire=RetiredAfterPageFlip rendered_scanout_cleanup=NoCleanupPending runtime_scanout_state=Retired rendered_scanout_in_flight=false cleanup_pending=false"
    );
}

fn live_session_composition_transaction(transaction: TransactionId) -> SurfaceTransaction {
    SurfaceTransaction {
        transaction,
        authority: AuthorityKind::SophiaX,
        surface: SurfaceId::new(9, 1),
        namespace: Some(NamespaceId::from_raw(47)),
        target_geometry: Rect {
            x: 20,
            y: 30,
            width: 640,
            height: 480,
        },
        target_content_size: Size {
            width: 640,
            height: 480,
        },
        target_buffer: BufferSource::CpuBuffer { handle: 0x990 },
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        }),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    }
}

#[cfg(feature = "gbm-probe")]
#[test]
fn real_runtime_rendered_scanout_evidence_reports_native_renderer_observation() {
    assert_eq!(
        real_atomic_runtime_rendered_scanout_renderer_observation(),
        LiveRendererRuntimeObservation {
            health: LiveRendererImportHealth::NativeImportCapable,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Enabled,
            selection: LiveRendererSelectionObservation::NativeImportCapable,
        }
    );
}

#[test]
fn libdrm_dependency_is_admitted_without_exposing_native_event_shape() {
    assert_eq!(
        libdrm_dependency_admission_report(),
        LibdrmDependencyAdmissionReport {
            status: LibdrmDependencyAdmissionStatus::TypedPageFlipEventAvailable,
        }
    );
}

#[test]
fn real_libdrm_event_validation_gate_is_explicit_and_reduced() {
    let skipped = LiveHardwareValidationGateReport::from_env_presence(
        LiveHardwareValidationTarget::LibdrmEvents,
        false,
    );
    assert_eq!(
        skipped,
        LiveHardwareValidationGateReport {
            target: LiveHardwareValidationTarget::LibdrmEvents,
            status: LiveHardwareValidationGateStatus::SkippedOptInRequired,
        }
    );
    assert!(!skipped.is_requested());
    assert_eq!(
        skipped.target.env_var(),
        "SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE"
    );

    let requested = LiveHardwareValidationGateReport::from_env_presence(
        LiveHardwareValidationTarget::LibdrmEvents,
        true,
    );
    assert_eq!(
        requested.status,
        LiveHardwareValidationGateStatus::Requested
    );
    assert!(requested.is_requested());

    assert_eq!(
        real_libdrm_events_validation_gate().target,
        LiveHardwareValidationTarget::LibdrmEvents
    );
}

#[test]
fn runtime_page_flip_observation_prefers_accepted_callback_over_later_stale_rejection() {
    let root = ready_drm_sysfs_fixture("runtime-page-flip-accepted-dominates-stale");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let (sender, receiver) = mpsc::sync_channel(4);
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 4));

    sender
        .try_send(LivePageFlipCallback {
            output: OutputId::from_raw(1),
            head: sophia_engine::RenderHeadId::from_raw(1),
            frame_serial: 62,
        })
        .expect("test channel should accept first callback");
    sender
        .try_send(LivePageFlipCallback {
            output: OutputId::from_raw(1),
            head: sophia_engine::RenderHeadId::from_raw(1),
            frame_serial: 61,
        })
        .expect("test channel should accept stale callback");

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain callbacks");

    assert_eq!(
        tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(62),
        }
    );
    assert_eq!(tick.page_flip_callbacks.drained, 2);
    assert_eq!(tick.page_flip_callbacks.accepted, 1);
    assert_eq!(tick.page_flip_callbacks.rejected_stale_frame_serial, 1);
    assert_eq!(
        tick.page_flip_callbacks
            .last_accepted
            .expect("accepted callback should be retained")
            .event
            .frame_serial,
        Some(62)
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn real_libdrm_event_validation_smoke_fails_closed_without_device_opening_smoke() {
    let skipped = LiveHardwareValidationSmokeReport::fail_closed_from_gate(
        LiveHardwareValidationGateReport::from_env_presence(
            LiveHardwareValidationTarget::LibdrmEvents,
            false,
        ),
    );
    assert_eq!(
        skipped,
        LiveHardwareValidationSmokeReport {
            target: LiveHardwareValidationTarget::LibdrmEvents,
            status: LiveHardwareValidationSmokeStatus::SkippedOptInRequired,
        }
    );

    let requested = LiveHardwareValidationSmokeReport::fail_closed_from_gate(
        LiveHardwareValidationGateReport::from_env_presence(
            LiveHardwareValidationTarget::LibdrmEvents,
            true,
        ),
    );
    assert_eq!(
        requested,
        LiveHardwareValidationSmokeReport {
            target: LiveHardwareValidationTarget::LibdrmEvents,
            status: LiveHardwareValidationSmokeStatus::BackendUnavailable,
        }
    );

    assert_eq!(
        real_libdrm_events_validation_smoke_report().target,
        LiveHardwareValidationTarget::LibdrmEvents
    );
}

#[test]
fn real_atomic_scanout_validation_gate_is_explicit_and_reduced() {
    let skipped = LiveHardwareValidationGateReport::from_env_presence(
        LiveHardwareValidationTarget::AtomicScanout,
        false,
    );
    assert_eq!(
        skipped,
        LiveHardwareValidationGateReport {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status: LiveHardwareValidationGateStatus::SkippedOptInRequired,
        }
    );
    assert_eq!(
        skipped.target.env_var(),
        "SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE"
    );

    let requested = LiveHardwareValidationGateReport::from_env_presence(
        LiveHardwareValidationTarget::AtomicScanout,
        true,
    );
    assert_eq!(
        requested.status,
        LiveHardwareValidationGateStatus::Requested
    );
    assert!(requested.is_requested());

    assert_eq!(
        real_atomic_scanout_validation_gate().target,
        LiveHardwareValidationTarget::AtomicScanout
    );
    assert_eq!(
        real_atomic_scanout_validation_smoke_report().target,
        LiveHardwareValidationTarget::AtomicScanout
    );
}

#[test]
fn real_atomic_scanout_card_selection_fails_closed_without_device_identity() {
    let missing_root = std::env::temp_dir().join("sophia-missing-dri-card-selection");
    let _ = std::fs::remove_dir_all(&missing_root);
    let missing = select_real_atomic_scanout_card_from_dev_dri(&missing_root);
    assert_eq!(
        missing.status,
        RealAtomicScanoutCardSelectionStatus::DeviceDirectoryUnavailable
    );
    assert_eq!(
        missing.status.failure_evidence().status,
        LibdrmNativeAtomicScanoutSmokeStatus::NoPrimaryCard
    );
    assert!(missing.card.is_none());
    assert!(missing.selection.is_none());
    let slot = LibdrmNativeOutputSlot::new(1).expect("slot one should be valid");
    let authority =
        LibdrmBackendFdAuthority::new(31).expect("nonzero authority generation should mint");
    let missing_session = missing.into_page_flip_session(
        slot,
        OutputId::from_raw(1),
        sophia_engine::RenderHeadId::from_raw(1),
        authority,
    );
    assert_eq!(
        missing_session.status,
        RealAtomicScanoutPageFlipSessionStatus::CardSelectionFailed
    );
    assert_eq!(
        missing_session.card_selection_status,
        RealAtomicScanoutCardSelectionStatus::DeviceDirectoryUnavailable
    );
    assert_eq!(
        missing_session
            .failure_evidence()
            .expect("failed session should reduce to smoke evidence")
            .status,
        LibdrmNativeAtomicScanoutSmokeStatus::NoPrimaryCard
    );
    assert!(missing_session.session.is_none());

    let empty_root = std::env::temp_dir().join("sophia-empty-dri-card-selection");
    let _ = std::fs::remove_dir_all(&empty_root);
    std::fs::create_dir_all(&empty_root).unwrap();
    let empty = select_real_atomic_scanout_card_from_dev_dri(&empty_root);
    assert_eq!(
        empty.status,
        RealAtomicScanoutCardSelectionStatus::NoPrimaryCardNodes
    );
    assert_eq!(
        empty.status.failure_evidence().status,
        LibdrmNativeAtomicScanoutSmokeStatus::NoPrimaryCard
    );
    assert!(empty.card.is_none());
    assert!(empty.selection.is_none());
    std::fs::remove_dir_all(empty_root).unwrap();
}

#[test]
fn real_atomic_scanout_page_flip_wait_policy_matches_hardware_smoke_budget() {
    let policy = RealAtomicScanoutPageFlipWaitPolicy::hardware_smoke();

    assert_eq!(policy.max_read, 4);
    assert_eq!(policy.max_emit, 1);
    assert_eq!(policy.timeout, std::time::Duration::from_secs(8));
    assert_eq!(policy.sleep, std::time::Duration::from_millis(5));
}

#[cfg(feature = "gbm-probe")]
#[test]
fn real_atomic_scanout_smoke_config_rejects_zero_identity_fields() {
    let policy = RealAtomicScanoutPageFlipWaitPolicy::hardware_smoke();
    let config = RealAtomicScanoutSmokeConfig::from_raw(1, 7, 9, policy)
        .expect("nonzero slot and authority should mint smoke config");

    assert_eq!(config.slot.raw(), 1);
    assert_eq!(config.output.raw(), 7);
    assert_eq!(config.authority.generation(), 9);
    assert_eq!(config.wait_policy, policy);
    assert!(RealAtomicScanoutSmokeConfig::from_raw(0, 7, 9, policy).is_none());
    assert!(RealAtomicScanoutSmokeConfig::from_raw(1, 7, 0, policy).is_none());
}

#[test]
fn atomic_scanout_preflight_reduces_host_readiness_without_identity() {
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(false, 2, 2, 2, 2, 2),
        LiveAtomicScanoutPreflightReport {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status: LiveAtomicScanoutPreflightStatus::DeviceDirectoryUnavailable,
            primary_card_nodes: 0,
            openable_primary_card_nodes: 0,
            atomic_capable_primary_card_nodes: 0,
            scanout_target_primary_card_nodes: 0,
            atomic_property_primary_card_nodes: 0,
        }
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 0, 0, 0, 0, 0),
        LiveAtomicScanoutPreflightReport {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status: LiveAtomicScanoutPreflightStatus::NoPrimaryCardNodes,
            primary_card_nodes: 0,
            openable_primary_card_nodes: 0,
            atomic_capable_primary_card_nodes: 0,
            scanout_target_primary_card_nodes: 0,
            atomic_property_primary_card_nodes: 0,
        }
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 2, 0, 0, 0, 0),
        LiveAtomicScanoutPreflightReport {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status: LiveAtomicScanoutPreflightStatus::PrimaryCardOpenUnavailable,
            primary_card_nodes: 2,
            openable_primary_card_nodes: 0,
            atomic_capable_primary_card_nodes: 0,
            scanout_target_primary_card_nodes: 0,
            atomic_property_primary_card_nodes: 0,
        }
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 2, 1, 0, 0, 0),
        LiveAtomicScanoutPreflightReport {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status: LiveAtomicScanoutPreflightStatus::AtomicClientCapabilityUnavailable,
            primary_card_nodes: 2,
            openable_primary_card_nodes: 1,
            atomic_capable_primary_card_nodes: 0,
            scanout_target_primary_card_nodes: 0,
            atomic_property_primary_card_nodes: 0,
        }
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 2, 2, 1, 0, 0),
        LiveAtomicScanoutPreflightReport {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status: LiveAtomicScanoutPreflightStatus::KmsScanoutTargetUnavailable,
            primary_card_nodes: 2,
            openable_primary_card_nodes: 2,
            atomic_capable_primary_card_nodes: 1,
            scanout_target_primary_card_nodes: 0,
            atomic_property_primary_card_nodes: 0,
        }
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 2, 2, 1, 1, 0),
        LiveAtomicScanoutPreflightReport {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status: LiveAtomicScanoutPreflightStatus::AtomicPropertyDiscoveryUnavailable,
            primary_card_nodes: 2,
            openable_primary_card_nodes: 2,
            atomic_capable_primary_card_nodes: 1,
            scanout_target_primary_card_nodes: 1,
            atomic_property_primary_card_nodes: 0,
        }
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 2, 2, 1, 1, 1),
        LiveAtomicScanoutPreflightReport {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status: LiveAtomicScanoutPreflightStatus::CandidatePrimaryCardsAtomicReady,
            primary_card_nodes: 2,
            openable_primary_card_nodes: 2,
            atomic_capable_primary_card_nodes: 1,
            scanout_target_primary_card_nodes: 1,
            atomic_property_primary_card_nodes: 1,
        }
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(
            true,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX
        )
        .primary_card_nodes,
        LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(
            true,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX
        )
        .openable_primary_card_nodes,
        LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(
            true,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX
        )
        .atomic_capable_primary_card_nodes,
        LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(
            true,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX
        )
        .scanout_target_primary_card_nodes,
        LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(
            true,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX
        )
        .atomic_property_primary_card_nodes,
        LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 2, 1, 2, 2, 2)
            .atomic_capable_primary_card_nodes,
        1
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 2, 2, 1, 2, 2)
            .scanout_target_primary_card_nodes,
        1
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 2, 2, 2, 1, 2)
            .atomic_property_primary_card_nodes,
        1
    );

    let real = real_atomic_scanout_preflight_report();
    println!("{}", real.reduced_log_line());
    assert_eq!(real.target, LiveHardwareValidationTarget::AtomicScanout);
    assert!(real.primary_card_nodes <= LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS);
    assert!(real.openable_primary_card_nodes <= LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS);
    assert!(
        real.atomic_capable_primary_card_nodes <= LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS
    );
    assert!(
        real.scanout_target_primary_card_nodes <= LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS
    );
    assert!(
        real.atomic_property_primary_card_nodes <= LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS
    );
    assert!(real.atomic_capable_primary_card_nodes <= real.openable_primary_card_nodes);
    assert!(real.scanout_target_primary_card_nodes <= real.atomic_capable_primary_card_nodes);
    assert!(real.atomic_property_primary_card_nodes <= real.scanout_target_primary_card_nodes);
    assert!(
        real.reduced_log_line()
            .starts_with("sophia_atomic_scanout_preflight schema=5 target=AtomicScanout status=")
    );
}

#[test]
fn libdrm_fd_authority_is_generation_checked_and_reduced() {
    assert_eq!(LibdrmBackendFdAuthority::new(0), None);

    let authority =
        LibdrmBackendFdAuthority::new(9).expect("nonzero generation should mint authority token");
    assert_eq!(authority.generation(), 9);
    assert_eq!(
        libdrm_fd_authority_report(authority),
        LibdrmBackendFdAuthorityReport {
            status: LibdrmBackendFdAuthorityStatus::BackendOwned,
        }
    );
}

#[test]
fn native_libdrm_event_adapter_skeleton_reports_ready_without_opening_devices() {
    assert_eq!(
        native_libdrm_event_adapter_report(),
        LibdrmNativeEventAdapterReport {
            status: LibdrmNativeEventAdapterStatus::SkeletonReady,
        }
    );
}

#[test]
fn native_libdrm_event_adapter_accepts_authority_without_polling() {
    let authority =
        LibdrmBackendFdAuthority::new(12).expect("nonzero generation should mint authority token");

    assert_eq!(
        native_libdrm_event_adapter_report_for_authority(authority),
        LibdrmNativeEventAdapterReport {
            status: LibdrmNativeEventAdapterStatus::SkeletonReady,
        }
    );
}

#[test]
fn native_libdrm_page_flip_source_constructs_from_authority_without_reading_events() {
    let authority =
        LibdrmBackendFdAuthority::new(13).expect("nonzero generation should mint authority token");
    let source = LibdrmNativePageFlipSource::from_authority(authority);

    assert_eq!(
        source.report(),
        LibdrmNativePageFlipSourceReport {
            status: LibdrmNativePageFlipSourceStatus::ConstructedWithoutPolling,
        }
    );
}
