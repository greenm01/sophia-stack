#![cfg(feature = "atomic-scanout-live")]

use sophia_backend_live::{
    LibdrmNativeOutputCapability, LibdrmNativeOutputTiming, LibdrmNativeVrrPropertyDiscoveryStatus,
    project_live_output_authority_snapshot,
};
use sophia_cli::live_output_authority::*;
use sophia_engine::{HeadlessOutput, OutputTopologyTransactionFailure, RenderHeadId};
use sophia_protocol::*;

fn capability(head: u64, connector: &str, selected: (u32, u32)) -> LibdrmNativeOutputCapability {
    let selected = LibdrmNativeOutputTiming::new(selected.0, selected.1, 60_000);
    LibdrmNativeOutputCapability::new(
        OutputId::from_raw(1),
        u32::try_from(head).unwrap(),
        connector,
        [
            LibdrmNativeOutputTiming::new(2_560, 1_440, 60_000),
            LibdrmNativeOutputTiming::new(1_920, 1_080, 60_000),
        ],
        Some(selected),
        selected,
        LibdrmNativeVrrPropertyDiscoveryStatus::Discovered,
    )
    .unwrap()
    .bind_head(RenderHeadId::from_raw(head))
    .unwrap()
}

#[test]
fn preparation_wait_prioritizes_cancellation_then_readiness_then_timeout() {
    // Escalation spent, so a deadline still rejects. The unspent case is
    // covered separately below.
    let observation =
        |cancellation_requested, ordinary_settlement_idle, native_quiescent, deadline_reached| {
            OutputTopologyPreparationWaitObservation {
                cancellation_requested,
                ordinary_settlement_idle,
                native_quiescent,
                deadline_reached,
                escalation_available: false,
            }
        };

    assert_eq!(
        reduce_output_topology_preparation_wait(observation(true, true, true, true)),
        OutputTopologyPreparationWaitDecision::Cancel
    );
    assert_eq!(
        reduce_output_topology_preparation_wait(observation(false, true, true, true)),
        OutputTopologyPreparationWaitDecision::Begin
    );
    assert_eq!(
        reduce_output_topology_preparation_wait(observation(false, true, false, true)),
        OutputTopologyPreparationWaitDecision::TimedOut
    );
    assert_eq!(
        reduce_output_topology_preparation_wait(observation(false, false, true, false)),
        OutputTopologyPreparationWaitDecision::Wait
    );
}

/// A wait blocks on owners that can only advance while it waits, so the first
/// expiry forces quiescence rather than reporting a stall it never tried to
/// clear. The escalation is one-shot: a second expiry rejects.
#[test]
fn preparation_wait_escalates_once_before_it_rejects() {
    let expired = |escalation_available| OutputTopologyPreparationWaitObservation {
        cancellation_requested: false,
        ordinary_settlement_idle: true,
        native_quiescent: false,
        deadline_reached: true,
        escalation_available,
    };

    assert_eq!(
        reduce_output_topology_preparation_wait(expired(true)),
        OutputTopologyPreparationWaitDecision::Escalate
    );
    assert_eq!(
        reduce_output_topology_preparation_wait(expired(false)),
        OutputTopologyPreparationWaitDecision::TimedOut
    );

    // Readiness still wins over an available escalation: nothing is skipped
    // when the wait can simply proceed.
    assert_eq!(
        reduce_output_topology_preparation_wait(OutputTopologyPreparationWaitObservation {
            cancellation_requested: false,
            ordinary_settlement_idle: true,
            native_quiescent: true,
            deadline_reached: true,
            escalation_available: true,
        }),
        OutputTopologyPreparationWaitDecision::Begin
    );
}

fn fixture() -> (Vec<LibdrmNativeOutputCapability>, OutputAuthoritySnapshot) {
    let capabilities = vec![
        capability(11, "DP-1", (2_560, 1_440)),
        capability(12, "DP-2", (1_920, 1_080)),
    ];
    let snapshot = project_live_output_authority_snapshot(
        &capabilities,
        &[HeadlessOutput {
            id: OutputId::from_raw(1),
            size: Size {
                width: 2_560,
                height: 1_440,
            },
            scale: 1,
        }],
        7,
    )
    .unwrap();
    (capabilities, snapshot)
}

#[test]
fn hardware_publication_replaces_the_baseline_only_at_a_fresh_epoch() {
    let (_, snapshot) = fixture();
    let mut owner = LiveOutputAuthorityOwner::new(3, snapshot.clone()).unwrap();
    let mut replacement = snapshot.clone();
    replacement.topology_epoch = 8;
    replacement.heads[0].label = "Display 1 reconnected".to_owned();

    owner
        .replace_published_snapshot(replacement.clone())
        .unwrap();
    assert_eq!(owner.published(), &replacement);
    assert_eq!(owner.connection_epoch(), 3);

    let mut stale = replacement.clone();
    stale.topology_epoch = 7;
    assert!(matches!(
        owner.replace_published_snapshot(stale),
        Err(LiveOutputAuthorityOwnerError::StalePublishedSnapshot)
    ));
    let mut aliased = replacement.clone();
    aliased.heads[0].label = "same epoch, different facts".to_owned();
    assert!(matches!(
        owner.replace_published_snapshot(aliased),
        Err(LiveOutputAuthorityOwnerError::StalePublishedSnapshot)
    ));
    owner.replace_published_snapshot(replacement).unwrap();
}

#[test]
fn hardware_publication_cannot_replace_an_active_protocol_candidate() {
    let (capabilities, snapshot) = fixture();
    let mut owner = LiveOutputAuthorityOwner::new(4, snapshot.clone()).unwrap();
    owner
        .admit(
            TransactionId::from_raw(90),
            &split_candidate(&snapshot, 4, OutputTopologyIntent::Apply),
            &capabilities,
        )
        .unwrap();
    let mut replacement = snapshot;
    replacement.topology_epoch = 8;
    assert!(matches!(
        owner.replace_published_snapshot(replacement),
        Err(LiveOutputAuthorityOwnerError::ActiveCandidate)
    ));
}

fn mode(snapshot: &OutputAuthoritySnapshot, head: u64, width: i32) -> DisplayModeId {
    snapshot
        .heads
        .iter()
        .find(|descriptor| descriptor.head.raw() == head)
        .unwrap()
        .modes
        .iter()
        .find(|mode| mode.pixel_size.width == width)
        .unwrap()
        .mode
}

fn split_candidate(
    snapshot: &OutputAuthoritySnapshot,
    connection_epoch: u64,
    intent: OutputTopologyIntent,
) -> OutputV1Proposal {
    OutputV1Proposal {
        connection_epoch,
        candidate: OutputTopologyCandidate {
            base_topology_epoch: snapshot.topology_epoch,
            intent,
            primary_group_index: 0,
            heads: vec![
                OutputHeadTargetProposal {
                    head: DisplayHeadId::from_raw(11),
                    head_generation: snapshot.heads[0].generation,
                    mode: mode(snapshot, 11, 2_560),
                    transform: OutputTransform::Normal,
                    vrr: OutputVrrPolicy::Disabled,
                },
                OutputHeadTargetProposal {
                    head: DisplayHeadId::from_raw(12),
                    head_generation: snapshot.heads[1].generation,
                    mode: mode(snapshot, 12, 1_920),
                    transform: OutputTransform::Normal,
                    vrr: OutputVrrPolicy::Disabled,
                },
            ],
            groups: vec![
                OutputLogicalGroupProposal {
                    output: OutputId::from_raw(1),
                    logical: Rect {
                        x: 0,
                        y: 0,
                        width: 2_560,
                        height: 1_440,
                    },
                    members: vec![OutputGroupMember {
                        head: DisplayHeadId::from_raw(11),
                        mapping: OutputHeadMapping::Exact,
                    }],
                },
                OutputLogicalGroupProposal {
                    output: OutputId::INVALID,
                    logical: Rect {
                        x: 2_560,
                        y: 0,
                        width: 1_920,
                        height: 1_080,
                    },
                    members: vec![OutputGroupMember {
                        head: DisplayHeadId::from_raw(12),
                        mapping: OutputHeadMapping::Exact,
                    }],
                },
            ],
        },
    }
}

fn disable_secondary_candidate(
    snapshot: &OutputAuthoritySnapshot,
    connection_epoch: u64,
) -> OutputV1Proposal {
    OutputV1Proposal {
        connection_epoch,
        candidate: OutputTopologyCandidate {
            base_topology_epoch: snapshot.topology_epoch,
            intent: OutputTopologyIntent::Apply,
            primary_group_index: 0,
            heads: vec![OutputHeadTargetProposal {
                head: DisplayHeadId::from_raw(11),
                head_generation: snapshot.heads[0].generation,
                mode: mode(snapshot, 11, 2_560),
                transform: OutputTransform::Normal,
                vrr: OutputVrrPolicy::Disabled,
            }],
            groups: vec![OutputLogicalGroupProposal {
                output: OutputId::from_raw(1),
                logical: Rect {
                    x: 0,
                    y: 0,
                    width: 2_560,
                    height: 1_440,
                },
                members: vec![OutputGroupMember {
                    head: DisplayHeadId::from_raw(11),
                    mapping: OutputHeadMapping::Exact,
                }],
            }],
        },
    }
}

#[test]
fn live_output_owner_publishes_split_outputs_only_after_every_first_presentation() {
    let (capabilities, snapshot) = fixture();
    let mut owner = LiveOutputAuthorityOwner::new(3, snapshot.clone()).unwrap();
    let proposal = split_candidate(&snapshot, 3, OutputTopologyIntent::Apply);
    assert_eq!(
        owner
            .admit(TransactionId::from_raw(5), &proposal, &capabilities)
            .unwrap(),
        LiveOutputAuthorityAdmission::Prepared
    );
    let effect = owner.active_effect().unwrap();
    assert_eq!(effect.transaction, TransactionId::from_raw(5));
    assert_eq!(effect.base_topology_epoch, 7);
    assert_eq!(effect.candidate_topology_epoch, 8);
    assert_eq!(effect.published_snapshot, snapshot);
    assert_eq!(
        effect.candidate_snapshot,
        owner.active_candidate_snapshot().unwrap().clone()
    );
    assert_ne!(effect.candidate_snapshot, effect.published_snapshot);
    assert_eq!(effect.resolved.targets[0].target_generation, 2);
    assert_eq!(owner.published(), &snapshot);
    let resolved = owner.active_resolved().unwrap();
    assert_eq!(resolved.outputs.len(), 2);
    assert_eq!(resolved.outputs[1].id, OutputId::from_raw(2));
    assert_ne!(resolved.targets[0].output, resolved.targets[1].output);

    owner.mark_prepared(RenderHeadId::from_raw(11)).unwrap();
    owner.mark_prepared(RenderHeadId::from_raw(12)).unwrap();
    owner.begin_apply().unwrap();
    owner.mark_applied(RenderHeadId::from_raw(12)).unwrap();
    owner.mark_applied(RenderHeadId::from_raw(11)).unwrap();
    assert_eq!(
        owner.mark_first_presented(OutputId::from_raw(1)).unwrap(),
        sophia_engine::OutputTopologyTransactionTransition::Accepted
    );
    assert!(matches!(
        owner.settle_terminal(),
        Err(LiveOutputAuthorityOwnerError::NotTerminal)
    ));
    owner.mark_first_presented(OutputId::from_raw(2)).unwrap();
    let settlement = owner.settle_terminal().unwrap();
    assert_eq!(settlement.outcome.kind, OutputV1OutcomeKind::Committed);
    assert_eq!(settlement.outcome.topology_epoch, 8);
    let published = settlement.published_snapshot.unwrap();
    assert_eq!(published.groups.len(), 2);
    assert_eq!(published.primary_output, OutputId::from_raw(1));
    assert_eq!(
        published.heads[0].generation,
        snapshot.heads[0].generation + 1
    );
    assert_eq!(
        published.heads[1].current_mode,
        Some(mode(&snapshot, 12, 1_920))
    );
    assert_eq!(owner.published(), &published);
}

#[test]
fn validation_and_preparation_failure_do_not_consume_fresh_output_identity() {
    let (capabilities, snapshot) = fixture();
    let mut owner = LiveOutputAuthorityOwner::new(4, snapshot.clone()).unwrap();
    let validation = split_candidate(&snapshot, 4, OutputTopologyIntent::ValidateOnly);
    let LiveOutputAuthorityAdmission::Validated(validated) = owner
        .admit(TransactionId::from_raw(1), &validation, &capabilities)
        .unwrap()
    else {
        panic!("validation-only proposal entered apply preparation");
    };
    assert_eq!(validated.transaction, TransactionId::from_raw(1));
    assert_eq!(validated.outcome.kind, OutputV1OutcomeKind::Validated);

    let apply = split_candidate(&snapshot, 4, OutputTopologyIntent::Apply);
    owner
        .admit(TransactionId::from_raw(2), &apply, &capabilities)
        .unwrap();
    assert_eq!(
        owner.active_resolved().unwrap().outputs[1].id,
        OutputId::from_raw(2)
    );
    owner
        .fail(OutputTopologyTransactionFailure::Preparation)
        .unwrap();
    let rejected = owner.settle_terminal().unwrap();
    assert_eq!(rejected.outcome.kind, OutputV1OutcomeKind::Rejected);
    assert_eq!(rejected.outcome.reason, OUTPUT_OUTCOME_REASON_PREPARATION);
    assert_eq!(owner.published(), &snapshot);

    owner
        .admit(TransactionId::from_raw(3), &apply, &capabilities)
        .unwrap();
    assert_eq!(
        owner.active_resolved().unwrap().outputs[1].id,
        OutputId::from_raw(2)
    );
}

#[test]
fn partial_apply_failure_rolls_back_without_publishing_candidate() {
    let (capabilities, snapshot) = fixture();
    let mut owner = LiveOutputAuthorityOwner::new(5, snapshot.clone()).unwrap();
    let apply = split_candidate(&snapshot, 5, OutputTopologyIntent::Apply);
    owner
        .admit(TransactionId::from_raw(9), &apply, &capabilities)
        .unwrap();
    owner.mark_prepared(RenderHeadId::from_raw(11)).unwrap();
    owner.mark_prepared(RenderHeadId::from_raw(12)).unwrap();
    owner.begin_apply().unwrap();
    owner.mark_applied(RenderHeadId::from_raw(11)).unwrap();
    owner.fail(OutputTopologyTransactionFailure::Apply).unwrap();
    owner.mark_rolled_back(RenderHeadId::from_raw(11)).unwrap();
    let settlement = owner.settle_terminal().unwrap();
    assert_eq!(settlement.outcome.kind, OutputV1OutcomeKind::RolledBack);
    assert_eq!(settlement.outcome.reason, OUTPUT_OUTCOME_REASON_APPLY);
    assert!(settlement.published_snapshot.is_none());
    assert_eq!(owner.published(), &snapshot);
}

#[test]
fn disabled_connected_head_remains_required_through_apply_and_rollback() {
    let (capabilities, snapshot) = fixture();
    let mut owner = LiveOutputAuthorityOwner::new(6, snapshot.clone()).unwrap();
    let proposal = disable_secondary_candidate(&snapshot, 6);
    owner
        .admit(TransactionId::from_raw(10), &proposal, &capabilities)
        .unwrap();
    assert_eq!(
        owner.active_resolved().unwrap().disabled_heads,
        vec![sophia_backend_live::LiveOutputAuthorityDisabledHead {
            head: RenderHeadId::from_raw(12),
            target_generation: 2,
        }]
    );
    assert_eq!(
        owner.mark_prepared(RenderHeadId::from_raw(11)).unwrap(),
        sophia_engine::OutputTopologyTransactionTransition::Accepted
    );
    assert_eq!(
        owner.mark_prepared(RenderHeadId::from_raw(12)).unwrap(),
        sophia_engine::OutputTopologyTransactionTransition::PhaseReady
    );
    owner.begin_apply().unwrap();
    assert_eq!(
        owner.mark_applied(RenderHeadId::from_raw(11)).unwrap(),
        sophia_engine::OutputTopologyTransactionTransition::Accepted
    );
    assert_eq!(
        owner.mark_applied(RenderHeadId::from_raw(12)).unwrap(),
        sophia_engine::OutputTopologyTransactionTransition::PhaseReady
    );
}

#[test]
fn physical_observation_batches_are_all_or_none_at_the_authority_boundary() {
    let (capabilities, snapshot) = fixture();
    let mut owner = LiveOutputAuthorityOwner::new(7, snapshot.clone()).unwrap();
    let apply = split_candidate(&snapshot, 7, OutputTopologyIntent::Apply);
    owner
        .admit(TransactionId::from_raw(11), &apply, &capabilities)
        .unwrap();

    assert!(matches!(
        owner.mark_prepared_batch(&[RenderHeadId::from_raw(11), RenderHeadId::from_raw(99),]),
        Err(LiveOutputAuthorityOwnerError::TransactionInvariant)
    ));
    assert_eq!(
        owner.active_phase(),
        Some(sophia_engine::OutputTopologyTransactionPhase::Preparing),
        "the valid prefix of a rejected batch must not be retained"
    );
    assert_eq!(
        owner
            .mark_prepared_batch(&[RenderHeadId::from_raw(11), RenderHeadId::from_raw(12),])
            .unwrap(),
        sophia_engine::OutputTopologyTransactionTransition::PhaseReady
    );
    owner.begin_apply().unwrap();
    assert_eq!(
        owner
            .mark_applied_batch(&[RenderHeadId::from_raw(12), RenderHeadId::from_raw(11),])
            .unwrap(),
        sophia_engine::OutputTopologyTransactionTransition::PhaseReady
    );
    let outputs = owner
        .active_resolved()
        .unwrap()
        .outputs
        .iter()
        .map(|output| output.id)
        .collect::<Vec<_>>();
    assert_eq!(
        owner.mark_first_presented_batch(&outputs).unwrap(),
        sophia_engine::OutputTopologyTransactionTransition::PhaseReady
    );
    assert_eq!(
        owner.settle_terminal().unwrap().outcome.kind,
        OutputV1OutcomeKind::Committed
    );
}
