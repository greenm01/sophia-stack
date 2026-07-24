#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
#[test]
fn live_runtime_assembly_rejects_invalid_native_frame_target_before_device_open() {
    let root = ready_drm_sysfs_fixture("runtime-native-frame-target-allocation-invalid");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    let invalid_target = assembly.observe_gbm_egl_frame_target_size(Size {
        width: 0,
        height: 720,
    });
    let allocation = assembly
        .allocate_native_gbm_egl_frame_target_with_gbm_device(&MissingRenderDevice)
        .expect("invalid startup target should produce reduced native allocation report");

    assert_eq!(
        allocation,
        LiveGbmEglFrameTargetAllocationReport {
            status: LiveGbmEglFrameTargetAllocationStatus::InvalidTarget,
            target: invalid_target,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn page_flip_callback_intake_accepts_only_matching_monotonic_callbacks() {
    let mut intake = LivePageFlipCallbackIntake::new(OutputId::from_raw(7));

    assert_eq!(
        intake.observe(LivePageFlipCallback {
            output: OutputId::from_raw(8),
            frame_serial: 41,
        }),
        LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::RejectedUnexpectedOutput,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::WaitingForOutput,
                frame_serial: None,
            },
        }
    );
    assert_eq!(intake.last_frame_serial(), None);

    assert_eq!(
        intake.observe(LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 41,
        }),
        LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::Accepted,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(41),
            },
        }
    );
    assert_eq!(intake.last_frame_serial(), Some(41));

    assert_eq!(
        intake.observe(LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 41,
        }),
        LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::RejectedStaleFrameSerial,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Rejected,
                frame_serial: Some(41),
            },
        }
    );
    assert_eq!(intake.last_frame_serial(), Some(41));
}

#[test]
fn live_runtime_assembly_observes_reduced_page_flip_callbacks() {
    let root = ready_drm_sysfs_fixture("runtime-page-flip-callback");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    assert_eq!(
        assembly.observe_page_flip_callback(LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 17,
        }),
        LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::Accepted,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(17),
            },
        }
    );

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should succeed");

    assert_eq!(
        tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(17),
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_commits_atomic_scanout_after_accepted_page_flip() {
    let root = ready_drm_sysfs_fixture("runtime-atomic-scanout-page-flip");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let mut committer = FakeAtomicScanoutCommitter::default();

    let report = assembly.commit_atomic_scanout_after_page_flip_with(
        &mut committer,
        LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 31,
        },
        &PageFlipCommitOutcome::Committed {
            frame_serial: 31,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(61),
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![sophia_protocol::SurfaceId::new(111, 1)],
            },
        },
    );

    assert_eq!(
        report,
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::Committed,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(31),
            },
        }
    );
    assert_eq!(committer.committed_count(), 1);
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(31),
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_preserves_timed_out_atomic_scanout_status() {
    let root = ready_drm_sysfs_fixture("runtime-atomic-scanout-timed-out");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let mut committer = FakeAtomicScanoutCommitter::default();

    let report = assembly.commit_atomic_scanout_after_page_flip_with(
        &mut committer,
        LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 32,
        },
        &PageFlipCommitOutcome::Rejected {
            frame_serial: 32,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(63),
                outcome: TransactionOutcome::TimedOut,
                applied_surfaces: Vec::new(),
            },
        },
    );

    assert_eq!(
        report,
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::TimedOut,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Rejected,
                frame_serial: Some(32),
            },
        }
    );
    assert_eq!(committer.committed_count(), 0);
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Rejected,
            frame_serial: Some(32),
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_rejects_stale_page_flip_before_atomic_scanout_commit() {
    let root = ready_drm_sysfs_fixture("runtime-atomic-scanout-stale-page-flip");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let mut committer = FakeAtomicScanoutCommitter::default();

    assembly.observe_page_flip_callback(LivePageFlipCallback {
        output: OutputId::from_raw(1),
        frame_serial: 41,
    });
    let report = assembly.commit_atomic_scanout_after_page_flip_with(
        &mut committer,
        LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 41,
        },
        &PageFlipCommitOutcome::Committed {
            frame_serial: 41,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(62),
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![sophia_protocol::SurfaceId::new(112, 1)],
            },
        },
    );

    assert_eq!(
        report,
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::Rejected,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Rejected,
                frame_serial: Some(41),
            },
        }
    );
    assert_eq!(committer.committed_count(), 0);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_drains_bounded_page_flip_callback_queue() {
    let root = ready_drm_sysfs_fixture("runtime-page-flip-callback-queue");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let (sender, receiver) = mpsc::sync_channel(2);
    sender
        .try_send(LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 22,
        })
        .expect("test channel should accept first callback");
    sender
        .try_send(LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 23,
        })
        .expect("test channel should accept second callback");
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 1));

    let first_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain first callback");
    assert_eq!(
        first_tick.page_flip_callbacks,
        LivePageFlipCallbackQueueReport {
            drained: 1,
            accepted: 1,
            rejected_unexpected_output: 0,
            rejected_stale_frame_serial: 0,
            last_accepted: Some(LivePageFlipCallbackReport {
                decision: LivePageFlipCallbackDecision::Accepted,
                event: LivePageFlipEvent {
                    status: LivePageFlipEventStatus::Presented,
                    frame_serial: Some(22),
                },
            }),
            disconnected: false,
            max_reached: true,
        }
    );
    assert_eq!(
        first_tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(22),
        }
    );

    drop(sender);
    let second_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain queued callback");
    assert_eq!(
        second_tick.page_flip_callbacks,
        LivePageFlipCallbackQueueReport {
            drained: 1,
            accepted: 1,
            rejected_unexpected_output: 0,
            rejected_stale_frame_serial: 0,
            last_accepted: Some(LivePageFlipCallbackReport {
                decision: LivePageFlipCallbackDecision::Accepted,
                event: LivePageFlipEvent {
                    status: LivePageFlipEventStatus::Presented,
                    frame_serial: Some(23),
                },
            }),
            disconnected: false,
            max_reached: true,
        }
    );
    assert_eq!(
        second_tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(23),
        }
    );

    let disconnected_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report disconnected callback queue");
    assert_eq!(
        disconnected_tick.page_flip_callbacks,
        LivePageFlipCallbackQueueReport {
            drained: 0,
            accepted: 0,
            rejected_unexpected_output: 0,
            rejected_stale_frame_serial: 0,
            last_accepted: None,
            disconnected: true,
            max_reached: false,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fake_page_flip_callback_source_feeds_bounded_runtime_queue() {
    let root = ready_drm_sysfs_fixture("fake-page-flip-source");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let (sender, receiver) = mpsc::sync_channel(1);
    let mut source = FakePageFlipCallbackSource::new([
        LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 31,
        },
        LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 32,
        },
    ]);

    assert_eq!(
        source.emit_ready(&sender, 4),
        LivePageFlipCallbackSourceReport {
            emitted: 1,
            queued_remaining: 1,
            backpressure: true,
            disconnected: false,
            max_reached: false,
        }
    );
    assert_eq!(source.queued_len(), 1);

    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 4));
    let first_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain first fake callback");
    assert_eq!(
        first_tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(31),
        }
    );

    assert_eq!(
        source.emit_ready(&sender, 4),
        LivePageFlipCallbackSourceReport {
            emitted: 1,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        }
    );
    let second_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain second fake callback");
    assert_eq!(
        second_tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(32),
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn dependency_policy_allows_libdrm_and_libinput_at_live_backend_seams() {
    assert!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::LibDrm,
            LiveBackendDependencyUse::Discovery,
        )
        .is_allowed()
    );
    assert!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::LibInput,
            LiveBackendDependencyUse::RuntimePolling,
        )
        .is_allowed()
    );
}

#[test]
fn dependency_policy_defers_gpu_and_shared_memory_imports() {
    assert_eq!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::Gbm,
            LiveBackendDependencyUse::RendererImport,
        ),
        LiveBackendDependencyDecision::Deferred {
            required_boundary: "live renderer import boundary",
        }
    );
    assert_eq!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::DmaBuf,
            LiveBackendDependencyUse::Discovery,
        ),
        LiveBackendDependencyDecision::Deferred {
            required_boundary: "live renderer import boundary",
        }
    );
    assert_eq!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::MitShm,
            LiveBackendDependencyUse::SharedMemoryImport,
        ),
        LiveBackendDependencyDecision::Deferred {
            required_boundary: "bounded shared-memory import boundary",
        }
    );
    assert_eq!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::Wgpu,
            LiveBackendDependencyUse::RendererImport,
        ),
        LiveBackendDependencyDecision::Deferred {
            required_boundary: "validated GBM/EGL startup, drawing, and presentation seams",
        }
    );
    assert_eq!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::Wgpu,
            LiveBackendDependencyUse::RuntimePolling,
        ),
        LiveBackendDependencyDecision::Deferred {
            required_boundary: "validated GBM/EGL startup, drawing, and presentation seams",
        }
    );
}
