use std::sync::mpsc::sync_channel;
use std::time::{Duration, Instant};

use sophia_cli::session_control::{
    SESSION_CONTROL_ACKNOWLEDGEMENT_TIMEOUT, SESSION_CONTROL_CAPACITY,
    SESSION_CONTROL_QUEUE_TIMEOUT, SessionControlFailure, SessionControlQueue,
};
use sophia_protocol::{SurfaceId, TransactionId};
use sophia_x_authority::{
    XAuthorityClientControlAck, XAuthorityClientControlCommand, XAuthorityControlAck,
    XAuthorityControlCommand, XAuthorityControlKind, XAuthorityControlOutcome,
    XServerFrontendClientId,
};

fn surface(index: u32) -> SurfaceId {
    SurfaceId::new(index, 1)
}

fn control(
    client: u64,
    transaction: u64,
    surface: SurfaceId,
    kind: XAuthorityControlKind,
) -> XAuthorityClientControlCommand {
    let transaction = TransactionId::from_raw(transaction);
    let command = match kind {
        XAuthorityControlKind::AdmitSurface => XAuthorityControlCommand::AdmitSurface {
            transaction,
            surface,
            geometry: sophia_protocol::Rect {
                x: 0,
                y: 0,
                width: 800,
                height: 600,
            },
        },
        XAuthorityControlKind::ConfigureSurface => XAuthorityControlCommand::ConfigureSurface {
            transaction,
            surface,
            geometry: sophia_protocol::Rect {
                x: 0,
                y: 0,
                width: 800,
                height: 600,
            },
        },
        XAuthorityControlKind::SetPresentationState => {
            XAuthorityControlCommand::SetPresentationState {
                transaction,
                surface,
                state: sophia_protocol::PolicyPresentationState::default(),
            }
        }
        XAuthorityControlKind::RestorePresentationState => {
            XAuthorityControlCommand::RestorePresentationState {
                transaction,
                surface,
                state: sophia_protocol::PolicyPresentationState::default(),
            }
        }
        XAuthorityControlKind::FocusSurface => XAuthorityControlCommand::FocusSurface {
            transaction,
            surface,
        },
        XAuthorityControlKind::ClearFocus => XAuthorityControlCommand::ClearFocus {
            transaction,
            surface,
        },
        XAuthorityControlKind::CloseSurface => XAuthorityControlCommand::CloseSurface {
            transaction,
            surface,
        },
        XAuthorityControlKind::WithdrawSurface => XAuthorityControlCommand::WithdrawSurface {
            transaction,
            surface,
        },
    };
    XAuthorityClientControlCommand {
        client: XServerFrontendClientId::from_raw(client),
        command,
    }
}

fn acknowledgement(command: XAuthorityClientControlCommand) -> XAuthorityClientControlAck {
    XAuthorityClientControlAck {
        client: command.client,
        acknowledgement: XAuthorityControlAck {
            kind: command.command.kind(),
            transaction: command.command.transaction(),
            surface: command.command.surface(),
            outcome: XAuthorityControlOutcome::Delivered,
        },
    }
}

#[test]
fn configure_and_close_controls_can_be_in_flight_together() {
    let (sender, commands) = sync_channel(SESSION_CONTROL_CAPACITY);
    let (acknowledgements, receiver) = sync_channel(SESSION_CONTROL_CAPACITY);
    let now = Instant::now();
    let mut queue = SessionControlQueue::default();
    queue
        .enqueue(
            control(1, 1, surface(1), XAuthorityControlKind::ConfigureSurface),
            now,
        )
        .unwrap();
    queue
        .enqueue(
            control(1, 2, surface(1), XAuthorityControlKind::CloseSurface),
            now,
        )
        .unwrap();
    let mut completions = Vec::new();
    queue
        .service(&sender, &receiver, now, &mut completions)
        .unwrap();
    let first = commands.try_recv().unwrap();
    let second = commands.try_recv().unwrap();
    acknowledgements.send(acknowledgement(second)).unwrap();
    acknowledgements.send(acknowledgement(first)).unwrap();
    queue
        .service(
            &sender,
            &receiver,
            now + Duration::from_millis(2),
            &mut completions,
        )
        .unwrap();
    assert_eq!(completions.len(), 2);
    assert_eq!(queue.pending_len(), 0);
    assert_eq!(queue.metrics().delivered, 2);
}

#[test]
fn only_configure_controls_may_yield_priority_to_native_frames() {
    let now = Instant::now();
    let mut queue = SessionControlQueue::default();
    queue
        .enqueue(
            control(1, 1, surface(1), XAuthorityControlKind::ConfigureSurface),
            now,
        )
        .unwrap();
    assert!(!queue.has_non_configure_pending());

    queue
        .enqueue(
            control(1, 2, surface(1), XAuthorityControlKind::FocusSurface),
            now,
        )
        .unwrap();
    assert!(queue.has_non_configure_pending());
}

#[test]
fn focus_controls_are_globally_serialized() {
    let (sender, commands) = sync_channel(SESSION_CONTROL_CAPACITY);
    let (acknowledgements, receiver) = sync_channel(SESSION_CONTROL_CAPACITY);
    let now = Instant::now();
    let mut queue = SessionControlQueue::default();
    let first = control(1, 1, surface(1), XAuthorityControlKind::FocusSurface);
    let second = control(2, 2, surface(2), XAuthorityControlKind::ClearFocus);
    queue.enqueue(first, now).unwrap();
    queue.enqueue(second, now).unwrap();
    let mut completions = Vec::new();
    queue
        .service(&sender, &receiver, now, &mut completions)
        .unwrap();
    assert_eq!(commands.try_recv().unwrap(), first);
    assert!(commands.try_recv().is_err());
    acknowledgements.send(acknowledgement(first)).unwrap();
    queue
        .service(
            &sender,
            &receiver,
            now + Duration::from_millis(1),
            &mut completions,
        )
        .unwrap();
    assert_eq!(commands.try_recv().unwrap(), second);
}

#[test]
fn acknowledgement_kind_is_part_of_exact_correlation() {
    let (sender, commands) = sync_channel(SESSION_CONTROL_CAPACITY);
    let (acknowledgements, receiver) = sync_channel(SESSION_CONTROL_CAPACITY);
    let now = Instant::now();
    let mut queue = SessionControlQueue::default();
    let command = control(1, 7, surface(1), XAuthorityControlKind::FocusSurface);
    queue.enqueue(command, now).unwrap();
    let mut completions = Vec::new();
    queue
        .service(&sender, &receiver, now, &mut completions)
        .unwrap();
    let _ = commands.recv().unwrap();
    let mut wrong = acknowledgement(command);
    wrong.acknowledgement.kind = XAuthorityControlKind::ClearFocus;
    acknowledgements.send(wrong).unwrap();
    assert_eq!(
        queue
            .service(&sender, &receiver, now, &mut completions)
            .unwrap_err(),
        SessionControlFailure::UnexpectedAcknowledgement
    );
}

#[test]
fn undispatched_controls_expire_at_the_total_deadline() {
    let (sender, _commands) = sync_channel(0);
    let (_acknowledgements, receiver) = sync_channel(SESSION_CONTROL_CAPACITY);
    let now = Instant::now();
    let mut queue = SessionControlQueue::default();
    queue
        .enqueue(
            control(1, 1, surface(1), XAuthorityControlKind::CloseSurface),
            now,
        )
        .unwrap();
    let mut completions = Vec::new();
    queue
        .service(&sender, &receiver, now, &mut completions)
        .unwrap();
    queue
        .service(
            &sender,
            &receiver,
            now + SESSION_CONTROL_QUEUE_TIMEOUT,
            &mut completions,
        )
        .unwrap();
    assert_eq!(
        completions[0].failure,
        Some(SessionControlFailure::TimedOut)
    );
    assert_eq!(queue.metrics().timed_out, 1);
}

#[test]
fn dispatched_control_gets_an_independent_acknowledgement_deadline() {
    let (sender, commands) = sync_channel(SESSION_CONTROL_CAPACITY);
    let (acknowledgements, receiver) = sync_channel(SESSION_CONTROL_CAPACITY);
    let now = Instant::now();
    let command = control(1, 1, surface(1), XAuthorityControlKind::FocusSurface);
    let mut queue = SessionControlQueue::default();
    queue.enqueue(command, now).unwrap();
    let mut completions = Vec::new();

    let dispatched_at = now + SESSION_CONTROL_QUEUE_TIMEOUT - Duration::from_millis(1);
    queue
        .service(&sender, &receiver, dispatched_at, &mut completions)
        .unwrap();
    assert_eq!(commands.recv().unwrap(), command);
    queue
        .service(
            &sender,
            &receiver,
            dispatched_at + SESSION_CONTROL_ACKNOWLEDGEMENT_TIMEOUT - Duration::from_millis(1),
            &mut completions,
        )
        .unwrap();
    assert!(completions.is_empty());

    acknowledgements.send(acknowledgement(command)).unwrap();
    queue
        .service(
            &sender,
            &receiver,
            dispatched_at + SESSION_CONTROL_ACKNOWLEDGEMENT_TIMEOUT,
            &mut completions,
        )
        .unwrap();
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].failure, None);
    assert_eq!(queue.metrics().timed_out, 0);
}

#[test]
fn dispatch_prerequisite_does_not_consume_the_queue_deadline() {
    let (sender, commands) = sync_channel(SESSION_CONTROL_CAPACITY);
    let (acknowledgements, receiver) = sync_channel(SESSION_CONTROL_CAPACITY);
    let now = Instant::now();
    let command = control(1, 1, surface(1), XAuthorityControlKind::ClearFocus);
    let mut queue = SessionControlQueue::default();
    queue.enqueue(command, now).unwrap();
    let mut completions = Vec::new();

    let prerequisite_completed_at = now + SESSION_CONTROL_QUEUE_TIMEOUT * 2;
    queue
        .service_when(
            &sender,
            &receiver,
            prerequisite_completed_at,
            &mut completions,
            false,
        )
        .unwrap();
    assert!(completions.is_empty());
    assert!(commands.try_recv().is_err());
    queue
        .service_when(
            &sender,
            &receiver,
            prerequisite_completed_at,
            &mut completions,
            true,
        )
        .unwrap();
    assert_eq!(commands.recv().unwrap(), command);

    acknowledgements.send(acknowledgement(command)).unwrap();
    queue
        .service(
            &sender,
            &receiver,
            prerequisite_completed_at + Duration::from_millis(1),
            &mut completions,
        )
        .unwrap();
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].failure, None);
    assert!(completions[0].queue_dwell >= SESSION_CONTROL_QUEUE_TIMEOUT * 2);
}

#[test]
fn duplicate_and_over_capacity_controls_fail_closed() {
    let now = Instant::now();
    let mut queue = SessionControlQueue::default();
    let duplicate = control(1, 1, surface(1), XAuthorityControlKind::CloseSurface);
    queue.enqueue(duplicate, now).unwrap();
    assert_eq!(
        queue.enqueue(duplicate, now).unwrap_err(),
        SessionControlFailure::Duplicate
    );
    for index in 1..SESSION_CONTROL_CAPACITY {
        queue
            .enqueue(
                control(
                    1,
                    index as u64 + 1,
                    surface(index as u32 + 1),
                    XAuthorityControlKind::ConfigureSurface,
                ),
                now,
            )
            .unwrap();
    }
    assert_eq!(
        queue
            .enqueue(
                control(1, 99, surface(99), XAuthorityControlKind::CloseSurface),
                now,
            )
            .unwrap_err(),
        SessionControlFailure::Capacity
    );
}

#[test]
fn rejected_acknowledgement_is_reported_as_a_completion() {
    let (sender, commands) = sync_channel(SESSION_CONTROL_CAPACITY);
    let (acknowledgements, receiver) = sync_channel(SESSION_CONTROL_CAPACITY);
    let now = Instant::now();
    let command = control(1, 1, surface(1), XAuthorityControlKind::CloseSurface);
    let mut queue = SessionControlQueue::default();
    queue.enqueue(command, now).unwrap();
    let mut completions = Vec::new();
    queue
        .service(&sender, &receiver, now, &mut completions)
        .unwrap();
    let _ = commands.recv().unwrap();
    let mut rejected = acknowledgement(command);
    rejected.acknowledgement.outcome = XAuthorityControlOutcome::UnknownSurface;
    acknowledgements.send(rejected).unwrap();
    queue
        .service(
            &sender,
            &receiver,
            now + Duration::from_millis(1),
            &mut completions,
        )
        .unwrap();
    assert_eq!(
        completions[0].failure,
        Some(SessionControlFailure::Rejected(
            XAuthorityControlOutcome::UnknownSurface
        ))
    );
    assert_eq!(queue.metrics().rejected, 1);
    assert!(
        completions[0]
            .failure
            .is_some_and(|failure| failure.is_stale_target_for(completions[0].key.kind))
    );
    assert!(
        completions[0].failure.is_some_and(
            |failure| !failure.is_stale_target_for(XAuthorityControlKind::FocusSurface)
        )
    );

    let gone = control(1, 2, surface(2), XAuthorityControlKind::FocusSurface);
    queue.enqueue(gone, now).unwrap();
    queue
        .service(&sender, &receiver, now, &mut completions)
        .unwrap();
    let _ = commands.recv().unwrap();
    let mut disconnected = acknowledgement(gone);
    disconnected.acknowledgement.outcome = XAuthorityControlOutcome::ClientGone;
    acknowledgements.send(disconnected).unwrap();
    queue
        .service(
            &sender,
            &receiver,
            now + Duration::from_millis(1),
            &mut completions,
        )
        .unwrap();
    assert!(
        completions[1]
            .failure
            .is_some_and(|failure| failure.is_stale_target_for(completions[1].key.kind))
    );
}

#[test]
fn dispatch_barrier_holds_control_until_input_release_is_acknowledged() {
    let (sender, commands) = sync_channel(SESSION_CONTROL_CAPACITY);
    let (_acknowledgements, receiver) = sync_channel(SESSION_CONTROL_CAPACITY);
    let now = Instant::now();
    let command = control(1, 1, surface(1), XAuthorityControlKind::CloseSurface);
    let mut queue = SessionControlQueue::default();
    queue.enqueue(command, now).unwrap();
    let mut completions = Vec::new();

    queue
        .service_when(&sender, &receiver, now, &mut completions, false)
        .unwrap();
    assert!(commands.try_recv().is_err());
    assert_eq!(queue.metrics().dispatched, 0);

    queue
        .service_when(
            &sender,
            &receiver,
            now + Duration::from_millis(1),
            &mut completions,
            true,
        )
        .unwrap();
    assert_eq!(commands.try_recv().unwrap(), command);
}
