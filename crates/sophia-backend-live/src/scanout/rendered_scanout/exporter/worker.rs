use super::discovery::PendingRenderedFrame;
use super::frame_slots::{
    LiveRendererFrameSlotAcquire, LiveRendererFrameSlotMetrics, LiveRendererFrameSlotMetricsHandle,
    LiveRendererFrameSlotPool, LiveRendererFrameSlotToken,
};
use crate::api::*;
use sophia_renderer_live::{
    LiveMixedCompositionError, LiveNativePersistentRenderStats, LiveRendererImageId,
    LiveRendererImageSnapshot, LiveRendererScanoutBufferDescriptor,
    LiveRendererScanoutBufferExportDetail, LiveRendererScanoutBufferExportStatus,
    NativeGbmOwnedScanoutBuffer, NativeGbmOwnedScanoutBufferExportReport,
    NativeGbmRenderedScanoutContext, NativeGbmRenderedScanoutContextStatus,
};
use std::collections::BTreeMap;
use std::io;
use std::os::fd::AsFd;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{
    Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel,
};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

const WORKER_COMMAND_CAPACITY: usize = 32;
const WORKER_RESULT_CAPACITY: usize = 2;
const WORKER_FREE_CPU_BUFFER_CAPACITY: usize = 3;
pub const LIVE_RENDERER_WORKER_SOFT_STALL: Duration = Duration::from_millis(100);
pub const LIVE_RENDERER_WORKER_HARD_STALL: Duration = Duration::from_secs(1);
const WORKER_MAINTENANCE_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LiveRendererWorkerRequestId(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LiveRendererWorkerLeaseId(u64);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LiveRendererWorkerMetrics {
    pub requests: usize,
    pub completions: usize,
    pub failures: usize,
    pub soft_stalls: usize,
    pub hard_stalls: usize,
    pub release_enqueue_failures: usize,
    pub max_request_age: Duration,
    pub frame_slots: LiveRendererFrameSlotMetrics,
}

#[derive(Debug)]
pub struct NativeGbmRendererWorkerScanoutLease {
    descriptor: LiveRendererScanoutBufferDescriptor,
    lease_id: LiveRendererWorkerLeaseId,
    slot_token: LiveRendererFrameSlotToken,
    command_sender: SyncSender<WorkerCommand>,
    release_enqueue_failures: Arc<AtomicUsize>,
}

impl NativeGbmRendererWorkerScanoutLease {
    pub const fn descriptor(&self) -> LiveRendererScanoutBufferDescriptor {
        self.descriptor
    }

    pub const fn lease_id(&self) -> LiveRendererWorkerLeaseId {
        self.lease_id
    }

    pub const fn slot_token(&self) -> LiveRendererFrameSlotToken {
        self.slot_token
    }

    pub fn export_scanout_dma_buf_fds(
        &self,
    ) -> io::Result<Option<(u8, [Option<std::os::fd::OwnedFd>; 4])>> {
        Ok(None)
    }
}

impl Drop for NativeGbmRendererWorkerScanoutLease {
    fn drop(&mut self) {
        if self
            .command_sender
            .try_send(WorkerCommand::Release {
                lease_id: self.lease_id,
                slot_token: self.slot_token,
            })
            .is_err()
        {
            self.release_enqueue_failures
                .fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub(super) struct NativeGbmRendererWorker {
    command_sender: SyncSender<WorkerCommand>,
    result_receiver: Receiver<WorkerResult>,
    thread: Option<thread::JoinHandle<()>>,
    next_request_id: u64,
    in_flight: Option<InFlightRequest>,
    context_status: Option<NativeGbmRenderedScanoutContextStatus>,
    persistent_render_stats: LiveNativePersistentRenderStats,
    composition_nonzero_rgb_pixels: usize,
    release_enqueue_failures: Arc<AtomicUsize>,
    frame_slot_metrics: LiveRendererFrameSlotMetricsHandle,
    metrics: LiveRendererWorkerMetrics,
    quarantined: bool,
}

impl NativeGbmRendererWorker {
    pub fn spawn<D>(device: io::Result<D>) -> io::Result<Self>
    where
        D: AsFd + Send + 'static,
    {
        let (command_sender, command_receiver) = sync_channel(WORKER_COMMAND_CAPACITY);
        let (result_sender, result_receiver) = sync_channel(WORKER_RESULT_CAPACITY);
        let frame_slot_metrics = LiveRendererFrameSlotMetricsHandle::default();
        let worker_frame_slot_metrics = frame_slot_metrics.clone();
        let thread = thread::Builder::new()
            .name("sophia-render-gpu".to_owned())
            .spawn(move || {
                run_worker(
                    device,
                    command_receiver,
                    result_sender,
                    worker_frame_slot_metrics,
                )
            })?;
        Ok(Self {
            command_sender,
            result_receiver,
            thread: Some(thread),
            next_request_id: 1,
            in_flight: None,
            context_status: None,
            persistent_render_stats: LiveNativePersistentRenderStats::default(),
            composition_nonzero_rgb_pixels: 0,
            release_enqueue_failures: Arc::new(AtomicUsize::new(0)),
            frame_slot_metrics,
            metrics: LiveRendererWorkerMetrics::default(),
            quarantined: false,
        })
    }

    pub const fn in_flight(&self) -> bool {
        self.in_flight.is_some()
    }

    pub const fn context_status(&self) -> Option<NativeGbmRenderedScanoutContextStatus> {
        self.context_status
    }

    pub const fn persistent_render_stats(&self) -> LiveNativePersistentRenderStats {
        self.persistent_render_stats
    }

    pub const fn composition_nonzero_rgb_pixels(&self) -> usize {
        self.composition_nonzero_rgb_pixels
    }

    pub fn metrics(&self) -> LiveRendererWorkerMetrics {
        LiveRendererWorkerMetrics {
            release_enqueue_failures: self.release_enqueue_failures.load(Ordering::Relaxed),
            frame_slots: self.frame_slot_metrics.snapshot(),
            ..self.metrics
        }
    }

    pub fn submit(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
        frame: PendingRenderedFrame,
        preferred_modifiers: Vec<u64>,
    ) -> Result<(), LiveRendererScanoutBufferExportDetail> {
        if self.quarantined || self.in_flight.is_some() {
            return Err(LiveRendererScanoutBufferExportDetail::WorkerPending);
        }
        let request_id = LiveRendererWorkerRequestId(self.next_request_id);
        self.next_request_id = self.next_request_id.saturating_add(1);
        let command = WorkerCommand::Render {
            request_id,
            target,
            frame,
            preferred_modifiers,
        };
        self.command_sender
            .try_send(command)
            .map_err(|error| match error {
                TrySendError::Full(_) => LiveRendererScanoutBufferExportDetail::WorkerQueueFull,
                TrySendError::Disconnected(_) => {
                    LiveRendererScanoutBufferExportDetail::WorkerDisconnected
                }
            })?;
        self.in_flight = Some(InFlightRequest {
            request_id,
            submitted_at: Instant::now(),
            soft_stall_reported: false,
        });
        self.metrics.requests = self.metrics.requests.saturating_add(1);
        if trace_worker_request(request_id) {
            tracing::debug!(
                "sophia_renderer_worker schema=2 status=request_submitted request={} requests={}",
                request_id.0,
                self.metrics.requests,
            );
        }
        Ok(())
    }

    pub fn poll(&mut self) -> WorkerPoll {
        let Some(mut in_flight) = self.in_flight.take() else {
            return WorkerPoll::Idle;
        };
        match self.result_receiver.try_recv() {
            Ok(result) => {
                self.in_flight = None;
                let age = in_flight.submitted_at.elapsed();
                self.metrics.max_request_age = self.metrics.max_request_age.max(age);
                self.context_status = Some(result.context_status);
                self.persistent_render_stats = result.persistent_render_stats;
                self.composition_nonzero_rgb_pixels = result.composition_nonzero_rgb_pixels;
                if trace_worker_request(in_flight.request_id) {
                    tracing::debug!(
                        "sophia_renderer_worker schema=2 status=request_received request={} expected={} age_ms={}",
                        result.request_id.0,
                        in_flight.request_id.0,
                        age.as_millis(),
                    );
                }
                if result.request_id != in_flight.request_id {
                    self.metrics.failures = self.metrics.failures.saturating_add(1);
                    self.quarantined = true;
                    return WorkerPoll::Failed(
                        LiveRendererScanoutBufferExportDetail::WorkerDisconnected,
                    );
                }
                match result.outcome {
                    WorkerOutcome::Exported {
                        descriptor,
                        lease_id,
                        slot_token,
                    } => {
                        self.metrics.completions = self.metrics.completions.saturating_add(1);
                        WorkerPoll::Exported(NativeGbmRendererWorkerScanoutLease {
                            descriptor,
                            lease_id,
                            slot_token,
                            command_sender: self.command_sender.clone(),
                            release_enqueue_failures: Arc::clone(&self.release_enqueue_failures),
                        })
                    }
                    WorkerOutcome::Failed(detail) => {
                        self.metrics.failures = self.metrics.failures.saturating_add(1);
                        WorkerPoll::Failed(detail)
                    }
                    WorkerOutcome::Deferred(frame) => WorkerPoll::Deferred(frame),
                }
            }
            Err(TryRecvError::Disconnected) => {
                self.in_flight = None;
                self.quarantined = true;
                self.metrics.failures = self.metrics.failures.saturating_add(1);
                WorkerPoll::Failed(LiveRendererScanoutBufferExportDetail::WorkerDisconnected)
            }
            Err(TryRecvError::Empty) => {
                let age = in_flight.submitted_at.elapsed();
                self.metrics.max_request_age = self.metrics.max_request_age.max(age);
                if age >= LIVE_RENDERER_WORKER_HARD_STALL {
                    self.in_flight = None;
                    self.quarantined = true;
                    self.metrics.hard_stalls = self.metrics.hard_stalls.saturating_add(1);
                    self.metrics.failures = self.metrics.failures.saturating_add(1);
                    WorkerPoll::HardStalled(age)
                } else {
                    let mut soft_stall_started = false;
                    if age >= LIVE_RENDERER_WORKER_SOFT_STALL && !in_flight.soft_stall_reported {
                        in_flight.soft_stall_reported = true;
                        soft_stall_started = true;
                        self.metrics.soft_stalls = self.metrics.soft_stalls.saturating_add(1);
                    }
                    self.in_flight = Some(in_flight);
                    WorkerPoll::Pending {
                        age,
                        soft_stall_started,
                    }
                }
            }
        }
    }

    pub fn evict_renderer_image(
        &self,
        image_id: LiveRendererImageId,
    ) -> Result<bool, LiveRendererScanoutBufferExportDetail> {
        self.renderer_image_transition(|completion_sender| WorkerCommand::Evict {
            image_id,
            completion_sender,
        })
    }

    pub fn promote_renderer_image(
        &self,
        image_id: LiveRendererImageId,
    ) -> Result<bool, LiveRendererScanoutBufferExportDetail> {
        self.renderer_image_transition(|completion_sender| WorkerCommand::Promote {
            image_id,
            completion_sender,
        })
    }

    pub fn rollback_renderer_image(
        &self,
        image_id: LiveRendererImageId,
    ) -> Result<bool, LiveRendererScanoutBufferExportDetail> {
        self.renderer_image_transition(|completion_sender| WorkerCommand::Rollback {
            image_id,
            completion_sender,
        })
    }

    pub fn export_promoted_renderer_image(
        &mut self,
        image_id: LiveRendererImageId,
    ) -> Result<Option<LiveRendererImageSnapshot>, LiveRendererScanoutBufferExportDetail> {
        if self.in_flight.is_some() {
            return Err(LiveRendererScanoutBufferExportDetail::WorkerPending);
        }
        let (completion_sender, completion_receiver) = sync_channel(1);
        self.command_sender
            .try_send(WorkerCommand::ExportPromotedImage {
                image_id,
                completion_sender,
            })
            .map_err(reduce_worker_command_send_error)?;
        completion_receiver
            .recv_timeout(WORKER_MAINTENANCE_TIMEOUT)
            .map_err(reduce_worker_maintenance_receive_error)?
    }

    pub fn restore_promoted_renderer_image(
        &mut self,
        snapshot: LiveRendererImageSnapshot,
    ) -> Result<bool, LiveRendererScanoutBufferExportDetail> {
        if self.in_flight.is_some() {
            return Err(LiveRendererScanoutBufferExportDetail::WorkerPending);
        }
        let (completion_sender, completion_receiver) = sync_channel(1);
        self.command_sender
            .try_send(WorkerCommand::RestorePromotedImage {
                snapshot,
                completion_sender,
            })
            .map_err(reduce_worker_command_send_error)?;
        let completion = completion_receiver
            .recv_timeout(WORKER_MAINTENANCE_TIMEOUT)
            .map_err(reduce_worker_maintenance_receive_error)?;
        self.persistent_render_stats = completion.persistent_render_stats;
        completion.result
    }

    fn renderer_image_transition(
        &self,
        command: impl FnOnce(
            SyncSender<Result<bool, LiveRendererScanoutBufferExportDetail>>,
        ) -> WorkerCommand,
    ) -> Result<bool, LiveRendererScanoutBufferExportDetail> {
        let (completion_sender, completion_receiver) = sync_channel(1);
        self.command_sender
            .try_send(command(completion_sender))
            .map_err(|error| match error {
                TrySendError::Full(_) => LiveRendererScanoutBufferExportDetail::WorkerQueueFull,
                TrySendError::Disconnected(_) => {
                    LiveRendererScanoutBufferExportDetail::WorkerDisconnected
                }
            })?;
        completion_receiver
            .recv_timeout(WORKER_MAINTENANCE_TIMEOUT)
            .map_err(|error| match error {
                RecvTimeoutError::Timeout => LiveRendererScanoutBufferExportDetail::WorkerStalled,
                RecvTimeoutError::Disconnected => {
                    LiveRendererScanoutBufferExportDetail::WorkerDisconnected
                }
            })?
    }

    pub fn discard_in_flight_for_maintenance(
        &mut self,
    ) -> Result<bool, LiveRendererScanoutBufferExportDetail> {
        if self.in_flight.is_none() {
            return Ok(false);
        }
        let deadline = Instant::now() + WORKER_MAINTENANCE_TIMEOUT;
        loop {
            match self.poll() {
                WorkerPoll::Idle => return Ok(false),
                WorkerPoll::Exported(lease) => {
                    drop(lease);
                    return Ok(true);
                }
                WorkerPoll::Deferred(_) => return Ok(true),
                WorkerPoll::Failed(detail) => return Err(detail),
                WorkerPoll::HardStalled(_) => {
                    return Err(LiveRendererScanoutBufferExportDetail::WorkerStalled);
                }
                WorkerPoll::Pending { .. } => {
                    if Instant::now() >= deadline {
                        return Err(LiveRendererScanoutBufferExportDetail::WorkerStalled);
                    }
                    thread::sleep(Duration::from_millis(1));
                }
            }
        }
    }

    pub fn clear_renderer_images(
        &mut self,
    ) -> Result<usize, LiveRendererScanoutBufferExportDetail> {
        if self.in_flight.is_some() {
            return Err(LiveRendererScanoutBufferExportDetail::WorkerPending);
        }
        let (completion_sender, completion_receiver) = sync_channel(1);
        self.command_sender
            .try_send(WorkerCommand::ClearImages { completion_sender })
            .map_err(|error| match error {
                TrySendError::Full(_) => LiveRendererScanoutBufferExportDetail::WorkerQueueFull,
                TrySendError::Disconnected(_) => {
                    LiveRendererScanoutBufferExportDetail::WorkerDisconnected
                }
            })?;
        let completion = completion_receiver
            .recv_timeout(WORKER_MAINTENANCE_TIMEOUT)
            .map_err(|error| match error {
                RecvTimeoutError::Timeout => LiveRendererScanoutBufferExportDetail::WorkerStalled,
                RecvTimeoutError::Disconnected => {
                    LiveRendererScanoutBufferExportDetail::WorkerDisconnected
                }
            })?;
        self.persistent_render_stats = completion.persistent_render_stats;
        completion.result
    }
}

impl Drop for NativeGbmRendererWorker {
    fn drop(&mut self) {
        let mut shutdown = WorkerCommand::Shutdown;
        loop {
            match self.command_sender.try_send(shutdown) {
                Ok(()) | Err(TrySendError::Disconnected(_)) => break,
                Err(TrySendError::Full(command)) => {
                    shutdown = command;
                    while self.result_receiver.try_recv().is_ok() {}
                    thread::yield_now();
                }
            }
        }
        if let Some(thread) = self.thread.take() {
            while !thread.is_finished() {
                while self.result_receiver.try_recv().is_ok() {}
                std::thread::sleep(Duration::from_millis(1));
            }
            let _ = thread.join();
        }
    }
}

pub(super) enum WorkerPoll {
    Idle,
    Pending {
        age: Duration,
        soft_stall_started: bool,
    },
    Exported(NativeGbmRendererWorkerScanoutLease),
    Deferred(PendingRenderedFrame),
    Failed(LiveRendererScanoutBufferExportDetail),
    HardStalled(Duration),
}

struct InFlightRequest {
    request_id: LiveRendererWorkerRequestId,
    submitted_at: Instant,
    soft_stall_reported: bool,
}

enum WorkerCommand {
    Render {
        request_id: LiveRendererWorkerRequestId,
        target: LiveGbmEglFrameTargetRecord,
        frame: PendingRenderedFrame,
        preferred_modifiers: Vec<u64>,
    },
    Evict {
        image_id: LiveRendererImageId,
        completion_sender: SyncSender<Result<bool, LiveRendererScanoutBufferExportDetail>>,
    },
    Promote {
        image_id: LiveRendererImageId,
        completion_sender: SyncSender<Result<bool, LiveRendererScanoutBufferExportDetail>>,
    },
    Rollback {
        image_id: LiveRendererImageId,
        completion_sender: SyncSender<Result<bool, LiveRendererScanoutBufferExportDetail>>,
    },
    ExportPromotedImage {
        image_id: LiveRendererImageId,
        completion_sender: SyncSender<
            Result<Option<LiveRendererImageSnapshot>, LiveRendererScanoutBufferExportDetail>,
        >,
    },
    RestorePromotedImage {
        snapshot: LiveRendererImageSnapshot,
        completion_sender: SyncSender<WorkerRestoreImageResult>,
    },
    ClearImages {
        completion_sender: SyncSender<WorkerMaintenanceResult>,
    },
    Release {
        lease_id: LiveRendererWorkerLeaseId,
        slot_token: LiveRendererFrameSlotToken,
    },
    Shutdown,
}

struct WorkerResult {
    request_id: LiveRendererWorkerRequestId,
    context_status: NativeGbmRenderedScanoutContextStatus,
    persistent_render_stats: LiveNativePersistentRenderStats,
    composition_nonzero_rgb_pixels: usize,
    outcome: WorkerOutcome,
}

struct WorkerMaintenanceResult {
    result: Result<usize, LiveRendererScanoutBufferExportDetail>,
    persistent_render_stats: LiveNativePersistentRenderStats,
}

struct WorkerRestoreImageResult {
    result: Result<bool, LiveRendererScanoutBufferExportDetail>,
    persistent_render_stats: LiveNativePersistentRenderStats,
}

enum WorkerOutcome {
    Exported {
        descriptor: LiveRendererScanoutBufferDescriptor,
        lease_id: LiveRendererWorkerLeaseId,
        slot_token: LiveRendererFrameSlotToken,
    },
    Deferred(PendingRenderedFrame),
    Failed(LiveRendererScanoutBufferExportDetail),
}

fn run_worker<D>(
    device: io::Result<D>,
    command_receiver: Receiver<WorkerCommand>,
    result_sender: SyncSender<WorkerResult>,
    frame_slot_metrics: LiveRendererFrameSlotMetricsHandle,
) where
    D: AsFd + Send + 'static,
{
    let report = NativeGbmRenderedScanoutContext::from_backend_device_result(device);
    let context_status = report.status;
    let mut context = report.context;
    let mut leases = BTreeMap::<LiveRendererWorkerLeaseId, WorkerLeaseBuffer>::new();
    let mut free_cpu_buffers = Vec::<ReusableCpuBuffer>::new();
    let mut next_lease_id = 1_u64;
    let mut frame_slots = LiveRendererFrameSlotPool::with_metrics(frame_slot_metrics);
    let mut slot_damage = WorkerSlotDamage::new();

    while let Ok(command) = command_receiver.recv() {
        match command {
            WorkerCommand::Render {
                request_id,
                target,
                frame,
                preferred_modifiers,
            } => {
                let frame_kind = pending_frame_kind_name(&frame);
                if trace_worker_request(request_id) {
                    tracing::debug!(
                        "sophia_renderer_worker schema=2 status=render_started request={} frame_kind={frame_kind}",
                        request_id.0,
                    );
                }
                let outcome = context.as_mut().map_or_else(
                    || {
                        WorkerOutcome::Failed(
                            LiveRendererScanoutBufferExportDetail::BackendDeviceUnavailable,
                        )
                    },
                    |context| {
                        render_frame(
                            context,
                            target,
                            frame,
                            &preferred_modifiers,
                            &mut leases,
                            &mut free_cpu_buffers,
                            &mut next_lease_id,
                            &mut frame_slots,
                            &mut slot_damage,
                        )
                    },
                );
                if trace_worker_request(request_id) {
                    tracing::debug!(
                        "sophia_renderer_worker schema=2 status=render_finished request={} frame_kind={frame_kind} outcome={}",
                        request_id.0,
                        worker_outcome_name(&outcome),
                    );
                }
                let persistent_render_stats = context.as_ref().map_or_else(
                    LiveNativePersistentRenderStats::default,
                    NativeGbmRenderedScanoutContext::persistent_render_stats,
                );
                let composition_nonzero_rgb_pixels = context.as_ref().map_or(
                    0,
                    NativeGbmRenderedScanoutContext::composition_nonzero_rgb_pixels,
                );
                if result_sender
                    .send(WorkerResult {
                        request_id,
                        context_status,
                        persistent_render_stats,
                        composition_nonzero_rgb_pixels,
                        outcome,
                    })
                    .is_err()
                {
                    break;
                }
            }
            WorkerCommand::Evict {
                image_id,
                completion_sender,
            } => {
                let result = context
                    .as_mut()
                    .map_or(Ok(false), |context| context.evict_renderer_image(image_id));
                let _ = completion_sender.send(result);
            }
            WorkerCommand::Promote {
                image_id,
                completion_sender,
            } => {
                let result = context.as_mut().map_or(Ok(false), |context| {
                    context.promote_renderer_image(image_id)
                });
                let _ = completion_sender.send(result);
            }
            WorkerCommand::Rollback {
                image_id,
                completion_sender,
            } => {
                let result = context.as_mut().map_or(Ok(false), |context| {
                    context.rollback_renderer_image(image_id)
                });
                let _ = completion_sender.send(result);
            }
            WorkerCommand::ExportPromotedImage {
                image_id,
                completion_sender,
            } => {
                let result = context.as_ref().map_or(Ok(None), |context| {
                    context.export_promoted_renderer_image(image_id)
                });
                let _ = completion_sender.send(result);
            }
            WorkerCommand::RestorePromotedImage {
                snapshot,
                completion_sender,
            } => {
                let result = context.as_mut().map_or(Ok(false), |context| {
                    context.restore_promoted_renderer_image(snapshot)
                });
                let persistent_render_stats = context.as_ref().map_or_else(
                    LiveNativePersistentRenderStats::default,
                    NativeGbmRenderedScanoutContext::persistent_render_stats,
                );
                let _ = completion_sender.send(WorkerRestoreImageResult {
                    result,
                    persistent_render_stats,
                });
            }
            WorkerCommand::ClearImages { completion_sender } => {
                let result = context.as_mut().map_or(Ok(0), |context| {
                    context
                        .clear_renderer_images()
                        .map_err(LiveRendererScanoutBufferExportDetail::from)
                });
                let persistent_render_stats = context.as_ref().map_or_else(
                    LiveNativePersistentRenderStats::default,
                    NativeGbmRenderedScanoutContext::persistent_render_stats,
                );
                let _ = completion_sender.send(WorkerMaintenanceResult {
                    result,
                    persistent_render_stats,
                });
            }
            WorkerCommand::Release {
                lease_id,
                slot_token,
            } => {
                if leases
                    .get(&lease_id)
                    .is_none_or(|lease| lease.slot_token != slot_token)
                {
                    frame_slots.refuse_stale_release();
                    continue;
                }
                let lease = leases
                    .remove(&lease_id)
                    .expect("worker lease identity checked above");
                let WorkerLeaseBuffer { buffer, cpu, .. } = lease;
                match cpu {
                    Some(metadata) if free_cpu_buffers.len() < WORKER_FREE_CPU_BUFFER_CAPACITY => {
                        free_cpu_buffers.push(ReusableCpuBuffer {
                            buffer,
                            checksum: metadata.checksum,
                            damage_snapshot: metadata.damage_snapshot,
                        });
                    }
                    Some(_) | None => drop(buffer),
                }
                let _ = frame_slots.release(slot_token);
            }
            WorkerCommand::Shutdown => break,
        }
    }
}

fn reduce_worker_command_send_error<T>(
    error: TrySendError<T>,
) -> LiveRendererScanoutBufferExportDetail {
    match error {
        TrySendError::Full(_) => LiveRendererScanoutBufferExportDetail::WorkerQueueFull,
        TrySendError::Disconnected(_) => LiveRendererScanoutBufferExportDetail::WorkerDisconnected,
    }
}

fn reduce_worker_maintenance_receive_error(
    error: RecvTimeoutError,
) -> LiveRendererScanoutBufferExportDetail {
    match error {
        RecvTimeoutError::Timeout => LiveRendererScanoutBufferExportDetail::WorkerStalled,
        RecvTimeoutError::Disconnected => LiveRendererScanoutBufferExportDetail::WorkerDisconnected,
    }
}

fn pending_frame_kind_name(frame: &PendingRenderedFrame) -> &'static str {
    match frame {
        PendingRenderedFrame::Cpu { .. } => "cpu",
        PendingRenderedFrame::DmaBuf(_) => "dmabuf",
        PendingRenderedFrame::Mixed(_) => "mixed",
    }
}

fn trace_worker_request(request: LiveRendererWorkerRequestId) -> bool {
    static MINIMUM: OnceLock<Option<u64>> = OnceLock::new();
    MINIMUM
        .get_or_init(|| {
            std::env::var("SOPHIA_LIVE_RENDERER_WORKER_TRACE_AFTER_REQUEST")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
        })
        .as_ref()
        .copied()
        .is_some_and(|minimum| request.0 >= minimum)
}

fn worker_outcome_name(outcome: &WorkerOutcome) -> &'static str {
    match outcome {
        WorkerOutcome::Exported { .. } => "exported",
        WorkerOutcome::Deferred(_) => "deferred",
        WorkerOutcome::Failed(_) => "failed",
    }
}

struct WorkerCpuBufferMetadata {
    checksum: u64,
    damage_snapshot: Option<sophia_engine::OutputFrameDamageSnapshot>,
}

struct WorkerLeaseBuffer {
    buffer: NativeGbmOwnedScanoutBuffer,
    cpu: Option<WorkerCpuBufferMetadata>,
    slot_token: LiveRendererFrameSlotToken,
}

struct ReusableCpuBuffer {
    buffer: NativeGbmOwnedScanoutBuffer,
    checksum: u64,
    damage_snapshot: Option<sophia_engine::OutputFrameDamageSnapshot>,
}

fn render_frame<D>(
    context: &mut NativeGbmRenderedScanoutContext<D>,
    target: LiveGbmEglFrameTargetRecord,
    frame: PendingRenderedFrame,
    preferred_modifiers: &[u64],
    leases: &mut BTreeMap<LiveRendererWorkerLeaseId, WorkerLeaseBuffer>,
    free_cpu_buffers: &mut Vec<ReusableCpuBuffer>,
    next_lease_id: &mut u64,
    frame_slots: &mut LiveRendererFrameSlotPool,
    slot_damage: &mut WorkerSlotDamage,
) -> WorkerOutcome
where
    D: AsFd,
{
    let slot_token = match frame_slots.try_acquire() {
        LiveRendererFrameSlotAcquire::Acquired(token) => token,
        LiveRendererFrameSlotAcquire::Deferred => return WorkerOutcome::Deferred(frame),
        LiveRendererFrameSlotAcquire::IncarnationExhausted => {
            return WorkerOutcome::Failed(
                LiveRendererScanoutBufferExportDetail::RetainedBufferMissing,
            );
        }
    };
    let outcome = render_frame_in_slot(
        context,
        target,
        frame,
        preferred_modifiers,
        leases,
        free_cpu_buffers,
        next_lease_id,
        slot_token,
        slot_damage,
    );
    if matches!(outcome, WorkerOutcome::Failed(_)) {
        // A render that failed may have written part of its damage, so the
        // slot holds neither its old content nor its new one.
        slot_damage.invalidate(slot_token.slot_id());
        let _ = frame_slots.release(slot_token);
    }
    outcome
}

#[allow(clippy::too_many_arguments)]
fn render_frame_in_slot<D>(
    context: &mut NativeGbmRenderedScanoutContext<D>,
    target: LiveGbmEglFrameTargetRecord,
    frame: PendingRenderedFrame,
    preferred_modifiers: &[u64],
    leases: &mut BTreeMap<LiveRendererWorkerLeaseId, WorkerLeaseBuffer>,
    free_cpu_buffers: &mut Vec<ReusableCpuBuffer>,
    next_lease_id: &mut u64,
    slot_token: LiveRendererFrameSlotToken,
    slot_damage: &mut WorkerSlotDamage,
) -> WorkerOutcome
where
    D: AsFd,
{
    let frame_slot = slot_token.slot_id().index();
    let (report, cpu) = match frame {
        PendingRenderedFrame::Cpu {
            frame,
            checksum,
            damage_snapshot,
        } => {
            let reused = free_cpu_buffers
                .iter()
                .rposition(|candidate| candidate.buffer.descriptor().size == frame.size)
                .and_then(|index| {
                    let mut candidate = free_cpu_buffers.swap_remove(index);
                    let damage = reusable_cpu_buffer_damage(
                        candidate.checksum,
                        candidate.damage_snapshot.as_ref(),
                        checksum,
                        damage_snapshot.as_ref(),
                        frame.size,
                    );
                    match context.rewrite_xrgb8888_owned_scanout_buffer_damage(
                        &mut candidate.buffer,
                        &frame,
                        &damage,
                    ) {
                        Ok(()) => {
                            let damaged_pixels = damage.iter().fold(0_u64, |total, rect| {
                                let pixels = i64::from(rect.width)
                                    .max(0)
                                    .saturating_mul(i64::from(rect.height).max(0));
                                total.saturating_add(u64::try_from(pixels).unwrap_or(u64::MAX))
                            });
                            tracing::info!(
                                "sophia_renderer_worker schema=1 status=cpu_buffer_reused damage_rects={} damaged_pixels={}",
                                damage.len(),
                                damaged_pixels,
                            );
                            Some(NativeGbmOwnedScanoutBufferExportReport::new(
                                LiveRendererScanoutBufferExportStatus::Exported,
                                LiveRendererScanoutBufferExportDetail::Exported,
                                Some(candidate.buffer),
                            ))
                        }
                        Err(detail) => {
                            tracing::warn!(
                                "sophia_renderer_worker schema=1 status=cpu_buffer_reuse_failed detail={detail:?}"
                            );
                            None
                        }
                    }
                });
            let report = reused.unwrap_or_else(|| {
                context.export_xrgb8888_owned_scanout_buffer_with_modifiers_in_frame_slot(
                    frame_slot,
                    target,
                    &frame,
                    preferred_modifiers,
                )
            });
            (
                report,
                Some(WorkerCpuBufferMetadata {
                    checksum,
                    damage_snapshot,
                }),
            )
        }
        PendingRenderedFrame::DmaBuf(frame) => (
            context.export_dmabuf_owned_scanout_buffer_with_modifiers_in_frame_slot(
                frame_slot,
                target,
                frame.as_frame(),
                preferred_modifiers,
            ),
            None,
        ),
        PendingRenderedFrame::Mixed(frame) => {
            // What the slot's buffer would owe at each age it might report. The
            // age is only knowable inside the render, so every answer travels
            // with the frame and the renderer picks the one that applies.
            let repaint = slot_damage.repaint_table(
                slot_token.slot_id(),
                frame.output_damage_snapshot.as_ref(),
                target.size,
            );
            let report = match context.export_owned_mixed_frame_with_modifiers_in_frame_slot(
                frame_slot,
                target,
                &frame,
                preferred_modifiers,
                repaint.as_ref(),
            ) {
                Ok(report) => report,
                Err(LiveMixedCompositionError::Renderer(detail)) => {
                    return WorkerOutcome::Failed(detail);
                }
                Err(_) => {
                    return WorkerOutcome::Failed(
                        LiveRendererScanoutBufferExportDetail::InvalidTarget,
                    );
                }
            };
            slot_damage.settle(
                slot_token.slot_id(),
                matches!(
                    report.status,
                    LiveRendererScanoutBufferExportStatus::Exported
                ),
                report.target_generation,
                frame.output_damage_snapshot.clone(),
            );
            (report, None)
        }
    };
    if report.status != LiveRendererScanoutBufferExportStatus::Exported {
        return WorkerOutcome::Failed(report.detail);
    }
    let Some(buffer) = report.buffer else {
        return WorkerOutcome::Failed(LiveRendererScanoutBufferExportDetail::RetainedBufferMissing);
    };
    let descriptor = buffer.descriptor();
    let Some(next_id) = next_lease_id.checked_add(1) else {
        return WorkerOutcome::Failed(LiveRendererScanoutBufferExportDetail::RetainedBufferMissing);
    };
    let lease_id = LiveRendererWorkerLeaseId(*next_lease_id);
    *next_lease_id = next_id;
    leases.insert(
        lease_id,
        WorkerLeaseBuffer {
            buffer,
            cpu,
            slot_token,
        },
    );
    WorkerOutcome::Exported {
        descriptor,
        lease_id,
        slot_token,
    }
}

include!("worker/damage.rs");
mod tests;
