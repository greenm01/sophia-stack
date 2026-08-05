use std::sync::mpsc::{
    Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use sophia_engine::{WmSocketIncoming, WmSocketTransport};
use sophia_protocol::{
    TransactionId, WmPolicyAck, WmPolicyUpdate, WmRequestPacket, WmResponsePacket,
};

const WM_TRANSPORT_WORK_CAPACITY: usize = 1;
const WM_TRANSPORT_POLICY_CAPACITY: usize = 1;
const WM_TRANSPORT_IDLE_POLL: Duration = Duration::from_millis(10);

struct WmTransportWork {
    request: WmRequestPacket,
}

pub(super) struct WmTransportCompletion {
    pub(super) transaction: TransactionId,
    pub(super) result: Result<WmResponsePacket, String>,
    pub(super) elapsed: Duration,
}

pub(super) enum WmTransportPolicyEvent {
    Update(WmPolicyUpdate),
    Failed(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WmTransportSubmitError {
    Busy,
    Disconnected,
}

pub(super) struct WmTransportWorker {
    work: Option<SyncSender<WmTransportWork>>,
    completions: Receiver<WmTransportCompletion>,
    policy_updates: Receiver<WmTransportPolicyEvent>,
    policy_acknowledgements: Option<SyncSender<WmPolicyAck>>,
    thread: Option<JoinHandle<()>>,
}

impl WmTransportWorker {
    pub(super) fn new(mut transport: WmSocketTransport) -> Result<Self, std::io::Error> {
        let response_timeout = transport.response_timeout();
        let (work_sender, work_receiver) =
            sync_channel::<WmTransportWork>(WM_TRANSPORT_WORK_CAPACITY);
        let (completion_sender, completion_receiver) = sync_channel(WM_TRANSPORT_WORK_CAPACITY);
        let (policy_sender, policy_receiver) = sync_channel(WM_TRANSPORT_POLICY_CAPACITY);
        let (policy_ack_sender, policy_ack_receiver) = sync_channel(WM_TRANSPORT_POLICY_CAPACITY);
        let thread = std::thread::Builder::new()
            .name("sophia-wm-transport".to_owned())
            .spawn(move || {
                let mut awaiting_policy_ack = false;
                let mut pending_request: Option<(TransactionId, Instant)> = None;
                loop {
                    if pending_request
                        .as_ref()
                        .is_some_and(|(_, started)| started.elapsed() >= response_timeout)
                    {
                        let (transaction, started) = pending_request
                            .take()
                            .expect("expired WM request should remain pending");
                        let completion = WmTransportCompletion {
                            transaction,
                            result: Err("WM response timed out".to_owned()),
                            elapsed: started.elapsed(),
                        };
                        if completion_sender.send(completion).is_err() {
                            break;
                        }
                    }

                    if awaiting_policy_ack {
                        match policy_ack_receiver.recv_timeout(WM_TRANSPORT_IDLE_POLL) {
                            Ok(ack) => {
                                if let Err(error) = transport.acknowledge_policy_update(ack) {
                                    let _ = policy_sender
                                        .send(WmTransportPolicyEvent::Failed(error.to_string()));
                                    break;
                                }
                                awaiting_policy_ack = false;
                            }
                            Err(RecvTimeoutError::Timeout) => continue,
                            Err(RecvTimeoutError::Disconnected) => break,
                        }
                    }

                    if pending_request.is_none() {
                        match work_receiver.try_recv() {
                            Ok(work) => {
                                let transaction = work.request.transaction;
                                let started = Instant::now();
                                if let Err(error) = transport.send_request(&work.request) {
                                    let completion = WmTransportCompletion {
                                        transaction,
                                        result: Err(error.to_string()),
                                        elapsed: started.elapsed(),
                                    };
                                    if completion_sender.send(completion).is_err() {
                                        break;
                                    }
                                    continue;
                                }
                                pending_request = Some((transaction, started));
                            }
                            Err(TryRecvError::Empty) => {}
                            Err(TryRecvError::Disconnected) => break,
                        }
                    }

                    match transport.poll_incoming(WM_TRANSPORT_IDLE_POLL) {
                        Ok(Some(WmSocketIncoming::PolicyUpdate(update))) => {
                            if policy_sender
                                .send(WmTransportPolicyEvent::Update(update))
                                .is_err()
                            {
                                break;
                            }
                            awaiting_policy_ack = true;
                        }
                        Ok(Some(WmSocketIncoming::Response(response))) => {
                            let Some((transaction, started)) = pending_request.take() else {
                                let _ = policy_sender.send(WmTransportPolicyEvent::Failed(
                                    "WM response arrived without an in-flight request".to_owned(),
                                ));
                                break;
                            };
                            let result = if response.transaction == transaction {
                                Ok(response)
                            } else {
                                Err(format!(
                                    "WM transport completion mismatch: expected={} actual={}",
                                    transaction.raw(),
                                    response.transaction.raw(),
                                ))
                            };
                            let completion = WmTransportCompletion {
                                transaction,
                                result,
                                elapsed: started.elapsed(),
                            };
                            if completion_sender.send(completion).is_err() {
                                break;
                            }
                        }
                        Ok(None) => {}
                        Err(error) => {
                            if let Some((transaction, started)) = pending_request.take() {
                                let _ = completion_sender.send(WmTransportCompletion {
                                    transaction,
                                    result: Err(error.to_string()),
                                    elapsed: started.elapsed(),
                                });
                            }
                            let _ = policy_sender
                                .send(WmTransportPolicyEvent::Failed(error.to_string()));
                            break;
                        }
                    }
                }
            })?;
        Ok(Self {
            work: Some(work_sender),
            completions: completion_receiver,
            policy_updates: policy_receiver,
            policy_acknowledgements: Some(policy_ack_sender),
            thread: Some(thread),
        })
    }

    pub(super) fn try_submit(
        &self,
        request: WmRequestPacket,
    ) -> Result<(), WmTransportSubmitError> {
        self.work
            .as_ref()
            .ok_or(WmTransportSubmitError::Disconnected)?
            .try_send(WmTransportWork { request })
            .map_err(|error| match error {
                TrySendError::Full(_) => WmTransportSubmitError::Busy,
                TrySendError::Disconnected(_) => WmTransportSubmitError::Disconnected,
            })
    }

    pub(super) fn try_complete(
        &self,
    ) -> Result<Option<WmTransportCompletion>, WmTransportSubmitError> {
        match self.completions.try_recv() {
            Ok(completion) => Ok(Some(completion)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(WmTransportSubmitError::Disconnected),
        }
    }

    pub(super) fn try_policy_event(
        &self,
    ) -> Result<Option<WmTransportPolicyEvent>, WmTransportSubmitError> {
        match self.policy_updates.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(WmTransportSubmitError::Disconnected),
        }
    }

    pub(super) fn try_acknowledge_policy(
        &self,
        acknowledgement: WmPolicyAck,
    ) -> Result<(), WmTransportSubmitError> {
        self.policy_acknowledgements
            .as_ref()
            .ok_or(WmTransportSubmitError::Disconnected)?
            .try_send(acknowledgement)
            .map_err(|error| match error {
                TrySendError::Full(_) => WmTransportSubmitError::Busy,
                TrySendError::Disconnected(_) => WmTransportSubmitError::Disconnected,
            })
    }
}

impl Drop for WmTransportWorker {
    fn drop(&mut self) {
        self.work.take();
        self.policy_acknowledgements.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
