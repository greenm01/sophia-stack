#[path = "atomic_scanout_evidence.rs"]
mod atomic_scanout_evidence;

#[test]
fn native_libdrm_primary_plane_resources_validate_size_and_lifetime() {
    let oversized_size = Size {
        width: 65_536,
        height: 720,
    };
    let oversized_selected =
        select_native_primary_plane_target(&kms_selection_device_with_mode_size(oversized_size))
            .selection
            .expect("oversized fake KMS target should still select before resource validation");
    let oversized_buffer = FakeDrmBuffer::xrgb8888(oversized_size);

    let oversized_modeset = create_native_primary_plane_resources(
        &FakeNativePrimaryPlaneResourceDevice {
            imported_buffers: std::cell::Cell::new(0),
            closed_buffers: std::cell::Cell::new(0),
            destroyed_framebuffers: std::cell::Cell::new(0),
            mode_blob: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            framebuffer: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            destroy_framebuffer: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            destroy_mode_blob: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        },
        oversized_selected,
        &oversized_buffer,
    );
    assert_eq!(
        oversized_modeset.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidSelectionSize
    );
    assert!(oversized_modeset.resources.is_none());
    assert!(oversized_modeset.cleanup.is_none());

    let oversized_page_flip = create_native_primary_plane_page_flip_resources(
        &FakeNativePrimaryPlaneResourceDevice {
            imported_buffers: std::cell::Cell::new(0),
            closed_buffers: std::cell::Cell::new(0),
            destroyed_framebuffers: std::cell::Cell::new(0),
            mode_blob: Ok(15),
            framebuffer: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            destroy_framebuffer: Ok(()),
            destroy_mode_blob: Ok(()),
        },
        oversized_selected,
        &oversized_buffer,
    );
    assert_eq!(
        oversized_page_flip.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidSelectionSize
    );
    assert!(oversized_page_flip.resources.is_none());
    assert!(oversized_page_flip.cleanup.is_none());

    let selected = select_native_primary_plane_target(&full_kms_selection_device())
        .selection
        .expect("complete KMS path should select a target");
    let mismatched = create_native_primary_plane_resources(
        &full_primary_plane_resource_device(),
        selected,
        &scanout_buffer(Size {
            width: 1920,
            height: 1080,
        }),
    );
    assert_eq!(
        mismatched.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::BufferSizeMismatch
    );
    assert!(mismatched.resources.is_none());

    let invalid_pitch = create_native_primary_plane_resources(
        &FakeNativePrimaryPlaneResourceDevice {
            imported_buffers: std::cell::Cell::new(0),
            closed_buffers: std::cell::Cell::new(0),
            destroyed_framebuffers: std::cell::Cell::new(0),
            mode_blob: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            framebuffer: Ok(framebuffer_handle()),
            destroy_framebuffer: Ok(()),
            destroy_mode_blob: Ok(()),
        },
        selected,
        &FakeDrmBuffer::xrgb8888(selected.size()).with_pitch(1280 * 4 - 1),
    );
    assert_eq!(
        invalid_pitch.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidBuffer
    );
    assert!(invalid_pitch.resources.is_none());
    assert!(invalid_pitch.cleanup.is_none());

    let argb_format = create_native_primary_plane_page_flip_resources(
        &FakeNativePrimaryPlaneResourceDevice {
            imported_buffers: std::cell::Cell::new(0),
            closed_buffers: std::cell::Cell::new(0),
            destroyed_framebuffers: std::cell::Cell::new(0),
            mode_blob: Ok(15),
            framebuffer: Ok(framebuffer_handle()),
            destroy_framebuffer: Ok(()),
            destroy_mode_blob: Ok(()),
        },
        selected,
        &FakeDrmBuffer::xrgb8888(selected.size()).with_format(drm::buffer::DrmFourcc::Argb8888),
    );
    assert_eq!(
        argb_format.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::Created
    );
    assert!(argb_format.resources.is_some());
    assert!(argb_format.cleanup.is_none());

    let invalid_format = create_native_primary_plane_page_flip_resources(
        &FakeNativePrimaryPlaneResourceDevice {
            imported_buffers: std::cell::Cell::new(0),
            closed_buffers: std::cell::Cell::new(0),
            destroyed_framebuffers: std::cell::Cell::new(0),
            mode_blob: Ok(15),
            framebuffer: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            destroy_framebuffer: Ok(()),
            destroy_mode_blob: Ok(()),
        },
        selected,
        &FakeDrmBuffer::xrgb8888(selected.size()).with_format(drm::buffer::DrmFourcc::Rgb565),
    );
    assert_eq!(
        invalid_format.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidBuffer
    );
    assert!(invalid_format.resources.is_none());
    assert!(invalid_format.cleanup.is_none());

    let multi_plane_without_modifier = create_native_primary_plane_page_flip_resources(
        &FakeNativePrimaryPlaneResourceDevice {
            imported_buffers: std::cell::Cell::new(0),
            closed_buffers: std::cell::Cell::new(0),
            destroyed_framebuffers: std::cell::Cell::new(0),
            mode_blob: Ok(15),
            framebuffer: Ok(framebuffer_handle()),
            destroy_framebuffer: Ok(()),
            destroy_mode_blob: Ok(()),
        },
        selected,
        &FakeDrmBuffer::xrgb8888(selected.size()).with_two_planes(),
    );
    assert_eq!(
        multi_plane_without_modifier.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidBuffer
    );
    assert_eq!(
        multi_plane_without_modifier.framebuffer,
        Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::NotAttempted)
    );
    assert!(multi_plane_without_modifier.resources.is_none());
    assert!(multi_plane_without_modifier.cleanup.is_none());

    let multi_plane_linear_modifier = create_native_primary_plane_page_flip_resources(
        &FakeNativePrimaryPlaneResourceDevice {
            imported_buffers: std::cell::Cell::new(0),
            closed_buffers: std::cell::Cell::new(0),
            destroyed_framebuffers: std::cell::Cell::new(0),
            mode_blob: Ok(15),
            framebuffer: Ok(framebuffer_handle()),
            destroy_framebuffer: Ok(()),
            destroy_mode_blob: Ok(()),
        },
        selected,
        &FakeDrmBuffer::xrgb8888(selected.size())
            .with_two_planes()
            .with_modifier(drm::buffer::DrmModifier::Linear),
    );
    assert_eq!(
        multi_plane_linear_modifier.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidBuffer
    );
    assert_eq!(
        multi_plane_linear_modifier.framebuffer,
        Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::NotAttempted)
    );
    assert!(multi_plane_linear_modifier.resources.is_none());
    assert!(multi_plane_linear_modifier.cleanup.is_none());

    let multi_plane_non_linear_modifier = create_native_primary_plane_page_flip_resources(
        &full_primary_plane_resource_device(),
        selected,
        &FakeDrmBuffer::xrgb8888(selected.size())
            .with_two_planes()
            .with_modifier(drm::buffer::DrmModifier::I915_x_tiled),
    );
    assert_eq!(
        multi_plane_non_linear_modifier.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::Created
    );
    assert_eq!(
        multi_plane_non_linear_modifier.framebuffer,
        Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::CreatedWithAddFb2Modifiers)
    );
    assert!(multi_plane_non_linear_modifier.resources.is_some());
    assert!(multi_plane_non_linear_modifier.cleanup.is_none());

    let multi_plane_non_linear_addfb2_failure = create_native_primary_plane_page_flip_resources(
        &FakeModifierOnlyPrimaryPlaneResourceDevice {
            mode_blob: Ok(15),
            framebuffer_with_modifiers: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            fallback_framebuffer: Ok(framebuffer_handle()),
            destroy_framebuffer: Ok(()),
            destroy_mode_blob: Ok(()),
        },
        selected,
        &FakeDrmBuffer::xrgb8888(selected.size())
            .with_two_planes()
            .with_modifier(drm::buffer::DrmModifier::I915_x_tiled),
    );
    assert_eq!(
        multi_plane_non_linear_addfb2_failure.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::FramebufferCreateFailed
    );
    assert_eq!(
        multi_plane_non_linear_addfb2_failure.framebuffer,
        Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::AddFb2ModifiersFailed)
    );
    assert!(multi_plane_non_linear_addfb2_failure.resources.is_none());
    assert!(multi_plane_non_linear_addfb2_failure.cleanup.is_none());

    let zero_mode_blob = create_native_primary_plane_resources(
        &FakeNativePrimaryPlaneResourceDevice {
            imported_buffers: std::cell::Cell::new(0),
            closed_buffers: std::cell::Cell::new(0),
            destroyed_framebuffers: std::cell::Cell::new(0),
            mode_blob: Ok(0),
            framebuffer: Ok(framebuffer_handle()),
            destroy_framebuffer: Ok(()),
            destroy_mode_blob: Ok(()),
        },
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        zero_mode_blob.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::ModeBlobCreateFailed
    );
    assert!(zero_mode_blob.resources.is_none());
    assert!(zero_mode_blob.cleanup.is_none());

    let created = create_native_primary_plane_resources(
        &full_primary_plane_resource_device(),
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        created.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::Created
    );
    let destroyed = destroy_native_primary_plane_resources(
        &full_primary_plane_resource_device(),
        created
            .resources
            .expect("created resources should be destroyable"),
    );
    assert_eq!(
        destroyed.status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_primary_plane_page_flip_resources_do_not_require_mode_blob() {
    let selected = select_native_primary_plane_target(&full_kms_selection_device())
        .selection
        .expect("complete KMS path should select a target");
    let mode_unavailable = FakeNativePrimaryPlaneResourceDevice {
        destroyed_framebuffers: std::cell::Cell::new(0),
        mode_blob: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        destroy_mode_blob: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_primary_plane_resource_device()
    };

    let modeset = create_native_primary_plane_resources(
        &mode_unavailable,
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        modeset.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::ModeBlobCreateFailed
    );

    let page_flip = create_native_primary_plane_page_flip_resources(
        &mode_unavailable,
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        page_flip.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::Created
    );
    let destroyed = destroy_native_primary_plane_resources(
        &mode_unavailable,
        page_flip
            .resources
            .expect("page-flip resources should carry only a framebuffer"),
    );
    assert_eq!(
        destroyed.status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_renderer_scanout_buffer_rejects_invalid_renderer_descriptors() {
    let target = LiveGbmEglFrameTargetRecord::new(Size {
        width: 1280,
        height: 720,
    });
    let mut argb_exporter =
        FakeRendererScanoutBufferExporter::new(LiveRendererScanoutBufferExportStatus::Exported)
            .with_descriptor(1280 * 4, LIVE_RENDERER_SCANOUT_FORMAT_ARGB8888, 19);
    let argb_buffer = argb_exporter
        .export_scanout_buffer(target)
        .descriptor
        .and_then(LibdrmRendererScanoutBuffer::from_descriptor)
        .expect("ARGB8888 renderer scanout descriptors should be accepted");
    assert_eq!(
        drm::buffer::Buffer::format(&argb_buffer),
        drm::buffer::DrmFourcc::Argb8888
    );
    assert_eq!(
        drm::buffer::PlanarBuffer::format(&argb_buffer),
        drm::buffer::DrmFourcc::Argb8888
    );
    assert_eq!(
        drm::buffer::PlanarBuffer::pitches(&argb_buffer),
        [1280 * 4, 0, 0, 0]
    );
    assert_eq!(
        drm::buffer::PlanarBuffer::offsets(&argb_buffer),
        [0, 0, 0, 0]
    );
    assert_eq!(
        drm::buffer::PlanarBuffer::handles(&argb_buffer)[0],
        Some(drm::control::from_u32(19).expect("test GEM handle should be nonzero"))
    );
    assert_eq!(drm::buffer::PlanarBuffer::modifier(&argb_buffer), None);

    let linear_descriptor =
        sophia_renderer_live::LiveRendererScanoutBufferDescriptor::new_with_planes(
            Size {
                width: 1280,
                height: 720,
            },
            1280 * 4,
            LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            20,
            sophia_renderer_live::LiveRendererScanoutBufferPlanes {
                count: 1,
                handles: [20, 0, 0, 0],
                pitches: [1280 * 4, 0, 0, 0],
                offsets: [0, 0, 0, 0],
                modifier: Some(u64::from(drm::buffer::DrmModifier::Linear)),
            },
        );
    let linear_buffer = LibdrmRendererScanoutBuffer::from_descriptor(linear_descriptor)
        .expect("linear modified descriptors should stay valid");
    assert_eq!(
        drm::buffer::PlanarBuffer::modifier(&linear_buffer),
        Some(drm::buffer::DrmModifier::Linear)
    );
    let selected = select_native_primary_plane_target(&full_kms_selection_device())
        .selection
        .expect("complete KMS path should select a target");
    let linear_resources = create_native_primary_plane_page_flip_resources(
        &full_primary_plane_resource_device(),
        selected,
        &linear_buffer,
    );
    assert_eq!(
        linear_resources.framebuffer,
        Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::CreatedWithAddFb2Modifiers)
    );
    assert_eq!(
        linear_resources.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::Created
    );

    let linear_fallback_resources = create_native_primary_plane_page_flip_resources(
        &FakeModifierOnlyPrimaryPlaneResourceDevice {
            mode_blob: Ok(15),
            framebuffer_with_modifiers: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            fallback_framebuffer: Ok(framebuffer_handle()),
            destroy_framebuffer: Ok(()),
            destroy_mode_blob: Ok(()),
        },
        selected,
        &linear_buffer,
    );
    assert_eq!(
        linear_fallback_resources.framebuffer,
        Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::CreatedWithAddFb2)
    );
    assert_eq!(
        linear_fallback_resources.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::Created
    );

    let tiled_descriptor =
        sophia_renderer_live::LiveRendererScanoutBufferDescriptor::new_with_planes(
            Size {
                width: 1280,
                height: 720,
            },
            1280 * 4,
            LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            21,
            sophia_renderer_live::LiveRendererScanoutBufferPlanes {
                count: 1,
                handles: [21, 0, 0, 0],
                pitches: [1280 * 4, 0, 0, 0],
                offsets: [0, 0, 0, 0],
                modifier: Some(u64::from(drm::buffer::DrmModifier::I915_x_tiled)),
            },
        );
    let tiled_buffer = LibdrmRendererScanoutBuffer::from_descriptor(tiled_descriptor)
        .expect("nonlinear modified descriptors should stay valid");
    assert_eq!(
        drm::buffer::PlanarBuffer::modifier(&tiled_buffer),
        Some(drm::buffer::DrmModifier::I915_x_tiled)
    );

    let mut invalid_exporter =
        FakeRendererScanoutBufferExporter::new(LiveRendererScanoutBufferExportStatus::Exported)
            .with_descriptor(0, LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, 17);
    let invalid_descriptor = invalid_exporter.export_scanout_buffer(target).descriptor;
    assert!(invalid_descriptor.is_none());

    let mut unsupported_format =
        FakeRendererScanoutBufferExporter::new(LiveRendererScanoutBufferExportStatus::Exported)
            .with_descriptor(1280 * 4, 0, 17);
    assert!(
        unsupported_format
            .export_scanout_buffer(target)
            .descriptor
            .and_then(LibdrmRendererScanoutBuffer::from_descriptor)
            .is_none()
    );

    let forged_ready = sophia_renderer_live::LiveRendererScanoutBufferDescriptor {
        status: sophia_renderer_live::LiveRendererScanoutBufferStatus::Ready,
        size: Size {
            width: -1,
            height: 720,
        },
        ..scanout_descriptor(Size {
            width: 1280,
            height: 720,
        })
    };
    assert!(LibdrmRendererScanoutBuffer::from_descriptor(forged_ready).is_none());

    let forged_undersized_pitch = sophia_renderer_live::LiveRendererScanoutBufferDescriptor {
        status: sophia_renderer_live::LiveRendererScanoutBufferStatus::Ready,
        pitch: 1280 * 4 - 1,
        ..scanout_descriptor(Size {
            width: 1280,
            height: 720,
        })
    };
    assert!(LibdrmRendererScanoutBuffer::from_descriptor(forged_undersized_pitch).is_none());

    let submit = submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor(
        &full_primary_plane_scanout_device(),
        select_native_primary_plane_target(&full_kms_selection_device()),
        forged_ready,
    );
    assert_eq!(
        submit.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::ScanoutBufferUnavailable
    );
    assert_eq!(
        submit.scanout_buffer,
        sophia_renderer_live::LiveRendererScanoutBufferStatus::Invalid
    );
}

#[test]
fn native_libdrm_primary_plane_resource_creation_fails_closed() {
    let selected = select_native_primary_plane_target(&full_kms_selection_device())
        .selection
        .expect("complete KMS path should select a target");

    let mode_failed = FakeNativePrimaryPlaneResourceDevice {
        destroyed_framebuffers: std::cell::Cell::new(0),
        mode_blob: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_primary_plane_resource_device()
    };
    let created = create_native_primary_plane_resources(
        &mode_failed,
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        created.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::ModeBlobCreateFailed
    );
    assert!(created.resources.is_none());

    let mode_missing = FakeNativePrimaryPlaneResourceDevice {
        destroyed_framebuffers: std::cell::Cell::new(0),
        mode_blob: Err(io::Error::from(io::ErrorKind::InvalidInput)),
        ..full_primary_plane_resource_device()
    };
    let created = create_native_primary_plane_resources(
        &mode_missing,
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        created.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::MissingMode
    );
    assert!(created.resources.is_none());

    let framebuffer_failed = FakeNativePrimaryPlaneResourceDevice {
        framebuffer: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_primary_plane_resource_device()
    };
    let created = create_native_primary_plane_resources(
        &framebuffer_failed,
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        created.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::FramebufferCreateFailed
    );
    assert!(created.resources.is_none());
    assert!(created.cleanup.is_none());

    let framebuffer_failed_and_cleanup_failed = FakeNativePrimaryPlaneResourceDevice {
        framebuffer: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        destroy_mode_blob: Err(io::Error::other("test mode blob destroy failed")),
        ..full_primary_plane_resource_device()
    };
    let created = create_native_primary_plane_resources(
        &framebuffer_failed_and_cleanup_failed,
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        created.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::FramebufferCreateFailed
    );
    assert!(created.resources.is_none());
    let cleanup = created
        .cleanup
        .expect("failed framebuffer registration must retain failed mode blob cleanup");
    assert_eq!(
        cleanup.retry(&full_primary_plane_resource_device()).status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_primary_plane_resources_import_dma_buf_handles_for_framebuffer() {
    let selected = select_native_primary_plane_target(&full_kms_selection_device())
        .selection
        .expect("complete KMS path should select a target");
    let created = create_native_primary_plane_page_flip_resources_from_dma_bufs(
        &full_prime_primary_plane_resource_device(),
        selected,
        scanout_descriptor(selected.size()),
        test_dma_buf_plane_fds(),
    );

    assert_eq!(
        created.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::Created
    );
    assert_eq!(
        created.framebuffer,
        Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::CreatedWithAddFb2)
    );
    let destroyed = destroy_native_primary_plane_resources(
        &full_prime_primary_plane_resource_device(),
        created
            .resources
            .expect("created PRIME resources should be destroyable"),
    );
    assert_eq!(
        destroyed.status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_primary_plane_resources_retain_imported_handles_after_framebuffer_failure() {
    let selected = select_native_primary_plane_target(&full_kms_selection_device())
        .selection
        .expect("complete KMS path should select a target");
    let created = create_native_primary_plane_page_flip_resources_from_dma_bufs(
        &FakePrimePrimaryPlaneResourceDevice {
            framebuffer: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            close_buffer: Err(io::Error::other("test imported buffer close failed")),
            ..full_prime_primary_plane_resource_device()
        },
        selected,
        scanout_descriptor(selected.size()),
        test_dma_buf_plane_fds(),
    );

    assert_eq!(
        created.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::FramebufferCreateFailed
    );
    assert_eq!(
        created.framebuffer,
        Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::AddFb2ThenLegacyAddFbFailed)
    );
    let cleanup = created
        .cleanup
        .expect("failed framebuffer creation should retain imported buffer cleanup debt");
    assert_eq!(
        cleanup
            .retry(&full_prime_primary_plane_resource_device())
            .status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_primary_plane_resources_keep_imported_handle_cleanup_retryable_after_retire() {
    let selected = select_native_primary_plane_target(&full_kms_selection_device())
        .selection
        .expect("complete KMS path should select a target");
    let created = create_native_primary_plane_resources_from_dma_bufs(
        &full_prime_primary_plane_resource_device(),
        selected,
        scanout_descriptor(selected.size()),
        test_dma_buf_plane_fds(),
    );
    let resources = created
        .resources
        .expect("created PRIME modeset resources should be destroyable");
    let close_failed = FakePrimePrimaryPlaneResourceDevice {
        close_buffer: Err(io::Error::other("test imported buffer close failed")),
        ..full_prime_primary_plane_resource_device()
    };
    let destroyed = destroy_native_primary_plane_resources(&close_failed, resources);

    assert_eq!(
        destroyed.status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::ImportedBufferCloseFailed
    );
    let cleanup = destroyed
        .cleanup
        .expect("failed imported buffer close should retain cleanup debt");
    assert_eq!(
        cleanup
            .retry(&full_prime_primary_plane_resource_device())
            .status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}
