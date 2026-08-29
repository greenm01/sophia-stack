//! The renderer thread: what one shared worker does for every output of a
//! device group.
//!
//! Split from its handle so the two halves stay legible apart. This side owns
//! per-output state and the render itself; the handle side owns the protocol
//! a caller sees.

use super::*;
use crate::api::*;
use std::collections::BTreeMap;
use std::io;
use std::os::fd::AsFd;
use std::sync::mpsc::{Receiver, SyncSender};

/// One output's private state inside a shared worker.
///
/// Slots, damage history, leases, and reusable buffers are all per output.
/// Buffer reuse in particular is matched on size alone, so two outputs of the
/// same mode sharing a pool would hand one screen's content to the other.
struct WorkerOutputState {
    reply: SyncSender<WorkerResult>,
    frame_slots: LiveRendererFrameSlotPool,
    slot_damage: WorkerSlotDamage,
    leases: BTreeMap<LiveRendererWorkerLeaseId, WorkerLeaseBuffer>,
    free_cpu_buffers: Vec<ReusableCpuBuffer>,
}

impl WorkerOutputState {
    fn new(
        reply: SyncSender<WorkerResult>,
        frame_slot_metrics: LiveRendererFrameSlotMetricsHandle,
    ) -> Self {
        Self {
            reply,
            frame_slots: LiveRendererFrameSlotPool::with_metrics(frame_slot_metrics),
            slot_damage: WorkerSlotDamage::new(),
            leases: BTreeMap::new(),
            free_cpu_buffers: Vec::new(),
        }
    }
}

pub(super) fn run_worker<D>(device: io::Result<D>, command_receiver: Receiver<WorkerCommand>)
where
    D: AsFd + Send + 'static,
{
    let report = NativeGbmRenderedScanoutContext::from_backend_device_result(device);
    let context_status = report.status;
    let mut context = report.context;
    let mut outputs = BTreeMap::<LiveRendererWorkerOutputKey, WorkerOutputState>::new();
    let mut next_lease_id = 1_u64;

    while let Ok(command) = command_receiver.recv() {
        match command {
            WorkerCommand::Register {
                output,
                reply,
                frame_slot_metrics,
            } => {
                // Two outputs answering to one key would share slots, leases,
                // and a reply route while believing each was its own. The key
                // is composed to make that impossible; this says so out loud
                // rather than leaving it to an argument about how the caller
                // builds keys.
                if outputs.contains_key(&output) {
                    tracing::error!(
                        "sophia_renderer_worker schema=3 status=duplicate_output output={}",
                        output.raw(),
                    );
                    continue;
                }
                outputs.insert(output, WorkerOutputState::new(reply, frame_slot_metrics));
            }
            WorkerCommand::Deregister { output } => {
                // Dropping the state returns this output's buffers and slots
                // together. Nothing else may reclaim them: they are leased
                // against its incarnations, not the device's.
                outputs.remove(&output);
            }
            WorkerCommand::Render {
                output,
                request_id,
                target,
                frame,
                preferred_modifiers,
            } => {
                let frame_kind = pending_frame_kind_name(&frame);
                if trace_worker_request(request_id) {
                    tracing::debug!(
                        "sophia_renderer_worker schema=2 status=render_started output={} request={} frame_kind={frame_kind}",
                        output.raw(),
                        request_id.0,
                    );
                }
                // A render for an output that never registered has nowhere to
                // report and no slots to draw into. Dropping it is the only
                // honest answer; the caller's request stays outstanding and
                // its stall ladder reports it.
                let Some(state) = outputs.get_mut(&output) else {
                    continue;
                };
                let outcome = context.as_mut().map_or_else(
                    || {
                        WorkerOutcome::Failed(
                            LiveRendererScanoutBufferExportDetail::BackendDeviceUnavailable,
                        )
                    },
                    |context| {
                        render_frame(
                            context,
                            output,
                            target,
                            frame,
                            &preferred_modifiers,
                            &mut next_lease_id,
                            state,
                        )
                    },
                );
                state
                    .frame_slots
                    .metrics_handle()
                    .store_damage(state.slot_damage.metrics());
                if trace_worker_request(request_id) {
                    tracing::debug!(
                        "sophia_renderer_worker schema=2 status=render_finished output={} request={} frame_kind={frame_kind} outcome={}",
                        output.raw(),
                        request_id.0,
                        worker_outcome_name(&outcome),
                    );
                }
                let persistent_render_stats = context.as_ref().map_or_else(
                    LiveNativePersistentRenderStats::default,
                    NativeGbmRenderedScanoutContext::persistent_render_stats,
                );
                let composition_nonzero_rgb_pixels = context.as_ref().map_or(0, |context| {
                    context.composition_nonzero_rgb_pixels(output.target_set())
                });
                // To this output's own channel: the result cannot reach
                // another output because there is no route to one.
                if state
                    .reply
                    .send(WorkerResult {
                        output,
                        request_id,
                        context_status,
                        persistent_render_stats,
                        composition_nonzero_rgb_pixels,
                        outcome,
                    })
                    .is_err()
                {
                    outputs.remove(&output);
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
                output,
                lease_id,
                slot_token,
            } => {
                // A release is answered by the output that issued the lease
                // and by no other. Slot ids and incarnations are per output,
                // so a token from one head names a live slot in another's
                // pool; looking it up there would free a buffer a different
                // screen is still scanning out.
                let Some(state) = outputs.get_mut(&output) else {
                    continue;
                };
                if state
                    .leases
                    .get(&lease_id)
                    .is_none_or(|lease| lease.slot_token != slot_token)
                {
                    state.frame_slots.refuse_stale_release();
                    continue;
                }
                let lease = state
                    .leases
                    .remove(&lease_id)
                    .expect("worker lease identity checked above");
                let WorkerLeaseBuffer { buffer, cpu, .. } = lease;
                match cpu {
                    Some(metadata)
                        if state.free_cpu_buffers.len() < WORKER_FREE_CPU_BUFFER_CAPACITY =>
                    {
                        state.free_cpu_buffers.push(ReusableCpuBuffer {
                            buffer,
                            checksum: metadata.checksum,
                            damage_snapshot: metadata.damage_snapshot,
                        });
                    }
                    Some(_) | None => drop(buffer),
                }
                let _ = state.frame_slots.release(slot_token);
            }
            WorkerCommand::Shutdown => break,
        }
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
    output: LiveRendererWorkerOutputKey,
    target: LiveGbmEglFrameTargetRecord,
    frame: PendingRenderedFrame,
    preferred_modifiers: &[u64],
    next_lease_id: &mut u64,
    state: &mut WorkerOutputState,
) -> WorkerOutcome
where
    D: AsFd,
{
    let slot_token = match state.frame_slots.try_acquire() {
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
        output,
        target,
        frame,
        preferred_modifiers,
        next_lease_id,
        slot_token,
        state,
    );
    if matches!(outcome, WorkerOutcome::Failed(_)) {
        // A render that failed may have written part of its damage, so the
        // slot holds neither its old content nor its new one.
        state.slot_damage.invalidate(slot_token.slot_id());
        let _ = state.frame_slots.release(slot_token);
    }
    outcome
}

#[allow(clippy::too_many_arguments)]
fn render_frame_in_slot<D>(
    context: &mut NativeGbmRenderedScanoutContext<D>,
    output: LiveRendererWorkerOutputKey,
    target: LiveGbmEglFrameTargetRecord,
    frame: PendingRenderedFrame,
    preferred_modifiers: &[u64],
    next_lease_id: &mut u64,
    slot_token: LiveRendererFrameSlotToken,
    state: &mut WorkerOutputState,
) -> WorkerOutcome
where
    D: AsFd,
{
    let target_set = output.target_set();
    let frame_slot = slot_token.slot_id().index();
    let (report, cpu) = match frame {
        PendingRenderedFrame::Cpu {
            frame,
            checksum,
            damage_snapshot,
        } => {
            let reused = state.free_cpu_buffers
                .iter()
                .rposition(|candidate| candidate.buffer.descriptor().size == frame.size)
                .and_then(|index| {
                    let mut candidate = state.free_cpu_buffers.swap_remove(index);
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
                    target_set,
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
                target_set,
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
            let repaint = state.slot_damage.repaint_table(
                slot_token.slot_id(),
                frame.output_damage_snapshot.as_ref(),
                target.size,
            );
            let report = match context.export_owned_mixed_frame_with_modifiers_in_frame_slot(
                target_set,
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
            state.slot_damage.settle(
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
    state.leases.insert(
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
