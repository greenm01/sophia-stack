use crate::{
    CompositorDisplayCommand as Command, CompositorNodeId, CompositorRect, CompositorRgb8,
    CompositorText, DescriptorOverlayNodeRole as Role, DescriptorOverlayProjection,
};
use sophia_protocol::{Rect, ShellReferenceCandidate, ShellShortcutCatalog};

/// Geometry is Engine-owned. The shell supplies content order and presentation
/// intent, and never receives coordinates or application capabilities.
pub fn reference_sheet_projection(
    candidate: &ShellReferenceCandidate,
    catalog: &ShellShortcutCatalog,
    projection: u64,
    bounds: Rect,
    mut measure: impl FnMut(&str, u16) -> (i32, i32),
) -> Result<(DescriptorOverlayProjection, u16, u16), &'static str> {
    sophia_protocol::validate_shell_reference_candidate(candidate)
        .map_err(|_| "invalid reference candidate")?;
    sophia_protocol::validate_shell_shortcut_catalog(catalog)
        .map_err(|_| "invalid shortcut catalog")?;
    if projection == 0
        || candidate.connection_epoch != catalog.connection_epoch
        || candidate.catalog_generation != catalog.generation
        || bounds.is_empty()
    {
        return Err("stale reference catalog or output");
    }
    let rows = &candidate.entries;
    if rows
        .iter()
        .any(|row| !catalog.entries.iter().any(|e| e.slot == row.slot))
    {
        return Err("unknown shortcut slot");
    }
    let s = &candidate.style;
    let p = i32::from(s.padding);
    let gap = i32::from(s.row_gap);
    let (title_w, title_h) = measure(&s.title, s.title_size);
    let row_h = measure("Mg", s.body_size).1.max(18);
    let available = bounds.height - i32::from(s.margin) * 2;
    let capacity = ((available - p * 3 - title_h + gap) / (row_h + gap)).max(1) as usize;
    let mut cols = rows
        .len()
        .max(1)
        .div_ceil(capacity)
        .min(usize::from(s.columns));
    let tallest = rows.len().max(1).min(capacity) as i32;
    let mut key_w = measure("(not bound)", s.body_size).0;
    let mut label_w = 0;
    for row in rows {
        key_w = key_w.max(measure(&row.key, s.body_size).0);
        label_w = label_w.max(measure(&row.label, s.body_size).0);
    }
    let col_w = key_w + i32::from(s.key_gap) + label_w;
    let width = (p * 2 + cols as i32 * col_w + (cols as i32 - 1) * i32::from(s.column_gap))
        .max(360)
        .min((i64::from(bounds.width) * 9 / 10) as i32);
    // Long labels must not push a later column entirely outside the panel.
    // Reduce column count only when the full key and a label cannot fit.
    let available_column =
        |cols: usize| (width - p * 2 - (cols as i32 - 1) * i32::from(s.column_gap)) / cols as i32;
    while cols > 1 && available_column(cols) < key_w + i32::from(s.key_gap) + i32::from(s.body_size)
    {
        cols -= 1;
    }
    let col_w = col_w.min(available_column(cols));
    let page_capacity = capacity * cols;
    let pages = rows.len().max(1).div_ceil(page_capacity) as u16;
    let page = candidate.page.min(pages - 1);
    let height = p * 3 + title_h + tallest * row_h + (tallest - 1) * gap;
    if width <= p * 2 || height > bounds.height || width < key_w + p * 2 {
        return Err("output too small for reference sheet");
    }
    let geometry = Rect {
        x: bounds.x + (bounds.width - width) / 2,
        y: bounds.y + (bounds.height - height) / 2,
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
    let text = |slot, role, g, label: String, size: u16, color| {
        Command::Text(CompositorText {
            node: node(slot, role),
            generation: projection,
            geometry: g,
            text: label,
            font_size_millis: u32::from(size) * 1000,
            color: rgb(color),
        })
    };
    let mut commands = vec![rect(u16::MAX, Role::Panel, geometry, s.colors[0])];
    let border = i32::from(s.border);
    for (i, g) in [
        Rect {
            height: border,
            ..geometry
        },
        Rect {
            y: geometry.y + height - border,
            height: border,
            ..geometry
        },
        Rect {
            width: border,
            ..geometry
        },
        Rect {
            x: geometry.x + width - border,
            width: border,
            ..geometry
        },
    ]
    .into_iter()
    .enumerate()
    {
        commands.push(rect(i as u16, Role::Panel, g, s.colors[1]));
    }
    commands.push(text(
        u16::MAX,
        Role::Label,
        Rect {
            x: geometry.x + p.max((width - title_w) / 2),
            y: geometry.y + p,
            width: width - p * 2,
            height: title_h,
        },
        s.title.clone(),
        s.title_size,
        s.colors[5],
    ));
    for (i, row) in rows
        .iter()
        .skip(usize::from(page) * page_capacity)
        .take(page_capacity)
        .enumerate()
    {
        let x = geometry.x + p + (i / capacity) as i32 * (col_w + i32::from(s.column_gap));
        let y = geometry.y + p * 2 + title_h + (i % capacity) as i32 * (row_h + gap);
        let remaining = (geometry.x + width - p - x).max(0);
        if remaining == 0 {
            continue;
        }
        commands.push(rect(
            row.slot,
            Role::Row,
            Rect {
                x: x - 8,
                y: y - 5,
                width: (key_w + 16).min(remaining + 8),
                height: row_h + 10,
            },
            s.colors[4],
        ));
        commands.push(text(
            row.slot,
            Role::Label,
            Rect {
                x,
                y,
                width: key_w.min(remaining),
                height: row_h,
            },
            row.key.clone(),
            s.body_size,
            s.colors[2],
        ));
        let label_x = x + key_w + i32::from(s.key_gap);
        let label_width = (col_w - key_w - i32::from(s.key_gap)).max(0);
        if label_width > 0 {
            commands.push(text(
                row.slot,
                Role::Attention,
                Rect {
                    x: label_x,
                    y,
                    width: label_width,
                    height: row_h,
                },
                row.label.clone(),
                s.body_size,
                s.colors[3],
            ));
        }
    }
    Ok((
        DescriptorOverlayProjection {
            output: candidate.output,
            generation: candidate.candidate_generation,
            geometry,
            commands,
            targets: Vec::new(),
        },
        page,
        pages,
    ))
}
