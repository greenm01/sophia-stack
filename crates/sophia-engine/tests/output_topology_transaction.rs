use sophia_engine::*;
use sophia_protocol::OutputId;

fn head(raw: u64) -> RenderHeadId {
    RenderHeadId::from_raw(raw)
}

#[test]
fn topology_waits_for_prepare_apply_and_every_first_presentation() {
    let mut transaction = OutputTopologyTransaction::new(
        7,
        8,
        [head(1), head(2), head(3)],
        [OutputId::from_raw(1), OutputId::from_raw(2)],
    )
    .unwrap();
    assert_eq!(
        transaction.begin_apply(),
        OutputTopologyTransactionTransition::OutOfOrder
    );
    transaction.mark_prepared(head(1));
    transaction.mark_prepared(head(2));
    assert_eq!(
        transaction.mark_prepared(head(3)),
        OutputTopologyTransactionTransition::PhaseReady
    );
    transaction.begin_apply();
    transaction.mark_applied(head(3));
    transaction.mark_applied(head(1));
    assert_eq!(
        transaction.mark_applied(head(2)),
        OutputTopologyTransactionTransition::PhaseReady
    );
    assert_eq!(
        transaction.phase(),
        OutputTopologyTransactionPhase::AwaitingFirstPresentation
    );
    assert_eq!(
        transaction.mark_first_presented(OutputId::from_raw(2)),
        OutputTopologyTransactionTransition::Accepted
    );
    assert_eq!(
        transaction.mark_first_presented(OutputId::from_raw(1)),
        OutputTopologyTransactionTransition::PhaseReady
    );
    assert_eq!(
        transaction.phase(),
        OutputTopologyTransactionPhase::Committed
    );
}

#[test]
fn partial_apply_failure_can_only_roll_back() {
    let mut transaction =
        OutputTopologyTransaction::new(2, 3, [head(1), head(2)], [OutputId::from_raw(1)]).unwrap();
    transaction.mark_prepared(head(1));
    transaction.mark_prepared(head(2));
    transaction.begin_apply();
    transaction.mark_applied(head(1));
    transaction.fail(OutputTopologyTransactionFailure::Apply);
    assert_eq!(
        transaction.phase(),
        OutputTopologyTransactionPhase::RollingBack
    );
    assert_eq!(
        transaction.mark_first_presented(OutputId::from_raw(1)),
        OutputTopologyTransactionTransition::OutOfOrder
    );
    assert_eq!(
        transaction.mark_rolled_back(head(1)),
        OutputTopologyTransactionTransition::PhaseReady
    );
    assert_eq!(
        transaction.phase(),
        OutputTopologyTransactionPhase::RolledBack
    );
}

#[test]
fn preparation_failure_never_enters_physical_apply() {
    let mut transaction =
        OutputTopologyTransaction::new(2, 3, [head(1), head(2)], [OutputId::from_raw(1)]).unwrap();
    transaction.mark_prepared(head(1));
    transaction.fail(OutputTopologyTransactionFailure::Preparation);
    assert_eq!(transaction.phase(), OutputTopologyTransactionPhase::Failed);
    assert_eq!(
        transaction.begin_apply(),
        OutputTopologyTransactionTransition::Terminal
    );
}
