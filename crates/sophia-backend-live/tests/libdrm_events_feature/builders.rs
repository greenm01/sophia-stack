#[test]
fn native_libdrm_primary_plane_resources_accept_already_closed_imported_handle() {
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
    let already_closed = FakePrimePrimaryPlaneResourceDevice {
        close_buffer: Err(io::Error::from(io::ErrorKind::InvalidInput)),
        ..full_prime_primary_plane_resource_device()
    };

    let destroyed = destroy_native_primary_plane_resources(&already_closed, resources);

    assert_eq!(
        destroyed.status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
    assert!(destroyed.cleanup.is_none());
}

#[test]
fn native_libdrm_primary_plane_selection_reduces_missing_resource_groups() {
    let disconnected = FakeNativeKmsSelectionDevice {
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
    };
    assert_eq!(
        select_native_primary_plane_target(&disconnected).status,
        LibdrmNativePrimaryPlaneSelectionStatus::NoConnectedConnector
    );

    let modeless = FakeNativeKmsSelectionDevice {
        connector_snapshot: Ok(LibdrmNativeConnectorSnapshot::new(
            true,
            Some(encoder_handle()),
            [encoder_handle()],
            None,
        )),
        ..full_kms_selection_device()
    };
    assert_eq!(
        select_native_primary_plane_target(&modeless).status,
        LibdrmNativePrimaryPlaneSelectionStatus::NoUsableMode
    );

    let no_encoder = FakeNativeKmsSelectionDevice {
        connector_snapshot: Ok(LibdrmNativeConnectorSnapshot::new(
            true,
            None,
            [],
            Some(Size {
                width: 1280,
                height: 720,
            }),
        )),
        ..full_kms_selection_device()
    };
    assert_eq!(
        select_native_primary_plane_target(&no_encoder).status,
        LibdrmNativePrimaryPlaneSelectionStatus::NoUsableEncoder
    );

    let incompatible_crtc = FakeNativeKmsSelectionDevice {
        encoder_snapshot: Ok(LibdrmNativeEncoderSnapshot::new(None, [])),
        ..full_kms_selection_device()
    };
    assert_eq!(
        select_native_primary_plane_target(&incompatible_crtc).status,
        LibdrmNativePrimaryPlaneSelectionStatus::NoCompatibleCrtc
    );

    let no_primary_plane = FakeNativeKmsSelectionDevice {
        plane_type: Ok(Some(drm::control::PlaneType::Overlay)),
        ..full_kms_selection_device()
    };
    assert_eq!(
        select_native_primary_plane_target(&no_primary_plane).status,
        LibdrmNativePrimaryPlaneSelectionStatus::NoCompatiblePrimaryPlane
    );
}

#[test]
fn native_libdrm_primary_plane_selection_fails_closed_on_read_error() {
    let read_failed = FakeNativeKmsSelectionDevice {
        connectors: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_kms_selection_device()
    };
    let selection = select_native_primary_plane_target(&read_failed);

    assert_eq!(
        selection.status,
        LibdrmNativePrimaryPlaneSelectionStatus::ReadFailed
    );
    assert!(selection.selection.is_none());

    let plane_read_failed = FakeNativeKmsSelectionDevice {
        plane_snapshot: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_kms_selection_device()
    };
    let selection = select_native_primary_plane_target(&plane_read_failed);

    assert_eq!(
        selection.status,
        LibdrmNativePrimaryPlaneSelectionStatus::ReadFailed
    );
    assert!(selection.selection.is_none());
}

#[test]
fn native_libdrm_primary_plane_builder_creates_submit_ready_request() {
    let build = build_native_primary_plane_atomic_request(
        primary_plane_objects(Size {
            width: 1280,
            height: 720,
        }),
        primary_plane_properties(),
    );

    assert_eq!(build.status, LibdrmNativeAtomicRequestBuildStatus::Built);
    let request = build.request.expect("valid objects should build request");
    assert_eq!(
        request.reduced_scope(),
        LibdrmNativeAtomicCommitRequestScope::Modeset
    );
    assert_eq!(
        request.reduced_flags(),
        LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: false,
            test_only: false,
        }
    );

    let mut committer =
        NativeLibdrmAtomicScanoutCommitter::new(FakeNativeAtomicCommitDevice { result: Ok(()) });
    assert_eq!(
        committer.submit_native_atomic_commit(request),
        LibdrmNativeAtomicCommitSubmitReport {
            status: LibdrmNativeAtomicCommitSubmitStatus::Submitted,
        }
    );
}

#[test]
fn native_libdrm_primary_plane_page_flip_builder_creates_plane_only_request() {
    let build = build_native_primary_plane_page_flip_atomic_request(
        primary_plane_objects(Size {
            width: 1280,
            height: 720,
        }),
        primary_plane_properties(),
    );

    assert_eq!(build.status, LibdrmNativeAtomicRequestBuildStatus::Built);
    let request = build
        .request
        .expect("valid objects should build page-flip request");
    assert_eq!(
        request.reduced_scope(),
        LibdrmNativeAtomicCommitRequestScope::PageFlip
    );
    assert_eq!(
        request.reduced_flags(),
        LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: false,
            test_only: false,
        }
    );
}

#[test]
fn native_vrr_page_flip_builder_fails_closed_without_enable_property() {
    let missing = build_native_primary_plane_page_flip_atomic_request_with_vrr(
        primary_plane_objects(Size {
            width: 1280,
            height: 720,
        }),
        primary_plane_properties(),
        true,
    );
    assert_eq!(
        missing.status,
        LibdrmNativeAtomicRequestBuildStatus::MissingVrrProperty
    );
    assert!(missing.request.is_none());

    let built = build_native_primary_plane_page_flip_atomic_request_with_vrr(
        primary_plane_objects(Size {
            width: 1280,
            height: 720,
        }),
        primary_plane_properties().with_crtc_vrr_enabled(Some(property_handle(116))),
        true,
    );
    assert_eq!(built.status, LibdrmNativeAtomicRequestBuildStatus::Built);
    assert!(built.request.is_some());
}

#[test]
fn native_vrr_modeset_builder_sets_activation_only_with_enable_property() {
    let missing = build_native_primary_plane_atomic_request_with_vrr(
        primary_plane_objects(Size {
            width: 1280,
            height: 720,
        }),
        primary_plane_properties(),
        true,
    );
    assert_eq!(
        missing.status,
        LibdrmNativeAtomicRequestBuildStatus::MissingVrrProperty
    );
    assert!(missing.request.is_none());

    let built = build_native_primary_plane_atomic_request_with_vrr(
        primary_plane_objects(Size {
            width: 1280,
            height: 720,
        }),
        primary_plane_properties().with_crtc_vrr_enabled(Some(property_handle(116))),
        true,
    );
    assert_eq!(built.status, LibdrmNativeAtomicRequestBuildStatus::Built);
    assert_eq!(
        built
            .request
            .expect("VRR modeset should build")
            .reduced_scope(),
        LibdrmNativeAtomicCommitRequestScope::Modeset
    );
}

#[test]
fn native_libdrm_primary_plane_modeset_builder_requires_mode_blob() {
    let objects = LibdrmNativePrimaryPlaneObjects::new_with_optional_mode_blob(
        connector_handle(),
        crtc_handle(),
        plane_handle(),
        framebuffer_handle(),
        None,
        Size {
            width: 1280,
            height: 720,
        },
    );

    let modeset = build_native_primary_plane_atomic_request(objects, primary_plane_properties());
    assert_eq!(
        modeset.status,
        LibdrmNativeAtomicRequestBuildStatus::MissingModeBlob
    );
    assert!(modeset.request.is_none());

    let zero_mode_blob_objects = LibdrmNativePrimaryPlaneObjects::new_with_optional_mode_blob(
        connector_handle(),
        crtc_handle(),
        plane_handle(),
        framebuffer_handle(),
        Some(0),
        Size {
            width: 1280,
            height: 720,
        },
    );
    let zero_mode_blob = build_native_primary_plane_atomic_request(
        zero_mode_blob_objects,
        primary_plane_properties(),
    );
    assert_eq!(
        zero_mode_blob.status,
        LibdrmNativeAtomicRequestBuildStatus::MissingModeBlob
    );
    assert!(zero_mode_blob.request.is_none());

    let page_flip =
        build_native_primary_plane_page_flip_atomic_request(objects, primary_plane_properties());
    assert_eq!(
        page_flip.status,
        LibdrmNativeAtomicRequestBuildStatus::Built
    );
    assert!(page_flip.request.is_some());
}

#[test]
fn native_libdrm_primary_plane_builder_rejects_invalid_size() {
    let zero_width = build_native_primary_plane_atomic_request(
        primary_plane_objects(Size {
            width: 0,
            height: 720,
        }),
        primary_plane_properties(),
    );
    assert_eq!(
        zero_width.status,
        LibdrmNativeAtomicRequestBuildStatus::InvalidSize
    );
    assert!(zero_width.request.is_none());

    let negative_height = build_native_primary_plane_atomic_request(
        primary_plane_objects(Size {
            width: 1280,
            height: -1,
        }),
        primary_plane_properties(),
    );
    assert_eq!(
        negative_height.status,
        LibdrmNativeAtomicRequestBuildStatus::InvalidSize
    );
    assert!(negative_height.request.is_none());

    let oversized_width = build_native_primary_plane_atomic_request(
        primary_plane_objects(Size {
            width: 65_536,
            height: 720,
        }),
        primary_plane_properties(),
    );
    assert_eq!(
        oversized_width.status,
        LibdrmNativeAtomicRequestBuildStatus::InvalidSize
    );
    assert!(oversized_width.request.is_none());

    let oversized_height = build_native_primary_plane_page_flip_atomic_request(
        primary_plane_objects(Size {
            width: 1280,
            height: 65_536,
        }),
        primary_plane_properties(),
    );
    assert_eq!(
        oversized_height.status,
        LibdrmNativeAtomicRequestBuildStatus::InvalidSize
    );
    assert!(oversized_height.request.is_none());
}

#[test]
fn native_libdrm_atomic_committer_reduces_submit_results() {
    let mut committer =
        NativeLibdrmAtomicScanoutCommitter::new(FakeNativeAtomicCommitDevice { result: Ok(()) });
    assert_eq!(
        committer.submit_native_atomic_commit(LibdrmNativeAtomicCommitRequest::new(
            drm::control::atomic::AtomicModeReq::new()
        )),
        LibdrmNativeAtomicCommitSubmitReport {
            status: LibdrmNativeAtomicCommitSubmitStatus::Submitted,
        }
    );
    assert_eq!(committer.submitted_count(), 1);
    assert_eq!(committer.rejected_count(), 0);

    let mut would_block = NativeLibdrmAtomicScanoutCommitter::new(FakeNativeAtomicCommitDevice {
        result: Err(io::Error::from(io::ErrorKind::WouldBlock)),
    });
    assert_eq!(
        would_block.submit_native_atomic_commit(LibdrmNativeAtomicCommitRequest::new(
            drm::control::atomic::AtomicModeReq::new()
        )),
        LibdrmNativeAtomicCommitSubmitReport {
            status: LibdrmNativeAtomicCommitSubmitStatus::WouldBlock,
        }
    );
    assert_eq!(would_block.submitted_count(), 0);
    assert_eq!(would_block.rejected_count(), 0);

    let mut rejected = NativeLibdrmAtomicScanoutCommitter::new(FakeNativeAtomicCommitDevice {
        result: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
    });
    assert_eq!(
        rejected.submit_native_atomic_commit(LibdrmNativeAtomicCommitRequest::new(
            drm::control::atomic::AtomicModeReq::new()
        )),
        LibdrmNativeAtomicCommitSubmitReport {
            status: LibdrmNativeAtomicCommitSubmitStatus::Rejected,
        }
    );
    assert_eq!(rejected.submitted_count(), 0);
    assert_eq!(rejected.rejected_count(), 1);
}

#[path = "native_page_flip.rs"]
mod native_page_flip;

fn ready_drm_sysfs_fixture(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("sophia-backend-live-{name}"));
    let _ = std::fs::remove_dir_all(&root);
    let connector = root.join("card0-HDMI-A-1");
    std::fs::create_dir_all(&connector).unwrap();
    write_fixture_file(&connector, "status", "connected\n");
    write_fixture_file(&connector, "modes", "1280x720\n");
    write_fixture_file(&connector, "connector_id", "42\n");
    write_fixture_file(&connector, "crtc_id", "99\n");
    root
}

fn multi_output_drm_sysfs_fixture(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("sophia-backend-live-{name}"));
    let _ = std::fs::remove_dir_all(&root);
    let first = root.join("card0-DP-1");
    let second = root.join("card0-HDMI-A-1");
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    write_fixture_file(&first, "status", "connected\n");
    write_fixture_file(&first, "modes", "1920x1080\n");
    write_fixture_file(&first, "connector_id", "1234\n");
    write_fixture_file(&first, "crtc_id", "2234\n");
    write_fixture_file(&second, "status", "connected\n");
    write_fixture_file(&second, "modes", "2560x1440\n");
    write_fixture_file(&second, "connector_id", "9876\n");
    write_fixture_file(&second, "crtc_id", "8876\n");
    root
}

fn write_fixture_file(root: &std::path::Path, name: &str, contents: &str) {
    std::fs::write(root.join(name), contents).unwrap();
}

#[cfg(feature = "gbm-probe")]
#[path = "atomic_scanout_hardware_smoke.rs"]
mod atomic_scanout_hardware_smoke;
