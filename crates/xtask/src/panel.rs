//! Opt-in X11 reference client launcher. Physical session ownership stays outside xtask.
use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn run(root: &Path, arguments: &[String]) -> Result<(), String> {
    let mut renderer = "gpu";
    let mut probe = false;
    let mut quickshell = std::env::var_os("SOPHIA_QUICKSHELL_BIN").map(PathBuf::from);
    let mut wm = std::env::var_os("SOPHIA_HAGIA_BIN").map(PathBuf::from);
    let mut display = None;
    let mut directory = None;
    for argument in arguments {
        if argument == "--probe" {
            probe = true;
        } else if let Some(value) = argument.strip_prefix("--renderer=") {
            renderer = value;
        } else if let Some(value) = argument.strip_prefix("--quickshell=") {
            quickshell = Some(PathBuf::from(value));
        } else if let Some(value) = argument.strip_prefix("--wm=") {
            wm = Some(PathBuf::from(value));
        } else if let Some(value) = argument.strip_prefix("--display=") {
            display = Some(value.to_owned());
        } else if let Some(value) = argument.strip_prefix("--output=") {
            directory = Some(PathBuf::from(value));
        } else {
            return Err(format!("unknown panel option {argument:?}"));
        }
    }
    if !matches!(renderer, "gpu" | "software") {
        return Err("panel renderer must be gpu or software".to_owned());
    }
    if probe && renderer != "software" {
        return Err(
            "the isolated probe requires --renderer=software; use live mode for GPU".to_owned(),
        );
    }
    if !probe && (display.is_some() || wm.is_some()) {
        return Err(
            "--display and --wm belong to --probe; live mode inherits the current session"
                .to_owned(),
        );
    }
    if !probe && std::env::var_os("DISPLAY").is_none() {
        return Err(
            "live mode needs DISPLAY and the current session's Xauthority environment".to_owned(),
        );
    }
    let quickshell = quickshell
        .ok_or("set SOPHIA_QUICKSHELL_BIN or --quickshell=/absolute/path")?
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let wm = if probe {
        Some(
            wm.ok_or("--probe needs --wm=/absolute/path/to/hagia")?
                .canonicalize()
                .map_err(|error| error.to_string())?,
        )
    } else {
        None
    };
    let directory = directory.unwrap_or_else(|| {
        std::env::temp_dir().join(format!(
            "sophia-panel-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ))
    });
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .map_err(|error| error.to_string())?;
    let directory = directory
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let fixture = root.join("tools/fixtures/quickshell_sophia/shell.qml");
    let fixture_copy = directory.join("shell.qml");
    private_copy(&fixture, &fixture_copy)?;
    let mut identity = format!(
        "schema=1\nmode={}\nrequested_renderer={renderer}\nquickshell={}\n",
        if probe { "isolated" } else { "live" },
        quickshell.display()
    );
    for path in [&quickshell, &fixture_copy] {
        identity.push_str(&digest(path)?);
    }
    let version = Command::new(&quickshell)
        .arg("--version")
        .output()
        .map_err(|error| error.to_string())?;
    identity.push_str(&String::from_utf8_lossy(&version.stdout));
    identity.push_str(&String::from_utf8_lossy(&version.stderr));
    let mut command = if let Some(wm) = wm {
        let sophia = root.join("target/debug/sophia");
        identity.push_str(&digest(&sophia)?);
        identity.push_str(&digest(&wm)?);
        let core = directory.join("core.kdl");
        let desktop = directory.join("desktop.kdl");
        private_copy(
            &root.join("tools/fixtures/quickshell_sophia/core.kdl"),
            &core,
        )?;
        private_copy(
            &root.join("tools/fixtures/quickshell_sophia/desktop.kdl"),
            &desktop,
        )?;
        let mut command = Command::new(sophia);
        command
            .args([
                "session",
                "run",
                "--no-input",
                "--session-mode=normal",
                "--session-start=panel",
                "--session-app=terminal=/usr/bin/xterm",
                "--session-app=browser=/usr/bin/firefox",
                "--session-action-app=terminal=terminal",
                "--session-action-app=browser=browser",
                "--session-start=terminal",
                "--session-app-arg=terminal=-cm",
                "--session-app-arg=terminal=-dc",
                "--session-app-arg=terminal=-fn",
                "--session-app-arg=terminal=6x13",
                "--session-app-arg=terminal=-e",
                "--session-app-arg=terminal=/bin/sh",
                "--session-app-arg=terminal=-c",
                "--session-app-arg=terminal=printf 'Sophia panel witness\n'; sleep 12",
                "--max-runtime-ms=15000",
                "--wm-interface=sophia_wm_v1",
            ])
            .arg(format!("--config={}", core.display()))
            .arg(format!("--desktop-profile={}", desktop.display()))
            .arg(format!("--wm-process={}", wm.display()))
            .arg(format!("--session-app=panel={}", quickshell.display()))
            .arg("--session-app-arg=panel=--path")
            .arg(format!(
                "--session-app-arg=panel={}",
                fixture_copy.display()
            ))
            .arg(format!(
                "--display={}",
                display.unwrap_or_else(|| format!(":{}", 20000 + std::process::id() % 30000))
            ))
            .env("SOPHIA_PANEL_EXERCISE", "1");
        command
    } else {
        let mut command = Command::new(quickshell);
        command
            .arg("--path")
            .arg(&fixture_copy)
            .env_remove("SOPHIA_PANEL_EXERCISE");
        command
    };
    command
        .env_remove("WAYLAND_DISPLAY")
        .env("QT_QPA_PLATFORM", "xcb")
        .env("QSG_INFO", "1")
        .env(
            "QT_LOGGING_RULES",
            "qt.scenegraph.general=true;qt.rhi.general=true",
        );
    if renderer == "software" {
        command
            .env("QT_QUICK_BACKEND", "software")
            .env_remove("QSG_RHI_BACKEND");
    } else {
        command
            .env_remove("QT_QUICK_BACKEND")
            .env("QSG_RHI_BACKEND", "opengl")
            .env_remove("LIBGL_ALWAYS_SOFTWARE");
    }
    identity.push_str(&format!("command={command:?}\n"));
    fs::write(directory.join("identity.txt"), identity).map_err(|error| error.to_string())?;
    let log = File::create(directory.join("session.log")).map_err(|error| error.to_string())?;
    super::print_lines(vec![format!("Panel evidence: {}", directory.display())]);
    super::print_lines(vec![format!(
        "Requested renderer: {renderer}. Inspect session.log for the actual device/backend; this is not GPU acceptance."
    )]);
    let status = command
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|error| error.to_string())?)
        .stderr(log)
        .status()
        .map_err(|error| error.to_string())?;
    fs::write(directory.join("exit.txt"), format!("{status}\n"))
        .map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(format!(
            "panel run failed: {status}; see {}",
            directory.display()
        ));
    }
    if probe {
        let log =
            fs::read_to_string(directory.join("session.log")).map_err(|error| error.to_string())?;
        let verdict = sophia_conformance::panel::verify(&log)?;
        fs::write(directory.join("verdict.txt"), &verdict).map_err(|error| error.to_string())?;
        super::print_lines(vec![verdict]);
    }
    Ok(())
}

fn private_copy(source: &Path, destination: &Path) -> Result<(), String> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destination)
        .map_err(|error| error.to_string())?;
    std::io::copy(
        &mut File::open(source).map_err(|error| error.to_string())?,
        &mut output,
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn digest(path: &Path) -> Result<String, String> {
    let result = Command::new("sha256sum")
        .arg("--")
        .arg(path)
        .output()
        .map_err(|error| error.to_string())?;
    if !result.status.success() {
        return Err(format!("cannot fingerprint {}", path.display()));
    }
    Ok(String::from_utf8_lossy(&result.stdout).into_owned())
}
