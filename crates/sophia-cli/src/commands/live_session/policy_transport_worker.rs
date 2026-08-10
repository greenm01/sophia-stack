use std::sync::mpsc::{
    Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel,
};
use std::thread::JoinHandle;
use std::time::Duration;

use sophia_protocol::{
    PolicyActionRegistration, PolicyConfiguration, PolicyProjectionOutcome,
    PolicyProjectionProposal, PolicyProjectionRequest, PolicySceneSnapshot,
    PolicySessionOperationRequest, TransactionId, decode_wm_v1_policy_projection,
    encode_wm_v1_policy_snapshot,
};
use sophia_runtime::{PolicyClientEvent, PolicyWmSessionTransport, QueuedPolicyProjection};

const POLICY_TRANSPORT_CAPACITY: usize = 1;

pub(super) enum PolicyTransportCommand {
    ConfigurationOutcome {
        transaction: TransactionId,
        generation: u64,
        outcome: PolicyProjectionOutcome,
    },
    Cycle {
        snapshot_transaction: TransactionId,
        request_transaction: TransactionId,
        scene: PolicySceneSnapshot,
        actions: Vec<PolicyActionRegistration>,
        request: PolicyProjectionRequest,
    },
    ProjectionOutcome {
        transaction: TransactionId,
        request_id: u64,
        scene_generation: u64,
        outcome: PolicyProjectionOutcome,
        expect_session_operation: bool,
    },
    SessionOperationOutcome {
        transaction: TransactionId,
        request_id: u64,
        outcome: PolicyProjectionOutcome,
    },
    Stop,
}

pub(super) enum PolicyTransportEvent {
    Negotiated,
    ReadyForCycle,
    Configuration {
        transaction: TransactionId,
        configuration: PolicyConfiguration,
    },
    Dirty(sophia_protocol::PolicyDirtyRequest),
    Projection(PolicyProjectionProposal),
    SessionOperation {
        transaction: TransactionId,
        request: PolicySessionOperationRequest,
    },
    Failed(String),
}

pub(super) struct PolicyTransportWorker {
    commands: Option<SyncSender<PolicyTransportCommand>>,
    events: Receiver<PolicyTransportEvent>,
    thread: Option<JoinHandle<()>>,
}

impl PolicyTransportWorker {
    pub(super) fn new(
        mut transport: PolicyWmSessionTransport,
        connection_epoch: u64,
    ) -> Result<Self, std::io::Error> {
        let (command_sender, command_receiver) = sync_channel(POLICY_TRANSPORT_CAPACITY);
        let (event_sender, event_receiver) = sync_channel(POLICY_TRANSPORT_CAPACITY);
        let thread = std::thread::Builder::new()
            .name("sophia-policy-v1".to_owned())
            .spawn(move || {
                let result = run_policy_transport(
                    &mut transport,
                    connection_epoch,
                    &command_receiver,
                    &event_sender,
                );
                if let Err(error) = result {
                    let _ = event_sender.try_send(PolicyTransportEvent::Failed(error));
                }
                let _ = transport.disconnect();
            })?;
        Ok(Self {
            commands: Some(command_sender),
            events: event_receiver,
            thread: Some(thread),
        })
    }

    pub(super) fn try_command(
        &self,
        command: PolicyTransportCommand,
    ) -> Result<(), PolicyTransportCommand> {
        let Some(commands) = self.commands.as_ref() else {
            return Err(command);
        };
        match commands.try_send(command) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(command) | TrySendError::Disconnected(command)) => Err(command),
        }
    }

    pub(super) fn try_event(&self) -> Result<Option<PolicyTransportEvent>, ()> {
        match self.events.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(()),
        }
    }
}

impl Drop for PolicyTransportWorker {
    fn drop(&mut self) {
        if let Some(commands) = self.commands.take() {
            let _ = commands.try_send(PolicyTransportCommand::Stop);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_policy_transport(
    transport: &mut PolicyWmSessionTransport,
    connection_epoch: u64,
    commands: &Receiver<PolicyTransportCommand>,
    events: &SyncSender<PolicyTransportEvent>,
) -> Result<(), String> {
    transport
        .accept_and_negotiate(connection_epoch, Duration::from_secs(4))
        .map_err(|error| error.to_string())?;
    events
        .send(PolicyTransportEvent::Negotiated)
        .map_err(|_| "policy owner event channel disconnected".to_owned())?;

    let configuration = transport
        .receive_client_event()
        .map_err(|error| error.to_string())?;
    let PolicyClientEvent::Configuration {
        transaction,
        configuration,
    } = configuration
    else {
        return Err("policy client did not configure before its first snapshot".to_owned());
    };
    events
        .send(PolicyTransportEvent::Configuration {
            transaction,
            configuration,
        })
        .map_err(|_| "policy owner event channel disconnected".to_owned())?;

    loop {
        let command = match commands.recv_timeout(Duration::from_millis(10)) {
            Ok(command) => command,
            Err(RecvTimeoutError::Timeout) => {
                match transport
                    .try_receive_client_event()
                    .map_err(|error| error.to_string())?
                {
                    Some(PolicyClientEvent::Dirty { request, .. }) => events
                        .send(PolicyTransportEvent::Dirty(request))
                        .map_err(|_| "policy owner event channel disconnected".to_owned())?,
                    Some(_) => {
                        return Err("policy client sent an out-of-phase control message".to_owned());
                    }
                    None => {}
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err("policy owner command channel disconnected".to_owned());
            }
        };
        match command {
            PolicyTransportCommand::ConfigurationOutcome {
                transaction,
                generation,
                outcome,
            } => {
                transport
                    .send_configuration_outcome(transaction, generation, outcome)
                    .map_err(|error| error.to_string())?;
                if outcome == PolicyProjectionOutcome::Committed {
                    events
                        .send(PolicyTransportEvent::ReadyForCycle)
                        .map_err(|_| "policy owner event channel disconnected".to_owned())?;
                }
            }
            PolicyTransportCommand::Cycle {
                snapshot_transaction,
                request_transaction,
                scene,
                actions,
                request,
            } => {
                let snapshot = encode_wm_v1_policy_snapshot(
                    snapshot_transaction,
                    connection_epoch,
                    &scene,
                    &actions,
                )
                .map_err(|error| format!("policy snapshot encode failed: {error:?}"))?;
                transport
                    .send_snapshot(
                        snapshot.transaction,
                        &snapshot.begin,
                        &snapshot.chunks,
                        &snapshot.end,
                    )
                    .map_err(|error| error.to_string())?;
                transport
                    .send_projection_request(request_transaction, &request)
                    .map_err(|error| error.to_string())?;
                let mut projection_started = false;
                let proposal = loop {
                    match transport
                        .receive_client_event()
                        .map_err(|error| error.to_string())?
                    {
                        PolicyClientEvent::ProjectionPending => projection_started = true,
                        PolicyClientEvent::Projection(QueuedPolicyProjection::Admitted(
                            projection,
                        )) => {
                            break decode_wm_v1_policy_projection(&projection.into_wire_transfer())
                                .map_err(|error| {
                                    format!("policy projection decode failed: {error:?}")
                                })?;
                        }
                        PolicyClientEvent::Dirty { request, .. } if !projection_started => {
                            events
                                .send(PolicyTransportEvent::Dirty(request))
                                .map_err(|_| {
                                    "policy owner event channel disconnected".to_owned()
                                })?;
                        }
                        PolicyClientEvent::Projection(QueuedPolicyProjection::Discarded {
                            ..
                        }) => {
                            return Err("policy projection transfer was discarded".to_owned());
                        }
                        _ => {
                            return Err(
                                "policy client sent a control message during projection transfer"
                                    .to_owned(),
                            );
                        }
                    }
                };
                events
                    .send(PolicyTransportEvent::Projection(proposal))
                    .map_err(|_| "policy owner event channel disconnected".to_owned())?;
            }
            PolicyTransportCommand::ProjectionOutcome {
                transaction,
                request_id,
                scene_generation,
                outcome,
                expect_session_operation,
            } => {
                transport
                    .send_projection_outcome(transaction, request_id, scene_generation, outcome)
                    .map_err(|error| error.to_string())?;
                if expect_session_operation && outcome == PolicyProjectionOutcome::Committed {
                    let event = transport
                        .receive_client_event()
                        .map_err(|error| error.to_string())?;
                    let PolicyClientEvent::SessionOperation {
                        transaction,
                        request,
                    } = event
                    else {
                        return Err(
                            "policy client omitted its committed session operation".to_owned()
                        );
                    };
                    events
                        .send(PolicyTransportEvent::SessionOperation {
                            transaction,
                            request,
                        })
                        .map_err(|_| "policy owner event channel disconnected".to_owned())?;
                } else {
                    events
                        .send(PolicyTransportEvent::ReadyForCycle)
                        .map_err(|_| "policy owner event channel disconnected".to_owned())?;
                }
            }
            PolicyTransportCommand::SessionOperationOutcome {
                transaction,
                request_id,
                outcome,
            } => {
                transport
                    .send_session_operation_outcome(transaction, request_id, outcome)
                    .map_err(|error| error.to_string())?;
                events
                    .send(PolicyTransportEvent::ReadyForCycle)
                    .map_err(|_| "policy owner event channel disconnected".to_owned())?;
            }
            PolicyTransportCommand::Stop => return Ok(()),
        }
    }
}
