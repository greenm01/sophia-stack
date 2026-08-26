#[test]
fn wm_transaction_update_maps_to_runtime_observation() {
    let update = WmTransactionUpdate {
        commit: TransactionCommit {
            transaction: TransactionId::from_raw(63),
            outcome: TransactionOutcome::Committed,
            applied_surfaces: Vec::new(),
        },
    };

    assert_eq!(
        runtime_observation_from_wm_transaction_update(&update),
        SessionRuntimeObservation::WmLayoutReady
    );
}

#[test]
fn authority_transaction_commit_maps_to_reduced_runtime_observation() {
    let commit = TransactionCommit {
        transaction: TransactionId::from_raw(88),
        outcome: TransactionOutcome::Committed,
        applied_surfaces: vec![SurfaceId::new(1, 1), SurfaceId::new(2, 1)],
    };

    assert_eq!(
        runtime_observation_from_authority_transaction_commit(&commit),
        SessionRuntimeObservation::AuthorityTransactionObserved {
            outcome: TransactionOutcome::Committed,
            applied_surface_count: 2,
        }
    );
}

#[test]
fn authority_removal_batch_drops_the_committed_surface() {
    let engine = HeadlessEngine::default();
    let surface = SurfaceId::new(77, 1);
    let mut committed = vec![CommittedSurfaceState {
        surface,
        committed_generation: 4,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 60,
        },
        content: sophia_protocol::SurfaceContentSet::singleton(BufferSource::CpuBuffer { handle: 77 }, sophia_protocol::Size { width: 80, height: 60 }),
        damage: Region::empty(),
    }];

    let commit = AuthorityTransactionIntake::new(TransactionId::from_raw(89), Vec::new())
        .with_surface_removals(vec![surface])
        .commit(&engine, &mut committed);

    assert_eq!(commit.outcome, TransactionOutcome::Committed);
    assert!(commit.applied_surfaces.is_empty());
    assert!(committed.is_empty());
}

#[test]
fn slow_client_visual_decisions_map_to_count_only_runtime_observation() {
    let surface_a = SurfaceId::new(1, 1);
    let surface_b = SurfaceId::new(2, 1);
    let decisions = [
        SlowClientVisualDecision::PreserveCommitted {
            surface: surface_a,
            committed: None,
        },
        SlowClientVisualDecision::ReplanAtCommittedExtent {
            surface: surface_b,
            committed: None,
            extent: Some(Size {
                width: 100,
                height: 100,
            }),
        },
        SlowClientVisualDecision::NotTimedOut {
            surface: SurfaceId::new(3, 1),
            readiness: SurfaceTransactionCommitReadiness::Ready,
        },
    ];

    assert_eq!(
        runtime_observation_from_slow_client_visual_decisions(&decisions),
        SessionRuntimeObservation::SlowClientVisualDecisionsObserved {
            timeout_count: 2,
            preserved_count: 1,
            degraded_count: 1,
        }
    );
}

#[test]
fn frame_reports_map_to_runtime_render_observations() {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let mut last_committed = LastCommittedLayout::default();
    let session_report = engine
        .run_session_tick(
            SessionTickRequest {
                output: output.id,
                frame_serial: 77,
                layers: SessionLayerSource::Fresh(vec![test_layer(0, 0, 0, Region::empty())]),
            },
            &mut last_committed,
        )
        .unwrap();

    assert_eq!(
        runtime_observation_from_session_tick_report(&session_report),
        SessionRuntimeObservation::FrameRendered { frame_serial: 77 }
    );

    let render_report = engine.render_frame(&session_report.frame).unwrap();

    assert_eq!(
        runtime_observation_from_render_frame_report(&render_report),
        SessionRuntimeObservation::FrameRendered { frame_serial: 77 }
    );
}

#[test]
fn portal_commands_map_to_runtime_portal_observation() {
    let commands = [
        PortalCommand::DropNotification {
            transfer: PortalTransferId::from_raw(1),
        },
        PortalCommand::DeliverNotification {
            transfer: PortalTransferId::from_raw(2),
        },
    ];

    assert_eq!(
        runtime_observation_from_portal_commands(&commands),
        SessionRuntimeObservation::PortalCommandsReady { count: 2 }
    );
}

#[test]
fn chrome_updates_map_to_runtime_chrome_observations() {
    let notification_updates = [
        NotificationChromeUpdate::Staged {
            transfer: PortalTransferId::from_raw(1),
        },
        NotificationChromeUpdate::Presented {
            transfer: PortalTransferId::from_raw(1),
        },
        NotificationChromeUpdate::Dismissed {
            transfer: PortalTransferId::from_raw(1),
        },
        NotificationChromeUpdate::Ignored,
    ];

    assert_eq!(
        runtime_observation_from_notification_chrome_updates(&notification_updates),
        SessionRuntimeObservation::ChromeCommandsReady { count: 2 }
    );

    let metadata_updates = [
        MetadataChromeUpdate::Upserted {
            surface: SurfaceId::new(1, 1),
        },
        MetadataChromeUpdate::Removed {
            surface: SurfaceId::new(2, 1),
        },
        MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::InvalidLabel),
    ];

    assert_eq!(
        runtime_observation_from_metadata_chrome_updates(&metadata_updates),
        SessionRuntimeObservation::ChromeCommandsReady { count: 2 }
    );
}

#[test]
fn headless_session_driver_executes_runtime_commands_to_idle() {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let mut driver = HeadlessSessionDriver::new(engine);
    let transaction = TransactionId::from_raw(80);

    let report = driver
        .run_tick(HeadlessSessionDriverTick {
            output: output.id,
            frame_serial: 90,
            x_event_count: 1,
            layers: vec![test_layer(1, 0, 0, Region::empty())],
            wm_update: Some(WmTransactionUpdate {
                commit: TransactionCommit {
                    transaction,
                    outcome: TransactionOutcome::Committed,
                    applied_surfaces: vec![SurfaceId::new(1, 1)],
                },
            }),
            portal_commands: vec![PortalCommand::DropNotification {
                transfer: PortalTransferId::from_raw(1),
            }],
            chrome_command_count: 2,
        })
        .unwrap();

    assert_eq!(
        report.runtime_commands,
        vec![
            SessionRuntimeCommand::PollXEvents,
            SessionRuntimeCommand::RequestWmLayout,
            SessionRuntimeCommand::ScheduleFrame,
            SessionRuntimeCommand::RenderFrame { frame_serial: 90 },
            SessionRuntimeCommand::SubmitScanout { frame_serial: 90 },
            SessionRuntimeCommand::DrainPortalCommands,
            SessionRuntimeCommand::PresentChrome,
        ]
    );
    assert_eq!(report.runtime_state.phase, SessionRuntimePhase::Idle);
    assert_eq!(report.runtime_state.x_events_polled, 1);
    assert_eq!(report.runtime_state.frames_rendered, 1);
    assert_eq!(report.runtime_state.scanout_submissions, 1);
    assert_eq!(report.runtime_state.in_flight_scanouts, 1);
    assert_eq!(report.runtime_state.portal_commands_drained, 1);
    assert_eq!(report.runtime_state.chrome_commands_presented, 2);
    assert_eq!(report.cached_layers, 1);
    assert_eq!(
        report
            .session_tick
            .as_ref()
            .map(|tick| tick.frame.frame_serial),
        Some(90)
    );
}

#[test]
fn headless_session_driver_executes_through_runtime_adapter_trait() {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let mut driver = HeadlessSessionDriver::new(engine);
    let mut adapter = HeadlessRuntimeAdapter {
        x_event_count: 1,
        layers: vec![test_layer(1, 0, 0, Region::empty())],
        wm_update: None,
        portal_commands: Vec::new(),
        chrome_command_count: 0,
    };

    let report = driver
        .run_with_adapter(output.id, 92, &mut adapter)
        .expect("headless adapter should drive one runtime tick");

    assert_eq!(report.runtime_state.phase, SessionRuntimePhase::Idle);
    assert_eq!(report.runtime_state.frames_rendered, 1);
    assert_eq!(report.runtime_state.scanout_submissions, 1);
    assert_eq!(
        report.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Submitted)
    );
    assert_eq!(
        report
            .session_tick
            .as_ref()
            .map(|tick| tick.frame.frame_serial),
        Some(92)
    );
}

#[test]
fn live_runtime_driver_adapter_executes_through_shared_command_executor() {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let mut driver = HeadlessSessionDriver::new(engine);
    let mut adapter = LiveRuntimeDriverAdapter::from_intake(LiveRuntimeDriverIntake {
        x_event_count: 1,
        authority_commits: Vec::new(),
        authority_batches: Vec::new(),
        wm_update: None,
        portal_commands: vec![PortalCommand::DropNotification {
            transfer: PortalTransferId::from_raw(3),
        }],
        chrome_command_count: 1,
        layers: vec![test_layer(1, 0, 0, Region::empty())],
        committed_surfaces: Vec::new(),
        scanout_submit_state: None,
        scanout_lifecycle_states: Vec::new(),
    });

    let report = driver
        .run_with_adapter(output.id, 93, &mut adapter)
        .expect("live adapter intake should drive one runtime tick");

    assert_eq!(report.runtime_state.phase, SessionRuntimePhase::Idle);
    assert_eq!(report.runtime_state.x_events_polled, 1);
    assert_eq!(report.runtime_state.scanout_submissions, 1);
    assert_eq!(report.runtime_state.portal_commands_drained, 1);
    assert_eq!(report.runtime_state.chrome_commands_presented, 1);
    assert_eq!(
        report
            .session_tick
            .as_ref()
            .map(|tick| tick.frame.frame_serial),
        Some(93)
    );
}

#[test]
fn live_runtime_driver_adapter_builds_from_nonblocking_intake_values() {
    let update = WmTransactionUpdate {
        commit: TransactionCommit {
            transaction: TransactionId::from_raw(83),
            outcome: TransactionOutcome::Committed,
            applied_surfaces: vec![SurfaceId::new(1, 1)],
        },
    };

    let adapter = LiveRuntimeDriverAdapter::from_intake(LiveRuntimeDriverIntake {
        x_event_count: 2,
        authority_commits: Vec::new(),
        authority_batches: Vec::new(),
        wm_update: Some(update.clone()),
        portal_commands: vec![PortalCommand::DropNotification {
            transfer: PortalTransferId::from_raw(3),
        }],
        chrome_command_count: 4,
        layers: vec![test_layer(1, 0, 0, Region::empty())],
        committed_surfaces: Vec::new(),
        scanout_submit_state: Some(RuntimeScanoutState::Submitted),
        scanout_lifecycle_states: Vec::new(),
    });

    assert_eq!(adapter.x, LiveXRuntimeAdapter::from_polled_event_count(2));
    assert_eq!(
        adapter.wm,
        LiveWmRuntimeAdapter::from_transaction_update(update)
    );
    assert_eq!(
        adapter.portal.drain_observation(),
        SessionRuntimeObservation::PortalCommandsReady { count: 1 }
    );
    assert_eq!(
        adapter.chrome.present_observation(),
        SessionRuntimeObservation::ChromeCommandsReady { count: 4 }
    );
    assert_eq!(adapter.renderer.layers.len(), 1);
    assert_eq!(
        adapter.scanout.submit_observation(77),
        SessionRuntimeObservation::ScanoutStateChanged {
            state: RuntimeScanoutState::Submitted,
            frame_serial: Some(77),
        }
    );
}
