#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use super::{LiveRenderedScanoutBufferExport, LiveRenderedScanoutBufferExporter};
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use crate::api::*;

use super::{NativeGbmRendererWorker, NativeGbmRendererWorkerScanoutLease, WorkerPoll};
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use sophia_renderer_live::{
    LiveCpuComposedFrame, LiveRendererScanoutBufferExportDetail,
    LiveRendererScanoutBufferExportStatus, NativeGbmOwnedScanoutBuffer,
    NativeGbmRenderedScanoutContext, NativeGbmRenderedScanoutContextStatus,
};

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
pub(super) enum PendingRenderedFrame {
    Cpu {
        frame: LiveCpuComposedFrame,
        checksum: u64,
    },
    DmaBuf(sophia_renderer_live::LiveOwnedDmaBufFrame),
    Mixed(sophia_renderer_live::LiveOwnedMixedCompositionFrame),
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
pub struct NativeGbmRenderedScanoutBufferDiscoveryExporter<R>
where
    R: RenderDeviceDiscoveryBackend,
{
    discovery: R,
    context: Option<NativeGbmRenderedScanoutContext<R::Device>>,
    worker: Option<NativeGbmRendererWorker>,
    worker_frame_kind: Option<PendingRenderedFrameKind>,
    context_status: Option<NativeGbmRenderedScanoutContextStatus>,
    context_open_attempts: usize,
    export_attempts: usize,
    preferred_modifiers: Vec<u64>,
    last_target: Option<LiveGbmEglFrameTargetRecord>,
    last_target_lifecycle: Option<LiveGbmEglFrameTargetLifecycleReport>,
    last_export_status: Option<LiveRendererScanoutBufferExportStatus>,
    pending_frame: Option<PendingRenderedFrame>,
    cpu_frame_export_attempts: usize,
    dmabuf_frame_export_attempts: usize,
    dmabuf_frame_exports: usize,
    mixed_frame_export_attempts: usize,
    mixed_frame_exports: usize,
    last_cpu_frame_checksum: Option<u64>,
    last_cpu_frame_export_status: Option<LiveRendererScanoutBufferExportStatus>,
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
#[derive(Debug)]
pub enum NativeGbmRenderedScanoutOwner {
    Inline(NativeGbmOwnedScanoutBuffer),
    Worker(NativeGbmRendererWorkerScanoutLease),
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingRenderedFrameKind {
    Cpu,
    DmaBuf,
    Mixed,
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl<R> NativeGbmRenderedScanoutBufferDiscoveryExporter<R>
where
    R: RenderDeviceDiscoveryBackend,
{
    pub fn new(discovery: R) -> Self {
        Self {
            discovery,
            context: None,
            worker: None,
            worker_frame_kind: None,
            context_status: None,
            context_open_attempts: 0,
            export_attempts: 0,
            preferred_modifiers: Vec::new(),
            last_target: None,
            last_target_lifecycle: None,
            last_export_status: None,
            pending_frame: None,
            cpu_frame_export_attempts: 0,
            dmabuf_frame_export_attempts: 0,
            dmabuf_frame_exports: 0,
            mixed_frame_export_attempts: 0,
            mixed_frame_exports: 0,
            last_cpu_frame_checksum: None,
            last_cpu_frame_export_status: None,
        }
    }

    pub fn new_worker(discovery: R) -> std::io::Result<Self>
    where
        R::Device: Send + 'static,
    {
        let device = discovery.open_render_device();
        let mut exporter = Self::new(discovery);
        exporter.context_open_attempts = 1;
        exporter.worker = Some(NativeGbmRendererWorker::spawn(device)?);
        Ok(exporter)
    }

    pub fn enable_worker(&mut self) -> std::io::Result<()>
    where
        R::Device: Send + 'static,
    {
        if self.worker.is_some() {
            return Ok(());
        }
        self.context_open_attempts = self.context_open_attempts.saturating_add(1);
        self.worker = Some(NativeGbmRendererWorker::spawn(
            self.discovery.open_render_device(),
        )?);
        self.context = None;
        self.context_status = None;
        Ok(())
    }

    pub fn with_preferred_modifiers(mut self, preferred_modifiers: impl Into<Vec<u64>>) -> Self {
        self.preferred_modifiers = reduced_preferred_scanout_modifiers(preferred_modifiers.into());
        self
    }

    pub const fn context_open_attempts(&self) -> usize {
        self.context_open_attempts
    }

    pub const fn export_attempts(&self) -> usize {
        self.export_attempts
    }

    pub const fn last_export_status(&self) -> Option<LiveRendererScanoutBufferExportStatus> {
        self.last_export_status
    }

    pub const fn last_target(&self) -> Option<LiveGbmEglFrameTargetRecord> {
        self.last_target
    }

    pub const fn last_target_lifecycle(&self) -> Option<LiveGbmEglFrameTargetLifecycleReport> {
        self.last_target_lifecycle
    }

    pub const fn context_status(&self) -> Option<NativeGbmRenderedScanoutContextStatus> {
        self.context_status
    }

    pub const fn context_ready(&self) -> bool {
        self.context.is_some()
            || matches!(
                self.context_status,
                Some(NativeGbmRenderedScanoutContextStatus::Ready)
            )
    }

    pub fn persistent_render_stats(&self) -> sophia_renderer_live::LiveNativePersistentRenderStats {
        self.worker.as_ref().map_or_else(
            || {
                self.context.as_ref().map_or_else(
                    sophia_renderer_live::LiveNativePersistentRenderStats::default,
                    NativeGbmRenderedScanoutContext::persistent_render_stats,
                )
            },
            NativeGbmRendererWorker::persistent_render_stats,
        )
    }

    pub fn discovery(&self) -> &R {
        &self.discovery
    }

    pub fn discovery_mut(&mut self) -> &mut R {
        &mut self.discovery
    }

    pub fn set_pending_cpu_frame(&mut self, frame: LiveCpuComposedFrame) {
        let checksum = cpu_frame_checksum(&frame);
        self.set_pending_cpu_frame_with_checksum(frame, checksum);
    }

    pub fn set_pending_cpu_frame_with_checksum(
        &mut self,
        frame: LiveCpuComposedFrame,
        checksum: u64,
    ) {
        self.pending_frame = Some(PendingRenderedFrame::Cpu { frame, checksum });
    }

    pub const fn pending_cpu_frame(&self) -> bool {
        matches!(self.pending_frame, Some(PendingRenderedFrame::Cpu { .. }))
    }

    pub fn set_pending_dmabuf_frame(&mut self, frame: sophia_renderer_live::LiveOwnedDmaBufFrame) {
        self.pending_frame = Some(PendingRenderedFrame::DmaBuf(frame));
    }

    pub const fn pending_dmabuf_frame(&self) -> bool {
        matches!(self.pending_frame, Some(PendingRenderedFrame::DmaBuf(_)))
    }

    pub fn set_pending_mixed_frame(
        &mut self,
        frame: sophia_renderer_live::LiveOwnedMixedCompositionFrame,
    ) {
        self.pending_frame = Some(PendingRenderedFrame::Mixed(frame));
    }

    pub const fn pending_mixed_frame(&self) -> bool {
        matches!(self.pending_frame, Some(PendingRenderedFrame::Mixed(_)))
    }

    pub const fn pending_frame(&self) -> bool {
        self.pending_frame.is_some()
            || matches!(self.worker.as_ref(), Some(worker) if worker.in_flight())
    }

    pub const fn worker_in_flight(&self) -> bool {
        matches!(self.worker.as_ref(), Some(worker) if worker.in_flight())
    }

    pub fn evict_renderer_image(
        &mut self,
        image_id: sophia_renderer_live::LiveRendererImageId,
    ) -> Result<bool, sophia_renderer_live::LiveRendererScanoutBufferExportDetail> {
        if let Some(worker) = &self.worker {
            return Ok(worker.evict_renderer_image(image_id));
        }
        self.context
            .as_mut()
            .map_or(Ok(false), |context| context.evict_renderer_image(image_id))
    }

    pub fn clear_renderer_images(
        &mut self,
    ) -> Result<usize, sophia_renderer_live::LiveRendererScanoutBufferExportDetail> {
        if let Some(worker) = &mut self.worker {
            return worker.clear_renderer_images();
        }
        self.context.as_mut().map_or(
            Ok(0),
            sophia_renderer_live::NativeGbmRenderedScanoutContext::clear_renderer_images,
        )
    }

    pub const fn cpu_frame_export_attempts(&self) -> usize {
        self.cpu_frame_export_attempts
    }

    pub const fn dmabuf_frame_export_attempts(&self) -> usize {
        self.dmabuf_frame_export_attempts
    }

    pub const fn dmabuf_frame_exports(&self) -> usize {
        self.dmabuf_frame_exports
    }

    pub const fn mixed_frame_export_attempts(&self) -> usize {
        self.mixed_frame_export_attempts
    }

    pub const fn mixed_frame_exports(&self) -> usize {
        self.mixed_frame_exports
    }

    pub const fn last_cpu_frame_checksum(&self) -> Option<u64> {
        self.last_cpu_frame_checksum
    }

    pub const fn last_cpu_frame_export_status(
        &self,
    ) -> Option<LiveRendererScanoutBufferExportStatus> {
        self.last_cpu_frame_export_status
    }

    pub fn composition_nonzero_rgb_pixels(&self) -> usize {
        if let Some(worker) = &self.worker {
            return worker.composition_nonzero_rgb_pixels();
        }
        self.context
            .as_ref()
            .map_or(0, |context| context.composition_nonzero_rgb_pixels())
    }
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl<R> LiveRenderedScanoutBufferExporter for NativeGbmRenderedScanoutBufferDiscoveryExporter<R>
where
    R: RenderDeviceDiscoveryBackend,
{
    type Owner = NativeGbmRenderedScanoutOwner;

    fn export_rendered_scanout_buffer(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
    ) -> LiveRenderedScanoutBufferExport<Self::Owner> {
        self.export_attempts = self.export_attempts.saturating_add(1);
        let target_lifecycle =
            LiveGbmEglFrameTargetLifecycleReport::from_size_update(self.last_target, target);
        self.last_target = Some(target);
        self.last_target_lifecycle = Some(target_lifecycle);

        if !target.is_valid_scanout_target() {
            self.last_export_status = Some(LiveRendererScanoutBufferExportStatus::InvalidTarget);
            return LiveRenderedScanoutBufferExport::new(
                LiveRendererScanoutBufferExportStatus::InvalidTarget,
                LiveRendererScanoutBufferExportDetail::InvalidTarget,
                None,
                None,
            );
        }

        if self.worker.is_some() {
            return self.export_from_worker(target);
        }

        if self.context.is_none() {
            self.context_open_attempts = self.context_open_attempts.saturating_add(1);
            let report = NativeGbmRenderedScanoutContext::from_backend_device_result(
                self.discovery.open_render_device(),
            );
            self.context_status = Some(report.status);
            self.context = report.context;
        }

        let Some(context) = &mut self.context else {
            let status = match self.context_status {
                Some(NativeGbmRenderedScanoutContextStatus::Degraded) => {
                    LiveRendererScanoutBufferExportStatus::Degraded
                }
                Some(NativeGbmRenderedScanoutContextStatus::Ready) => {
                    LiveRendererScanoutBufferExportStatus::Degraded
                }
                Some(NativeGbmRenderedScanoutContextStatus::Unavailable) | None => {
                    LiveRendererScanoutBufferExportStatus::Unavailable
                }
            };
            self.last_export_status = Some(status);
            return LiveRenderedScanoutBufferExport::new(
                status,
                LiveRendererScanoutBufferExportDetail::from_status(status),
                None,
                None,
            );
        };

        let report = match self.pending_frame.take() {
            Some(PendingRenderedFrame::Mixed(frame)) => {
                self.mixed_frame_export_attempts =
                    self.mixed_frame_export_attempts.saturating_add(1);
                match context.export_owned_mixed_frame_with_modifiers(
                    target,
                    &frame,
                    &self.preferred_modifiers,
                ) {
                    Ok(report) => {
                        if report.status == LiveRendererScanoutBufferExportStatus::Exported {
                            self.mixed_frame_exports = self.mixed_frame_exports.saturating_add(1);
                        }
                        report
                    }
                    Err(_) => sophia_renderer_live::NativeGbmOwnedScanoutBufferExportReport::new(
                        LiveRendererScanoutBufferExportStatus::InvalidTarget,
                        LiveRendererScanoutBufferExportDetail::InvalidTarget,
                        None,
                    ),
                }
            }
            Some(PendingRenderedFrame::DmaBuf(frame)) => {
                self.dmabuf_frame_export_attempts =
                    self.dmabuf_frame_export_attempts.saturating_add(1);
                let report = context.export_dmabuf_owned_scanout_buffer_with_modifiers(
                    target,
                    frame.as_frame(),
                    &self.preferred_modifiers,
                );
                if report.status == LiveRendererScanoutBufferExportStatus::Exported {
                    self.dmabuf_frame_exports = self.dmabuf_frame_exports.saturating_add(1);
                }
                report
            }
            Some(PendingRenderedFrame::Cpu { frame, checksum }) => {
                self.cpu_frame_export_attempts = self.cpu_frame_export_attempts.saturating_add(1);
                self.last_cpu_frame_checksum = Some(checksum);
                let report = context.export_xrgb8888_owned_scanout_buffer_with_modifiers(
                    target,
                    &frame,
                    &self.preferred_modifiers,
                );
                self.last_cpu_frame_export_status = Some(report.status);
                report
            }
            None => context.export_rendered_owned_scanout_buffer_with_modifiers(
                target,
                &self.preferred_modifiers,
            ),
        };
        let descriptor = report.buffer.as_ref().map(|buffer| buffer.descriptor());
        self.last_export_status = Some(report.status);
        LiveRenderedScanoutBufferExport::new(
            report.status,
            report.detail,
            descriptor,
            report.buffer.map(NativeGbmRenderedScanoutOwner::Inline),
        )
    }
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl<R> NativeGbmRenderedScanoutBufferDiscoveryExporter<R>
where
    R: RenderDeviceDiscoveryBackend,
{
    fn export_from_worker(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
    ) -> LiveRenderedScanoutBufferExport<NativeGbmRenderedScanoutOwner> {
        let worker = self
            .worker
            .as_mut()
            .expect("worker export path requires a renderer worker");
        match worker.poll() {
            WorkerPoll::Exported(lease) => {
                let kind = self.worker_frame_kind.take();
                match kind {
                    Some(PendingRenderedFrameKind::Cpu) => {
                        self.last_cpu_frame_export_status =
                            Some(LiveRendererScanoutBufferExportStatus::Exported);
                    }
                    Some(PendingRenderedFrameKind::DmaBuf) => {
                        self.dmabuf_frame_exports = self.dmabuf_frame_exports.saturating_add(1);
                    }
                    Some(PendingRenderedFrameKind::Mixed) => {
                        self.mixed_frame_exports = self.mixed_frame_exports.saturating_add(1);
                    }
                    None => {}
                }
                self.context_status = worker.context_status();
                self.last_export_status = Some(LiveRendererScanoutBufferExportStatus::Exported);
                let descriptor = lease.descriptor();
                return LiveRenderedScanoutBufferExport::new(
                    LiveRendererScanoutBufferExportStatus::Exported,
                    LiveRendererScanoutBufferExportDetail::Exported,
                    Some(descriptor),
                    Some(NativeGbmRenderedScanoutOwner::Worker(lease)),
                );
            }
            WorkerPoll::Failed(detail) => {
                self.worker_frame_kind = None;
                self.context_status = worker.context_status();
                self.last_export_status = Some(LiveRendererScanoutBufferExportStatus::Degraded);
                tracing::warn!("sophia_renderer_worker schema=1 status=failed detail={detail:?}");
                return LiveRenderedScanoutBufferExport::new(
                    LiveRendererScanoutBufferExportStatus::Degraded,
                    detail,
                    None,
                    None,
                );
            }
            WorkerPoll::HardStalled(age) => {
                self.worker_frame_kind = None;
                self.last_export_status = Some(LiveRendererScanoutBufferExportStatus::Degraded);
                tracing::error!(
                    "sophia_renderer_worker schema=1 status=hard_stall age_ms={} action=quarantine",
                    age.as_millis(),
                );
                return LiveRenderedScanoutBufferExport::new(
                    LiveRendererScanoutBufferExportStatus::Degraded,
                    LiveRendererScanoutBufferExportDetail::WorkerStalled,
                    None,
                    None,
                );
            }
            WorkerPoll::Pending {
                age,
                soft_stall_started,
            } => {
                if soft_stall_started {
                    tracing::warn!(
                        "sophia_renderer_worker schema=1 status=soft_stall age_ms={}",
                        age.as_millis(),
                    );
                }
                self.last_export_status = Some(LiveRendererScanoutBufferExportStatus::Pending);
                return worker_pending_export();
            }
            WorkerPoll::Idle => {}
        }

        let Some(frame) = self.pending_frame.take() else {
            self.last_export_status = Some(LiveRendererScanoutBufferExportStatus::Degraded);
            return LiveRenderedScanoutBufferExport::new(
                LiveRendererScanoutBufferExportStatus::Degraded,
                LiveRendererScanoutBufferExportDetail::RetainedBufferMissing,
                None,
                None,
            );
        };
        let kind = match &frame {
            PendingRenderedFrame::Cpu { checksum, .. } => {
                self.cpu_frame_export_attempts = self.cpu_frame_export_attempts.saturating_add(1);
                self.last_cpu_frame_checksum = Some(*checksum);
                PendingRenderedFrameKind::Cpu
            }
            PendingRenderedFrame::DmaBuf(_) => {
                self.dmabuf_frame_export_attempts =
                    self.dmabuf_frame_export_attempts.saturating_add(1);
                PendingRenderedFrameKind::DmaBuf
            }
            PendingRenderedFrame::Mixed(_) => {
                self.mixed_frame_export_attempts =
                    self.mixed_frame_export_attempts.saturating_add(1);
                PendingRenderedFrameKind::Mixed
            }
        };
        match worker.submit(target, frame, self.preferred_modifiers.clone()) {
            Ok(()) => {
                self.worker_frame_kind = Some(kind);
                self.last_export_status = Some(LiveRendererScanoutBufferExportStatus::Pending);
                worker_pending_export()
            }
            Err(detail) => {
                self.last_export_status = Some(LiveRendererScanoutBufferExportStatus::Degraded);
                LiveRenderedScanoutBufferExport::new(
                    LiveRendererScanoutBufferExportStatus::Degraded,
                    detail,
                    None,
                    None,
                )
            }
        }
    }

    pub fn worker_metrics(&self) -> Option<super::LiveRendererWorkerMetrics> {
        self.worker.as_ref().map(NativeGbmRendererWorker::metrics)
    }
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
fn worker_pending_export<Owner>() -> LiveRenderedScanoutBufferExport<Owner> {
    LiveRenderedScanoutBufferExport::new(
        LiveRendererScanoutBufferExportStatus::Pending,
        LiveRendererScanoutBufferExportDetail::WorkerPending,
        None,
        None,
    )
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
fn reduced_preferred_scanout_modifiers(mut modifiers: Vec<u64>) -> Vec<u64> {
    let mut reduced = Vec::new();
    for modifier in modifiers.drain(..) {
        if modifier == u64::MAX || reduced.contains(&modifier) {
            continue;
        }
        reduced.push(modifier);
        if reduced.len() >= MAX_PREFERRED_SCANOUT_MODIFIERS {
            break;
        }
    }
    reduced
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
const MAX_PREFERRED_SCANOUT_MODIFIERS: usize = 16;

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
fn cpu_frame_checksum(frame: &LiveCpuComposedFrame) -> u64 {
    frame
        .bytes
        .iter()
        .fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
        })
}
