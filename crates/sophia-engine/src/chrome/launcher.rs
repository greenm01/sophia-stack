use crate::{
    CompositorDisplayCommand as Command, CompositorNodeId, CompositorRect, CompositorRgb8,
    CompositorText, DescriptorOverlayNodeRole as Role, DescriptorOverlayProjection,
};
use sophia_protocol::{Rect, ShellApplicationCatalog, ShellLauncherCandidate};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LauncherProjection {
    pub overlay: DescriptorOverlayProjection,
    pub targets: Vec<(u16, Rect)>,
}

/// The catalog supplies labels; a shell candidate can never relabel a launch.
pub fn launcher_projection(
    candidate: &ShellLauncherCandidate,
    catalog: &ShellApplicationCatalog,
    query: &str,
    projection: u64,
    bounds: Rect,
    mut measure: impl FnMut(&str, u16) -> (i32, i32),
) -> Result<LauncherProjection, &'static str> {
    sophia_protocol::validate_shell_launcher_candidate(candidate)
        .map_err(|_| "invalid launcher candidate")?;
    sophia_protocol::validate_shell_application_catalog(catalog)
        .map_err(|_| "invalid application catalog")?;
    if candidate.connection_epoch != catalog.connection_epoch
        || candidate.catalog_generation != catalog.generation
        || projection == 0
        || !sophia_protocol::shell_launcher_text_valid(query, 256)
        || bounds.width < 160
        || bounds.height < 160
    {
        return Err("invalid launcher presentation");
    }
    let entries = candidate
        .entries
        .iter()
        .map(|slot| {
            catalog
                .entries
                .iter()
                .find(|e| e.slot == *slot)
                .ok_or("unknown application slot")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let font = candidate.font_size;
    let row_h = measure("Mg", font).1.max(i32::from(font)) + 12;
    let capacity = ((bounds.height - 112) / row_h).max(1) as usize;
    let selected = entries
        .iter()
        .position(|e| e.slot == candidate.selected)
        .unwrap_or(0);
    let first = selected
        .saturating_sub(capacity / 2)
        .min(entries.len().saturating_sub(capacity));
    let visible = entries
        .iter()
        .skip(first)
        .take(capacity)
        .collect::<Vec<_>>();
    let width = 600.min(bounds.width - 48);
    let height = 80 + row_h * visible.len().max(1) as i32;
    let geometry = Rect {
        x: bounds.x + (bounds.width - width) / 2,
        y: bounds.y + (bounds.height - height) / 3,
        width,
        height,
    };
    let node = |slot, role| CompositorNodeId::DescriptorOverlay {
        projection,
        slot,
        role,
    };
    let rgb = |c: u32| CompositorRgb8 {
        red: (c >> 16) as u8,
        green: (c >> 8) as u8,
        blue: c as u8,
    };
    let rect = |slot, role, g, color: u32| {
        Command::Rect(CompositorRect {
            node: node(slot, role),
            generation: projection,
            geometry: g,
            color: rgb(color),
            opacity: (color >> 24) as u8,
        })
    };
    let text = |slot, g, label: String, color: u32| {
        Command::Text(CompositorText {
            node: node(slot, Role::Label),
            generation: projection,
            geometry: g,
            text: label,
            font_size_millis: u32::from(font) * 1000,
            color: rgb(color),
        })
    };
    let mut commands = vec![rect(u16::MAX, Role::Panel, geometry, candidate.colors[0])];
    commands.push(text(
        u16::MAX,
        Rect {
            x: geometry.x + 20,
            y: geometry.y + 18,
            width: width - 40,
            height: row_h,
        },
        format!("Applications  > {query}"),
        candidate.colors[1],
    ));
    let mut targets = Vec::new();
    for (index, entry) in visible.into_iter().enumerate() {
        let row = Rect {
            x: geometry.x + 12,
            y: geometry.y + 60 + index as i32 * row_h,
            width: width - 24,
            height: row_h,
        };
        let selected = entry.slot == candidate.selected;
        if selected {
            commands.push(rect(entry.slot, Role::Row, row, candidate.colors[2]));
        }
        let label = if entry.available {
            entry.label.clone()
        } else {
            format!("{} (unavailable)", entry.label)
        };
        commands.push(text(
            entry.slot,
            Rect {
                x: row.x + 8,
                y: row.y + 6,
                width: row.width - 16,
                height: row_h - 6,
            },
            label,
            if selected {
                candidate.colors[3]
            } else {
                candidate.colors[1]
            },
        ));
        if entry.available {
            targets.push((entry.slot, row));
        }
    }
    if entries.is_empty() {
        commands.push(text(
            0,
            Rect {
                x: geometry.x + 20,
                y: geometry.y + 60,
                width: width - 40,
                height: row_h,
            },
            "No applications found".into(),
            candidate.colors[1],
        ));
    }
    Ok(LauncherProjection {
        overlay: DescriptorOverlayProjection {
            output: candidate.output,
            generation: candidate.candidate_generation,
            geometry,
            commands,
            targets: Vec::new(),
        },
        targets,
    })
}
