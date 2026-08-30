//! What the run measured, and whether it measured enough to say anything.
//!
//! The roadmap row asks whether a direct frame costs less than a composed
//! one. That is a comparison, so it needs both populations from one session
//! on one head -- which is exactly what the overlay proof produces, direct
//! flips outside its window and composed frames inside it.
//!
//! This checks the shape of the answer, never its values. A threshold on
//! microseconds would fail on a host whose display engine stalls a modeset
//! (this one does), and the row says measure, not gate.

use crate::direct_scanout::{record_after_marker, reject_duplicate_fields};

const MARKER: &str = "sophia_live_direct_scanout_cost schema=1 ";

/// One population's reported distribution.
pub struct PopulationCost {
    pub population: String,
    pub frames: usize,
    pub submit_flip_frames: usize,
    pub offer_submit_p50: u32,
    pub offer_submit_p99: u32,
    pub submit_flip_p50: u32,
    pub saturated: bool,
}

fn field<'a>(record: &'a str, name: &str) -> Option<&'a str> {
    record.split_whitespace().find_map(|field| {
        field
            .split_once('=')
            .filter(|(key, _)| *key == name)
            .map(|(_, value)| value)
    })
}

fn number(record: &str, name: &str, log: &str) -> Result<u32, String> {
    field(record, name)
        .ok_or_else(|| format!("a cost record has no {name}: {log}"))?
        .parse()
        .map_err(|_| format!("a cost record has a malformed {name}: {log}"))
}

/// Read every cost record in one session's evidence.
pub fn read(text: &str, log: &str) -> Result<Vec<PopulationCost>, String> {
    let mut populations = Vec::new();
    for line in text.lines() {
        let Some(record) = record_after_marker(line, MARKER) else {
            continue;
        };
        reject_duplicate_fields(record, "cost").map_err(|error| format!("{error}: {log}"))?;
        let population = field(record, "population")
            .ok_or_else(|| format!("a cost record names no population: {log}"))?
            .to_owned();
        if !["direct", "composed"].contains(&population.as_str()) {
            return Err(format!(
                "a cost record names an unknown population {population:?}: {log}"
            ));
        }
        if populations
            .iter()
            .any(|existing: &PopulationCost| existing.population == population)
        {
            return Err(format!(
                "the {population} population was reported twice: {log}"
            ));
        }
        populations.push(PopulationCost {
            frames: number(record, "frames", log)? as usize,
            submit_flip_frames: number(record, "submit_flip_frames", log)? as usize,
            offer_submit_p50: number(record, "offer_submit_us_p50", log)?,
            offer_submit_p99: number(record, "offer_submit_us_p99", log)?,
            submit_flip_p50: number(record, "submit_flip_us_p50", log)?,
            saturated: field(record, "saturated") == Some("true"),
            population,
        });
    }
    Ok(populations)
}

/// Require a comparison: both populations, each with frames, none truncated.
///
/// Absent records are not this function's business -- a run that predates the
/// instrumentation is a different proof, not a failed one, and the caller
/// decides whether to ask.
pub fn check(text: &str, log: &str) -> Result<Vec<String>, String> {
    let populations = read(text, log)?;
    if populations.is_empty() {
        return Err(format!(
            "this run measured no frame costs, so it cannot answer whether a direct frame costs less than a composed one: {log}"
        ));
    }
    for population in ["direct", "composed"] {
        let Some(measured) = populations
            .iter()
            .find(|candidate| candidate.population == population)
        else {
            return Err(format!(
                "no {population} frames were measured, so there is nothing to compare against: {log}"
            ));
        };
        if measured.frames == 0 || measured.submit_flip_frames == 0 {
            // Both halves or neither. A population measured on one side only
            // is a hole in the instrument, and it is worth naming which side:
            // the offer half being empty while frames reached glass is what
            // happens when an export is filed under the wrong population.
            return Err(format!(
                "the {population} population was measured on only one side ({} offer samples, {} flip samples), so its cost is not comparable: {log}",
                measured.frames, measured.submit_flip_frames
            ));
        }
        if measured.saturated {
            return Err(format!(
                "the {population} population filled its sample reservoir, so its summary describes a prefix of the run rather than the run: {log}"
            ));
        }
    }
    let direct = populations
        .iter()
        .find(|candidate| candidate.population == "direct")
        .expect("checked above");
    let composed = populations
        .iter()
        .find(|candidate| candidate.population == "composed")
        .expect("checked above");
    Ok(vec![format!(
        "sophia_direct_scanout_cost schema=1 status=measured direct_frames={} direct_offer_submit_p50={} direct_offer_submit_p99={} direct_submit_flip_p50={} composed_frames={} composed_offer_submit_p50={} composed_offer_submit_p99={} composed_submit_flip_p50={}",
        direct.frames,
        direct.offer_submit_p50,
        direct.offer_submit_p99,
        direct.submit_flip_p50,
        composed.frames,
        composed.offer_submit_p50,
        composed.offer_submit_p99,
        composed.submit_flip_p50,
    )])
}
