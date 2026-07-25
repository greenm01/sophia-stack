#![cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]

use sophia_backend_live::{
    LiveProductionCpuFrameQueueStatus, LiveProductionNativeSuspendOutcome,
    LiveProductionScanoutContent, LiveProductionVisualRuntime,
    live_production_transactions_require_gpu_scanout, reduce_live_production_cpu_frame_queue,
};
use sophia_engine::HeadlessOutput;
use sophia_protocol::{
    AuthorityKind, BufferSource, OutputId, Rect, Region, Size, SurfaceId, SurfaceTransaction,
    SurfaceTransactionReadiness, TransactionId, TransactionOutcome,
};

fn output() -> HeadlessOutput {
    HeadlessOutput {
        id: OutputId::from_raw(1),
        size: Size {
            width: 640,
            height: 480,
        },
        scale: 1,
    }
}

fn initial_transaction(previous_committed_generation: u64) -> SurfaceTransaction {
    SurfaceTransaction {
        transaction: TransactionId::from_raw(1),
        authority: AuthorityKind::SophiaX,
        surface: SurfaceId::new(1, 1),
        namespace: None,
        target_geometry: Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        },
        target_buffer: BufferSource::CpuBuffer { handle: 1 },
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        }),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation,
    }
}

#[test]
fn initial_surface_enters_visual_state_only_through_engine_commit() {
    let mut runtime = LiveProductionVisualRuntime::new(&[output()], None, None).expect("runtime");

    assert!(runtime.committed_surfaces().is_empty());
    let prepared = runtime
        .prepare_authority_transactions(TransactionId::from_raw(1), &[initial_transaction(0)], &[])
        .expect("prepare initial authority transaction");

    assert_eq!(prepared.authority_commits.len(), 1);
    assert_eq!(
        prepared.authority_commits[0].outcome,
        TransactionOutcome::Committed
    );
    assert_eq!(runtime.committed_surfaces().len(), 1);
    assert_eq!(runtime.committed_surfaces()[0].committed_generation, 1);
}

#[test]
fn initial_surface_cannot_seed_a_forged_generation() {
    let mut runtime = LiveProductionVisualRuntime::new(&[output()], None, None).expect("runtime");

    let prepared = runtime
        .prepare_authority_transactions(TransactionId::from_raw(1), &[initial_transaction(7)], &[])
        .expect("prepare malformed initial authority transaction");

    assert_eq!(
        prepared.authority_commits[0].outcome,
        TransactionOutcome::RejectedStaleSurface
    );
    assert!(runtime.committed_surfaces().is_empty());
}

#[test]
fn revoked_native_suspend_is_idempotent_without_active_scanout() {
    let output = output();
    let mut runtime =
        LiveProductionVisualRuntime::new(&[output], None, None).expect("headless runtime");

    let first = runtime
        .suspend_revoked_native_scanout(&[output])
        .expect("first revoked suspension");
    let second = runtime
        .suspend_revoked_native_scanout(&[output])
        .expect("duplicate revoked suspension");

    assert_eq!(first.abandoned_scanouts, 0);
    assert_eq!(
        first.outcome,
        LiveProductionNativeSuspendOutcome::ForcedDetachRevoked
    );
    assert_eq!(first.skipped_present, None);
    assert_eq!(second, first);
    assert_eq!(runtime.output_count(), 1);
}

#[test]
fn cpu_frame_queue_suppresses_only_matching_cpu_content() {
    let checksum = 42;
    let cpu = Some(LiveProductionScanoutContent::Cpu { checksum });
    let mixed = Some(LiveProductionScanoutContent::Mixed {
        transaction: TransactionId::from_raw(9),
        nonzero_rgb_pixels: 1,
    });

    assert_eq!(
        reduce_live_production_cpu_frame_queue(cpu, None, None, false, checksum),
        LiveProductionCpuFrameQueueStatus::UnchangedPending
    );
    assert_eq!(
        reduce_live_production_cpu_frame_queue(None, cpu, None, false, checksum),
        LiveProductionCpuFrameQueueStatus::UnchangedSubmitted
    );
    assert_eq!(
        reduce_live_production_cpu_frame_queue(None, None, cpu, true, checksum),
        LiveProductionCpuFrameQueueStatus::UnchangedPresented
    );
    assert_eq!(
        reduce_live_production_cpu_frame_queue(None, None, mixed, false, checksum),
        LiveProductionCpuFrameQueueStatus::Queued
    );
    assert_eq!(
        reduce_live_production_cpu_frame_queue(None, None, cpu, false, checksum + 1),
        LiveProductionCpuFrameQueueStatus::Queued
    );
}

#[test]
fn unchanged_initial_modeset_frame_requires_one_event_bearing_submission() {
    let checksum = 42;
    let cpu = Some(LiveProductionScanoutContent::Cpu { checksum });

    assert_eq!(
        reduce_live_production_cpu_frame_queue(None, None, cpu, false, checksum),
        LiveProductionCpuFrameQueueStatus::BaselineRequired
    );
    assert_eq!(
        reduce_live_production_cpu_frame_queue(None, None, cpu, true, checksum),
        LiveProductionCpuFrameQueueStatus::UnchangedPresented
    );
}

#[test]
fn gpu_scanout_preservation_follows_post_batch_active_transactions() {
    let mut gpu = initial_transaction(0);
    gpu.target_buffer = BufferSource::DmaBuf { handle: 7 };
    let cpu = initial_transaction(0);

    assert!(live_production_transactions_require_gpu_scanout(&[gpu]));
    assert!(!live_production_transactions_require_gpu_scanout(&[cpu]));
    assert!(!live_production_transactions_require_gpu_scanout(&[]));
}

#[test]
fn resize_epoch_hold_persists_across_native_service_boundaries() {
    let mut runtime = LiveProductionVisualRuntime::new(&[output()], None, None).expect("runtime");

    runtime.set_present_scheduling_blocked(true);
    assert!(runtime.diagnostics().present_scheduling_blocked);

    runtime.set_present_scheduling_blocked(false);
    assert!(!runtime.diagnostics().present_scheduling_blocked);
}
