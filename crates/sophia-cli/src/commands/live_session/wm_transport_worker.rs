use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use sophia_engine::WmSocketTransport;
use sophia_protocol::{TransactionId, WmRequestPacket, WmResponsePacket};

const WM_TRANSPORT_WORK_CAPACITY: usize = 1;

struct WmTransportWork {
    request: WmRequestPacket,
}

pub(super) struct WmTransportCompletion {
    pub(super) transaction: TransactionId,
    pub(super) result: Result<WmResponsePacket, String>,
    pub(super) elapsed: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WmTransportSubmitError {
    Busy,
    Disconnected,
}

pub(super) struct WmTransportWorker {
    work: Option<SyncSender<WmTransportWork>>,
    completions: Receiver<WmTransportCompletion>,
    thread: Option<JoinHandle<()>>,
}

impl WmTransportWorker {
    pub(super) fn new(mut transport: WmSocketTransport) -> Result<Self, std::io::Error> {
        let (work_sender, work_receiver) =
            sync_channel::<WmTransportWork>(WM_TRANSPORT_WORK_CAPACITY);
        let (completion_sender, completion_receiver) = sync_channel(WM_TRANSPORT_WORK_CAPACITY);
        let thread = std::thread::Builder::new()
            .name("sophia-wm-transport".to_owned())
            .spawn(move || {
                while let Ok(work) = work_receiver.recv() {
                    let transaction = work.request.transaction;
                    let started = Instant::now();
                    let result = transport
                        .request(&work.request)
                        .map_err(|error| error.to_string());
                    let completion = WmTransportCompletion {
                        transaction,
                        result,
                        elapsed: started.elapsed(),
                    };
                    if completion_sender.send(completion).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            work: Some(work_sender),
            completions: completion_receiver,
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
}

impl Drop for WmTransportWorker {
    fn drop(&mut self) {
        self.work.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
