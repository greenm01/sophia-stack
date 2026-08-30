//! Whether a cursor moving over a directly scanned frame stays on hardware.
//!
//! The roadmap asserts that the legacy cursor continues over directly
//! scanned frames on its own ioctl. Every direct-scanout archive agrees and
//! none of them tested it: `moves_coalesced=0`, `hardware_updates=1`, a
//! cursor initialized once and never moved. This checks the case they
//! skipped, because it is both the standing claim and the baseline the
//! atomic cursor plane will have to match.
//!
//! Shape only, never a duration. Motion-to-submit is reported for comparison
//! with the replacement; a threshold on it would fail on a host that stalls
//! modesets, which this one does.

use crate::direct_scanout::{record_after_marker, reject_duplicate_fields};

const PROOF: &str = "sophia_live_direct_scanout_cursor_proof schema=1 status=";
const CURSOR: &str = "sophia_live_session_cursor schema=4 ";

fn field<'a>(record: &'a str, name: &str) -> Option<&'a str> {
    record.split_whitespace().find_map(|field| {
        field
            .split_once('=')
            .filter(|(key, _)| *key == name)
            .map(|(_, value)| value)
    })
}

fn number(record: &str, name: &str, log: &str) -> Result<usize, String> {
    field(record, name)
        .ok_or_else(|| format!("the cursor record has no {name}: {log}"))?
        .parse()
        .map_err(|_| format!("the cursor record has a malformed {name}: {log}"))
}

/// Check one session's cursor motion over direct scanout.
pub fn check(text: &str, log: &str) -> Result<Vec<String>, String> {
    let mut started = None;
    let mut finished = None;
    for (index, line) in text.lines().enumerate() {
        let Some(rest) = record_after_marker(line, PROOF) else {
            continue;
        };
        match rest.split_whitespace().next().unwrap_or_default() {
            "started" if started.is_some() => {
                return Err(format!("the cursor proof started twice: {log}"));
            }
            "started" => started = Some((index, rest.to_owned())),
            "finished" if finished.is_some() => {
                return Err(format!("the cursor proof finished twice: {log}"));
            }
            "finished" => finished = Some((index, rest.to_owned())),
            _ => {}
        }
    }
    let (started_line, _) =
        started.ok_or_else(|| format!("the cursor never moved over a direct frame: {log}"))?;
    let (finished_line, finished_record) = finished.ok_or_else(|| {
        format!("the cursor proof never finished, so its motion was not bounded: {log}")
    })?;
    if finished_line < started_line {
        return Err(format!(
            "the cursor proof finished before it started: {log}"
        ));
    }
    let moves = number(&finished_record, "moves", log)?;
    if moves < 2 {
        return Err(format!(
            "the cursor visited {moves} positions, which is not motion: {log}"
        ));
    }

    // The cursor stayed on hardware for the whole of it. A cursor that fell
    // back to composition would still look like a moving cursor on screen
    // and would mean the opposite of what this proof claims.
    let cursor = text
        .lines()
        .rev()
        .find_map(|line| record_after_marker(line, CURSOR))
        .ok_or_else(|| format!("the session reported no cursor record: {log}"))?;
    reject_duplicate_fields(cursor, "cursor").map_err(|error| format!("{error}: {log}"))?;
    if field(cursor, "path") != Some("legacy_ioctl") {
        return Err(format!(
            "the cursor did not ride the legacy ioctl, so this is not that baseline: {log}"
        ));
    }
    if number(cursor, "hardware_failures", log)? != 0 {
        return Err(format!(
            "the hardware cursor failed while riding over direct frames: {log}"
        ));
    }
    let hardware_updates = number(cursor, "hardware_updates", log)?;
    if hardware_updates < 2 {
        return Err(format!(
            "the hardware cursor was updated {hardware_updates} times, so nothing moved on hardware: {log}"
        ));
    }

    // Flips continued across the motion. Without this the proof is satisfied
    // by a session that moved a cursor while direct scanout had stopped --
    // which is the one outcome it exists to rule out.
    let flipped_after = text
        .lines()
        .enumerate()
        .filter(|(index, line)| {
            *index > finished_line
                && record_after_marker(line, "sophia_live_direct_scanout schema=1 status=")
                    .is_some_and(|rest| rest.starts_with("flipped"))
        })
        .count();
    if flipped_after == 0 {
        return Err(format!(
            "no client buffer reached the plane after the cursor stopped moving, so the cursor may have ended direct scanout: {log}"
        ));
    }

    Ok(vec![format!(
        "sophia_direct_scanout_cursor schema=1 status=rode_hardware moves={moves} hardware_updates={hardware_updates} coalesced={} motion_to_submit_msec={} flips_after={flipped_after}",
        field(cursor, "moves_coalesced").unwrap_or("0"),
        field(cursor, "max_motion_to_submit_msec").unwrap_or("0"),
    )])
}
