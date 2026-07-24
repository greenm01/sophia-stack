#![cfg(feature = "libdrm-events")]

use std::{
    io,
    os::fd::{BorrowedFd, OwnedFd},
    sync::mpsc,
};

use sophia_backend_live::{
    CompositorBackendTickInput, FakeLibdrmNativePageFlipReader, FakeLibdrmPageFlipEventPoller,
    LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS,
    LIVE_RENDERED_PRIMARY_PLANE_SCANOUT_STALL_THRESHOLD_TICKS,
    LIVE_RENDERER_SCANOUT_FORMAT_ARGB8888, LibdrmBackendFdAuthority,
    LibdrmBackendFdAuthorityReport, LibdrmBackendFdAuthorityStatus,
    LibdrmDependencyAdmissionReport, LibdrmDependencyAdmissionStatus,
    LibdrmNativeAtomicCommitDevice, LibdrmNativeAtomicCommitFlagsReport,
    LibdrmNativeAtomicCommitRequest, LibdrmNativeAtomicCommitRequestScope,
    LibdrmNativeAtomicCommitSubmitReport, LibdrmNativeAtomicCommitSubmitStatus,
    LibdrmNativeAtomicRequestBuildStatus, LibdrmNativeAtomicScanoutPageFlipWaitStatus,
    LibdrmNativeAtomicScanoutSmokeEvidence, LibdrmNativeAtomicScanoutSmokePhase,
    LibdrmNativeAtomicScanoutSmokeStatus, LibdrmNativeConnectorSnapshot, LibdrmNativeCrtcRoute,
    LibdrmNativeEncoderSnapshot, LibdrmNativeEventAdapterReport, LibdrmNativeEventAdapterStatus,
    LibdrmNativeKmsSelectionDevice, LibdrmNativeOutputRoute, LibdrmNativeOutputSlot,
    LibdrmNativePageFlipCallback, LibdrmNativePageFlipDecodeReport,
    LibdrmNativePageFlipDecodeStatus, LibdrmNativePageFlipReadResult, LibdrmNativePageFlipReader,
    LibdrmNativePageFlipSource, LibdrmNativePageFlipSourceReport, LibdrmNativePageFlipSourceStatus,
    LibdrmNativePlaneFormatModifierSupportStatus, LibdrmNativePlaneFormatModifierTable,
    LibdrmNativePlaneFormatModifierTableParseStatus, LibdrmNativePlaneSnapshot,
    LibdrmNativePollerDiagnostics, LibdrmNativePrimaryPlaneFormatTableStatus,
    LibdrmNativePrimaryPlaneFramebufferCreateDetail, LibdrmNativePrimaryPlaneObjects,
    LibdrmNativePrimaryPlanePropertyDiscoveryStatus, LibdrmNativePrimaryPlanePropertyHandles,
    LibdrmNativePrimaryPlaneResourceCreateStatus, LibdrmNativePrimaryPlaneResourceDestroyStatus,
    LibdrmNativePrimaryPlaneResourceDevice, LibdrmNativePrimaryPlaneScanoutRetireResult,
    LibdrmNativePrimaryPlaneScanoutRetireStatus, LibdrmNativePrimaryPlaneScanoutSubmitPolicy,
    LibdrmNativePrimaryPlaneScanoutSubmitStatus, LibdrmNativePrimaryPlaneSelectionResult,
    LibdrmNativePrimaryPlaneSelectionSetStatus, LibdrmNativePrimaryPlaneSelectionStatus,
    LibdrmNativePropertyHandleSet, LibdrmNativePropertyLookupDevice, LibdrmNativeReadAndPollReport,
    LibdrmNativeReadLoopReport, LibdrmNativeReadLoopStatus,
    LibdrmNativeRenderedScanoutContextStatus, LibdrmNativeScanoutBufferFormatDetail,
    LibdrmNativeScanoutBufferModifierDetail, LibdrmNativeScanoutBufferPlaneDetail,
    LibdrmNativeVrrPropertyDiscoveryStatus, LibdrmPageFlipEventPollReport,
    LibdrmPageFlipEventPollStatus, LibdrmPageFlipEventPoller, LibdrmRendererScanoutBuffer,
    LiveAtomicScanoutPreflightReport, LiveAtomicScanoutPreflightStatus, LiveBackendConfig,
    LiveHardwareValidationGateReport, LiveHardwareValidationGateStatus,
    LiveHardwareValidationSmokeReport, LiveHardwareValidationSmokeStatus,
    LiveHardwareValidationTarget, LiveKmsScanoutTargetStatus, LiveLibdrmPollerDiagnostics,
    LiveLibdrmPollerDiagnosticsStatus, LiveLibdrmPollerStartupReport,
    LiveLibdrmPollerStartupStatus, LivePageFlipCallback, LivePageFlipCallbackDecision,
    LivePageFlipCallbackQueue, LivePageFlipCallbackReport, LivePageFlipCallbackSourceReport,
    LivePageFlipEvent, LivePageFlipEventStatus, LiveRenderedPrimaryPlaneScanoutBackpressureReport,
    LiveRenderedPrimaryPlaneScanoutBackpressureStatus, LiveRenderedPrimaryPlaneScanoutSubmitStatus,
    LiveRenderedScanoutBufferExport, LiveRenderedScanoutBufferExporter,
    LiveRenderedScanoutBufferPrimeSource, LiveRenderedScanoutDmaBufFds,
    LiveRendererScanoutBufferExportDetail, LiveRuntimeRenderedScanoutEvidenceFailureReport,
    LiveRuntimeRenderedScanoutEvidenceFailureStatus, LiveSessionCompositionSmokeStatus,
    LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus,
    LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus,
    LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus, NativeLibdrmAtomicScanoutCommitter,
    NativeLibdrmPageFlipEventPoller, NativeLibdrmPageFlipEventReader, OutputId, QueuedInputPoller,
    RealAtomicScanoutCardSelectionStatus, RealAtomicScanoutPageFlipSessionStatus,
    RealAtomicScanoutPageFlipWaitPolicy, RuntimeScanoutState, Size,
    build_native_primary_plane_atomic_request, build_native_primary_plane_atomic_request_with_vrr,
    build_native_primary_plane_page_flip_atomic_request,
    build_native_primary_plane_page_flip_atomic_request_with_vrr,
    create_native_primary_plane_page_flip_resources,
    create_native_primary_plane_page_flip_resources_from_dma_bufs,
    create_native_primary_plane_resources, create_native_primary_plane_resources_from_dma_bufs,
    decode_native_page_flip_batch, destroy_native_primary_plane_resources, discover_live_backend,
    discover_native_primary_plane_property_handles, discover_native_vrr_properties,
    libdrm_dependency_admission_report, libdrm_fd_authority_report,
    native_libdrm_event_adapter_report, native_libdrm_event_adapter_report_for_authority,
    real_atomic_scanout_preflight_report, real_atomic_scanout_validation_gate,
    real_atomic_scanout_validation_smoke_report, real_libdrm_events_validation_gate,
    real_libdrm_events_validation_smoke_report, reduce_native_page_flip_event,
    retire_native_primary_plane_scanout_after_page_flip,
    retire_rendered_primary_plane_scanout_after_page_flip, run_live_session_composition_smoke,
    select_native_primary_plane_target, select_native_primary_plane_targets,
    select_real_atomic_scanout_card_from_dev_dri,
    submit_native_primary_plane_scanout_from_renderer_descriptor,
    submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor,
    submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy,
};
#[cfg(feature = "libinput-events")]
use sophia_backend_live::{
    DeviceId, FakeLiveLibinputEventReader, InputEventPacket, LibinputDeviceDescriptor,
    LibinputDeviceKind, LiveBackendReadinessCollector, LiveBackendSessionLoop,
    LiveBackendSessionLoopPageFlipBudget, LiveBackendSessionLoopReadiness,
    LiveInputReadinessGateStatus, LiveInputReadinessGatedPoller, NativeLibinputEventPoller, SeatId,
};
#[cfg(feature = "gbm-probe")]
use sophia_backend_live::{
    LiveCpuComposedFrame, LiveGbmEglFrameTargetStatus, LiveProductionCursorPresentation,
    LiveProductionOutputRuntimeSet, LiveProductionOutputServiceState, LiveProductionScanoutContent,
    NativeGbmRenderedScanoutBufferDiscoveryExporter, NativeGbmRenderedScanoutContextStatus,
    RealAtomicScanoutSmokeConfig, RenderDeviceDiscoveryBackend,
    live_production_scanout_is_stable_present, reduce_live_production_async_service_observation,
};
#[cfg(feature = "gbm-probe")]
use sophia_backend_live::{
    LiveRendererImportHealth, LiveRendererImportPathStatus, LiveRendererRuntimeObservation,
    LiveRendererSelectionObservation, real_atomic_runtime_rendered_scanout_renderer_observation,
};
use sophia_engine::AuthorityTransactionIntake;
use sophia_protocol::{
    AuthorityKind, BufferSource, NamespaceId, Rect, Region, SurfaceId, SurfaceTransaction,
    SurfaceTransactionReadiness, TransactionId,
};
#[cfg(feature = "libinput-events")]
use sophia_protocol::{InputEventKind, Point};
use sophia_renderer_live::{
    FakeRendererScanoutBufferExporter, LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
    LiveGbmEglFrameTargetRecord, LiveRendererScanoutBufferExportStatus,
    LiveRendererScanoutBufferExporter,
};
#[cfg(feature = "gbm-probe")]
use sophia_renderer_live::{
    LiveGbmEglFrameTargetLifecycleReport, LiveGbmEglFrameTargetLifecycleStatus,
};

include!("libdrm_events_feature/evidence_gates.rs");
include!("libdrm_events_feature/fake_devices.rs");
include!("libdrm_events_feature/fixtures_and_outputs.rs");
include!("libdrm_events_feature/native_selection.rs");
include!("libdrm_events_feature/scanout_submission.rs");
include!("libdrm_events_feature/scanout_retirement.rs");
include!("libdrm_events_feature/runtime_ticks.rs");
include!("libdrm_events_feature/session_loop.rs");
include!("libdrm_events_feature/native_gbm.rs");
include!("libdrm_events_feature/resource_lifetime.rs");
include!("libdrm_events_feature/builders.rs");
