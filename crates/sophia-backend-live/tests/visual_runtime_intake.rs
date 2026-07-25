#![cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]

use sophia_backend_live::LiveProductionVisualRuntime;
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
    assert_eq!(first.skipped_present, None);
    assert_eq!(second, first);
    assert_eq!(runtime.output_count(), 1);
}
