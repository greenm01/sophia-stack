use super::*;

pub(super) struct SessionProcessGuard {
    pub(super) child: Option<Child>,
    pub(super) secondary_children: Vec<ManagedSessionChild>,
    pub(super) socket_path: Option<std::path::PathBuf>,
    pub(super) grouped: bool,
}

pub(super) struct ManagedSessionChild {
    pub(super) id: Option<String>,
    pub(super) child: Child,
}

impl ManagedSessionChild {
    pub(super) fn new(id: Option<String>, child: Child) -> Self {
        Self { id, child }
    }
}

pub(super) fn terminate_session_child(
    child: &mut Child,
    grouped: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let leader_exited = child.try_wait()?.is_some();
    if grouped {
        let pid = rustix::process::Pid::from_raw(child.id() as i32)
            .ok_or("session child PID is invalid")?;
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::TERM);
        if leader_exited {
            // A launcher can exit before helpers in its process group. The
            // group remains addressable by its original PGID even after the
            // leader is reaped, so explicitly drain those helpers as well.
            std::thread::sleep(Duration::from_millis(25));
            let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
            return Ok(());
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if child.try_wait()?.is_some() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    } else {
        if leader_exited {
            return Ok(());
        }
        child.kill()?;
    }
    child.wait()?;
    Ok(())
}

impl SessionProcessGuard {
    pub(super) fn new(
        child: Option<Child>,
        secondary_children: Vec<ManagedSessionChild>,
        socket_path: std::path::PathBuf,
        grouped: bool,
    ) -> Self {
        Self {
            child,
            secondary_children,
            socket_path: Some(socket_path),
            grouped,
        }
    }

    pub(super) fn children_mut(&mut self) -> (Option<&mut Child>, &mut Vec<ManagedSessionChild>) {
        (self.child.as_mut(), &mut self.secondary_children)
    }

    pub(super) fn add_secondary_child(&mut self, id: Option<String>, child: Child) {
        self.secondary_children
            .push(ManagedSessionChild::new(id, child));
    }

    pub(super) fn terminate(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(mut child) = self.child.take() {
            terminate_session_child(&mut child, self.grouped)?;
        }
        for mut child in self.secondary_children.drain(..) {
            terminate_session_child(&mut child.child, self.grouped)?;
        }
        if let Some(socket_path) = self.socket_path.as_ref() {
            match std::fs::remove_file(socket_path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}

impl Drop for SessionProcessGuard {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}
