//! The return to composition, read out of one session's evidence.
//!
//! `PresentFlipOwnership.tla` says an activation ends the eligibility episode,
//! that the frame the plane is scanning is retired by a *composed* successor
//! rather than evicted, and that eligibility returns only through a fresh
//! proof and a fresh atomic test. Those are four separate claims and the
//! counters alone cannot separate them: totals say a session flipped and
//! tested, not that it stopped flipping while an overlay was up.
//!
//! The proof control brackets the window with `activated` and `withdrawn`
//! records, and everything below is read against those brackets.

/// One session's overlay-proof window, located in the evidence.
struct Window {
    activated: usize,
    withdrawn: usize,
}

/// Find the bracketing records, or say the run never opened an overlay.
fn window(text: &str, log: &str) -> Result<Window, String> {
    let mut activated = None;
    let mut withdrawn = None;
    for (index, line) in text.lines().enumerate() {
        let Some(rest) = crate::direct_scanout::record_after_marker(
            line,
            "sophia_live_direct_scanout_overlay_proof schema=1 status=",
        ) else {
            continue;
        };
        match rest.split_whitespace().next().unwrap_or_default() {
            "activated" if activated.is_some() => {
                return Err(format!(
                    "the overlay opened twice, so its window cannot be paired: {log}"
                ));
            }
            "activated" => activated = Some(index),
            "withdrawn" if withdrawn.is_some() => {
                return Err(format!("the overlay closed twice: {log}"));
            }
            "withdrawn" => withdrawn = Some(index),
            _ => {}
        }
    }
    let activated = activated
        .ok_or_else(|| format!("the overlay never activated, so nothing returned: {log}"))?;
    let withdrawn = withdrawn.ok_or_else(|| {
        format!("the overlay never withdrew, so re-eligibility was never asked for: {log}")
    })?;
    if withdrawn < activated {
        return Err(format!("the overlay withdrew before it opened: {log}"));
    }
    Ok(Window {
        activated,
        withdrawn,
    })
}

/// Episode records, with the line they appeared on.
///
/// Located by marker rather than by prefix: episode records are emitted
/// through `tracing`, which decorates them with a timestamp and ANSI colour
/// in a verbose session log. Prefix-matching saw only bare lines, which was
/// none of them -- the fresh-test rule refused a run whose evidence plainly
/// contained the test, and `episode_sessions=0` in every earlier gate summary
/// was this same blindness passing vacuously.
fn episodes(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines().enumerate().filter_map(|(index, line)| {
        crate::direct_scanout::record_after_marker(
            line,
            "sophia_live_direct_scanout schema=1 status=",
        )
        .and_then(|rest| rest.split_whitespace().next())
        .map(|status| (index, status))
    })
}

/// Check one session's return to composition and back.
pub fn check(text: &str, log: &str) -> Result<Vec<String>, String> {
    let window = window(text, log)?;

    // Nothing may flip while the overlay is up. An activation ends the
    // eligibility episode, so a flip inside the window would be a frame that
    // reached the plane under a stamp the activation invalidated.
    if let Some((line, _)) = episodes(text)
        .filter(|(index, status)| {
            *status == "flipped" && *index > window.activated && *index < window.withdrawn
        })
        .next()
    {
        return Err(format!(
            "a client buffer reached the plane at line {line} while the overlay was up: {log}"
        ));
    }

    // The overlay's own frames had to be composed, and the verdict says which
    // command disqualified them. Without this the window could be satisfied by
    // a session that simply stopped drawing.
    let composed = text
        .lines()
        .filter(|line| {
            line.contains("sophia_live_direct_scanout_geometry schema=2")
                && line.contains("status=composition_required")
        })
        .count();
    if composed == 0 {
        return Err(format!(
            "no frame was refused for a painting command, so the overlay drew nothing: {log}"
        ));
    }

    // The displaced direct frame is retired by a composed successor. That path
    // -- `idle_superseded_direct_present` on a composed retirement -- is the
    // one the model calls `SuccessorComposedRetires`, and a run that never
    // retired inside the window never exercised it.
    let retired = text
        .lines()
        .enumerate()
        .filter(|(index, line)| {
            *index > window.activated
                && *index < window.withdrawn
                && line.contains("sophia_live_session_present schema=2 status=retired")
        })
        .count();
    if retired == 0 {
        return Err(format!(
            "no frame retired while the overlay was up, so no composed successor took the plane: {log}"
        ));
    }

    // Eligibility returns only through a fresh test. The exporter clears its
    // episode on any non-direct export, so a flip after the withdrawal that
    // was not preceded by a `test_passed` would mean the stamp survived the
    // overlay it should not have survived.
    let mut tested_after = false;
    for (index, status) in episodes(text) {
        if index <= window.withdrawn {
            continue;
        }
        match status {
            "test_passed" => tested_after = true,
            "flipped" if !tested_after => {
                return Err(format!(
                    "a flip at line {index} resumed without a fresh validating commit: {log}"
                ));
            }
            _ => {}
        }
    }
    if !tested_after {
        return Err(format!(
            "no validating commit followed the withdrawal, so re-eligibility was never proven: {log}"
        ));
    }

    Ok(vec![format!(
        "sophia_direct_scanout_overlay schema=1 status=returned composed_refusals={composed} retirements_in_window={retired}"
    )])
}
