#[cfg(feature = "libinput-events")]
#[test]
fn live_session_loop_tick_observes_readiness_then_retire_and_submit() {
    let root = ready_drm_sysfs_fixture("session-loop-ready-input-page-flip-scanout");
    let config = LiveBackendConfig::new(&root).with_input_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });
    let report = discover_live_backend(&config);
    let (sender, receiver) = mpsc::sync_channel(2);
    let poller = LiveInputReadinessGatedPoller::new(NativeLibinputEventPoller::new(
        FakeLiveLibinputEventReader::new([libinput_motion_event(1, 42.0, 24.0)]),
        4,
    ));
    let mut assembly = report
        .into_live_runtime_assembly(poller)
        .expect("ready backend should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 2));
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let submitted =
        assembly.submit_and_track_rendered_primary_plane_scanout_with(&device, &mut exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    let slot = LibdrmNativeOutputSlot::new(1).expect("slot one should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(
        LibdrmBackendFdAuthority::new(37).expect("nonzero authority should mint"),
    );
    let page_flip_poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(1),
        }]);
    let mut session_loop = LiveBackendSessionLoop::new(
        page_flip_poller,
        LiveBackendSessionLoopPageFlipBudget::new(4, 4),
    );
    let mut reader =
        FakeLibdrmNativePageFlipReader::new([LibdrmNativePageFlipCallback::new(slot, 103)]);
    let mut next_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let report = session_loop
        .run_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with(
            &mut assembly,
            CompositorBackendTickInput::default(),
            LiveBackendSessionLoopReadiness::all_ready(),
            &device,
            &mut next_exporter,
            &mut reader,
            &sender,
        )
        .expect("session loop tick should ingest readiness, retire, and submit");

    assert_eq!(
        report.input_gate.status,
        LiveInputReadinessGateStatus::Polled
    );
    assert_eq!(report.tick.engine.input_poll.polled, 1);
    assert_eq!(report.tick.engine.input_poll.accepted, 1);
    assert_eq!(assembly.assembly().input().source().pending_len(), 1);
    assert_eq!(
        report.native_page_flip.read_loop,
        LibdrmNativeReadLoopReport::callback_decoded(1)
            .expect("one callback should produce read evidence")
    );
    assert_eq!(
        report
            .tick
            .rendered_primary_plane_scanout_retire
            .expect("native page flip should retire in-flight scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        report
            .tick
            .rendered_primary_plane_scanout_submit
            .expect("runtime should submit the next scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(feature = "libinput-events")]
#[test]
fn live_session_loop_tick_drains_pending_page_flip_callbacks_without_new_readiness() {
    let root = ready_drm_sysfs_fixture("session-loop-pending-page-flip-without-readiness");
    let config = LiveBackendConfig::new(&root).with_input_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });
    let report = discover_live_backend(&config);
    let (sender, receiver) = mpsc::sync_channel(4);
    let poller = LiveInputReadinessGatedPoller::new(NativeLibinputEventPoller::new(
        FakeLiveLibinputEventReader::new([]),
        4,
    ));
    let mut assembly = report
        .into_live_runtime_assembly(poller)
        .expect("ready backend should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 4));
    let device = full_primary_plane_scanout_device();
    let mut initial_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let submitted = assembly
        .submit_and_track_rendered_primary_plane_scanout_with(&device, &mut initial_exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    let slot = LibdrmNativeOutputSlot::new(1).expect("slot one should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(
        LibdrmBackendFdAuthority::new(39).expect("nonzero authority should mint"),
    );
    let page_flip_poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(1),
        }]);
    let mut session_loop = LiveBackendSessionLoop::new(
        page_flip_poller,
        LiveBackendSessionLoopPageFlipBudget::new(4, 1),
    );
    let mut reader = FakeLibdrmNativePageFlipReader::new([
        LibdrmNativePageFlipCallback::new(slot, 201),
        LibdrmNativePageFlipCallback::new(slot, 202),
    ]);
    let mut first_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let first = session_loop
        .run_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with(
            &mut assembly,
            CompositorBackendTickInput::default(),
            LiveBackendSessionLoopReadiness::page_flip_ready(),
            &device,
            &mut first_exporter,
            &mut reader,
            &sender,
        )
        .expect("first tick should read native page flips and emit one callback");

    assert_eq!(
        first.native_page_flip.read_loop,
        LibdrmNativeReadLoopReport::callback_decoded(2)
            .expect("two native callbacks should decode")
    );
    assert_eq!(
        first.native_page_flip.poll.status,
        LibdrmPageFlipEventPollStatus::EmitLimitReached
    );
    assert_eq!(first.native_page_flip.poll.callbacks.emitted, 1);
    assert_eq!(first.native_page_flip.poll.callbacks.queued_remaining, 1);
    assert_eq!(reader.queued_len(), 0);
    assert_eq!(session_loop.page_flip_poller().pending_callback_count(), 1);
    assert_eq!(
        first
            .tick
            .rendered_primary_plane_scanout_retire
            .expect("first emitted callback should retire initial scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        first
            .tick
            .rendered_primary_plane_scanout_submit
            .expect("runtime should submit the next scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    let mut second_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let second = session_loop
        .run_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with(
            &mut assembly,
            CompositorBackendTickInput::default(),
            LiveBackendSessionLoopReadiness::idle(),
            &device,
            &mut second_exporter,
            &mut reader,
            &sender,
        )
        .expect("pending decoded callback should drain without fresh readiness");

    assert_eq!(
        second.native_page_flip.read_loop,
        LibdrmNativeReadLoopReport::callback_decoded(1)
            .expect("pending callback emission should be reduced as decoded")
    );
    assert_eq!(
        second.native_page_flip.poll.status,
        LibdrmPageFlipEventPollStatus::Emitted
    );
    assert_eq!(second.native_page_flip.poll.callbacks.emitted, 1);
    assert_eq!(second.native_page_flip.poll.callbacks.queued_remaining, 0);
    assert_eq!(reader.queued_len(), 0);
    assert_eq!(session_loop.page_flip_poller().pending_callback_count(), 0);
    assert_eq!(
        second
            .tick
            .rendered_primary_plane_scanout_retire
            .expect("pending callback should retire the next scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        second
            .tick
            .rendered_primary_plane_scanout_submit
            .expect("runtime should continue with the next scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_tick_rejects_rendered_scanout_when_kms_target_is_not_ready() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-kms-not-ready");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    assembly.observe_gbm_egl_frame_target_size(Size {
        width: 640,
        height: 480,
    });
    assert_eq!(
        assembly.kms_scanout_target_observation().status,
        LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch
    );

    let tick = assembly
        .run_tick_with_rendered_primary_plane_scanout_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut exporter,
        )
        .expect("runtime scanout command should fail closed before export");
    let submit = tick
        .rendered_primary_plane_scanout_submit
        .expect("active scanout submit should be reported");

    assert_eq!(
        submit.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady
    );
    assert_eq!(
        submit.scanout_target,
        LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch
    );
    assert_eq!(submit.export, None);
    assert_eq!(submit.submit, None);
    assert_eq!(submit.in_flight, false);
    assert_eq!(exporter.export_attempts(), 0);
    assert_eq!(
        tick.engine.runtime.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_direct_rendered_scanout_submit_rejects_not_ready_kms_target() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-direct-kms-not-ready");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 640,
        height: 480,
    });

    assembly.observe_gbm_egl_frame_target_size(Size {
        width: 640,
        height: 480,
    });

    let submitted = assembly.submit_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady
    );
    assert_eq!(
        submitted.scanout_target,
        LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch
    );
    assert_eq!(submitted.export, None);
    assert_eq!(submitted.submit, None);
    assert!(submitted.submission.is_none());
    assert_eq!(exporter.export_attempts(), 0);
    assert_eq!(
        submitted.runtime_scanout_state(),
        RuntimeScanoutState::Rejected
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_direct_rendered_scanout_submit_rechecks_native_kms_snapshot_before_export() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-native-kms-drift");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = FakeNativePrimaryPlaneScanoutDevice {
        selection: FakeNativeKmsSelectionDevice {
            connector_snapshot: Ok(LibdrmNativeConnectorSnapshot::new(
                false,
                Some(encoder_handle()),
                [encoder_handle()],
                Some(Size {
                    width: 1280,
                    height: 720,
                }),
            )),
            ..full_kms_selection_device()
        },
        ..full_primary_plane_scanout_device()
    };
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    assert_eq!(
        assembly.kms_scanout_target_observation().status,
        LiveKmsScanoutTargetStatus::Ready
    );

    let submitted = assembly.submit_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady
    );
    assert_eq!(
        submitted.scanout_target,
        LiveKmsScanoutTargetStatus::OutputUnavailable
    );
    assert_eq!(submitted.export, None);
    assert_eq!(submitted.submit, None);
    assert_eq!(exporter.export_attempts(), 0);
    assert!(submitted.submission.is_none());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_tick_defers_rendered_scanout_when_previous_submit_is_in_flight() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-submit-command-fail");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::unavailable();

    let tick = assembly
        .run_tick_with_rendered_primary_plane_scanout_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut exporter,
        )
        .expect("runtime scanout command should fail closed through reduced state");

    assert_eq!(
        tick.rendered_primary_plane_scanout_submit
            .expect("active scanout submit should be reported")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed
    );
    assert_eq!(tick.engine.runtime.runtime_state.scanout_rejections, 1);
    assert_eq!(
        tick.engine.runtime.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 0);

    std::fs::remove_dir_all(root).unwrap();
}
