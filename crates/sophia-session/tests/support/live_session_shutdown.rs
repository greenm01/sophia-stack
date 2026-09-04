#![cfg(test)]

use super::super::{
    AuthorityIngressState, SessionQuiescence, SessionQuiescenceDecision, SessionQuiescenceSnapshot,
    XAuthorityObservedTransactionBatch, XServerFrontendServiceCommand,
    disconnect_frontend_for_drain, drain_queued_authority_batches, observe_authority_ingress,
    stop_frontend_intake,
};
use sophia_protocol::TransactionId;
use std::collections::VecDeque;
use std::sync::mpsc::sync_channel;
use std::time::{Duration, Instant};

fn batch(transaction: u64) -> XAuthorityObservedTransactionBatch {
    super::super::wm_update_coordinator_batch(TransactionId::from_raw(transaction))
}

#[test]
fn opportunistic_drain_preserves_final_batch_before_disconnect() {
    let (sender, receiver) = sync_channel(2);
    sender.send(batch(1)).unwrap();
    sender.send(batch(2)).unwrap();
    drop(sender);
    assert_eq!(
        receiver.recv().unwrap().transaction,
        TransactionId::from_raw(1)
    );

    let mut queued = VecDeque::new();
    assert_eq!(
        drain_queued_authority_batches(&receiver, &mut queued, 2, Duration::from_millis(20),),
        AuthorityIngressState::Disconnected
    );
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].transaction, TransactionId::from_raw(2));
}

#[test]
fn quiescence_accepts_disconnect_only_after_buffered_work_settles() {
    let now = Instant::now();
    let mut quiescence = Some(SessionQuiescence::new(
        "test",
        now,
        Duration::from_millis(20),
    ));

    observe_authority_ingress(
        AuthorityIngressState::Disconnected,
        &mut quiescence,
        now + Duration::from_millis(1),
    )
    .unwrap();
    let quiescence = quiescence.unwrap();
    assert_eq!(
        quiescence.decision(
            now + Duration::from_millis(2),
            SessionQuiescenceSnapshot {
                pending_authority_batches: 1,
                ..SessionQuiescenceSnapshot::default()
            },
        ),
        SessionQuiescenceDecision::Pending
    );
    assert_eq!(
        quiescence.decision(
            now + Duration::from_millis(3),
            SessionQuiescenceSnapshot::default(),
        ),
        SessionQuiescenceDecision::Complete
    );
}

#[test]
fn authority_disconnect_before_quiescence_remains_fatal() {
    let mut quiescence = None;
    assert_eq!(
        observe_authority_ingress(
            AuthorityIngressState::Disconnected,
            &mut quiescence,
            Instant::now(),
        ),
        Err("persistent X authority transaction channel disconnected")
    );
}

#[test]
fn frontend_stop_is_idempotent_after_successful_stop() {
    let (sender, receiver) = sync_channel(1);
    let mut stopped = false;
    stop_frontend_intake(&sender, &mut stopped).unwrap();
    assert!(matches!(
        receiver.recv().unwrap(),
        XServerFrontendServiceCommand::StopAccepting
    ));
    drop(receiver);

    stop_frontend_intake(&sender, &mut stopped).unwrap();
    assert!(stopped);
}

#[test]
fn frontend_drain_disconnect_is_idempotent_after_successful_request() {
    let (sender, receiver) = sync_channel(1);
    let mut stopped = false;
    disconnect_frontend_for_drain(&sender, &mut stopped).unwrap();
    assert!(matches!(
        receiver.recv().unwrap(),
        XServerFrontendServiceCommand::DrainAndDisconnect
    ));
    drop(receiver);

    disconnect_frontend_for_drain(&sender, &mut stopped).unwrap();
    assert!(stopped);
}

#[test]
fn initial_frontend_stop_failure_is_retained() {
    let (sender, receiver) = sync_channel(1);
    drop(receiver);
    let mut stopped = false;

    assert!(stop_frontend_intake(&sender, &mut stopped).is_err());
    assert!(!stopped);
}
