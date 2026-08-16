use sophia_engine::{
    TransactionPresentationCohort, TransactionPresentationTerminal,
    TransactionPresentationTransition, applicable_output_retirement_set,
};
use sophia_protocol::{OutputId, Rect, TransactionId};

#[test]
fn applicable_output_set_includes_both_sides_of_a_cross_output_move() {
    let output_a = OutputId::from_raw(3);
    let output_b = OutputId::from_raw(9);
    let viewports = [
        (
            output_a,
            Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            },
        ),
        (
            output_b,
            Rect {
                x: 100,
                y: 0,
                width: 100,
                height: 100,
            },
        ),
    ];

    assert_eq!(
        applicable_output_retirement_set(
            &viewports,
            Some(Rect {
                x: 70,
                y: 10,
                width: 20,
                height: 20,
            }),
            Rect {
                x: 110,
                y: 10,
                width: 20,
                height: 20,
            },
        )
        .unwrap(),
        vec![output_a, output_b]
    );
}

#[test]
fn transaction_presentation_joins_independent_output_retirements_at_latest_time() {
    let output_a = OutputId::from_raw(3);
    let output_b = OutputId::from_raw(9);
    let transaction = TransactionId::from_raw(77);
    let mut cohort = TransactionPresentationCohort::new(transaction, [output_a, output_b]).unwrap();

    assert_eq!(
        cohort.mark_submitted(output_a),
        TransactionPresentationTransition::Accepted
    );
    assert_eq!(
        cohort.mark_retired(output_a, 900),
        TransactionPresentationTransition::Accepted
    );
    assert_eq!(
        cohort.mark_submitted(output_b),
        TransactionPresentationTransition::PhaseReady
    );
    assert_eq!(
        cohort.mark_retired(output_b, 1_200),
        TransactionPresentationTransition::PhaseReady
    );
    assert_eq!(
        cohort.terminal(),
        Some(TransactionPresentationTerminal::Presented {
            logical_sequence: transaction.raw(),
            ust_usec: 1_200,
        })
    );
}

#[test]
fn transaction_presentation_rejects_unknown_duplicate_and_early_retirement() {
    let output = OutputId::from_raw(3);
    let unknown = OutputId::from_raw(9);
    let mut cohort =
        TransactionPresentationCohort::new(TransactionId::from_raw(77), [output]).unwrap();

    assert_eq!(
        cohort.mark_retired(output, 1),
        TransactionPresentationTransition::RetiredBeforeSubmission
    );
    assert_eq!(
        cohort.mark_submitted(unknown),
        TransactionPresentationTransition::UnknownOutput
    );
    assert_eq!(
        cohort.mark_submitted(output),
        TransactionPresentationTransition::PhaseReady
    );
    assert_eq!(
        cohort.mark_submitted(output),
        TransactionPresentationTransition::Duplicate
    );
}
