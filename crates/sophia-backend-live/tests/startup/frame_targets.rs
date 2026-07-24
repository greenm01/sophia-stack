#[test]
fn live_runtime_assembly_blocks_page_flip_readiness_for_invalid_scanout_target() {
    let root = ready_drm_sysfs_fixture("runtime-kms-scanout-target-invalid");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    assembly.observe_gbm_egl_frame_target_size(Size {
        width: 0,
        height: 720,
    });

    assert_eq!(
        assembly.kms_scanout_target_observation(),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::InvalidFrameTarget,
            size: Some(Size {
                width: 0,
                height: 720,
            }),
        }
    );
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::InvalidFrameTarget,
            frame_serial: None,
        }
    );

    assembly
        .retire_gbm_egl_frame_target()
        .expect("invalid target should still retire through reduced lifecycle");
    assert_eq!(
        assembly.kms_scanout_target_observation(),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::FrameTargetUnavailable,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::FrameTargetUnavailable,
            frame_serial: None,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_blocks_page_flip_readiness_for_frame_target_size_mismatch() {
    let root = ready_drm_sysfs_fixture("runtime-kms-scanout-target-size-mismatch");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    assembly.observe_gbm_egl_frame_target_size(Size {
        width: 1920,
        height: 1080,
    });

    assert_eq!(
        assembly.kms_scanout_target_observation(),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch,
            size: Some(Size {
                width: 1920,
                height: 1080,
            }),
        }
    );
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::FrameTargetSizeMismatch,
            frame_serial: None,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_blocks_page_flip_until_frame_target_matches_output_resize() {
    let root = ready_drm_sysfs_fixture("runtime-output-size-mismatch");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let resized_output = Size {
        width: 1920,
        height: 1080,
    };

    assembly.observe_output_size(resized_output);

    assert_eq!(assembly.output_size_observation(), Some(resized_output));
    assert_eq!(
        assembly.kms_scanout_target_observation(),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::FrameTargetSizeMismatch,
            frame_serial: None,
        }
    );
    let mismatch_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report output resize mismatch");
    assert_eq!(mismatch_tick.output_size, Some(resized_output));
    assert_eq!(
        mismatch_tick.kms_scanout_target.status,
        LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch
    );

    let frame_target = assembly.observe_gbm_egl_frame_target_size(resized_output);

    assert_eq!(
        frame_target,
        LiveGbmEglFrameTargetRecord {
            status: LiveGbmEglFrameTargetStatus::Ready,
            size: resized_output,
        }
    );
    assert_eq!(
        assembly.kms_scanout_target_observation(),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::Ready,
            size: Some(resized_output),
        }
    );
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Ready,
            frame_serial: None,
        }
    );
    let ready_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report matched output and frame target");
    assert_eq!(ready_tick.output_size, Some(resized_output));
    assert_eq!(ready_tick.gbm_egl_frame_target, Some(frame_target));
    assert_eq!(
        ready_tick.kms_scanout_target,
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::Ready,
            size: Some(resized_output),
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_clears_stale_frame_target_allocation_on_output_resize() {
    let root = ready_drm_sysfs_fixture("runtime-output-resize-clears-allocation");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let mut allocator =
        FakeGbmEglFrameTargetAllocator::new(LiveGbmEglFrameTargetAllocationStatus::Ready);
    let original_size = assembly
        .output_size_observation()
        .expect("ready output size");
    let allocation = assembly
        .allocate_gbm_egl_frame_target(&mut allocator)
        .expect("ready target should allocate");

    assembly.observe_output_size(original_size);
    assert_eq!(
        assembly.gbm_egl_frame_target_allocation_observation(),
        Some(allocation)
    );

    assembly.observe_output_size(Size {
        width: 1920,
        height: 1080,
    });

    assert_eq!(assembly.gbm_egl_frame_target_allocation_observation(), None);
    assert_eq!(
        assembly.kms_scanout_target_observation().status,
        LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_keeps_degraded_scanout_target_reduced() {
    let root = ready_drm_sysfs_fixture("runtime-kms-scanout-target-degraded");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    assembly.observe_presentation_report(LiveRendererPresentationReport {
        status: LiveRendererPresentationStatus::Degraded,
    });

    assert_eq!(
        assembly.kms_scanout_target_observation(),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::Degraded,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Degraded,
            frame_serial: None,
        }
    );

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report reduced degraded target");
    assert_eq!(
        tick.kms_scanout_target,
        assembly.kms_scanout_target_observation()
    );
    assert_eq!(tick.page_flip, assembly.page_flip_observation());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_updates_reduced_gbm_egl_frame_target_size() {
    let root = ready_drm_sysfs_fixture("runtime-frame-target-size");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    let resized = assembly.observe_gbm_egl_frame_target_size(Size {
        width: 2560,
        height: 1440,
    });
    assert_eq!(
        resized,
        LiveGbmEglFrameTargetRecord {
            status: LiveGbmEglFrameTargetStatus::Ready,
            size: Size {
                width: 2560,
                height: 1440,
            },
        }
    );

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report resized target");
    assert_eq!(tick.gbm_egl_frame_target, Some(resized));

    let invalid = assembly.observe_gbm_egl_frame_target_size(Size {
        width: 0,
        height: 1440,
    });
    assert_eq!(
        invalid,
        LiveGbmEglFrameTargetRecord {
            status: LiveGbmEglFrameTargetStatus::InvalidSize,
            size: Size {
                width: 0,
                height: 1440,
            },
        }
    );

    let invalid_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report invalid reduced target");
    assert_eq!(invalid_tick.gbm_egl_frame_target, Some(invalid));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_reports_reduced_gbm_egl_frame_target_lifecycle() {
    let root = ready_drm_sysfs_fixture("runtime-frame-target-lifecycle");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let startup_target = assembly
        .gbm_egl_frame_target_observation()
        .expect("startup should seed reduced target");

    assert_eq!(
        assembly.gbm_egl_frame_target_lifecycle_observation(),
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Created,
            target: startup_target,
        })
    );

    let retained = assembly.observe_gbm_egl_frame_target_size(startup_target.size);
    assert_eq!(retained, startup_target);
    assert_eq!(
        assembly.gbm_egl_frame_target_lifecycle_observation(),
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Retained,
            target: startup_target,
        })
    );

    let resized = assembly.observe_gbm_egl_frame_target_size(Size {
        width: 1920,
        height: 1080,
    });
    assert_eq!(
        assembly.gbm_egl_frame_target_lifecycle_observation(),
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Resized,
            target: resized,
        })
    );

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report reduced lifecycle");
    assert_eq!(
        tick.gbm_egl_frame_target_lifecycle,
        assembly.gbm_egl_frame_target_lifecycle_observation()
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_reports_reduced_gbm_egl_frame_target_allocation() {
    let root = ready_drm_sysfs_fixture("runtime-frame-target-allocation");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let mut allocator =
        FakeGbmEglFrameTargetAllocator::new(LiveGbmEglFrameTargetAllocationStatus::Ready);

    let allocation = assembly
        .allocate_gbm_egl_frame_target(&mut allocator)
        .expect("ready startup target should allocate through reduced seam");
    assert_eq!(
        allocation,
        LiveGbmEglFrameTargetAllocationReport {
            status: LiveGbmEglFrameTargetAllocationStatus::Ready,
            target: LiveGbmEglFrameTargetRecord {
                status: LiveGbmEglFrameTargetStatus::Ready,
                size: Size {
                    width: 1280,
                    height: 720,
                },
            },
        }
    );
    assert_eq!(
        assembly.gbm_egl_frame_target_allocation_observation(),
        Some(allocation)
    );

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report reduced allocation");
    assert_eq!(tick.gbm_egl_frame_target_allocation, Some(allocation));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_reports_invalid_and_clears_stale_frame_target_allocation() {
    let root = ready_drm_sysfs_fixture("runtime-frame-target-allocation-invalid");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let mut allocator =
        FakeGbmEglFrameTargetAllocator::new(LiveGbmEglFrameTargetAllocationStatus::Ready);

    let ready = assembly
        .allocate_gbm_egl_frame_target(&mut allocator)
        .expect("ready startup target should allocate through reduced seam");
    assert_eq!(ready.status, LiveGbmEglFrameTargetAllocationStatus::Ready);

    let invalid_target = assembly.observe_gbm_egl_frame_target_size(Size {
        width: 0,
        height: 720,
    });
    assert_eq!(
        invalid_target.status,
        LiveGbmEglFrameTargetStatus::InvalidSize
    );
    assert_eq!(assembly.gbm_egl_frame_target_allocation_observation(), None);

    let invalid_allocation = assembly
        .allocate_gbm_egl_frame_target(&mut allocator)
        .expect("invalid target still reports a reduced allocation result");
    assert_eq!(
        invalid_allocation,
        LiveGbmEglFrameTargetAllocationReport {
            status: LiveGbmEglFrameTargetAllocationStatus::InvalidTarget,
            target: invalid_target,
        }
    );

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report reduced invalid allocation");
    assert_eq!(
        tick.gbm_egl_frame_target_allocation,
        Some(invalid_allocation)
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_retains_or_retires_reduced_frame_target_allocation() {
    let root = ready_drm_sysfs_fixture("runtime-frame-target-lifecycle-allocation");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let mut allocator =
        FakeGbmEglFrameTargetAllocator::new(LiveGbmEglFrameTargetAllocationStatus::Ready);

    let allocation = assembly
        .allocate_gbm_egl_frame_target(&mut allocator)
        .expect("ready startup target should allocate through reduced seam");
    let target = allocation.target;

    assembly.observe_gbm_egl_frame_target_size(target.size);
    assert_eq!(
        assembly.gbm_egl_frame_target_lifecycle_observation(),
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Retained,
            target,
        })
    );
    assert_eq!(
        assembly.gbm_egl_frame_target_allocation_observation(),
        Some(allocation)
    );

    let retired = assembly
        .retire_gbm_egl_frame_target()
        .expect("ready target should retire");
    assert_eq!(
        retired,
        LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Retired,
            target,
        }
    );
    assert_eq!(assembly.gbm_egl_frame_target_observation(), None);
    assert_eq!(assembly.gbm_egl_frame_target_allocation_observation(), None);

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report reduced retire lifecycle");
    assert_eq!(tick.gbm_egl_frame_target, None);
    assert_eq!(
        tick.gbm_egl_frame_target_lifecycle,
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Retired,
            target,
        })
    );
    assert_eq!(tick.gbm_egl_frame_target_allocation, None);

    fs::remove_dir_all(root).unwrap();
}

#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
struct MissingRenderDevice;

#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
impl sophia_backend_live::RenderDeviceDiscoveryBackend for MissingRenderDevice {
    type Device = std::fs::File;

    fn open_render_device(&self) -> std::io::Result<Self::Device> {
        Err(std::io::Error::from_raw_os_error(19))
    }
}

#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
#[test]
fn live_runtime_assembly_reports_native_frame_target_allocation_without_handles() {
    let root = ready_drm_sysfs_fixture("runtime-native-frame-target-allocation");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    let allocation = assembly
        .allocate_native_gbm_egl_frame_target_with_gbm_device(&MissingRenderDevice)
        .expect("ready startup target should produce reduced native allocation report");
    assert_eq!(
        allocation.status,
        LiveGbmEglFrameTargetAllocationStatus::Unavailable
    );
    assert_eq!(
        assembly.gbm_egl_frame_target_allocation_observation(),
        Some(allocation)
    );

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report reduced native allocation");
    assert_eq!(tick.gbm_egl_frame_target_allocation, Some(allocation));

    fs::remove_dir_all(root).unwrap();
}
