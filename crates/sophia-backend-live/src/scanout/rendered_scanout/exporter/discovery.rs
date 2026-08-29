#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use super::{LiveRenderedScanoutBufferExport, LiveRenderedScanoutBufferExporter};
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use crate::api::*;

use super::worker::LiveRendererWorkerOutputKey;
use super::{
    NativeGbmRendererWorker, NativeGbmRendererWorkerCore, NativeGbmRendererWorkerScanoutLease,
    WorkerPoll,
};
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
        damage_snapshot: Option<sophia_engine::OutputFrameDamageSnapshot>,
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
    /// Which output this exporter speaks for. It names the worker's reply
    /// route and, inside a shared render context, this output's own target
    /// slots -- the inline path uses it for the latter alone.
    pub(super) output: LiveRendererWorkerOutputKey,
    context: Option<NativeGbmRenderedScanoutContext<R::Device>>,
    worker: Option<NativeGbmRendererWorker>,
    worker_frame_kind: Option<PendingRenderedFrameKind>,
    context_status: Option<NativeGbmRenderedScanoutContextStatus>,
    context_open_attempts: usize,
    export_attempts: usize,
    preferred_modifiers: Vec<u64>,
    last_target: Option<LiveGbmEglFrameTargetRecord>,
    last_target_lifecycle: Option<LiveGbmEglFrameTargetLifecycleReport>,
    pub(super) last_export_status: Option<LiveRendererScanoutBufferExportStatus>,
    pub(super) pending_frame: Option<PendingRenderedFrame>,
    direct_cpu_bootstrap_armed: bool,
    direct_cpu_bootstrap_attempts: usize,
    direct_cpu_bootstrap_exports: usize,
    cpu_frame_export_attempts: usize,
    dmabuf_frame_export_attempts: usize,
    dmabuf_frame_exports: usize,
    mixed_frame_export_attempts: usize,
    mixed_frame_exports: usize,
    last_cpu_frame_checksum: Option<u64>,
    last_cpu_frame_export_status: Option<LiveRendererScanoutBufferExportStatus>,
    /// Whether this output may hand a client buffer straight to the plane.
    ///
    /// Off unless the session enabled it. A disabled exporter never even
    /// derives a candidate, so the flag's off state is the pre-row behaviour
    /// exactly, not a different path that happens to compose.
    pub(super) direct_scanout_enabled: bool,
    /// The composed form of a frame handed out directly, kept until the
    /// submission that took it says whether it reached a screen.
    ///
    /// This is the fallback ladder's whole mechanism. A direct attempt that
    /// the driver refuses does not lose its frame: the composed form is still
    /// here, and `fall_back_from_direct` reinstalls it as pending with its
    /// proof cleared, so the retry composes instead of refusing again.
    pub(super) direct_fallback: Option<sophia_renderer_live::LiveOwnedMixedCompositionFrame>,
    /// Whether the driver has already accepted a direct commit in this
    /// eligibility episode.
    ///
    /// Cleared by every export that is not direct, which is what makes the
    /// test happen on the composition-to-direct edge rather than once per
    /// session: an overlay opening composes one frame, and that alone means
    /// the next direct frame is validated afresh. See
    /// `PresentFlipOwnership.tla`, `ReProveAfterEpisodeChange`.
    pub(super) direct_scanout_tested: bool,
    pub(super) direct_scanout_attempts: usize,
    pub(super) direct_scanout_exports: usize,
    pub(super) direct_scanout_flips: usize,
    pub(super) direct_scanout_tests: usize,
    pub(super) direct_scanout_test_rejections: usize,
    pub(super) direct_scanout_refusals: usize,
    pub(super) direct_scanout_fallbacks: usize,
    pub(super) last_direct_scanout_refusal: Option<sophia_renderer_live::LiveDirectScanoutRefusal>,
    /// Frames the latest-wins cell dropped without rendering them.
    ///
    /// Holding one newest frame is the point, so a supersession is ordinary
    /// backpressure rather than a fault. It was invisible: the cell is an
    /// `Option`, so a newer frame overwrote an older one silently and a
    /// returned deferred frame was discarded silently. Counting it is what
    /// distinguishes a session that kept one frame pending from one that never
    /// had a second frame to keep.
    pending_frame_supersessions: usize,
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
#[derive(Debug)]
pub enum NativeGbmRenderedScanoutOwner {
    Inline(NativeGbmOwnedScanoutBuffer),
    Worker(NativeGbmRendererWorkerScanoutLease),
    /// A client's own buffer, on its way to the plane uncomposed.
    ///
    /// Unlike the other two this owns no compositor memory: it holds the
    /// duplicated plane descriptors and nothing else. It is also the only
    /// variant whose buffer lives in another process, which is why it reports
    /// `shares_kms_drm_file() == false` and takes the PRIME transport -- the
    /// branch that imports descriptors into the KMS file rather than handing
    /// over a GEM handle that only the renderer's file knows.
    Direct(sophia_renderer_live::LiveDirectScanoutBuffer),
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
            output: LiveRendererWorkerOutputKey::from_raw(0),
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
            direct_cpu_bootstrap_armed: false,
            direct_cpu_bootstrap_attempts: 0,
            direct_cpu_bootstrap_exports: 0,
            cpu_frame_export_attempts: 0,
            dmabuf_frame_export_attempts: 0,
            dmabuf_frame_exports: 0,
            mixed_frame_export_attempts: 0,
            mixed_frame_exports: 0,
            last_cpu_frame_checksum: None,
            last_cpu_frame_export_status: None,
            direct_scanout_enabled: false,
            direct_scanout_tested: false,
            direct_fallback: None,
            direct_scanout_attempts: 0,
            direct_scanout_exports: 0,
            direct_scanout_flips: 0,
            direct_scanout_tests: 0,
            direct_scanout_test_rejections: 0,
            direct_scanout_refusals: 0,
            direct_scanout_fallbacks: 0,
            last_direct_scanout_refusal: None,
            pending_frame_supersessions: 0,
        }
    }

    pub fn new_worker(discovery: R) -> std::io::Result<Self>
    where
        R::Device: Send + 'static,
    {
        let device = discovery.open_render_device();
        let mut exporter = Self::new(discovery);
        let output = exporter.output;
        exporter.context_open_attempts = 1;
        exporter.worker = Some(NativeGbmRendererWorker::spawn(device, output)?);
        Ok(exporter)
    }

    /// Name the output this exporter speaks for.
    ///
    /// Set before a worker is attached. Two exporters sharing a core must not
    /// share a key: it is the only thing separating their replies, their
    /// slots, and their leases.
    pub fn set_output(&mut self, output: LiveRendererWorkerOutputKey) {
        self.output = output;
    }

    pub const fn output(&self) -> LiveRendererWorkerOutputKey {
        self.output
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
            self.output,
        )?);
        self.context = None;
        self.context_status = None;
        Ok(())
    }

    /// Attach this output to a worker shared with the rest of its device
    /// group, rather than giving it a thread of its own.
    pub fn attach_shared_worker(&mut self, core: &std::sync::Arc<NativeGbmRendererWorkerCore>) {
        if self.worker.is_some() {
            return;
        }
        self.context_open_attempts = self.context_open_attempts.saturating_add(1);
        self.worker = Some(core.attach(self.output));
        self.context = None;
        self.context_status = None;
    }

    /// Arms the next CPU export as a direct-GBM-only bootstrap.
    ///
    /// This is intentionally incompatible with either renderer owner. Mirror
    /// initialization must never create an inline EGL context and then replace
    /// it with a worker while its first scanout buffer remains displayed.
    pub fn arm_direct_cpu_bootstrap(&mut self) -> Result<(), &'static str> {
        if self.worker.is_some() || self.context.is_some() || self.direct_cpu_bootstrap_armed {
            return Err("direct CPU bootstrap requires an uninitialized renderer owner");
        }
        self.direct_cpu_bootstrap_armed = true;
        Ok(())
    }

    pub const fn worker_enabled(&self) -> bool {
        self.worker.is_some()
    }

    pub const fn direct_cpu_bootstrap_attempts(&self) -> usize {
        self.direct_cpu_bootstrap_attempts
    }

    pub const fn direct_cpu_bootstrap_exports(&self) -> usize {
        self.direct_cpu_bootstrap_exports
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
        self.set_pending_cpu_frame_with_damage(frame, checksum, None);
    }

    pub fn set_pending_cpu_frame_with_damage(
        &mut self,
        frame: LiveCpuComposedFrame,
        checksum: u64,
        damage_snapshot: Option<sophia_engine::OutputFrameDamageSnapshot>,
    ) {
        self.replace_pending_frame(PendingRenderedFrame::Cpu {
            frame,
            checksum,
            damage_snapshot,
        });
    }

    /// Install the newest frame, counting whatever it displaced.
    pub(super) fn replace_pending_frame(&mut self, frame: PendingRenderedFrame) {
        if self.pending_frame.is_some() {
            self.pending_frame_supersessions = self.pending_frame_supersessions.saturating_add(1);
        }
        self.pending_frame = Some(frame);
    }

    /// One line per direct-scanout episode transition.
    ///
    /// The whole episode is observable from these: which output, which scene
    /// generation, and every step from a proven frame through the validating
    /// commit to a flip or a fall back. A physical gate asserts the shape of
    /// the sequence rather than a single count, because the counts alone
    /// cannot say whether a flip happened before its test or a fallback lost
    /// its frame.
    pub const fn pending_frame_supersessions(&self) -> usize {
        self.pending_frame_supersessions
    }

    pub const fn pending_cpu_frame(&self) -> bool {
        matches!(self.pending_frame, Some(PendingRenderedFrame::Cpu { .. }))
    }

    pub fn set_pending_dmabuf_frame(&mut self, frame: sophia_renderer_live::LiveOwnedDmaBufFrame) {
        self.replace_pending_frame(PendingRenderedFrame::DmaBuf(frame));
    }

    pub const fn pending_dmabuf_frame(&self) -> bool {
        matches!(self.pending_frame, Some(PendingRenderedFrame::DmaBuf(_)))
    }

    pub fn set_pending_mixed_frame(
        &mut self,
        frame: sophia_renderer_live::LiveOwnedMixedCompositionFrame,
    ) {
        self.replace_pending_frame(PendingRenderedFrame::Mixed(frame));
    }

    pub const fn pending_mixed_frame(&self) -> bool {
        matches!(self.pending_frame, Some(PendingRenderedFrame::Mixed(_)))
    }

    pub const fn pending_frame(&self) -> bool {
        self.pending_frame.is_some()
            || matches!(self.worker.as_ref(), Some(worker) if worker.in_flight())
    }

    /// Discards work that has not crossed into the renderer worker.
    ///
    /// An in-flight command must still be polled so its resulting lease can be
    /// released. Returning false makes that ownership distinction explicit to
    /// topology-abort code.
    pub fn discard_pending_frame(&mut self) -> bool {
        if self.worker_in_flight() {
            return false;
        }
        self.pending_frame.take().is_some()
    }

    pub const fn worker_in_flight(&self) -> bool {
        matches!(self.worker.as_ref(), Some(worker) if worker.in_flight())
    }

    pub const fn renderer_image_owner_initialized(&self) -> bool {
        self.worker.is_some() || self.context.is_some()
    }

    pub fn evict_renderer_image(
        &mut self,
        image_id: sophia_renderer_live::LiveRendererImageId,
    ) -> Result<bool, sophia_renderer_live::LiveRendererScanoutBufferExportDetail> {
        if let Some(worker) = &self.worker {
            return worker.evict_renderer_image(image_id);
        }
        self.context
            .as_mut()
            .map_or(Ok(false), |context| context.evict_renderer_image(image_id))
    }

    pub fn promote_renderer_image(
        &mut self,
        image_id: sophia_renderer_live::LiveRendererImageId,
    ) -> Result<bool, sophia_renderer_live::LiveRendererScanoutBufferExportDetail> {
        if let Some(worker) = &self.worker {
            return worker.promote_renderer_image(image_id);
        }
        self.context.as_mut().map_or(Ok(false), |context| {
            context.promote_renderer_image(image_id)
        })
    }

    pub fn rollback_renderer_image(
        &mut self,
        image_id: sophia_renderer_live::LiveRendererImageId,
    ) -> Result<bool, sophia_renderer_live::LiveRendererScanoutBufferExportDetail> {
        if let Some(worker) = &self.worker {
            return worker.rollback_renderer_image(image_id);
        }
        self.context.as_mut().map_or(Ok(false), |context| {
            context.rollback_renderer_image(image_id)
        })
    }

    pub fn export_promoted_renderer_image(
        &mut self,
        image_id: sophia_renderer_live::LiveRendererImageId,
    ) -> Result<
        Option<sophia_renderer_live::LiveRendererImageSnapshot>,
        sophia_renderer_live::LiveRendererScanoutBufferExportDetail,
    > {
        self.settle_worker_for_image_maintenance()?;
        if let Some(worker) = &mut self.worker {
            return worker.export_promoted_renderer_image(image_id);
        }
        self.context.as_ref().map_or(Ok(None), |context| {
            context.export_promoted_renderer_image(image_id)
        })
    }

    pub fn restore_promoted_renderer_image(
        &mut self,
        snapshot: sophia_renderer_live::LiveRendererImageSnapshot,
    ) -> Result<bool, sophia_renderer_live::LiveRendererScanoutBufferExportDetail> {
        if let Some(worker) = &mut self.worker {
            return worker.restore_promoted_renderer_image(snapshot);
        }
        self.context.as_mut().map_or(Ok(false), |context| {
            context.restore_promoted_renderer_image(snapshot)
        })
    }

    pub fn clear_renderer_images(
        &mut self,
    ) -> Result<usize, sophia_renderer_live::LiveRendererScanoutBufferExportDetail> {
        self.settle_worker_for_image_maintenance()?;
        if let Some(worker) = &mut self.worker {
            return worker.clear_renderer_images();
        }
        self.context.as_mut().map_or(
            Ok(0),
            sophia_renderer_live::NativeGbmRenderedScanoutContext::clear_renderer_images,
        )
    }

    fn settle_worker_for_image_maintenance(
        &mut self,
    ) -> Result<(), sophia_renderer_live::LiveRendererScanoutBufferExportDetail> {
        let Some(worker) = &mut self.worker else {
            return Ok(());
        };
        // Handoff or teardown has detached the skipped Present. Collect its
        // worker result before touching the older promoted image set.
        if worker.discard_in_flight_for_maintenance()? {
            self.worker_frame_kind = None;
            tracing::info!("sophia_renderer_worker schema=1 status=maintenance_frame_discarded");
        }
        Ok(())
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
        self.context.as_ref().map_or(0, |context| {
            context.composition_nonzero_rgb_pixels(self.output.target_set())
        })
    }
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl<R> LiveRenderedScanoutBufferExporter for NativeGbmRenderedScanoutBufferDiscoveryExporter<R>
where
    R: RenderDeviceDiscoveryBackend,
{
    type Owner = NativeGbmRenderedScanoutOwner;

    fn direct_scanout_test_required(&self) -> bool {
        !self.direct_scanout_tested
    }

    fn record_direct_scanout_test(&mut self, accepted: bool) {
        self.direct_scanout_tests = self.direct_scanout_tests.saturating_add(1);
        self.record_direct_scanout_episode(
            if accepted { "test_passed" } else { "test_rejected" },
            self.outstanding_direct_generation(),
            "none",
        );
        if accepted {
            self.direct_scanout_tested = true;
        } else {
            self.direct_scanout_test_rejections =
                self.direct_scanout_test_rejections.saturating_add(1);
            self.direct_scanout_tested = false;
        }
    }

    fn commit_direct_scanout(&mut self) {
        // Read before committing: committing drops the composed form the
        // identity is read from.
        let generation = self.outstanding_direct_generation();
        Self::commit_direct_scanout(self);
        self.record_direct_scanout_episode("flipped", generation, "none");
    }

    fn fall_back_from_direct(&mut self) -> bool {
        // A refusal ends the episode: whatever the driver objected to, the
        // next direct frame is a fresh question and gets a fresh test.
        self.direct_scanout_tested = false;
        let generation = self.outstanding_direct_generation();
        let fell_back = Self::fall_back_from_direct(self);
        if fell_back {
            self.record_direct_scanout_episode("fell_back", generation, "none");
        }
        fell_back
    }

    fn export_rendered_scanout_buffer(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
    ) -> LiveRenderedScanoutBufferExport<Self::Owner> {
        self.export_attempts = self.export_attempts.saturating_add(1);
        // Every export ends the eligibility episode unless it is itself
        // direct; the direct branch below restores this. Clearing first and
        // restoring in one place means a path added later cannot forget to
        // end the episode, only to continue it -- and forgetting to continue
        // costs one validating commit, while forgetting to end one costs a
        // flip the driver never agreed to.
        let continuing_episode = self.direct_scanout_tested;
        self.direct_scanout_tested = false;
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

        if self.direct_cpu_bootstrap_armed {
            self.direct_cpu_bootstrap_armed = false;
            self.direct_cpu_bootstrap_attempts =
                self.direct_cpu_bootstrap_attempts.saturating_add(1);
            let report = match self.pending_frame.take() {
                Some(PendingRenderedFrame::Cpu {
                    frame, checksum, ..
                }) => {
                    self.cpu_frame_export_attempts =
                        self.cpu_frame_export_attempts.saturating_add(1);
                    self.last_cpu_frame_checksum = Some(checksum);
                    sophia_renderer_live::NativeGbmScanoutBufferExporter::export_direct_cpu_owned_scanout_buffer_from_backend_device_result(
                        self.discovery.open_render_device(),
                        target,
                        &frame,
                    )
                }
                Some(frame) => {
                    self.pending_frame = Some(frame);
                    sophia_renderer_live::NativeGbmOwnedScanoutBufferExportReport::new(
                        LiveRendererScanoutBufferExportStatus::InvalidTarget,
                        LiveRendererScanoutBufferExportDetail::InvalidTarget,
                        None,
                    )
                }
                None => sophia_renderer_live::NativeGbmOwnedScanoutBufferExportReport::new(
                    LiveRendererScanoutBufferExportStatus::Degraded,
                    LiveRendererScanoutBufferExportDetail::RetainedBufferMissing,
                    None,
                ),
            };
            if report.status == LiveRendererScanoutBufferExportStatus::Exported {
                self.direct_cpu_bootstrap_exports =
                    self.direct_cpu_bootstrap_exports.saturating_add(1);
            }
            let descriptor = report.buffer.as_ref().map(|buffer| buffer.descriptor());
            self.last_cpu_frame_export_status = Some(report.status);
            self.last_export_status = Some(report.status);
            return LiveRenderedScanoutBufferExport::new(
                report.status,
                report.detail,
                descriptor,
                report.buffer.map(NativeGbmRenderedScanoutOwner::Inline),
            );
        }

        // The direct path, ahead of both the worker and the inline context
        // because it needs neither: no render happens, no target slot is
        // acquired, and the buffer that reaches the plane is the client's.
        if let Some(export) = self.try_direct_scanout_export(target, continuing_episode) {
            return export;
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
                    Err(sophia_renderer_live::LiveMixedCompositionError::Renderer(detail)) => {
                        sophia_renderer_live::NativeGbmOwnedScanoutBufferExportReport::new(
                            detail.status(),
                            detail,
                            None,
                        )
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
            Some(PendingRenderedFrame::Cpu {
                frame, checksum, ..
            }) => {
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
            WorkerPoll::Deferred(frame) => {
                self.worker_frame_kind = None;
                if self.pending_frame.is_none() {
                    self.pending_frame = Some(frame);
                } else {
                    // A newer frame arrived while this one waited for a slot.
                    // Latest-wins discards the returned one; it is the same
                    // supersession as an overwrite and is counted as one.
                    self.pending_frame_supersessions =
                        self.pending_frame_supersessions.saturating_add(1);
                }
                self.context_status = worker.context_status();
                self.last_export_status = Some(LiveRendererScanoutBufferExportStatus::Pending);
                return worker_pending_export();
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
