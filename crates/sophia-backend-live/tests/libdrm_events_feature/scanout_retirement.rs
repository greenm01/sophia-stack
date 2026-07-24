#[test]
fn persistent_rendered_scanout_retires_only_the_replaced_displayed_buffer() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-persistent");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly")
        .with_persistent_rendered_primary_plane_scanout();
    let device = full_primary_plane_scanout_device();
    let size = Size {
        width: 1280,
        height: 720,
    };

    let first = assembly.submit_and_track_rendered_primary_plane_scanout_with(
        &device,
        &mut FakeRenderedScanoutExporter::exported(size),
    );
    assert_eq!(
        first.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    let first_presented = assembly.retire_tracked_rendered_primary_plane_scanout_after_page_flip(
        &device,
        &LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::Accepted,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(1),
            },
        },
    );
    assert_eq!(
        first_presented.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(first_presented.destroy, None);
    assert!(assembly.rendered_primary_plane_scanout_displayed());

    let second = assembly.submit_and_track_rendered_primary_plane_scanout_with(
        &device,
        &mut FakeRenderedScanoutExporter::exported(size),
    );
    assert_eq!(
        second.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    let second_presented = assembly.retire_tracked_rendered_primary_plane_scanout_after_page_flip(
        &device,
        &LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::Accepted,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(2),
            },
        },
    );
    assert_eq!(
        second_presented.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        second_presented.destroy,
        Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed)
    );
    assert!(assembly.rendered_primary_plane_scanout_displayed());
    assert!(!assembly.rendered_primary_plane_scanout_in_flight());
    assert!(!assembly.rendered_primary_plane_scanout_cleanup_pending());

    let shutdown = assembly.retire_displayed_rendered_primary_plane_scanout(&device);
    assert_eq!(
        shutdown.status,
        LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::CleanedUp
    );
    assert_eq!(
        shutdown.destroy,
        Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed)
    );
    assert!(!shutdown.cleanup_pending);
    assert!(!assembly.rendered_primary_plane_scanout_displayed());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_rejects_page_flip_replay_at_submission_baseline() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-baseline-replay");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let baseline = assembly.observe_page_flip_callback(LivePageFlipCallback {
        output: OutputId::from_raw(1),
        frame_serial: 55,
    });
    assert_eq!(baseline.decision, LivePageFlipCallbackDecision::Accepted);

    let submitted =
        assembly.submit_and_track_rendered_primary_plane_scanout_with(&device, &mut exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    let replay = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let waiting =
        assembly.retire_tracked_rendered_primary_plane_scanout_after_page_flip(&device, &replay);

    assert_eq!(
        waiting.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip
    );
    assert_eq!(waiting.runtime_scanout_state, None);
    assert_eq!(waiting.in_flight, true);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);

    let newer = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(56),
        },
    };
    let retired =
        assembly.retire_tracked_rendered_primary_plane_scanout_after_page_flip(&device, &newer);

    assert_eq!(
        retired.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        retired.runtime_scanout_state,
        Some(RuntimeScanoutState::Retired)
    );
    assert_eq!(retired.in_flight, false);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_does_not_track_failed_rendered_scanout_submit() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-tracked-fail");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::unavailable();

    let submitted =
        assembly.submit_and_track_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed
    );
    assert_eq!(
        submitted.runtime_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(submitted.in_flight, false);
    assert_eq!(submitted.in_flight_ticks, 0);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);
    assert_eq!(
        assembly.rendered_primary_plane_runtime_scanout_state(),
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 1);

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should observe rejected scanout submit state");

    assert_eq!(
        tick.runtime_scanout_states,
        vec![RuntimeScanoutState::Rejected]
    );
    assert_eq!(tick.engine.runtime.runtime_state.scanout_rejections, 1);
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 0);

    let accepted = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let retired =
        assembly.retire_tracked_rendered_primary_plane_scanout_after_page_flip(&device, &accepted);

    assert_eq!(
        retired.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::NoSubmission
    );
    assert_eq!(retired.runtime_scanout_state, None);
    assert_eq!(retired.in_flight, false);
    assert_eq!(retired.in_flight_ticks, 0);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_retains_submit_failure_cleanup_for_retry() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-submit-cleanup-retry");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let failing_device = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            destroy_framebuffer: Err(io::Error::other("test framebuffer destroy failed")),
            ..full_primary_plane_resource_device()
        },
        submit: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_primary_plane_scanout_device()
    };
    let retry_device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let submitted = assembly
        .submit_and_track_rendered_primary_plane_scanout_with(&failing_device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::PrimaryPlaneSubmitFailed
    );
    assert_eq!(
        submitted.submit,
        Some(LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicSubmitFailed)
    );
    assert_eq!(
        submitted.properties,
        Some(LibdrmNativePrimaryPlanePropertyDiscoveryStatus::Discovered)
    );
    assert_eq!(
        submitted.resources,
        Some(LibdrmNativePrimaryPlaneResourceCreateStatus::Created)
    );
    assert_eq!(
        submitted.request,
        Some(LibdrmNativeAtomicRequestBuildStatus::Built)
    );
    assert_eq!(
        submitted.commit_submit,
        Some(LibdrmNativeAtomicCommitSubmitStatus::Rejected)
    );
    assert_eq!(
        submitted.reduced_log_line(),
        "sophia_runtime_rendered_scanout_submit schema=6 status=PrimaryPlaneSubmitFailed scanout_target=Ready output_size=1280x720 target=Ready target_size=1280x720 export=Exported scanout_buffer=Ready buffer_format=Xrgb8888 buffer_modifier=Implicit buffer_planes=Single properties=Discovered format_table=Present resources=Created framebuffer=CreatedWithAddFb2 request=Built submit=AtomicSubmitFailed request_scope=PageFlip commit_page_flip_event=true commit_nonblocking=true commit_allow_modeset=false commit_test_only=false commit_submit=Rejected runtime_scanout_state=Rejected in_flight=false in_flight_ticks=0 cleanup_pending=true"
    );
    assert_eq!(
        submitted.scanout_buffer,
        Some(sophia_renderer_live::LiveRendererScanoutBufferStatus::Ready)
    );
    assert_eq!(
        submitted.runtime_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(submitted.in_flight, false);
    assert!(assembly.rendered_primary_plane_scanout_cleanup_pending());

    let mut blocked_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let blocked = assembly
        .submit_and_track_rendered_primary_plane_scanout_with(&retry_device, &mut blocked_exporter);
    assert_eq!(
        blocked.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::CleanupPending
    );
    assert_eq!(
        blocked.runtime_scanout_state,
        Some(RuntimeScanoutState::Deferred)
    );
    assert_eq!(
        assembly.rendered_primary_plane_runtime_scanout_state(),
        Some(RuntimeScanoutState::Deferred)
    );
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 2);

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should observe rejected then deferred scanout states");
    assert_eq!(
        tick.runtime_scanout_states,
        vec![RuntimeScanoutState::Rejected, RuntimeScanoutState::Deferred]
    );
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 0);

    let cleanup = assembly.retry_tracked_rendered_primary_plane_scanout_cleanup(&retry_device);
    assert_eq!(
        cleanup.status,
        LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::CleanedUp
    );
    assert_eq!(cleanup.cleanup_pending, false);
    assert!(!assembly.rendered_primary_plane_scanout_cleanup_pending());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_retains_failed_rendered_scanout_cleanup_for_retry() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-cleanup-retry");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let failing_device = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            destroy_framebuffer: Err(io::Error::other("test framebuffer destroy failed")),
            ..full_primary_plane_resource_device()
        },
        ..full_primary_plane_scanout_device()
    };
    let retry_device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let submitted = assembly
        .submit_and_track_rendered_primary_plane_scanout_with(&failing_device, &mut exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(submitted.in_flight, true);
    assert!(!assembly.rendered_primary_plane_scanout_cleanup_pending());

    let accepted = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let retired = assembly
        .retire_tracked_rendered_primary_plane_scanout_after_page_flip(&failing_device, &accepted);

    assert_eq!(
        retired.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::ResourceRetireFailed
    );
    assert_eq!(
        retired.runtime_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(retired.in_flight, false);
    assert_eq!(retired.cleanup_pending, true);
    assert_eq!(
        retired.reduced_log_line(),
        "sophia_runtime_rendered_scanout_retire schema=1 status=ResourceRetireFailed destroy=FramebufferDestroyFailed runtime_scanout_state=Rejected in_flight=false in_flight_ticks=0 cleanup_pending=true"
    );
    assert!(assembly.rendered_primary_plane_scanout_cleanup_pending());
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 1);

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should observe cleanup failure as rejected scanout state");
    assert_eq!(
        tick.runtime_scanout_states,
        vec![RuntimeScanoutState::Rejected]
    );
    assert_eq!(tick.rendered_primary_plane_scanout_cleanup_pending, true);

    let mut blocked_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let blocked = assembly
        .submit_and_track_rendered_primary_plane_scanout_with(&retry_device, &mut blocked_exporter);
    assert_eq!(
        blocked.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::CleanupPending
    );
    assert_eq!(
        blocked.runtime_scanout_state,
        Some(RuntimeScanoutState::Deferred)
    );
    assert_eq!(blocked.in_flight, false);
    assert!(assembly.rendered_primary_plane_scanout_cleanup_pending());

    let cleanup = assembly.retry_tracked_rendered_primary_plane_scanout_cleanup(&retry_device);

    assert_eq!(
        cleanup.status,
        LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::CleanedUp
    );
    assert_eq!(
        cleanup.destroy,
        Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed)
    );
    assert_eq!(
        cleanup.reduced_log_line(),
        "sophia_runtime_rendered_scanout_cleanup schema=1 status=CleanedUp destroy=Destroyed cleanup_pending=false"
    );
    assert_eq!(cleanup.cleanup_pending, false);
    assert!(!assembly.rendered_primary_plane_scanout_cleanup_pending());

    let no_cleanup = assembly.retry_tracked_rendered_primary_plane_scanout_cleanup(&retry_device);
    assert_eq!(
        no_cleanup.status,
        LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::NoCleanupPending
    );
    assert_eq!(no_cleanup.destroy, None);
    assert_eq!(
        no_cleanup.reduced_log_line(),
        "sophia_runtime_rendered_scanout_cleanup schema=1 status=NoCleanupPending destroy=none cleanup_pending=false"
    );
    assert_eq!(no_cleanup.cleanup_pending, false);

    let clean_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should observe cleared cleanup state");
    assert_eq!(
        clean_tick.rendered_primary_plane_scanout_cleanup_pending,
        false
    );

    std::fs::remove_dir_all(root).unwrap();
}

