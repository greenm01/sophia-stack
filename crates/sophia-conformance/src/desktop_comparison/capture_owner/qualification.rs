//! One excluded-from-measurement visual cursor qualification.

use super::{
    SessionAttestation, current_uid, elapsed_micros, protect_owner_directory, read_attestation,
    record_fields, session_attestation_path, source_commit, supervisor_identity_is_live,
    validate_active_profile, validate_session,
};
use crate::desktop_comparison::{ScheduledSample, next_scheduled};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};
use x11rb::connection::Connection as _;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, CreateGCAux, CreateWindowAux, EventMask, PropMode, Rectangle,
    WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;

const TARGETS: usize = 4;
const TARGET_EDGE: u16 = 112;
const TIMEOUT: Duration = Duration::from_secs(20);

pub fn qualify(repo: &Path, run: &Path) -> Result<Vec<String>, String> {
    let scheduled = next_scheduled(run)?
        .ok_or_else(|| "desktop comparison matrix is already complete".to_owned())?;
    if scheduled.stack != "sophia" || scheduled.order != 1 {
        return Ok(vec![format!(
            "desktop_comparison_cursor_qualification schema=1 status=not_required order={} stack={}",
            scheduled.order, scheduled.stack
        )]);
    }
    let attestation = read_attestation(&session_attestation_path()?)?;
    validate_session(run, &scheduled, &attestation)?;
    validate_active_profile(repo, run, &attestation)?;
    let candidate = source_commit(run)?;
    let seed = candidate_seed(&candidate)?;
    let (motion_events, clicks, elapsed_usec) = run_window(seed)?;
    if clicks != TARGETS || motion_events == 0 {
        return Err(
            "visual cursor qualification did not observe motion through every target".to_owned(),
        );
    }
    if !supervisor_identity_is_live(&attestation)? {
        return Err("Sophia exited during visual cursor qualification".to_owned());
    }
    write_qualification(
        &scheduled,
        &attestation,
        &candidate,
        motion_events,
        clicks,
        elapsed_usec,
    )?;
    Ok(vec![format!(
        "desktop_comparison_cursor_qualification schema=1 status=passed order={} targets={} motion_events={}",
        scheduled.order, clicks, motion_events
    )])
}

pub(super) fn measurement_fields(
    run: &Path,
    scheduled: &ScheduledSample,
    attestation: &SessionAttestation,
) -> Result<String, String> {
    if scheduled.stack != "sophia" || scheduled.order != 1 {
        return Ok(
            "cursor_qualification=not_required cursor_targets=0 cursor_motion_events=0".to_owned(),
        );
    }
    let path = qualification_path()?;
    let metadata = fs::metadata(&path)
        .map_err(|error| format!("visual cursor qualification is missing: {error}"))?;
    if !metadata.is_file() || metadata.uid() != current_uid()? || metadata.mode() & 0o077 != 0 {
        return Err("visual cursor qualification must be an owner-only regular file".to_owned());
    }
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("could not read visual cursor qualification: {error}"))?;
    let line = super::one_record(
        &source,
        "desktop_comparison_cursor_qualification schema=1 status=passed ",
    )?;
    let fields = record_fields(line)?;
    let required = |name: &str| {
        fields
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| format!("cursor qualification lacks {name}"))
    };
    for (name, expected) in [
        ("candidate", source_commit(run)?),
        ("supervisor_pid", attestation.supervisor_pid.to_string()),
        (
            "supervisor_start_ticks",
            attestation.supervisor_start_ticks.to_string(),
        ),
    ] {
        if required(name)? != expected {
            return Err(format!("cursor qualification mismatches {name}"));
        }
    }
    let targets = required("targets")?
        .parse::<usize>()
        .map_err(|_| "cursor qualification target count is malformed")?;
    let motions = required("motion_events")?
        .parse::<usize>()
        .map_err(|_| "cursor qualification motion count is malformed")?;
    if targets != TARGETS || motions == 0 {
        return Err("cursor qualification lacks complete motion-and-click evidence".to_owned());
    }
    Ok(format!(
        "cursor_qualification=passed cursor_targets={targets} cursor_motion_events={motions}"
    ))
}

fn candidate_seed(candidate: &str) -> Result<u64, String> {
    u64::from_str_radix(
        candidate
            .get(..16)
            .ok_or("comparison candidate identity is too short")?,
        16,
    )
    .map_err(|_| "comparison candidate identity is not hexadecimal".to_owned())
}

fn run_window(mut seed: u64) -> Result<(usize, usize, u64), String> {
    let display = std::env::var("DISPLAY")
        .map_err(|_| "DISPLAY is unset during cursor qualification".to_owned())?;
    let (connection, screen_number) = x11rb::connect(Some(&display))
        .map_err(|error| format!("could not connect cursor qualification client: {error}"))?;
    let screen = connection
        .setup()
        .roots
        .get(screen_number)
        .ok_or("cursor qualification DISPLAY names no screen")?;
    let width = 900u16.min(screen.width_in_pixels.saturating_sub(80));
    let height = 620u16.min(screen.height_in_pixels.saturating_sub(80));
    if width < TARGET_EDGE.saturating_mul(2) || height < TARGET_EDGE.saturating_mul(2) {
        return Err("cursor qualification display is too small".to_owned());
    }
    let window = connection
        .generate_id()
        .map_err(|error| error.to_string())?;
    let font = connection
        .generate_id()
        .map_err(|error| error.to_string())?;
    let background = connection
        .generate_id()
        .map_err(|error| error.to_string())?;
    let target = connection
        .generate_id()
        .map_err(|error| error.to_string())?;
    let instruction = connection
        .generate_id()
        .map_err(|error| error.to_string())?;
    connection
        .create_window(
            screen.root_depth,
            window,
            screen.root,
            40,
            40,
            width,
            height,
            0,
            WindowClass::INPUT_OUTPUT,
            screen.root_visual,
            &CreateWindowAux::new()
                .background_pixel(0x0010_1820)
                .event_mask(
                    EventMask::EXPOSURE
                        | EventMask::STRUCTURE_NOTIFY
                        | EventMask::POINTER_MOTION
                        | EventMask::BUTTON_PRESS,
                ),
        )
        .map_err(|error| error.to_string())?;
    connection
        .open_font(font, b"fixed")
        .map_err(|error| error.to_string())?;
    connection
        .change_property8(
            PropMode::REPLACE,
            window,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            b"Move the visible cursor and click each changing target",
        )
        .map_err(|error| error.to_string())?;
    connection
        .create_gc(
            background,
            window,
            &CreateGCAux::new().foreground(0x0010_1820),
        )
        .map_err(|error| error.to_string())?;
    connection
        .create_gc(target, window, &CreateGCAux::new().foreground(0x0000_ff66))
        .map_err(|error| error.to_string())?;
    connection
        .create_gc(
            instruction,
            window,
            &CreateGCAux::new().foreground(0x00ff_ffff).font(font),
        )
        .map_err(|error| error.to_string())?;
    connection
        .map_window(window)
        .map_err(|error| error.to_string())?;
    connection.flush().map_err(|error| error.to_string())?;

    let started = Instant::now();
    let mut target_index = 0usize;
    let mut motions = 0usize;
    let mut current = next_target(&mut seed, width, height);
    draw(
        &connection,
        window,
        background,
        target,
        instruction,
        width,
        height,
        current,
    )?;
    while started.elapsed() < TIMEOUT && target_index < TARGETS {
        match connection
            .poll_for_event()
            .map_err(|error| error.to_string())?
        {
            Some(Event::MotionNotify(event)) if event.event == window => {
                motions = motions.saturating_add(1);
            }
            Some(Event::ButtonPress(event)) if event.event == window => {
                let x = event.event_x;
                let y = event.event_y;
                if x >= current.x
                    && y >= current.y
                    && x < current.x.saturating_add(TARGET_EDGE as i16)
                    && y < current.y.saturating_add(TARGET_EDGE as i16)
                {
                    target_index = target_index.saturating_add(1);
                    if target_index < TARGETS {
                        current = next_target(&mut seed, width, height);
                        draw(
                            &connection,
                            window,
                            background,
                            target,
                            instruction,
                            width,
                            height,
                            current,
                        )?;
                    }
                }
            }
            Some(Event::Expose(event)) if event.window == window && event.count == 0 => {
                draw(
                    &connection,
                    window,
                    background,
                    target,
                    instruction,
                    width,
                    height,
                    current,
                )?;
            }
            Some(Event::DestroyNotify(event)) if event.window == window => {
                return Err("cursor qualification window was closed".to_owned());
            }
            Some(_) => {}
            None => thread::sleep(Duration::from_millis(5)),
        }
    }
    connection
        .destroy_window(window)
        .map_err(|error| error.to_string())?;
    connection
        .close_font(font)
        .map_err(|error| error.to_string())?;
    connection.flush().map_err(|error| error.to_string())?;
    if target_index != TARGETS {
        return Err(format!(
            "cursor qualification timed out after {target_index}/{TARGETS} targets"
        ));
    }
    Ok((motions, target_index, elapsed_micros(started)))
}

fn next_target(seed: &mut u64, width: u16, height: u16) -> Rectangle {
    *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    let x_span = width.saturating_sub(TARGET_EDGE).saturating_sub(32).max(1);
    let x = 16 + u16::try_from(*seed % u64::from(x_span)).unwrap_or_default();
    *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    let y_span = height.saturating_sub(TARGET_EDGE).saturating_sub(32).max(1);
    let y = 16 + u16::try_from(*seed % u64::from(y_span)).unwrap_or_default();
    Rectangle {
        x: i16::try_from(x).unwrap_or(i16::MAX),
        y: i16::try_from(y).unwrap_or(i16::MAX),
        width: TARGET_EDGE,
        height: TARGET_EDGE,
    }
}

fn draw<C: x11rb::connection::Connection>(
    connection: &C,
    window: u32,
    background: u32,
    target: u32,
    instruction: u32,
    width: u16,
    height: u16,
    current: Rectangle,
) -> Result<(), String> {
    connection
        .poly_fill_rectangle(
            window,
            background,
            &[Rectangle {
                x: 0,
                y: 0,
                width,
                height,
            }],
        )
        .map_err(|error| error.to_string())?;
    connection
        .poly_fill_rectangle(window, target, &[current])
        .map_err(|error| error.to_string())?;
    connection
        .poly_text8(
            window,
            instruction,
            18,
            28,
            b"Move the visible pointer and click each green target (4 total)",
        )
        .map_err(|error| error.to_string())?;
    connection.flush().map_err(|error| error.to_string())
}

fn write_qualification(
    scheduled: &ScheduledSample,
    attestation: &SessionAttestation,
    candidate: &str,
    motions: usize,
    targets: usize,
    elapsed_usec: u64,
) -> Result<(), String> {
    let path = qualification_path()?;
    let parent = path.parent().ok_or("qualification path has no parent")?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create qualification directory: {error}"))?;
    protect_owner_directory(parent)?;
    let partial = parent.join(format!(
        "cursor-qualification.{}.partial",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
        .map_err(|error| format!("could not create cursor qualification: {error}"))?;
    writeln!(
        file,
        "desktop_comparison_cursor_qualification schema=1 status=passed order={} candidate={} supervisor_pid={} supervisor_start_ticks={} targets={} motion_events={} elapsed_usec={}",
        scheduled.order,
        candidate,
        attestation.supervisor_pid,
        attestation.supervisor_start_ticks,
        targets,
        motions,
        elapsed_usec,
    )
    .map_err(|error| format!("could not write cursor qualification: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("could not sync cursor qualification: {error}"))?;
    fs::set_permissions(&partial, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("could not protect cursor qualification: {error}"))?;
    fs::rename(&partial, &path)
        .map_err(|error| format!("could not publish cursor qualification: {error}"))
}

fn qualification_path() -> Result<PathBuf, String> {
    Ok(session_attestation_path()?
        .parent()
        .ok_or("session attestation path has no parent")?
        .join("cursor-qualification.kdl"))
}
