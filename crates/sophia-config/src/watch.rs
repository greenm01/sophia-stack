use std::ffi::OsStr;
use std::fmt;
use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
use std::thread;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(25);
const QUIET_DEBOUNCE: Duration = Duration::from_millis(100);
const MAX_DEBOUNCE: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigWatchEvent {
    Changed,
    Overflow,
}

#[derive(Debug)]
pub enum ConfigWatchError {
    MissingParent,
    MissingFilename,
    Initialize(String),
    AddWatch(String),
}

impl fmt::Display for ConfigWatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingParent => formatter.write_str("configuration path has no parent"),
            Self::MissingFilename => formatter.write_str("configuration path has no filename"),
            Self::Initialize(error) => write!(formatter, "initialize config watcher: {error}"),
            Self::AddWatch(error) => write!(formatter, "watch config directory: {error}"),
        }
    }
}

impl std::error::Error for ConfigWatchError {}

pub struct ConfigWatcher {
    receiver: Receiver<ConfigWatchEvent>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl ConfigWatcher {
    pub fn spawn(path: &Path) -> Result<Self, ConfigWatchError> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or(ConfigWatchError::MissingParent)?
            .to_path_buf();
        let filename = path
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or(ConfigWatchError::MissingFilename)?
            .to_owned();
        let (sender, receiver) = sync_channel(8);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = spawn_platform_watcher(parent, filename, sender, worker_stop)?;
        Ok(Self {
            receiver,
            stop,
            worker: Some(worker),
        })
    }

    pub fn try_recv(&self) -> Result<ConfigWatchEvent, TryRecvError> {
        self.receiver.try_recv()
    }
}

impl Drop for ConfigWatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(target_os = "linux")]
fn spawn_platform_watcher(
    parent: PathBuf,
    filename: std::ffi::OsString,
    sender: SyncSender<ConfigWatchEvent>,
    stop: Arc<AtomicBool>,
) -> Result<thread::JoinHandle<()>, ConfigWatchError> {
    use std::os::unix::ffi::OsStrExt;

    use rustix::fs::inotify::{self, CreateFlags, WatchFlags};

    let inotify = inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK)
        .map_err(|error| ConfigWatchError::Initialize(error.to_string()))?;
    inotify::add_watch(
        &inotify,
        parent.as_os_str().as_bytes(),
        WatchFlags::ATTRIB
            | WatchFlags::CLOSE_WRITE
            | WatchFlags::CREATE
            | WatchFlags::DELETE
            | WatchFlags::MOVED_FROM
            | WatchFlags::MOVED_TO,
    )
    .map_err(|error| ConfigWatchError::AddWatch(error.to_string()))?;

    Ok(thread::spawn(move || {
        let mut first_change = None;
        let mut last_change = None;
        while !stop.load(Ordering::Acquire) {
            let observed = drain_linux_events(&inotify, &filename);
            let now = Instant::now();
            match observed {
                LinuxWatchObservation::Changed => {
                    first_change.get_or_insert(now);
                    last_change = Some(now);
                }
                LinuxWatchObservation::Overflow => {
                    let _ = sender.try_send(ConfigWatchEvent::Overflow);
                    first_change = None;
                    last_change = None;
                }
                LinuxWatchObservation::Idle => {}
            }
            if first_change.is_some_and(|first| now.duration_since(first) >= MAX_DEBOUNCE)
                || last_change.is_some_and(|last| now.duration_since(last) >= QUIET_DEBOUNCE)
            {
                let _ = sender.try_send(ConfigWatchEvent::Changed);
                first_change = None;
                last_change = None;
            }
            thread::sleep(POLL_INTERVAL);
        }
    }))
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LinuxWatchObservation {
    Idle,
    Changed,
    Overflow,
}

#[cfg(target_os = "linux")]
fn drain_linux_events(inotify: &rustix::fd::OwnedFd, filename: &OsStr) -> LinuxWatchObservation {
    use std::os::unix::ffi::OsStrExt;

    use rustix::fs::inotify::{ReadFlags, Reader};
    use rustix::io::Errno;

    let mut buffer = [MaybeUninit::uninit(); 4_096];
    let mut reader = Reader::new(inotify, &mut buffer);
    let mut observation = LinuxWatchObservation::Idle;
    loop {
        match reader.next() {
            Ok(event) if event.events().contains(ReadFlags::QUEUE_OVERFLOW) => {
                return LinuxWatchObservation::Overflow;
            }
            Ok(event)
                if event
                    .file_name()
                    .is_some_and(|name| name.to_bytes() == filename.as_bytes()) =>
            {
                observation = LinuxWatchObservation::Changed;
            }
            Ok(_) => {}
            Err(Errno::AGAIN) => return observation,
            Err(_) => return LinuxWatchObservation::Overflow,
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn spawn_platform_watcher(
    _parent: PathBuf,
    _filename: std::ffi::OsString,
    _sender: SyncSender<ConfigWatchEvent>,
    _stop: Arc<AtomicBool>,
) -> Result<thread::JoinHandle<()>, ConfigWatchError> {
    Err(ConfigWatchError::Initialize(
        "hot reload currently requires Linux inotify".to_owned(),
    ))
}
