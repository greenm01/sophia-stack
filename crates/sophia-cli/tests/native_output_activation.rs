#![cfg(feature = "atomic-scanout-live")]

use sophia_backend_live::{
    LibdrmNativeOutputCapability, LibdrmNativeOutputTiming, LibdrmNativeVrrPropertyDiscoveryStatus,
};
use sophia_cli::desktop_output_activation::{
    NativeOutputActivationDisposition, NativeOutputActivationEffect, NativeOutputActivationFailure,
    NativeOutputActivationModel, NativeOutputActivationMsg, NativeOutputActivationPhase,
    NativeOutputActivationSettlement, NativeOutputEffectCompletion, NativeOutputRollbackSettlement,
    reduce_native_output_activation,
};
use sophia_cli::desktop_output_topology::{
    NativeOutputActivationPlan, prepare_native_output_activation_plan,
    project_native_output_topology,
};
use sophia_config::{
    ConfigDigest, ConfigGeneration, DesktopOutputCandidate, reconcile_desktop_output_candidate,
};
use sophia_engine::HeadlessOutput;
use sophia_protocol::{OutputId, Size};

fn plan(generation: u64, digest: u8) -> NativeOutputActivationPlan {
    let selected = LibdrmNativeOutputTiming::new(1920, 1080, 60_000);
    let capability = LibdrmNativeOutputCapability::new(
        OutputId::from_raw(1),
        1,
        "DP-1",
        [selected],
        Some(selected),
        selected,
        LibdrmNativeVrrPropertyDiscoveryStatus::Unsupported,
    )
    .unwrap();
    let capabilities = [capability];
    let outputs = [HeadlessOutput {
        id: OutputId::from_raw(1),
        size: Size {
            width: 1920,
            height: 1080,
        },
        scale: 1,
    }];
    let topology = project_native_output_topology(&capabilities, &outputs).unwrap();
    let candidate = DesktopOutputCandidate {
        generation: ConfigGeneration::from_raw(generation),
        digest: ConfigDigest::new([digest; 32]),
        inherit_sophia: true,
        named: Vec::new(),
    };
    let reconciliation = reconcile_desktop_output_candidate(&candidate, &topology).unwrap();
    prepare_native_output_activation_plan(&capabilities, &topology, &reconciliation).unwrap()
}

fn begin(
    plan: NativeOutputActivationPlan,
) -> sophia_cli::desktop_output_activation::NativeOutputActivationUpdate {
    reduce_native_output_activation(
        &NativeOutputActivationModel::default(),
        NativeOutputActivationMsg::Begin(plan),
    )
}

#[test]
fn reducer_is_deterministic_and_does_not_replace_in_flight_work() {
    let model = NativeOutputActivationModel::default();
    let candidate = plan(2, 2);
    let first = reduce_native_output_activation(
        &model,
        NativeOutputActivationMsg::Begin(candidate.clone()),
    );
    let repeated =
        reduce_native_output_activation(&model, NativeOutputActivationMsg::Begin(candidate));
    assert_eq!(first, repeated);

    let competing =
        reduce_native_output_activation(&first.model, NativeOutputActivationMsg::Begin(plan(3, 9)));
    assert_eq!(
        competing.disposition,
        NativeOutputActivationDisposition::IgnoredBusy
    );
    assert_eq!(competing.model, first.model);
    assert!(competing.effect.is_none());
}

#[test]
fn activation_advances_only_through_matching_typed_completions() {
    let started = begin(plan(3, 3));
    assert_eq!(
        started.disposition,
        NativeOutputActivationDisposition::Started
    );
    assert_eq!(started.model.phase(), NativeOutputActivationPhase::Testing);
    let NativeOutputActivationEffect::Test { key, plan } = started.effect.unwrap() else {
        panic!("begin must emit a test effect");
    };
    assert_eq!(plan.generation(), key.generation());

    let tested = reduce_native_output_activation(
        &started.model,
        NativeOutputActivationMsg::TestCompleted {
            key,
            completion: NativeOutputEffectCompletion::Succeeded,
        },
    );
    assert_eq!(tested.model.phase(), NativeOutputActivationPhase::Applying);
    assert!(matches!(
        tested.effect,
        Some(NativeOutputActivationEffect::Apply { key: effect_key, .. }) if effect_key == key
    ));

    let applied = reduce_native_output_activation(
        &tested.model,
        NativeOutputActivationMsg::ApplyCompleted {
            key,
            completion: NativeOutputEffectCompletion::Succeeded,
        },
    );
    assert_eq!(
        applied.disposition,
        NativeOutputActivationDisposition::Activated
    );
    assert_eq!(
        applied.model.phase(),
        NativeOutputActivationPhase::Activated
    );
    assert_eq!(
        applied.model.settlement(),
        Some(NativeOutputActivationSettlement::Activated { key })
    );
    assert!(applied.model.pending_plan().is_none());
    assert!(applied.effect.is_none());
}

#[test]
fn apply_failure_requires_rollback_before_terminal_rejection() {
    let expected_plan = plan(4, 4);
    let started = begin(expected_plan.clone());
    let key = started.effect.as_ref().unwrap().key();
    let tested = reduce_native_output_activation(
        &started.model,
        NativeOutputActivationMsg::TestCompleted {
            key,
            completion: NativeOutputEffectCompletion::Succeeded,
        },
    );
    let failed_apply = reduce_native_output_activation(
        &tested.model,
        NativeOutputActivationMsg::ApplyCompleted {
            key,
            completion: NativeOutputEffectCompletion::Failed(
                NativeOutputActivationFailure::TimedOut,
            ),
        },
    );

    assert_eq!(
        failed_apply.model.phase(),
        NativeOutputActivationPhase::RollingBack
    );
    let NativeOutputActivationEffect::Rollback {
        key: rollback_key,
        plan: rollback_plan,
    } = failed_apply.effect.unwrap()
    else {
        panic!("apply failure must emit rollback");
    };
    assert_eq!(rollback_key, key);
    assert_eq!(rollback_plan, expected_plan);
    assert!(failed_apply.model.settlement().is_none());

    let rolled_back = reduce_native_output_activation(
        &failed_apply.model,
        NativeOutputActivationMsg::RollbackCompleted {
            key,
            completion: NativeOutputEffectCompletion::Succeeded,
        },
    );
    assert_eq!(
        rolled_back.model.phase(),
        NativeOutputActivationPhase::Rejected
    );
    assert_eq!(
        rolled_back.model.settlement(),
        Some(NativeOutputActivationSettlement::Rejected {
            key,
            cause: NativeOutputActivationFailure::TimedOut,
            rollback: NativeOutputRollbackSettlement::Succeeded,
        })
    );
    assert!(rolled_back.model.pending_plan().is_none());
}

#[test]
fn stale_duplicate_and_out_of_order_completions_cannot_advance_state() {
    let started = begin(plan(5, 5));
    let key = started.effect.as_ref().unwrap().key();
    let stale_key = begin(plan(6, 6)).effect.unwrap().key();

    for message in [
        NativeOutputActivationMsg::TestCompleted {
            key: stale_key,
            completion: NativeOutputEffectCompletion::Succeeded,
        },
        NativeOutputActivationMsg::ApplyCompleted {
            key,
            completion: NativeOutputEffectCompletion::Succeeded,
        },
        NativeOutputActivationMsg::RollbackCompleted {
            key,
            completion: NativeOutputEffectCompletion::Succeeded,
        },
    ] {
        let ignored = reduce_native_output_activation(&started.model, message);
        assert_eq!(
            ignored.disposition,
            NativeOutputActivationDisposition::IgnoredStale
        );
        assert_eq!(ignored.model, started.model);
        assert!(ignored.effect.is_none());
    }

    let tested = reduce_native_output_activation(
        &started.model,
        NativeOutputActivationMsg::TestCompleted {
            key,
            completion: NativeOutputEffectCompletion::Succeeded,
        },
    );
    let duplicate = reduce_native_output_activation(
        &tested.model,
        NativeOutputActivationMsg::TestCompleted {
            key,
            completion: NativeOutputEffectCompletion::Succeeded,
        },
    );
    assert_eq!(
        duplicate.disposition,
        NativeOutputActivationDisposition::IgnoredStale
    );
    assert_eq!(duplicate.model, tested.model);
}

#[test]
fn test_rejection_discards_candidate_without_rollback() {
    let started = begin(plan(7, 7));
    let key = started.effect.as_ref().unwrap().key();
    let rejected = reduce_native_output_activation(
        &started.model,
        NativeOutputActivationMsg::TestCompleted {
            key,
            completion: NativeOutputEffectCompletion::Failed(
                NativeOutputActivationFailure::Rejected,
            ),
        },
    );

    assert_eq!(
        rejected.disposition,
        NativeOutputActivationDisposition::Rejected
    );
    assert_eq!(
        rejected.model.phase(),
        NativeOutputActivationPhase::Rejected
    );
    assert_eq!(
        rejected.model.settlement(),
        Some(NativeOutputActivationSettlement::Rejected {
            key,
            cause: NativeOutputActivationFailure::Rejected,
            rollback: NativeOutputRollbackSettlement::NotRequired,
        })
    );
    assert!(rejected.model.pending_plan().is_none());
    assert!(rejected.effect.is_none());

    let repeated = reduce_native_output_activation(
        &rejected.model,
        NativeOutputActivationMsg::Begin(plan(8, 8)),
    );
    assert_eq!(
        repeated.disposition,
        NativeOutputActivationDisposition::IgnoredTerminal
    );
    assert_eq!(repeated.model, rejected.model);
}

#[test]
fn rollback_failure_is_terminal_and_preserves_both_causes() {
    let started = begin(plan(9, 9));
    let key = started.effect.as_ref().unwrap().key();
    let tested = reduce_native_output_activation(
        &started.model,
        NativeOutputActivationMsg::TestCompleted {
            key,
            completion: NativeOutputEffectCompletion::Succeeded,
        },
    );
    let failed_apply = reduce_native_output_activation(
        &tested.model,
        NativeOutputActivationMsg::ApplyCompleted {
            key,
            completion: NativeOutputEffectCompletion::Failed(
                NativeOutputActivationFailure::Disconnected,
            ),
        },
    );
    let failed_rollback = reduce_native_output_activation(
        &failed_apply.model,
        NativeOutputActivationMsg::RollbackCompleted {
            key,
            completion: NativeOutputEffectCompletion::Failed(
                NativeOutputActivationFailure::Invalidated,
            ),
        },
    );

    assert_eq!(
        failed_rollback.disposition,
        NativeOutputActivationDisposition::RecoveryFailed
    );
    assert_eq!(
        failed_rollback.model.phase(),
        NativeOutputActivationPhase::RecoveryFailed
    );
    assert_eq!(
        failed_rollback.model.settlement(),
        Some(NativeOutputActivationSettlement::Rejected {
            key,
            cause: NativeOutputActivationFailure::Disconnected,
            rollback: NativeOutputRollbackSettlement::Failed(
                NativeOutputActivationFailure::Invalidated
            ),
        })
    );
}
