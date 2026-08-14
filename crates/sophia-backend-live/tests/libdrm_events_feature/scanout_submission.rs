#[test]
fn native_libdrm_primary_plane_scanout_submit_chains_renderer_descriptor_to_atomic_commit() {
    let device = full_primary_plane_scanout_device();
    let result = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );

    assert_eq!(
        result.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        result.selection,
        LibdrmNativePrimaryPlaneSelectionStatus::Selected
    );
    assert_eq!(
        result.properties,
        Some(LibdrmNativePrimaryPlanePropertyDiscoveryStatus::Discovered)
    );
    assert_eq!(
        result.resources,
        Some(LibdrmNativePrimaryPlaneResourceCreateStatus::Created)
    );
    assert_eq!(
        result.request,
        Some(LibdrmNativeAtomicRequestBuildStatus::Built)
    );
    assert_eq!(
        result.submit,
        Some(LibdrmNativeAtomicCommitSubmitStatus::Submitted)
    );
    assert_eq!(
        result.commit_flags,
        Some(LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: true,
            test_only: false,
        })
    );
    assert_eq!(
        result.request_scope,
        Some(LibdrmNativeAtomicCommitRequestScope::Modeset)
    );

    let retired = result
        .submission
        .expect("submitted scanout should retain resource ownership until page flip")
        .retire(&device);
    assert_eq!(
        retired.status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_primary_plane_scanout_submit_page_flip_policy_disallows_modeset() {
    let device = full_primary_plane_scanout_device();
    assert_eq!(
        LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip().expected_request_scope(),
        LibdrmNativeAtomicCommitRequestScope::PageFlip
    );
    assert_eq!(
        LibdrmNativePrimaryPlaneScanoutSubmitPolicy::modeset().expected_request_scope(),
        LibdrmNativeAtomicCommitRequestScope::Modeset
    );

    let selection = select_native_primary_plane_target(&device);
    let result =
        submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
            &device,
            selection,
            &[],
            scanout_descriptor(Size {
                width: 1280,
                height: 720,
            }),
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip(),
        );

    assert_eq!(
        result.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        result.commit_flags,
        Some(LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: false,
            test_only: false,
        })
    );
    assert_eq!(
        result.request_scope,
        Some(LibdrmNativeAtomicCommitRequestScope::PageFlip)
    );
    assert_eq!(
        result
            .submission
            .expect("page-flip submit should retain resources")
            .retire(&device)
            .status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );

    let mode_unavailable = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            mode_blob: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            destroy_mode_blob: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            ..full_primary_plane_resource_device()
        },
        ..full_primary_plane_scanout_device()
    };
    let selection = select_native_primary_plane_target(&mode_unavailable);
    let result =
        submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
            &mode_unavailable,
            selection,
            &[],
            scanout_descriptor(Size {
                width: 1280,
                height: 720,
            }),
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip(),
        );

    assert_eq!(
        result.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        result
            .submission
            .expect("page-flip submit should not retain a mode blob")
            .retire(&mode_unavailable)
            .status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_primary_plane_scanout_submit_retains_cleanup_after_submit_failure() {
    let device = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            destroy_framebuffer: Err(io::Error::other("test framebuffer destroy failed")),
            ..full_primary_plane_resource_device()
        },
        submit: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_primary_plane_scanout_device()
    };
    let selection = select_native_primary_plane_target(&device);
    let result =
        submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
            &device,
            selection,
            &[],
            scanout_descriptor(Size {
                width: 1280,
                height: 720,
            }),
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip(),
        );

    assert_eq!(
        result.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicSubmitFailed
    );
    assert_eq!(
        result.submit,
        Some(LibdrmNativeAtomicCommitSubmitStatus::Rejected)
    );
    assert!(result.submission.is_none());
    let cleanup = result
        .cleanup
        .expect("submit failure must retain failed cleanup");
    assert_eq!(
        cleanup.retry(&full_primary_plane_scanout_device()).status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_primary_plane_scanout_submit_retains_resource_creation_cleanup() {
    let device = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            framebuffer: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            destroy_mode_blob: Err(io::Error::other("test mode blob destroy failed")),
            ..full_primary_plane_resource_device()
        },
        ..full_primary_plane_scanout_device()
    };
    let selection = select_native_primary_plane_target(&device);
    let result =
        submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
            &device,
            selection,
            &[],
            scanout_descriptor(Size {
                width: 1280,
                height: 720,
            }),
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::modeset(),
        );

    assert_eq!(
        result.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::ResourceCreationUnavailable
    );
    assert_eq!(
        result.resources,
        Some(LibdrmNativePrimaryPlaneResourceCreateStatus::FramebufferCreateFailed)
    );
    assert!(result.request.is_none());
    assert!(result.submit.is_none());
    assert!(result.submission.is_none());
    let cleanup = result
        .cleanup
        .expect("resource creation failure must retain failed mode blob cleanup");
    assert_eq!(
        cleanup.retry(&full_primary_plane_scanout_device()).status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_primary_plane_scanout_submit_uses_supplied_selection_snapshot() {
    let device = full_primary_plane_scanout_device();
    let result = submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor(
        &device,
        LibdrmNativePrimaryPlaneSelectionResult {
            status: LibdrmNativePrimaryPlaneSelectionStatus::NoConnectedConnector,
            selection: None,
        },
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );

    assert_eq!(
        result.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::KmsTargetUnavailable
    );
    assert_eq!(
        result.selection,
        LibdrmNativePrimaryPlaneSelectionStatus::NoConnectedConnector
    );
    assert!(result.submit.is_none());
    assert!(result.submission.is_none());

    let selected = select_native_primary_plane_target(&device)
        .selection
        .expect("complete KMS path should produce a selected payload");
    let forged_not_selected =
        submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor(
            &device,
            LibdrmNativePrimaryPlaneSelectionResult {
                status: LibdrmNativePrimaryPlaneSelectionStatus::NoCompatiblePrimaryPlane,
                selection: Some(selected),
            },
            scanout_descriptor(Size {
                width: 1280,
                height: 720,
            }),
        );

    assert_eq!(
        forged_not_selected.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::KmsTargetUnavailable
    );
    assert_eq!(
        forged_not_selected.selection,
        LibdrmNativePrimaryPlaneSelectionStatus::NoCompatiblePrimaryPlane
    );
    assert!(forged_not_selected.properties.is_none());
    assert!(forged_not_selected.resources.is_none());
    assert!(forged_not_selected.request.is_none());
    assert!(forged_not_selected.submit.is_none());
    assert!(forged_not_selected.submission.is_none());
}

#[test]
fn native_libdrm_primary_plane_scanout_submit_fails_closed_for_bad_descriptor() {
    let device = full_primary_plane_scanout_device();
    let descriptor = sophia_renderer_live::LiveRendererScanoutBufferDescriptor::new(
        Size {
            width: 1280,
            height: 720,
        },
        0,
        LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        17,
    );
    let result = submit_native_primary_plane_scanout_from_renderer_descriptor(&device, descriptor);

    assert_eq!(
        result.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::ScanoutBufferUnavailable
    );
    assert_eq!(
        result.selection,
        LibdrmNativePrimaryPlaneSelectionStatus::Selected
    );
    assert!(result.properties.is_none());
    assert!(result.resources.is_none());
    assert!(result.request.is_none());
    assert!(result.submit.is_none());
    assert!(result.submission.is_none());

    let forged_undersized_pitch = sophia_renderer_live::LiveRendererScanoutBufferDescriptor {
        status: sophia_renderer_live::LiveRendererScanoutBufferStatus::Ready,
        pitch: 1280 * 4 - 1,
        ..scanout_descriptor(Size {
            width: 1280,
            height: 720,
        })
    };
    let undersized_pitch = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        forged_undersized_pitch,
    );

    assert_eq!(
        undersized_pitch.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::ScanoutBufferUnavailable
    );
    assert_eq!(
        undersized_pitch.scanout_buffer,
        sophia_renderer_live::LiveRendererScanoutBufferStatus::Invalid
    );
    assert!(undersized_pitch.properties.is_none());
    assert!(undersized_pitch.resources.is_none());
    assert!(undersized_pitch.request.is_none());
    assert!(undersized_pitch.submit.is_none());
    assert!(undersized_pitch.submission.is_none());
}

#[test]
fn native_libdrm_primary_plane_scanout_retires_only_after_accepted_page_flip() {
    let device = full_primary_plane_scanout_device();
    let result = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    let submission = result
        .submission
        .expect("submitted scanout should retain resource ownership");

    let retired = retire_native_primary_plane_scanout_after_page_flip(
        &device,
        submission,
        &LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::Accepted,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(42),
            },
        },
    );

    assert_eq!(
        retired.status,
        LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        retired.destroy,
        Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed)
    );
    assert!(retired.submission.is_none());
}

#[test]
fn native_libdrm_primary_plane_scanout_keeps_submission_until_page_flip_is_accepted() {
    let device = full_primary_plane_scanout_device();
    let result = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    let submission = result
        .submission
        .expect("submitted scanout should retain resource ownership");

    let waiting = retire_native_primary_plane_scanout_after_page_flip(
        &device,
        submission,
        &LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::RejectedStaleFrameSerial,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Rejected,
                frame_serial: Some(41),
            },
        },
    );

    assert_eq!(
        waiting.status,
        LibdrmNativePrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip
    );
    assert!(waiting.destroy.is_none());
    let submission = waiting
        .submission
        .expect("rejected page flip must return the in-flight resource owner");
    assert_eq!(
        submission.retire(&device).status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn live_runtime_assembly_submits_rendered_primary_plane_scanout_through_reduced_seam() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-submit");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let mut submitted = assembly.submit_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        submitted.runtime_scanout_state(),
        RuntimeScanoutState::Submitted
    );
    assert_eq!(
        submitted.export,
        Some(LiveRendererScanoutBufferExportStatus::Exported)
    );
    assert_eq!(
        submitted.scanout_buffer,
        Some(sophia_renderer_live::LiveRendererScanoutBufferStatus::Ready)
    );
    assert_eq!(
        submitted.submit,
        Some(LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip)
    );
    assert_eq!(
        submitted.request_scope,
        Some(LibdrmNativeAtomicCommitRequestScope::PageFlip)
    );
    assert_eq!(
        submitted.commit_flags,
        Some(LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: false,
            test_only: false,
        })
    );
    let submission = submitted
        .submission
        .take()
        .expect("rendered scanout submit should retain both owners");
    let callback = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let retired =
        retire_rendered_primary_plane_scanout_after_page_flip(&device, submission, &callback);

    assert_eq!(
        retired.status,
        LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        retired.runtime_scanout_state(),
        Some(RuntimeScanoutState::Retired)
    );
    assert_eq!(
        retired.destroy,
        Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed)
    );
    assert!(retired.submission.is_none());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_reports_invalid_rendered_scanout_buffer_status() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-invalid-buffer");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter {
        status: LiveRendererScanoutBufferExportStatus::Exported,
        descriptor: Some(sophia_renderer_live::LiveRendererScanoutBufferDescriptor {
            status: sophia_renderer_live::LiveRendererScanoutBufferStatus::Ready,
            size: Size {
                width: -1,
                height: 720,
            },
            ..scanout_descriptor(Size {
                width: 1280,
                height: 720,
            })
        }),
        owner: Some(FakeRenderedScanoutOwner { raw: 9 }),
        export_attempts: 0,
    };

    let submitted = assembly.submit_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::PrimaryPlaneSubmitFailed
    );
    assert_eq!(
        submitted.scanout_buffer,
        Some(sophia_renderer_live::LiveRendererScanoutBufferStatus::Invalid)
    );
    assert_eq!(
        submitted.submit,
        Some(LibdrmNativePrimaryPlaneScanoutSubmitStatus::ScanoutBufferUnavailable)
    );
    assert!(submitted.submission.is_none());
    assert!(submitted.cleanup.is_none());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_drops_resources_from_non_exported_rendered_scanout() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-non-exported-buffer");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter {
        status: LiveRendererScanoutBufferExportStatus::Unavailable,
        descriptor: Some(scanout_descriptor(Size {
            width: 1280,
            height: 720,
        })),
        owner: Some(FakeRenderedScanoutOwner { raw: 11 }),
        export_attempts: 0,
    };

    let submitted = assembly.submit_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed
    );
    assert_eq!(
        submitted.export,
        Some(LiveRendererScanoutBufferExportStatus::Unavailable)
    );
    assert_eq!(submitted.scanout_buffer, None);
    assert_eq!(submitted.submit, None);
    assert!(submitted.submission.is_none());
    assert!(submitted.cleanup.is_none());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_defers_a_pending_renderer_worker_without_native_submit() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-worker-pending");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter {
        status: LiveRendererScanoutBufferExportStatus::Pending,
        descriptor: None,
        owner: None,
        export_attempts: 0,
    };

    let submitted = assembly.submit_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportPending
    );
    assert_eq!(
        submitted.export,
        Some(LiveRendererScanoutBufferExportStatus::Pending)
    );
    assert_eq!(submitted.runtime_scanout_state(), RuntimeScanoutState::Deferred);
    assert_eq!(exporter.export_attempts(), 1);
    assert!(submitted.submission.is_none());
    assert!(submitted.cleanup.is_none());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_tracks_rendered_scanout_until_accepted_page_flip() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-tracked-retire");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(2),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::Idle,
            in_flight: false,
            in_flight_ticks: 0,
            threshold_ticks: 2,
        }
    );

    let submitted =
        assembly.submit_and_track_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        submitted.runtime_scanout_state,
        Some(RuntimeScanoutState::Submitted)
    );
    assert_eq!(
        submitted.request_scope,
        Some(LibdrmNativeAtomicCommitRequestScope::PageFlip)
    );
    assert!(submitted.in_flight);
    assert_eq!(submitted.in_flight_ticks, 0);
    assert!(assembly.rendered_primary_plane_scanout_in_flight());
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight_ticks(), 0);
    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(2),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::WaitingForPageFlip,
            in_flight: true,
            in_flight_ticks: 0,
            threshold_ticks: 2,
        }
    );
    assert_eq!(
        assembly.rendered_primary_plane_runtime_scanout_state(),
        Some(RuntimeScanoutState::Submitted)
    );

    let blocked =
        assembly.submit_and_track_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        blocked.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::AlreadyInFlight
    );
    assert_eq!(
        blocked.runtime_scanout_state,
        Some(RuntimeScanoutState::Deferred)
    );
    assert!(blocked.in_flight);
    assert_eq!(blocked.in_flight_ticks, 0);
    assert!(assembly.rendered_primary_plane_scanout_in_flight());
    assert_eq!(
        assembly.rendered_primary_plane_runtime_scanout_state(),
        Some(RuntimeScanoutState::Deferred)
    );
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 1);

    let aged_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should age in-flight scanout ownership");
    assert_eq!(
        aged_tick.runtime_scanout_states,
        vec![RuntimeScanoutState::Deferred]
    );
    assert_eq!(aged_tick.rendered_primary_plane_scanout_in_flight_ticks, 1);
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 0);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight_ticks(), 1);
    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(2),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::WaitingForPageFlip,
            in_flight: true,
            in_flight_ticks: 1,
            threshold_ticks: 2,
        }
    );

    let stalled_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should classify old in-flight scanout ownership");
    assert_eq!(
        stalled_tick.rendered_primary_plane_scanout_in_flight_ticks,
        2
    );
    assert_eq!(
        stalled_tick.rendered_primary_plane_scanout_backpressure,
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::StalledWaitingForPageFlip,
            in_flight: true,
            in_flight_ticks: 2,
            threshold_ticks: LIVE_RENDERED_PRIMARY_PLANE_SCANOUT_STALL_THRESHOLD_TICKS,
        }
    );
    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(2),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::StalledWaitingForPageFlip,
            in_flight: true,
            in_flight_ticks: 2,
            threshold_ticks: 2,
        }
    );
    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(0),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::WaitingForPageFlip,
            in_flight: true,
            in_flight_ticks: 2,
            threshold_ticks: 0,
        }
    );

    let stale = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::RejectedStaleFrameSerial,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Rejected,
            frame_serial: Some(54),
        },
    };
    let waiting =
        assembly.retire_tracked_rendered_primary_plane_scanout_after_page_flip(&device, &stale);

    assert_eq!(
        waiting.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip
    );
    assert_eq!(waiting.runtime_scanout_state, None);
    assert!(waiting.in_flight);
    assert_eq!(waiting.in_flight_ticks, 2);
    assert_eq!(
        waiting.reduced_log_line(),
        "sophia_runtime_rendered_scanout_retire schema=1 status=WaitingForAcceptedPageFlip destroy=none runtime_scanout_state=none in_flight=true in_flight_ticks=2 cleanup_pending=false"
    );
    assert!(assembly.rendered_primary_plane_scanout_in_flight());

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
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        retired.runtime_scanout_state,
        Some(RuntimeScanoutState::Retired)
    );
    assert_eq!(
        retired.reduced_log_line(),
        "sophia_runtime_rendered_scanout_retire schema=1 status=RetiredAfterPageFlip destroy=Destroyed runtime_scanout_state=Retired in_flight=false in_flight_ticks=0 cleanup_pending=false"
    );
    assert!(!retired.in_flight);
    assert_eq!(retired.in_flight_ticks, 0);
    assert!(!assembly.rendered_primary_plane_scanout_in_flight());
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight_ticks(), 0);
    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(2),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::Idle,
            in_flight: false,
            in_flight_ticks: 0,
            threshold_ticks: 2,
        }
    );
    assert_eq!(
        assembly.rendered_primary_plane_runtime_scanout_state(),
        Some(RuntimeScanoutState::Retired)
    );
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 1);

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should observe retired scanout state");

    assert_eq!(
        tick.runtime_scanout_states,
        vec![RuntimeScanoutState::Retired]
    );
    assert_eq!(tick.rendered_primary_plane_scanout_in_flight_ticks, 0);
    assert_eq!(tick.engine.runtime.runtime_state.scanout_retirements, 1);
    assert_eq!(
        tick.engine.runtime.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Submitted)
    );
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 0);

    std::fs::remove_dir_all(root).unwrap();
}
