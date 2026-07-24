use super::*;

#[test]
fn native_atomic_scanout_smoke_evidence_reports_resource_retire_failure() {
    let device = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            destroy_framebuffer: Err(io::Error::other("test framebuffer destroy failed")),
            ..full_primary_plane_resource_device()
        },
        ..full_primary_plane_scanout_device()
    };
    let mut submit = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    let submission = submit
        .submission
        .take()
        .expect("submitted scanout should retain resource ownership");
    let poll =
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 1,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        });
    let callback = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(42),
        },
    };
    let retired =
        retire_native_primary_plane_scanout_after_page_flip(&device, submission, &callback);
    let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
        LiveKmsScanoutTargetStatus::Ready,
        Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
        LiveRendererScanoutBufferExportStatus::Exported,
        Some(&submit),
        Some(&poll),
        Some(&callback),
        Some(&retired),
    );
    assert_eq!(
        evidence.status,
        LibdrmNativeAtomicScanoutSmokeStatus::RetireFailed
    );
    assert_eq!(
        evidence.retire,
        Some(LibdrmNativePrimaryPlaneScanoutRetireStatus::ResourceRetireFailed)
    );
    assert_eq!(
        evidence.page_flip_wait,
        Some(LibdrmNativeAtomicScanoutPageFlipWaitStatus::ResourceRetireFailed)
    );
    assert_eq!(
        evidence.retire_destroy,
        Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::FramebufferDestroyFailed)
    );
    assert!(evidence.retire_cleanup_pending);
}

#[test]
fn native_atomic_scanout_smoke_evidence_requires_destroyed_retire_resources() {
    let device = full_primary_plane_scanout_device();
    let submit = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    let poll =
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 1,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        });
    let callback = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(42),
        },
    };
    let inconsistent_retire = LibdrmNativePrimaryPlaneScanoutRetireResult {
        status: LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip,
        destroy: Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::FramebufferDestroyFailed),
        submission: None,
        cleanup: None,
    };
    let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
        LiveKmsScanoutTargetStatus::Ready,
        Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
        LiveRendererScanoutBufferExportStatus::Exported,
        Some(&submit),
        Some(&poll),
        Some(&callback),
        Some(&inconsistent_retire),
    );
    assert_eq!(
        evidence.status,
        LibdrmNativeAtomicScanoutSmokeStatus::RetireFailed
    );
    assert_eq!(
        evidence.retire_destroy,
        Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::FramebufferDestroyFailed)
    );
    assert_eq!(
        evidence.page_flip_wait,
        Some(LibdrmNativeAtomicScanoutPageFlipWaitStatus::RetireMissing)
    );
}

#[test]
fn native_atomic_scanout_smoke_evidence_records_reduced_early_failures() {
    for (evidence, status) in [
        (
            LibdrmNativeAtomicScanoutSmokeEvidence::smoke_child_timeout(),
            LibdrmNativeAtomicScanoutSmokeStatus::SmokeChildTimeout,
        ),
        (
            LibdrmNativeAtomicScanoutSmokeEvidence::no_primary_card(),
            LibdrmNativeAtomicScanoutSmokeStatus::NoPrimaryCard,
        ),
        (
            LibdrmNativeAtomicScanoutSmokeEvidence::primary_card_open_failed(),
            LibdrmNativeAtomicScanoutSmokeStatus::PrimaryCardOpenFailed,
        ),
        (
            LibdrmNativeAtomicScanoutSmokeEvidence::client_capability_failed(),
            LibdrmNativeAtomicScanoutSmokeStatus::ClientCapabilityFailed,
        ),
        (
            LibdrmNativeAtomicScanoutSmokeEvidence::kms_selection_failed(),
            LibdrmNativeAtomicScanoutSmokeStatus::KmsSelectionFailed,
        ),
        (
            LibdrmNativeAtomicScanoutSmokeEvidence::property_discovery_failed(),
            LibdrmNativeAtomicScanoutSmokeStatus::PropertyDiscoveryFailed,
        ),
    ] {
        assert_eq!(evidence.status, status);
    }
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            None,
            LiveRendererScanoutBufferExportStatus::Exported,
            None,
            None,
            None,
            None,
        )
        .status,
        LibdrmNativeAtomicScanoutSmokeStatus::RenderedContextUnavailable
    );
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
            LiveRendererScanoutBufferExportStatus::Unavailable,
            None,
            None,
            None,
            None,
        )
        .status,
        LibdrmNativeAtomicScanoutSmokeStatus::GbmExportFailed
    );
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            Some(LibdrmNativeRenderedScanoutContextStatus::Unavailable),
            LiveRendererScanoutBufferExportStatus::Unavailable,
            None,
            None,
            None,
            None,
        )
        .status,
        LibdrmNativeAtomicScanoutSmokeStatus::RenderedContextUnavailable
    );
    for (status, marker) in [
        (
            LibdrmNativeAtomicScanoutSmokeStatus::RetainedResourceMissing,
            "status=RetainedResourceMissing",
        ),
        (
            LibdrmNativeAtomicScanoutSmokeStatus::PageFlipReaderUnavailable,
            "status=PageFlipReaderUnavailable",
        ),
    ] {
        let mut evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
            LiveRendererScanoutBufferExportStatus::Exported,
            None,
            None,
            None,
            None,
        );
        evidence.status = status;
        assert!(evidence.reduced_log_line().contains(marker));
    }
}
