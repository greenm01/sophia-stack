#[test]
fn rendered_output_table_is_bounded_and_keeps_output_state_independent() {
    let mut table = LiveRenderedOutputTable::new();
    for raw in 1..=16 {
        assert_eq!(
            table.insert(LiveRenderedOutputState::ready(HeadlessOutput {
                id: OutputId::from_raw(raw),
                size: Size {
                    width: 640 + i32::try_from(raw).unwrap(),
                    height: 480,
                },
                scale: 1,
            })),
            LiveRenderedOutputTableUpdate::Inserted
        );
    }
    assert_eq!(table.len(), 16);
    assert_eq!(
        table.insert(LiveRenderedOutputState::ready(HeadlessOutput {
            id: OutputId::from_raw(17),
            size: Size {
                width: 800,
                height: 600,
            },
            scale: 1,
        })),
        LiveRenderedOutputTableUpdate::CapacityExceeded
    );
    assert_eq!(
        table.get(OutputId::from_raw(1)).unwrap().output_size(),
        Some(Size {
            width: 641,
            height: 480,
        })
    );
    assert_eq!(
        table.get(OutputId::from_raw(16)).unwrap().output_size(),
        Some(Size {
            width: 656,
            height: 480,
        })
    );
}
use sophia_engine::AuthorityTransactionIntake;
use sophia_protocol::{
    AuthorityKind, InputEventKind, InputEventPacket, LayerSnapshot, NamespaceId, Point, Rect,
    Region, ResizeSyncCapability, SurfaceId, SurfaceTransactionReadiness, Transform,
};
use sophia_protocol::{TransactionCommit, TransactionId, TransactionOutcome};

#[test]
fn live_backend_startup_can_seed_headless_assembly_from_sysfs_and_static_input() {
    let root = drm_sysfs_fixture("ready");
    let connector = root.join("card0-HDMI-A-1");
    fs::create_dir_all(&connector).unwrap();
    write_fixture_file(&connector, "status", "connected\n");
    write_fixture_file(&connector, "modes", "1920x1080\n");
    write_fixture_file(&connector, "connector_id", "42\n");
    write_fixture_file(&connector, "crtc_id", "99\n");
    let config = LiveBackendConfig::new(&root).with_input_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });

    let report = discover_live_backend(&config);

    assert_eq!(
        report.status(),
        &LiveCompositorBackendDiscoveryStatus::Ready
    );
    assert_eq!(
        report.selected_output(),
        Some(HeadlessOutput {
            id: OutputId::from_raw(1),
            size: Size {
                width: 1920,
                height: 1080,
            },
            scale: 1,
        })
    );
    assert_eq!(
        report.selected_gbm_egl_frame_target(),
        Some(LiveGbmEglFrameTargetRecord {
            status: LiveGbmEglFrameTargetStatus::Ready,
            size: Size {
                width: 1920,
                height: 1080,
            },
        })
    );
    let assembly = report
        .into_headless_assembly(QueuedInputPoller::default(), RendererSelection::CpuFallback)
        .expect("ready startup should seed assembly");
    assert_eq!(assembly.input().source().devices().count(), 1);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_backend_startup_fails_closed_without_connected_outputs() {
    let root = drm_sysfs_fixture("no-output");
    let config = LiveBackendConfig::new(&root);

    let report = discover_live_backend(&config);

    assert_eq!(
        report.status(),
        &LiveCompositorBackendDiscoveryStatus::NoOutputs
    );
    assert_eq!(report.selected_output(), None);
    assert_eq!(report.selected_gbm_egl_frame_target(), None);
    assert!(
        report
            .into_headless_assembly(QueuedInputPoller::default(), RendererSelection::CpuFallback)
            .is_none()
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_backend_startup_uses_cpu_renderer_until_native_import_is_configured() {
    let root = ready_drm_sysfs_fixture("renderer-default");
    let config = LiveBackendConfig::new(&root);

    let report = discover_live_backend(&config);

    assert_eq!(report.renderer_selection(), RendererSelection::CpuFallback);
    assert_eq!(
        report.renderer_import_status(),
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::CpuFallback,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Disabled,
        }
    );
    let assembly = report
        .into_configured_headless_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed assembly");
    assert_eq!(assembly.renderer(), RendererSelection::CpuFallback);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_backend_defaults_to_gpu_preferred_policy() {
    let config = LiveBackendConfig::new("/does/not/matter");

    assert_eq!(
        config.renderer_preference,
        LiveRendererPreference::GpuPreferred
    );
}

#[test]
fn renderer_preference_uses_cpu_only_without_native_status() {
    let root = ready_drm_sysfs_fixture("renderer-cpu-only");
    let config = LiveBackendConfig::new(&root)
        .with_renderer_import_boundary(LiveRendererImportBoundary::with_native_imports(true, true))
        .with_renderer_preference(LiveRendererPreference::CpuOnly);

    let report = discover_live_backend(&config);
    let assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed CPU-only live assembly");

    assert_eq!(
        assembly.assembly().renderer(),
        RendererSelection::CpuFallback
    );
    assert_eq!(
        assembly.renderer_observation(),
        LiveRendererRuntimeObservation {
            health: LiveRendererImportHealth::CpuFallback,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Disabled,
            selection: LiveRendererSelectionObservation::CpuFallback,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn renderer_preference_requires_gpu_when_requested() {
    let root = ready_drm_sysfs_fixture("renderer-gpu-required");
    let config =
        LiveBackendConfig::new(&root).with_renderer_preference(LiveRendererPreference::GpuRequired);

    let report = discover_live_backend(&config);

    assert!(
        report
            .into_live_runtime_assembly(QueuedInputPoller::default())
            .is_none()
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn renderer_preference_selects_import_renderer_only_for_native_capable_status() {
    let config = LiveBackendConfig::new("/does/not/matter");
    let report = discover_live_backend(&config);

    assert_eq!(
        report.renderer_selection_for_status(LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::NativeImportCapable,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Enabled,
        }),
        Some(RendererSelection::ImportCapable {
            import_xpixmap: false,
            import_dmabuf: true,
        })
    );
    assert_eq!(
        report.renderer_selection_for_status(LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::Degraded,
            xpixmap: LiveRendererImportPathStatus::Enabled,
            dmabuf: LiveRendererImportPathStatus::Degraded,
        }),
        Some(RendererSelection::CpuFallback)
    );
}

#[test]
fn live_backend_startup_admits_native_renderer_import_only_when_configured() {
    let root = ready_drm_sysfs_fixture("renderer-native");
    let config = LiveBackendConfig::new(&root).with_renderer_import_boundary(
        LiveRendererImportBoundary::with_native_imports(true, false),
    );

    let report = discover_live_backend(&config);

    assert_eq!(
        report.renderer_selection(),
        RendererSelection::ImportCapable {
            import_xpixmap: true,
            import_dmabuf: false,
        }
    );
    assert_eq!(
        report.renderer_import_status(),
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::NativeImportCapable,
            xpixmap: LiveRendererImportPathStatus::Enabled,
            dmabuf: LiveRendererImportPathStatus::Disabled,
        }
    );
    let assembly = report
        .into_configured_headless_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed assembly");
    assert_eq!(
        assembly.renderer(),
        RendererSelection::ImportCapable {
            import_xpixmap: true,
            import_dmabuf: false,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_reports_reduced_renderer_health_on_tick() {
    let root = ready_drm_sysfs_fixture("runtime-renderer-health");
    let config = LiveBackendConfig::new(&root).with_renderer_import_boundary(
        LiveRendererImportBoundary::with_native_imports(true, false),
    );

    let report = discover_live_backend(&config);
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    assert_eq!(
        assembly.renderer_observation(),
        LiveRendererRuntimeObservation {
            health: LiveRendererImportHealth::NativeImportCapable,
            xpixmap: LiveRendererImportPathStatus::Enabled,
            dmabuf: LiveRendererImportPathStatus::Disabled,
            selection: LiveRendererSelectionObservation::NativeImportCapable,
        }
    );
    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should succeed");
    assert_eq!(tick.renderer, assembly.renderer_observation());
    assert_eq!(
        tick.scanout,
        LiveScanoutReadinessReport {
            status: LiveScanoutReadinessStatus::Ready,
        }
    );
    assert_eq!(
        tick.kms_scanout_target,
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::Ready,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );
    assert_eq!(
        assembly.gbm_egl_frame_target_observation(),
        Some(LiveGbmEglFrameTargetRecord {
            status: LiveGbmEglFrameTargetStatus::Ready,
            size: Size {
                width: 1280,
                height: 720,
            },
        })
    );
    assert_eq!(
        tick.gbm_egl_frame_target,
        assembly.gbm_egl_frame_target_observation()
    );
    assert_eq!(
        tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Ready,
            frame_serial: None,
        }
    );
    assert_eq!(
        assembly.libdrm_poller_diagnostics(),
        LiveLibdrmPollerDiagnostics::not_configured()
    );
    assert_eq!(
        tick.libdrm_poller,
        LiveLibdrmPollerDiagnostics {
            status: LiveLibdrmPollerDiagnosticsStatus::NotConfigured,
            route_count: 0,
            pending_callbacks: 0,
            decoded_callbacks: 0,
            rejected_callbacks: 0,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_threads_scanout_and_page_flip_observations() {
    let root = ready_drm_sysfs_fixture("runtime-scanout-page-flip");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    assert_eq!(
        assembly.scanout_readiness_observation(),
        LiveScanoutReadinessReport {
            status: LiveScanoutReadinessStatus::Ready,
        }
    );
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Ready,
            frame_serial: None,
        }
    );
    assert_eq!(
        assembly.kms_scanout_target_observation(),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::Ready,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );

    assembly.observe_presentation_report(LiveRendererPresentationReport {
        status: LiveRendererPresentationStatus::Unavailable,
    });
    assert_eq!(
        assembly.scanout_readiness_observation(),
        LiveScanoutReadinessReport {
            status: LiveScanoutReadinessStatus::PresentationUnavailable,
        }
    );
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::PresentationUnavailable,
            frame_serial: None,
        }
    );
    assert_eq!(
        assembly.kms_scanout_target_observation(),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::PresentationUnavailable,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );

    let mut committer = FakeAtomicScanoutCommitter::default();
    let atomic_commit = assembly.commit_atomic_scanout_with(
        &mut committer,
        &PageFlipCommitOutcome::Committed {
            frame_serial: 121,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(71),
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![sophia_protocol::SurfaceId::new(101, 1)],
            },
        },
    );
    assert_eq!(
        atomic_commit,
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::Committed,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(121),
            },
        }
    );
    assert_eq!(committer.committed_count(), 1);
    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should succeed");

    assert_eq!(
        tick.scanout,
        LiveScanoutReadinessReport {
            status: LiveScanoutReadinessStatus::PresentationUnavailable,
        }
    );
    assert_eq!(
        tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(121),
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_runs_fake_compositor_loop_without_native_scanout() {
    let root = ready_drm_sysfs_fixture("runtime-fake-compositor-loop");
    let config = LiveBackendConfig::new(&root).with_input_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });
    let report = discover_live_backend(&config);
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::new(vec![motion_event(1, 10.0, 20.0)]))
        .expect("ready startup should seed live assembly");
    let mut layer = test_layer(42);
    let transaction_id = TransactionId::from_raw(420);
    let transaction = layer.to_surface_transaction(
        transaction_id,
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        0,
    );

    layer.geometry.x = 25;
    layer.geometry.y = 30;
    let tick = assembly
        .run_tick(CompositorBackendTickInput {
            x_event_count: 1,
            authority_commits: Vec::new(),
            authority_batches: vec![AuthorityTransactionIntake::new(
                transaction_id,
                vec![transaction],
            )],
            wm_update: None,
            portal_commands: Vec::new(),
            chrome_command_count: 1,
            layer_templates: vec![layer],
            scanout_submit_state: None,
            scanout_lifecycle_states: Vec::new(),
        })
        .expect("fake compositor runtime tick should complete");

    assert_eq!(tick.engine.input_poll.polled, 1);
    assert_eq!(tick.engine.input_poll.accepted, 1);
    assert!(tick.engine.input_poll.rejected.is_empty());
    assert_eq!(tick.engine.physical_input.poll, tick.engine.input_poll);
    assert_eq!(tick.engine.physical_input.pending_events, 1);
    assert_eq!(
        tick.engine.physical_input.routing_stage,
        PhysicalInputRoutingStage::PhysicalIntakeOnly
    );
    assert_eq!(tick.engine.runtime.runtime_state.x_events_polled, 1);
    assert_eq!(
        tick.engine
            .runtime
            .runtime_state
            .authority_transactions_committed,
        1
    );
    assert_eq!(
        tick.engine.runtime.runtime_state.authority_surfaces_applied,
        1
    );
    assert_eq!(tick.engine.runtime.runtime_state.frames_rendered, 1);
    assert_eq!(
        tick.engine.runtime.runtime_state.chrome_commands_presented,
        1
    );
    assert!(tick.engine.render.is_some());
    assert_eq!(
        tick.gbm_egl_frame_target_lifecycle
            .expect("startup target lifecycle should be observed")
            .status,
        LiveGbmEglFrameTargetLifecycleStatus::Created,
    );
    assert_eq!(
        tick.kms_scanout_target,
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::Ready,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );
    assert_eq!(
        tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Ready,
            frame_serial: None,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

