#![cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]

use sophia_backend_live::{
    LIVE_PRODUCTION_PAGE_FLIP_HARD_STALL, LiveProductionCpuFrameQueueStatus,
    LiveProductionMixedLayerSource, LiveProductionNativeSuspendOutcome,
    LiveProductionPageFlipWatchdogStatus, LiveProductionScanoutContent,
    LiveProductionVisualRuntime, live_production_mixed_layer_order,
    live_production_projection_requires_gpu_scanout, live_production_should_preserve_gpu_output,
    live_production_transactions_require_gpu_scanout, reduce_live_production_cpu_frame_queue,
    reduce_live_production_frame_defer, reduce_live_production_page_flip_watchdog,
};
use sophia_engine::HeadlessOutput;
use sophia_protocol::{
    AuthorityKind, BufferSource, OutputId, Rect, Region, Size, SurfaceId, SurfaceTransaction,
    SurfaceTransactionReadiness, TransactionId, TransactionOutcome,
};
use std::time::Duration;

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
fn mixed_layer_order_preserves_cpu_overlay_above_gpu_clients() {
    let lower_gpu = SurfaceId::new(1, 1);
    let current_gpu = SurfaceId::new(2, 1);
    let upper_cpu = SurfaceId::new(3, 1);

    assert_eq!(
        live_production_mixed_layer_order(
            &[lower_gpu, current_gpu, upper_cpu],
            current_gpu,
            &[upper_cpu],
            &[lower_gpu],
        ),
        vec![
            LiveProductionMixedLayerSource::RetainedDmaBuf(lower_gpu),
            LiveProductionMixedLayerSource::CurrentDmaBuf,
            LiveProductionMixedLayerSource::Cpu(upper_cpu),
        ]
    );
}

#[test]
fn mixed_layer_order_preserves_cpu_client_below_current_gpu() {
    let lower_cpu = SurfaceId::new(1, 1);
    let current_gpu = SurfaceId::new(2, 1);

    assert_eq!(
        live_production_mixed_layer_order(
            &[lower_cpu, current_gpu],
            current_gpu,
            &[lower_cpu],
            &[],
        ),
        vec![
            LiveProductionMixedLayerSource::Cpu(lower_cpu),
            LiveProductionMixedLayerSource::CurrentDmaBuf,
        ]
    );
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
    let cpu = Some(LiveProductionScanoutContent::Cpu {
        frame: sophia_backend_live::LiveProductionNativeFrameId::from_raw(1),
        checksum,
    });
    let mixed = Some(LiveProductionScanoutContent::MixedPresent {
        frame: sophia_backend_live::LiveProductionNativeFrameId::from_raw(2),
        transaction: TransactionId::from_raw(9),
        nonzero_rgb_pixels: 1,
    });

    assert_eq!(
        reduce_live_production_cpu_frame_queue(cpu, None, None, false, false, checksum),
        LiveProductionCpuFrameQueueStatus::UnchangedPending
    );
    assert_eq!(
        reduce_live_production_cpu_frame_queue(None, cpu, None, false, false, checksum),
        LiveProductionCpuFrameQueueStatus::UnchangedSubmitted
    );
    assert_eq!(
        reduce_live_production_cpu_frame_queue(None, None, cpu, false, true, checksum),
        LiveProductionCpuFrameQueueStatus::UnchangedPresented
    );
    assert_eq!(
        reduce_live_production_cpu_frame_queue(None, None, mixed, false, false, checksum),
        LiveProductionCpuFrameQueueStatus::Queued
    );
    assert_eq!(
        reduce_live_production_cpu_frame_queue(None, None, cpu, false, false, checksum + 1),
        LiveProductionCpuFrameQueueStatus::Queued
    );
    assert_eq!(
        reduce_live_production_cpu_frame_queue(None, None, mixed, true, false, checksum),
        LiveProductionCpuFrameQueueStatus::GpuFrameOwned
    );
    assert_eq!(
        reduce_live_production_cpu_frame_queue(mixed, None, None, false, false, checksum),
        LiveProductionCpuFrameQueueStatus::GpuFrameOwned
    );
    assert_eq!(
        reduce_live_production_cpu_frame_queue(None, mixed, None, false, false, checksum),
        LiveProductionCpuFrameQueueStatus::GpuFrameOwned
    );
}

#[test]
fn unchanged_initial_modeset_frame_requires_one_event_bearing_submission() {
    let checksum = 42;
    let cpu = Some(LiveProductionScanoutContent::Cpu {
        frame: sophia_backend_live::LiveProductionNativeFrameId::from_raw(1),
        checksum,
    });

    assert_eq!(
        reduce_live_production_cpu_frame_queue(None, None, cpu, false, false, checksum),
        LiveProductionCpuFrameQueueStatus::BaselineRequired
    );
    assert_eq!(
        reduce_live_production_cpu_frame_queue(None, None, cpu, false, true, checksum),
        LiveProductionCpuFrameQueueStatus::UnchangedPresented
    );
}

#[test]
fn page_flip_watchdog_fails_closed_after_its_hard_boundary() {
    assert_eq!(
        reduce_live_production_page_flip_watchdog(None, LIVE_PRODUCTION_PAGE_FLIP_HARD_STALL),
        LiveProductionPageFlipWatchdogStatus::Idle
    );
    assert_eq!(
        reduce_live_production_page_flip_watchdog(
            Some(LIVE_PRODUCTION_PAGE_FLIP_HARD_STALL - Duration::from_millis(1)),
            LIVE_PRODUCTION_PAGE_FLIP_HARD_STALL,
        ),
        LiveProductionPageFlipWatchdogStatus::Healthy
    );
    assert_eq!(
        reduce_live_production_page_flip_watchdog(
            Some(LIVE_PRODUCTION_PAGE_FLIP_HARD_STALL),
            LIVE_PRODUCTION_PAGE_FLIP_HARD_STALL,
        ),
        LiveProductionPageFlipWatchdogStatus::HardStall
    );
}

#[test]
fn gpu_scanout_preservation_follows_post_batch_active_transactions() {
    let mut gpu = initial_transaction(0);
    gpu.target_buffer = BufferSource::DmaBuf { handle: 7 };
    let cpu = initial_transaction(0);

    assert!(live_production_transactions_require_gpu_scanout(
        std::slice::from_ref(&gpu)
    ));
    assert!(!live_production_transactions_require_gpu_scanout(&[cpu]));
    assert!(!live_production_transactions_require_gpu_scanout(&[]));
    assert!(live_production_projection_requires_gpu_scanout(
        std::slice::from_ref(&gpu),
        std::slice::from_ref(&gpu.surface),
    ));
    assert!(!live_production_projection_requires_gpu_scanout(
        std::slice::from_ref(&gpu),
        &[],
    ));
}

#[test]
fn visibility_change_forces_a_frame_unless_a_retained_gpu_projection_is_queued() {
    assert!(!reduce_live_production_frame_defer(true, true, false));
    assert!(reduce_live_production_frame_defer(true, true, true));
    assert!(reduce_live_production_frame_defer(true, false, false));
    assert!(!reduce_live_production_frame_defer(false, false, false));
}

#[test]
fn submitted_gpu_present_blocks_a_cpu_frame_from_superseding_it() {
    assert!(live_production_should_preserve_gpu_output(
        true, true, false, false, false,
    ));
    assert!(live_production_should_preserve_gpu_output(
        true, true, false, true, false,
    ));
    assert!(!live_production_should_preserve_gpu_output(
        false, true, false, false, false,
    ));
    assert!(!live_production_should_preserve_gpu_output(
        true, false, false, false, false,
    ));
}
