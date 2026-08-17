// The owner's obligation to re-offer a cycle after a recoverable rejection.
//
// A physical run stranded here: the owner rejected a response as stale, queued
// nothing, and went idle. The client was waiting for the snapshot that rejection
// implies, hit its socket deadline, exited, and the resulting restarts exhausted
// the supervisor budget and killed the session.

use super::*;
use std::collections::{BTreeSet, VecDeque};

use crate::commands::live_session::{
    LivePublicPolicyCause, LiveWmProposalSource, LiveWmRequestAdmission,
    enqueue_public_policy_cause, materialize_public_dirty_cause, public_policy_rearm_after_outcome,
};

fn relayout_cause(outputs: &[u64]) -> LivePublicPolicyCause {
    LivePublicPolicyCause {
        source: LiveWmProposalSource::Relayout,
        cause: sophia_protocol::PolicyRequestCause::SceneChanged,
        affected_outputs: outputs.iter().copied().map(OutputId::from_raw).collect(),
    }
}

fn output_set(outputs: &[u64]) -> BTreeSet<OutputId> {
    outputs.iter().copied().map(OutputId::from_raw).collect()
}

#[test]
fn the_owner_rearms_for_exactly_the_outcomes_a_stateless_client_retries() {
    // The two halves of one contract. A client that recovers by awaiting a
    // fresh snapshot depends on the owner deciding to send one, so these tables
    // must not drift apart.
    for outcome in [
        sophia_protocol::PolicyProjectionOutcome::Committed,
        sophia_protocol::PolicyProjectionOutcome::RejectedStale,
        sophia_protocol::PolicyProjectionOutcome::RejectedInvalid,
        sophia_protocol::PolicyProjectionOutcome::TimedOut,
        sophia_protocol::PolicyProjectionOutcome::Disconnected,
    ] {
        let client_retries = matches!(
            sophia_wm_demo::stateless_reference_projection_decision(outcome),
            sophia_wm_demo::StatelessReferenceProjectionDecision::RetryFreshSnapshot
        );
        assert_eq!(
            public_policy_rearm_after_outcome(outcome),
            client_retries,
            "owner and client disagree about recovering from {outcome:?}"
        );
    }
}

#[test]
fn an_invalid_rejection_does_not_rearm() {
    // The scene did not move, so re-offering the cycle would spin on the same
    // faulty proposal rather than converging.
    assert!(!public_policy_rearm_after_outcome(
        sophia_protocol::PolicyProjectionOutcome::RejectedInvalid
    ));
    assert!(!public_policy_rearm_after_outcome(
        sophia_protocol::PolicyProjectionOutcome::Disconnected
    ));
}

#[test]
fn dirty_outputs_merge_into_one_queued_relayout() {
    let mut queue = VecDeque::from(vec![relayout_cause(&[1])]);
    let mut pending = output_set(&[2, 3]);

    materialize_public_dirty_cause(&mut queue, &mut pending, None);

    assert_eq!(queue.len(), 1, "merging must not grow the queue");
    assert_eq!(
        queue[0].affected_outputs,
        vec![
            OutputId::from_raw(1),
            OutputId::from_raw(2),
            OutputId::from_raw(3)
        ]
    );
    assert!(pending.is_empty());
}

#[test]
fn dirty_outputs_defer_while_a_relayout_is_in_flight() {
    let mut queue = VecDeque::new();
    let mut pending = output_set(&[2]);

    materialize_public_dirty_cause(
        &mut queue,
        &mut pending,
        Some(LiveWmProposalSource::Relayout),
    );

    assert!(queue.is_empty());
    assert_eq!(pending, output_set(&[2]), "deferred outputs are not lost");
}

#[test]
fn dirty_outputs_push_one_relayout_when_none_is_queued() {
    let mut queue = VecDeque::new();
    let mut pending = output_set(&[4, 5]);

    materialize_public_dirty_cause(&mut queue, &mut pending, None);

    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].source, LiveWmProposalSource::Relayout);
    assert_eq!(
        queue[0].cause,
        sophia_protocol::PolicyRequestCause::SceneChanged
    );
    assert!(pending.is_empty());
}

#[test]
fn a_repeated_rearm_never_grows_the_queue_beyond_one_relayout() {
    // A stale storm re-arms on every rejection. The queue must stay bounded.
    let mut queue = VecDeque::new();
    for _ in 0..8 {
        let mut pending = output_set(&[1, 2]);
        materialize_public_dirty_cause(&mut queue, &mut pending, None);
    }
    assert_eq!(queue.len(), 1);
}

#[test]
fn a_duplicate_relayout_admission_merges_the_replacement_output_set() {
    // A topology change enqueues a relayout naming the new live outputs. When
    // one is already queued the admission reports Duplicate, and dropping the
    // replacement would leave the queued cause naming an output that no longer
    // exists — which fails when the owner tries to issue it.
    let mut queue = VecDeque::from(vec![relayout_cause(&[1, 9])]);
    let admission = enqueue_public_policy_cause(&mut queue, None, false, relayout_cause(&[1, 2]));
    assert_eq!(admission, LiveWmRequestAdmission::Duplicate);

    let mut replacement = output_set(&[1, 2]);
    materialize_public_dirty_cause(&mut queue, &mut replacement, None);

    assert_eq!(queue.len(), 1);
    assert!(
        queue[0].affected_outputs.contains(&OutputId::from_raw(2)),
        "the replacement live output must survive a duplicate admission"
    );
}
