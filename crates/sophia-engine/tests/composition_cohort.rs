use sophia_engine::*;
use sophia_protocol::OutputId;

fn head(raw: u64) -> RenderHeadId {
    RenderHeadId::from_raw(raw)
}

fn candidate(head: u64, id: u64, checksum: u64) -> HeadFrameCandidate {
    HeadFrameCandidate {
        candidate: HeadFrameCandidateId::from_raw(id),
        output: OutputId::from_raw(4),
        scene_generation: 9,
        head: RenderHeadId::from_raw(head),
        target_generation: 3,
        logical_content_checksum: checksum,
    }
}

#[test]
fn cohort_requires_all_prepared_before_the_first_submission() {
    let mut cohort =
        OutputPresentationCohort::new(OutputId::from_raw(4), 9, [head(1), head(2)]).unwrap();
    assert_eq!(
        cohort.mark_prepared(candidate(1, 101, 0x55)),
        OutputPresentationTransition::Accepted
    );
    assert_eq!(
        cohort.mark_submitted(head(1)),
        OutputPresentationTransition::PrepareBarrierIncomplete
    );
    assert_eq!(
        cohort.mark_prepared(candidate(2, 102, 0x55)),
        OutputPresentationTransition::PhaseReady
    );
    assert_eq!(cohort.phase(), OutputPresentationPhase::Prepared);
    assert_eq!(
        cohort.mark_submitted(head(1)),
        OutputPresentationTransition::Accepted
    );
    assert_eq!(
        cohort.mark_submitted(head(2)),
        OutputPresentationTransition::PhaseReady
    );
    assert_eq!(cohort.phase(), OutputPresentationPhase::AwaitingFlips);
}

#[test]
fn cohort_joins_reordered_flips_and_cleanup_at_the_latest_ust() {
    let mut cohort =
        OutputPresentationCohort::new(OutputId::from_raw(4), 9, [head(1), head(2)]).unwrap();
    assert!(matches!(
        cohort.mark_prepared(candidate(1, 101, 0x55)),
        OutputPresentationTransition::Accepted
    ));
    assert!(matches!(
        cohort.mark_prepared(candidate(2, 102, 0x55)),
        OutputPresentationTransition::PhaseReady
    ));
    cohort.mark_submitted(head(1));
    cohort.mark_submitted(head(2));
    cohort.mark_flipped(head(2), 200);
    cohort.mark_cleanup_complete(head(2));
    cohort.mark_flipped(head(1), 300);
    assert_eq!(cohort.phase(), OutputPresentationPhase::SettlingCleanup);
    assert_eq!(
        cohort.mark_cleanup_complete(head(1)),
        OutputPresentationTransition::PhaseReady
    );
    assert_eq!(
        cohort.terminal(),
        Some(OutputPresentationTerminal::Presented {
            logical_sequence: 9,
            ust_usec: 300,
        })
    );
}

#[test]
fn cohort_rejects_checksum_and_candidate_identity_disagreement() {
    let mut cohort =
        OutputPresentationCohort::new(OutputId::from_raw(4), 9, [head(1), head(2)]).unwrap();
    cohort.mark_prepared(candidate(1, 101, 0x55));
    assert_eq!(
        cohort.mark_prepared(candidate(2, 102, 0x66)),
        OutputPresentationTransition::LogicalContentMismatch
    );
    assert_eq!(
        cohort.mark_prepared(candidate(2, 101, 0x55)),
        OutputPresentationTransition::CandidateIdentityCollision
    );
}

#[test]
fn lost_head_fails_without_shrinking_the_required_set() {
    let mut cohort =
        OutputPresentationCohort::new(OutputId::from_raw(4), 9, [head(1), head(2)]).unwrap();
    assert_eq!(
        cohort.mark_head_lost(head(2)),
        OutputPresentationTransition::PhaseReady
    );
    assert_eq!(
        cohort.terminal(),
        Some(OutputPresentationTerminal::Failed(
            OutputPresentationFailure::HeadLost(head(2))
        ))
    );
    assert_eq!(cohort.required_heads().count(), 2);
    assert_eq!(
        cohort.mark_prepared(candidate(1, 101, 0x55)),
        OutputPresentationTransition::Terminal
    );
}

#[test]
fn failed_partial_submission_still_drains_accepted_physical_owners() {
    let mut cohort =
        OutputPresentationCohort::new(OutputId::from_raw(4), 9, [head(1), head(2)]).unwrap();
    cohort.mark_prepared(candidate(1, 101, 0x55));
    cohort.mark_prepared(candidate(2, 102, 0x55));
    cohort.mark_submitted(head(1));
    assert!(cohort.fail(OutputPresentationFailure::Submission));
    assert!(!cohort.accepted_owners_settled());
    assert_eq!(
        cohort.mark_flipped(head(1), 700),
        OutputPresentationTransition::Accepted
    );
    assert_eq!(
        cohort.mark_cleanup_complete(head(1)),
        OutputPresentationTransition::PhaseReady
    );
    assert!(cohort.accepted_owners_settled());
    assert_eq!(
        cohort.terminal(),
        Some(OutputPresentationTerminal::Failed(
            OutputPresentationFailure::Submission
        ))
    );
    assert_eq!(
        cohort.mark_flipped(head(2), 800),
        OutputPresentationTransition::NotSubmitted
    );
}
