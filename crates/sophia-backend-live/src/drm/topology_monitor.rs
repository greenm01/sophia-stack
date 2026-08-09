use std::ffi::OsStr;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::thread::JoinHandle;

use rustix::event::{PollFd, PollFlags, Timespec, poll};

const DRM_TOPOLOGY_MONITOR_POLL_MSEC: i64 = 50;

fn saturating_increment(counter: &AtomicU64) {
    let _ = counter.fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
        Some(value.saturating_add(1))
    });
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveDrmTopologyRescanNotice {
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LiveDrmTopologyMonitorStats {
    pub observed: u64,
    pub coalesced: u64,
    pub delivered: u64,
}

pub struct LiveDrmTopologyMonitor {
    ready: Receiver<()>,
    health: Receiver<Result<(), String>>,
    stop: Arc<AtomicBool>,
    latest_sequence: Arc<AtomicU64>,
    observed: Arc<AtomicU64>,
    coalesced: Arc<AtomicU64>,
    delivered: u64,
    worker: Option<JoinHandle<()>>,
}

impl LiveDrmTopologyMonitor {
    pub fn open() -> io::Result<Self> {
        let (notice_sender, ready) = sync_channel(1);
        let (startup_sender, startup_receiver) = sync_channel(1);
        let (health_sender, health) = sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let latest_sequence = Arc::new(AtomicU64::new(0));
        let observed = Arc::new(AtomicU64::new(0));
        let coalesced = Arc::new(AtomicU64::new(0));
        let worker_stop = Arc::clone(&stop);
        let worker_sequence = Arc::clone(&latest_sequence);
        let worker_observed = Arc::clone(&observed);
        let worker_coalesced = Arc::clone(&coalesced);
        let worker = std::thread::spawn(move || {
            let monitor = udev::MonitorBuilder::new()
                .and_then(|builder| builder.match_subsystem("drm"))
                .and_then(udev::MonitorBuilder::listen);
            let monitor = match monitor {
                Ok(monitor) => {
                    let _ = startup_sender.send(Ok(()));
                    monitor
                }
                Err(error) => {
                    let _ = startup_sender.send(Err(error.to_string()));
                    return;
                }
            };
            let result = run_drm_topology_monitor(
                monitor,
                notice_sender,
                &worker_stop,
                &worker_sequence,
                &worker_observed,
                &worker_coalesced,
            );
            let _ = health_sender.try_send(result);
        });
        match startup_receiver.recv_timeout(std::time::Duration::from_secs(2)) {
            Ok(Ok(())) => Ok(Self {
                ready,
                health,
                stop,
                latest_sequence,
                observed,
                coalesced,
                delivered: 0,
                worker: Some(worker),
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(io::Error::other(error))
            }
            Err(_) => {
                stop.store(true, Ordering::Release);
                let _ = worker.join();
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "DRM topology monitor startup timed out",
                ))
            }
        }
    }

    pub fn poll_notice(&mut self) -> io::Result<Option<LiveDrmTopologyRescanNotice>> {
        self.worker_error()?;
        match self.ready.try_recv() {
            Ok(()) => {
                self.delivered = self.delivered.saturating_add(1);
                Ok(Some(LiveDrmTopologyRescanNotice {
                    sequence: self.latest_sequence.load(Ordering::Acquire),
                }))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                self.worker_error()?;
                Ok(None)
            }
        }
    }

    pub fn stats(&self) -> LiveDrmTopologyMonitorStats {
        LiveDrmTopologyMonitorStats {
            observed: self.observed.load(Ordering::Acquire),
            coalesced: self.coalesced.load(Ordering::Acquire),
            delivered: self.delivered,
        }
    }

    fn worker_error(&self) -> io::Result<()> {
        match self.health.try_recv() {
            Ok(Ok(())) | Err(TryRecvError::Empty) => Ok(()),
            Ok(Err(error)) => Err(io::Error::other(error)),
            Err(TryRecvError::Disconnected) if self.stop.load(Ordering::Acquire) => Ok(()),
            Err(TryRecvError::Disconnected) => {
                Err(io::Error::other("DRM topology monitor disconnected"))
            }
        }
    }
}

impl Drop for LiveDrmTopologyMonitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run_drm_topology_monitor(
    monitor: udev::MonitorSocket,
    sender: SyncSender<()>,
    stop: &AtomicBool,
    latest_sequence: &AtomicU64,
    observed: &AtomicU64,
    coalesced: &AtomicU64,
) -> Result<(), String> {
    let timeout = Timespec {
        tv_sec: 0,
        tv_nsec: DRM_TOPOLOGY_MONITOR_POLL_MSEC * 1_000_000,
    };
    while !stop.load(Ordering::Acquire) {
        let mut fds = [PollFd::new(&monitor, PollFlags::IN)];
        poll(&mut fds, Some(&timeout)).map_err(|error| error.to_string())?;
        for event in monitor.iter() {
            if event.event_type() != udev::EventType::Change
                || event.property_value("HOTPLUG") != Some(OsStr::new("1"))
            {
                continue;
            }
            let mut current = latest_sequence.load(Ordering::Acquire);
            loop {
                let next = current
                    .checked_add(1)
                    .ok_or("DRM topology notice sequence exhausted")?;
                match latest_sequence.compare_exchange_weak(
                    current,
                    next,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => break,
                    Err(observed_current) => current = observed_current,
                }
            }
            saturating_increment(observed);
            match sender.try_send(()) {
                Ok(()) => {}
                Err(TrySendError::Full(())) => {
                    saturating_increment(coalesced);
                }
                Err(TrySendError::Disconnected(())) => return Ok(()),
            }
        }
    }
    Ok(())
}

mod tests;
