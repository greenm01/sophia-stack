use std::sync::mpsc::{Receiver, Sender, SyncSender, TryRecvError, channel, sync_channel};
use std::thread::JoinHandle;
use std::time::Duration;

use sophia_protocol::{
    OutputAuthoritySnapshot, OutputV1Outcome, OutputV1OutcomeKind,
    SOPHIA_OUTPUT_OUTCOME_REASON_INVARIANT, TransactionId,
};

use crate::{
    AdmittedOutputProposal, OutputProposalAdmission, OutputSessionTransport, OutputTransportError,
    PolicyRoleEndpointError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutputTransportServiceCommand {
    ReplaceSupervisedPid {
        pid: u32,
    },
    PublishSnapshot {
        transaction: TransactionId,
        snapshot: OutputAuthoritySnapshot,
    },
    Settle {
        transaction: TransactionId,
        outcome: OutputV1Outcome,
    },
    Reply {
        transaction: TransactionId,
        outcome: OutputV1Outcome,
    },
    Stop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutputTransportServiceEvent {
    Connected {
        connection_epoch: u64,
    },
    Proposal {
        proposal: AdmittedOutputProposal,
        admission: OutputProposalAdmission,
    },
    Promoted(AdmittedOutputProposal),
    ProposalRejected {
        transaction: TransactionId,
        message: String,
    },
    Disconnected {
        connection_epoch: u64,
    },
    ConnectionRejected {
        message: String,
    },
    AssigneeReplaced {
        connection_epoch: u64,
        abandoned: Vec<AdmittedOutputProposal>,
    },
    Failed {
        message: String,
    },
}

enum OutputTransportServiceControl {
    PauseAcceptance {
        acknowledged: SyncSender<Vec<AdmittedOutputProposal>>,
    },
}

/// Optional, cancellable transport service for the exclusive output role.
///
/// Absence of a client is normal: the session's static output configuration
/// remains authoritative. The worker polls accept and proposal intake in short
/// bounded turns, so stopping supervision never depends on a WM or shell
/// connecting or completing a frame.
pub struct OutputTransportService {
    commands: Sender<OutputTransportServiceCommand>,
    controls: Sender<OutputTransportServiceControl>,
    events: Receiver<OutputTransportServiceEvent>,
    thread: Option<JoinHandle<()>>,
}

impl OutputTransportService {
    pub fn spawn(
        mut transport: OutputSessionTransport,
        first_connection_epoch: u64,
        snapshot_transaction: TransactionId,
        snapshot: OutputAuthoritySnapshot,
    ) -> Result<Self, std::io::Error> {
        let (command_sender, command_receiver) = channel();
        let (control_sender, control_receiver) = channel();
        let (event_sender, event_receiver) = channel();
        let thread = std::thread::Builder::new()
            .name("sophia-output-v1".to_owned())
            .spawn(move || {
                if let Err(message) = run_output_transport_service(
                    &mut transport,
                    first_connection_epoch,
                    snapshot_transaction,
                    snapshot,
                    &command_receiver,
                    &control_receiver,
                    &event_sender,
                ) {
                    let _ = event_sender.send(OutputTransportServiceEvent::Failed { message });
                }
                if transport.connection().selected_capabilities() != 0 {
                    let _ = transport.disconnect();
                }
            })?;
        Ok(Self {
            commands: command_sender,
            controls: control_sender,
            events: event_receiver,
            thread: Some(thread),
        })
    }

    pub fn command(
        &self,
        command: OutputTransportServiceCommand,
    ) -> Result<(), OutputTransportServiceCommand> {
        self.commands.send(command).map_err(|error| error.0)
    }

    pub fn try_event(&self) -> Result<Option<OutputTransportServiceEvent>, ()> {
        match self.events.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(()),
        }
    }

    pub fn event_timeout(
        &self,
        timeout: Duration,
    ) -> Result<OutputTransportServiceEvent, std::sync::mpsc::RecvTimeoutError> {
        self.events.recv_timeout(timeout)
    }

    /// Stops accepting the old assignee before its replacement is spawned.
    ///
    /// The replacement PID is unknowable until spawn returns. This synchronous
    /// barrier closes the accept race across that interval; a later
    /// `ReplaceSupervisedPid` command installs the new identity and resumes
    /// acceptance.
    pub fn pause_acceptance(
        &self,
        timeout: Duration,
    ) -> Result<Vec<AdmittedOutputProposal>, &'static str> {
        let (acknowledged, acknowledgement) = sync_channel(1);
        self.controls
            .send(OutputTransportServiceControl::PauseAcceptance { acknowledged })
            .map_err(|_| "output service control channel disconnected")?;
        acknowledgement
            .recv_timeout(timeout)
            .map_err(|_| "output service did not pause acceptance before its deadline")
    }
}

impl Drop for OutputTransportService {
    fn drop(&mut self) {
        let _ = self.commands.send(OutputTransportServiceCommand::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_output_transport_service(
    transport: &mut OutputSessionTransport,
    first_connection_epoch: u64,
    mut snapshot_transaction: TransactionId,
    mut snapshot: OutputAuthoritySnapshot,
    commands: &Receiver<OutputTransportServiceCommand>,
    controls: &Receiver<OutputTransportServiceControl>,
    events: &Sender<OutputTransportServiceEvent>,
) -> Result<(), String> {
    if first_connection_epoch == 0 || !snapshot_transaction.is_valid() {
        return Err("output service requires valid initial identities".to_owned());
    }
    snapshot
        .validate()
        .map_err(|error| format!("invalid initial output snapshot: {error:?}"))?;
    let mut connection_epoch = first_connection_epoch;
    let mut connected = false;
    let mut acceptance_paused = false;
    loop {
        while let Ok(control) = controls.try_recv() {
            match control {
                OutputTransportServiceControl::PauseAcceptance { acknowledged } => {
                    let abandoned = if connected {
                        transport.disconnect().map_err(|error| error.to_string())?
                    } else {
                        Vec::new()
                    };
                    connected = false;
                    acceptance_paused = true;
                    let _ = acknowledged.send(abandoned);
                }
            }
        }
        while let Ok(command) = commands.try_recv() {
            match command {
                OutputTransportServiceCommand::ReplaceSupervisedPid { pid } => {
                    let abandoned = if connected {
                        transport.disconnect().map_err(|error| error.to_string())?
                    } else {
                        Vec::new()
                    };
                    connected = false;
                    connection_epoch = connection_epoch
                        .checked_add(1)
                        .ok_or_else(|| "output connection epoch exhausted".to_owned())?;
                    transport
                        .authorize_supervised_pid(pid)
                        .map_err(|error| error.to_string())?;
                    acceptance_paused = false;
                    events
                        .send(OutputTransportServiceEvent::AssigneeReplaced {
                            connection_epoch,
                            abandoned,
                        })
                        .map_err(|_| "output owner event channel disconnected".to_owned())?;
                }
                OutputTransportServiceCommand::PublishSnapshot {
                    transaction,
                    snapshot: replacement,
                } => {
                    if !transaction.is_valid() {
                        return Err(
                            "output snapshot publication has an invalid transaction".to_owned()
                        );
                    }
                    replacement.validate().map_err(|error| {
                        format!("invalid output snapshot publication: {error:?}")
                    })?;
                    snapshot_transaction = transaction;
                    snapshot = replacement;
                    if connected {
                        transport
                            .send_snapshot(snapshot_transaction, &snapshot)
                            .map_err(|error| error.to_string())?;
                    }
                }
                OutputTransportServiceCommand::Settle {
                    transaction,
                    outcome,
                } => {
                    if !connected {
                        return Err("output settlement arrived without a client".to_owned());
                    }
                    transport
                        .send_outcome(transaction, outcome)
                        .map_err(|error| error.to_string())?;
                    if let Some(promoted) = transport
                        .settle_active(transaction)
                        .map_err(|error| error.to_string())?
                        .cloned()
                    {
                        events
                            .send(OutputTransportServiceEvent::Promoted(promoted))
                            .map_err(|_| "output owner event channel disconnected".to_owned())?;
                    }
                }
                OutputTransportServiceCommand::Reply {
                    transaction,
                    outcome,
                } => {
                    if !connected {
                        return Err("output reply arrived without a client".to_owned());
                    }
                    transport
                        .send_outcome(transaction, outcome)
                        .map_err(|error| error.to_string())?;
                }
                OutputTransportServiceCommand::Stop => return Ok(()),
            }
        }

        if acceptance_paused {
            std::thread::sleep(Duration::from_millis(2));
            continue;
        }
        if !connected {
            match transport.accept_and_negotiate(connection_epoch, Duration::from_millis(20)) {
                Ok(()) => {
                    connected = true;
                    transport
                        .send_snapshot(snapshot_transaction, &snapshot)
                        .map_err(|error| error.to_string())?;
                    events
                        .send(OutputTransportServiceEvent::Connected { connection_epoch })
                        .map_err(|_| "output owner event channel disconnected".to_owned())?;
                }
                Err(OutputTransportError::Endpoint(PolicyRoleEndpointError::AcceptTimedOut)) => {}
                Err(error) => {
                    events
                        .send(OutputTransportServiceEvent::ConnectionRejected {
                            message: error.to_string(),
                        })
                        .map_err(|_| "output owner event channel disconnected".to_owned())?;
                }
            }
            continue;
        }

        match transport.try_receive_admitted_proposal(&snapshot) {
            Ok(Some((proposal, admission))) => events
                .send(OutputTransportServiceEvent::Proposal {
                    proposal,
                    admission,
                })
                .map_err(|_| "output owner event channel disconnected".to_owned())?,
            Ok(None) => std::thread::sleep(Duration::from_millis(2)),
            Err(OutputTransportError::ProposalRejected { transaction, error }) => {
                transport
                    .send_outcome(
                        transaction,
                        OutputV1Outcome {
                            connection_epoch,
                            topology_epoch: snapshot.topology_epoch,
                            kind: OutputV1OutcomeKind::Rejected,
                            reason: SOPHIA_OUTPUT_OUTCOME_REASON_INVARIANT,
                        },
                    )
                    .map_err(|send_error| send_error.to_string())?;
                events
                    .send(OutputTransportServiceEvent::ProposalRejected {
                        transaction,
                        message: error.to_string(),
                    })
                    .map_err(|_| "output owner event channel disconnected".to_owned())?
            }
            Err(OutputTransportError::PeerDisconnected) => {
                let _ = transport.disconnect();
                events
                    .send(OutputTransportServiceEvent::Disconnected { connection_epoch })
                    .map_err(|_| "output owner event channel disconnected".to_owned())?;
                connection_epoch = connection_epoch
                    .checked_add(1)
                    .ok_or_else(|| "output connection epoch exhausted".to_owned())?;
                connected = false;
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}
