//! Acceptance of the isolated, fixed-geometry X11 reference fixture.
//! CPU commits and work-area transitions are not physical presentation evidence.
use crate::direct_scanout::record_after_marker;
use std::collections::{BTreeMap, BTreeSet};

pub fn verify(log: &str) -> Result<String, String> {
    if log.len() > 4 * 1024 * 1024 {
        return Err("panel evidence exceeds 4 MiB".to_owned());
    }
    let mut panel_generations = BTreeSet::new();
    let mut popup_generations = BTreeSet::new();
    let mut panel_surface = None;
    let mut popup_surface = None;
    let mut last_seen = BTreeMap::new();
    let mut last_popup_sample = 0;
    let mut last_panel_sample = 0;
    let mut reservation_stage = 0;
    let mut clean = false;
    let mut protocol_clean = false;
    let mut software = false;
    let mut isolated = false;
    let mut terminal = false;
    for line in log.lines() {
        if line.starts_with("Error:")
            || line.contains("status=rejected reason=")
            || line.contains("status=restart_requested")
            || line.contains("status=detected source=owner_loop")
        {
            return Err("panel evidence contains a session or policy failure".to_owned());
        }
        software |= line.contains("Loading backend software");
        let fields = line
            .split_whitespace()
            .filter_map(|field| field.split_once('='))
            .collect::<BTreeMap<_, _>>();
        if record_after_marker(line, "sophia_live_session schema=").is_some() {
            isolated |= fields.get("native_presentation") == Some(&"disabled")
                && fields.get("physical_input") == Some(&"disabled")
                && fields.get("wm_policy") == Some(&"external");
        }
        if record_after_marker(line, "sophia_live_cpu_surface ").is_some() {
            if fields.get("schema") != Some(&"1") || fields.get("truncated") != Some(&"false") {
                return Err("unsupported or truncated CPU surface evidence".to_owned());
            }
            let value = |name| fields.get(name).ok_or_else(|| format!("missing {name}"));
            let number = |name| -> Result<u64, String> {
                value(name)?.parse().map_err(|_| format!("invalid {name}"))
            };
            let seq = number("seq")?;
            let width = number("width")?;
            let height = number("height")?;
            let x = number("x")?;
            let y = number("y")?;
            let generation = number("buffer_generation")?;
            let surface = *value("surface")?;
            last_seen
                .entry(surface)
                .and_modify(|last: &mut u64| *last = (*last).max(seq))
                .or_insert(seq);
            if fields.get("visual_detail") == Some(&"true") {
                if (x, y, width, height) == (0, 0, 1280, 32) {
                    if panel_surface.is_some_and(|previous| previous != surface) {
                        return Err("panel identity changed during exercise".to_owned());
                    }
                    panel_surface = Some(surface);
                    panel_generations.insert(generation);
                    last_panel_sample = last_panel_sample.max(seq);
                } else if (x, y, width, height) == (1032, 32, 240, 112) {
                    if popup_surface.is_some_and(|previous| previous != surface) {
                        return Err("popup identity changed during exercise".to_owned());
                    }
                    popup_surface = Some(surface);
                    popup_generations.insert(generation);
                    last_popup_sample = last_popup_sample.max(seq);
                } else if y >= 32 && width > 240 && height > 112 {
                    terminal = true;
                }
            }
        }
        if record_after_marker(line, "sophia_live_work_area ").is_some()
            && fields.get("output") == Some(&"1")
        {
            if fields.get("schema") != Some(&"1") || fields.get("shell_reservations") != Some(&"0")
            {
                return Err("unexpected work-area evidence".to_owned());
            }
            let expected = if reservation_stage % 2 == 0 {
                ("32", "688", "1")
            } else {
                ("0", "720", "0")
            };
            if reservation_stage < 4
                && fields.get("y") == Some(&expected.0)
                && fields.get("height") == Some(&expected.1)
                && fields.get("app_reservations") == Some(&expected.2)
                && fields.get("x") == Some(&"0")
                && fields.get("width") == Some(&"1280")
            {
                reservation_stage += 1;
            }
        }
        if record_after_marker(line, "sophia_live_session_cleanup ").is_some() {
            clean |= fields.get("status") == Some(&"clean")
                && fields.get("namespace") == Some(&"revoked")
                && fields.get("app_groups") == Some(&"0")
                && fields.get("frontend_workers") == Some(&"0");
        }
        if record_after_marker(line, "sophia_live_session_protocol_error_tally ").is_some() {
            if fields.get("status") != Some(&"clean") || fields.get("count") != Some(&"0") {
                return Err("panel session reported X11 protocol errors".to_owned());
            }
            protocol_clean = true;
        }
    }
    if let Some(surface) = popup_surface {
        last_popup_sample = last_seen.get(surface).copied().unwrap_or(last_popup_sample);
    }
    if !isolated
        || !software
        || !clean
        || !protocol_clean
        || !terminal
        || panel_generations.len() < 3
        || popup_generations.len() < 2
        || last_panel_sample <= last_popup_sample
        || reservation_stage != 4
    {
        return Err(format!(
            "incomplete panel evidence: isolated={isolated} software={software} cleanup={clean} protocol={protocol_clean} terminal={terminal} panel_updates={} popup_updates={} reservation_steps={reservation_stage}",
            panel_generations.len(),
            popup_generations.len()
        ));
    }
    Ok("sophia_panel_probe schema=1 status=accepted content=cpu_committed reservation_cycle=complete popup=updated_then_withdrawn physical_input=unproven gpu_presentation=unproven".to_owned())
}
