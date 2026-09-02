//! Repository-owned desktop-comparison workloads.

use super::{READY_TIMEOUT, ScheduledSample, elapsed_micros, timing_population, write_new};
use std::fs;
use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

pub(super) struct WorkloadOwner {
    children: Vec<Child>,
    roots: Vec<u32>,
    sockets: Vec<PathBuf>,
    firefox: Option<FixtureServer>,
    resize: Option<thread::JoinHandle<Result<Vec<u64>, String>>>,
    resize_requested: bool,
    launch_usec: u64,
    settle_usec: u64,
    finished: bool,
}

impl WorkloadOwner {
    pub(super) fn launch(
        repo: &Path,
        attempt: &Path,
        scheduled: &ScheduledSample,
        duration: Duration,
    ) -> Result<Self, String> {
        let launched = Instant::now();
        let mut owner = Self {
            children: Vec::new(),
            roots: Vec::new(),
            sockets: Vec::new(),
            firefox: None,
            resize: None,
            resize_requested: scheduled.workload == "resize",
            launch_usec: 0,
            settle_usec: 0,
            finished: false,
        };
        match scheduled.workload.as_str() {
            "firefox-local" => owner.launch_firefox(repo, attempt)?,
            "kitty-burst-16" => {
                for index in 0..16 {
                    owner.launch_kitty(attempt, duration, index)?;
                }
                for socket in &owner.sockets {
                    wait_kitty(socket)?;
                }
            }
            "kitty-60s" | "resize" | "soak-2h" => {
                owner.launch_kitty(attempt, duration, 0)?;
                wait_kitty(&owner.sockets[0])?;
            }
            _ => return Err("prepared schedule contains an unknown workload".to_owned()),
        }
        owner.launch_usec = elapsed_micros(launched);
        owner.settle_usec = owner.launch_usec;
        Ok(owner)
    }

    fn launch_kitty(
        &mut self,
        attempt: &Path,
        duration: Duration,
        index: usize,
    ) -> Result<(), String> {
        let socket = attempt.join(format!("kitty-{index}.sock"));
        let executable = std::env::current_exe()
            .map_err(|error| format!("could not identify xtask executable: {error}"))?;
        let child = Command::new("kitty")
            .arg("--listen-on")
            .arg(format!("unix:{}", socket.display()))
            .arg("--override")
            .arg("allow_remote_control=yes")
            .arg("--class")
            .arg("sophia-desktop-comparison")
            .arg("--title")
            .arg(format!("Sophia comparison {index}"))
            .arg(executable)
            .args([
                "conformance",
                "desktop-comparison",
                "workload",
                "kitty-stream",
                &duration.as_secs().saturating_add(120).to_string(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("could not launch Kitty workload: {error}"))?;
        self.roots.push(child.id());
        self.children.push(child);
        self.sockets.push(socket);
        Ok(())
    }

    fn launch_firefox(&mut self, repo: &Path, attempt: &Path) -> Result<(), String> {
        let fixture =
            fs::read_to_string(repo.join("validation/desktop-comparison/firefox/index.html"))
                .map_err(|error| format!("could not read Firefox fixture: {error}"))?;
        let server = FixtureServer::start(fixture)?;
        let profile = attempt.join("firefox-profile");
        fs::create_dir(&profile)
            .map_err(|error| format!("could not create isolated Firefox profile: {error}"))?;
        fs::copy(
            repo.join("validation/desktop-comparison/firefox/user.js"),
            profile.join("user.js"),
        )
        .map_err(|error| format!("could not stage isolated Firefox profile: {error}"))?;
        let child = Command::new("firefox")
            .args(["--no-remote", "--new-instance", "--profile"])
            .arg(&profile)
            .arg(server.url())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("could not launch Firefox workload: {error}"))?;
        self.roots.push(child.id());
        self.children.push(child);
        server.wait_ready()?;
        self.firefox = Some(server);
        Ok(())
    }

    pub(super) fn begin_measured_work(&mut self) -> Result<(), String> {
        if self.resize_requested {
            let socket = self.sockets[0].clone();
            self.resize = Some(thread::spawn(move || {
                let mut latencies = Vec::with_capacity(120);
                for index in 0..120 {
                    let started = Instant::now();
                    let (width, height) = if index % 2 == 0 {
                        ("1280", "720")
                    } else {
                        ("1600", "900")
                    };
                    let status = Command::new("kitty")
                        .args([
                            "@",
                            "--to",
                            &format!("unix:{}", socket.display()),
                            "resize-os-window",
                            "--unit",
                            "pixels",
                            "--width",
                            width,
                            "--height",
                            height,
                        ])
                        .status()
                        .map_err(|error| format!("could not execute resize workload: {error}"))?;
                    if !status.success() {
                        return Err("Kitty rejected a resize workload request".to_owned());
                    }
                    latencies.push(elapsed_micros(started).max(1));
                    thread::sleep(Duration::from_millis(50));
                }
                Ok(latencies)
            }));
        }
        Ok(())
    }

    pub(super) fn root_pids(&self) -> impl Iterator<Item = u32> + '_ {
        self.roots.iter().copied()
    }

    pub(super) fn exited_early(&mut self) -> Result<bool, String> {
        for child in &mut self.children {
            if child
                .try_wait()
                .map_err(|error| format!("could not poll comparison workload: {error}"))?
                .is_some()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn finish(&mut self, attempt: &Path) -> Result<(), String> {
        let resize = match self.resize.take() {
            Some(handle) => handle
                .join()
                .map_err(|_| "resize workload thread panicked".to_owned())??,
            None => Vec::new(),
        };
        for child in &mut self.children {
            child
                .kill()
                .map_err(|error| format!("could not stop owned comparison workload: {error}"))?;
            let _ = child
                .wait()
                .map_err(|error| format!("could not reap owned comparison workload: {error}"))?;
        }
        self.firefox.take();
        for socket in &self.sockets {
            let _ = fs::remove_file(socket);
        }
        let (p50, p95, p99, maximum) = timing_population(&resize);
        write_new(
            &attempt.join("workload.log"),
            format!(
                "desktop_comparison_workload schema=1 status=complete launch_usec={} settle_usec={} resize_samples={} resize_p50_usec={} resize_p95_usec={} resize_p99_usec={} resize_max_usec={}\n",
                self.launch_usec,
                self.settle_usec,
                resize.len(),
                p50,
                p95,
                p99,
                maximum,
            )
            .as_bytes(),
        )?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for WorkloadOwner {
    fn drop(&mut self) {
        if !self.finished {
            for child in &mut self.children {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

struct FixtureServer {
    address: std::net::SocketAddr,
    ready: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl FixtureServer {
    fn start(fixture: String) -> Result<Self, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| format!("could not bind local Firefox fixture: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("could not configure local Firefox fixture: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("could not read local Firefox fixture address: {error}"))?;
        let ready = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_ready = Arc::clone(&ready);
        let thread_stop = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = serve_fixture(stream, &fixture, &thread_ready);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            address,
            ready,
            stop,
            thread: Some(handle),
        })
    }

    fn url(&self) -> String {
        format!("http://{}/", self.address)
    }

    fn wait_ready(&self) -> Result<(), String> {
        let deadline = Instant::now() + READY_TIMEOUT;
        while Instant::now() < deadline {
            if self.ready.load(Ordering::Relaxed) {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(20));
        }
        Err("Firefox fixture did not report readiness within 30 seconds".to_owned())
    }
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.address);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

fn serve_fixture(
    mut stream: TcpStream,
    fixture: &str,
    ready: &AtomicBool,
) -> Result<(), std::io::Error> {
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    let mut request = [0u8; 2048];
    let count = stream.read(&mut request)?;
    let first = std::str::from_utf8(&request[..count])
        .ok()
        .and_then(|source| source.lines().next())
        .unwrap_or_default();
    let is_ready = first.starts_with("GET /ready ");
    if is_ready {
        ready.store(true, Ordering::Relaxed);
    }
    let body = if is_ready { "ready" } else { fixture };
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        if is_ready {
            "text/plain"
        } else {
            "text/html; charset=utf-8"
        },
        body.len(),
        body,
    )
}

fn wait_kitty(socket: &Path) -> Result<(), String> {
    let deadline = Instant::now() + READY_TIMEOUT;
    while Instant::now() < deadline {
        if Command::new("kitty")
            .args(["@", "--to", &format!("unix:{}", socket.display()), "ls"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
        {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err("Kitty workload did not become remotely controllable within 30 seconds".to_owned())
}

pub fn run_stream(seconds: u64) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut state = 0x9e37_79b9_7f4a_7c15u64 ^ u64::from(std::process::id());
    let mut sequence = 0u64;
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    while Instant::now() < deadline {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        sequence = sequence.saturating_add(1);
        writeln!(output, "{sequence:08} {state:016x}")
            .and_then(|()| output.flush())
            .map_err(|error| format!("Kitty stream output failed: {error}"))?;
        thread::sleep(Duration::from_millis(16));
    }
    Ok(())
}
