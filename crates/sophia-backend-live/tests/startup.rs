use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use sophia_backend_live::{
    BufferSource, CompositorBackendTickInput, DeviceId, FakeAtomicScanoutCommitter,
    FakeGbmEglFrameTargetAllocator, FakePageFlipCallbackSource, HeadlessOutput,
    LibinputDeviceDescriptor, LibinputDeviceKind, LiveAtomicScanoutCommitReport,
    LiveAtomicScanoutCommitStatus, LiveAtomicScanoutCommitter, LiveBackendConfig,
    LiveBackendDependencyDecision, LiveBackendDependencyKind, LiveBackendDependencyUse,
    LiveCompositorBackendDiscoveryStatus, LiveGbmEglFrameTargetAllocationReport,
    LiveGbmEglFrameTargetAllocationStatus, LiveGbmEglFrameTargetLifecycleReport,
    LiveGbmEglFrameTargetLifecycleStatus, LiveGbmEglFrameTargetRecord, LiveGbmEglFrameTargetStatus,
    LiveKmsScanoutTargetReport, LiveKmsScanoutTargetStatus, LiveLibdrmPollerDiagnostics,
    LiveLibdrmPollerDiagnosticsStatus, LivePageFlipCallback, LivePageFlipCallbackDecision,
    LivePageFlipCallbackIntake, LivePageFlipCallbackQueue, LivePageFlipCallbackQueueReport,
    LivePageFlipCallbackReport, LivePageFlipCallbackSourceReport, LivePageFlipEvent,
    LivePageFlipEventStatus, LiveRenderedOutputState, LiveRenderedOutputTable,
    LiveRenderedOutputTableUpdate, LiveRendererImportBoundary, LiveRendererImportHealth,
    LiveRendererImportPathStatus, LiveRendererImportStartupStatus, LiveRendererPreference,
    LiveRendererPresentationReport, LiveRendererPresentationStatus, LiveRendererRuntimeObservation,
    LiveRendererSelectionObservation, LiveScanoutReadinessReport, LiveScanoutReadinessStatus,
    OutputId, PageFlipCommitOutcome, PhysicalInputRoutingStage, QueuedInputPoller,
    RendererSelection, SeatId, Size, discover_live_backend, live_backend_dependency_decision,
};

include!("startup/discovery.rs");
include!("startup/frame_targets.rs");
include!("startup/page_flips.rs");
include!("startup/readiness.rs");
include!("startup/support.rs");
