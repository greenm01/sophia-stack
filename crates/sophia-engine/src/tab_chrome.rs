use crate::{
    CompositorDisplayCommand, CompositorNodeId, CompositorRect, CompositorRgb8, CompositorText,
    PresentedChromeTarget, PresentedChromeTargetId,
};
use sophia_protocol::{OutputId, PolicyTabGroup, Rect, ShellTabGroup};

mod types;
pub use types::*;

/// Geometry and selection are already committed WM policy. Shell descriptors
/// supply labels and actions only; absent descriptors produce inert numbers.
pub fn tab_bar_projection(
    group: &PolicyTabGroup,
    generation: u64,
    descriptors: Option<&ShellTabGroup>,
) -> TabBarProjection {
    let mut result = TabBarProjection {
        policy: group.clone(),
        output: group.output,
        group: group.group,
        generation,
        geometry: group.geometry,
        commands: Vec::new(),
        targets: Vec::new(),
    };
    let count = group.members.len().max(1);
    for index in 0..count {
        let left = i64::from(group.geometry.width) * index as i64 / count as i64;
        let right = i64::from(group.geometry.width) * (index + 1) as i64 / count as i64;
        if right == left {
            continue;
        }
        let geometry = Rect {
            x: group.geometry.x + left as i32,
            width: (right - left) as i32,
            ..group.geometry
        };
        let selected =
            group.members.get(index).copied() == group.selected && group.selected.is_some();
        let d = descriptors.and_then(|g| g.entries.get(index));
        let slot = index as u16;
        result
            .commands
            .push(CompositorDisplayCommand::Rect(CompositorRect {
                opacity: 255,
                node: CompositorNodeId::TabBar {
                    output: group.output,
                    group: group.group,
                    slot,
                    label: false,
                },
                generation,
                geometry,
                color: if selected && group.focused {
                    CompositorRgb8 {
                        red: 48,
                        green: 92,
                        blue: 140,
                    }
                } else if selected {
                    CompositorRgb8 {
                        red: 65,
                        green: 65,
                        blue: 70,
                    }
                } else {
                    CompositorRgb8 {
                        red: 30,
                        green: 30,
                        blue: 34,
                    }
                },
            }));
        let text = d
            .and_then(|d| d.label.as_ref())
            .map(|l| l.text.clone())
            .unwrap_or_else(|| {
                if group.members.is_empty() {
                    "Empty frame".into()
                } else {
                    (index + 1).to_string()
                }
            });
        if geometry.width > 8 && !text.is_empty() {
            result
                .commands
                .push(CompositorDisplayCommand::Text(CompositorText {
                    node: CompositorNodeId::TabBar {
                        output: group.output,
                        group: group.group,
                        slot,
                        label: true,
                    },
                    // The raster key includes text, geometry, font and color.
                    // A new action candidate alone must not rerasterize labels.
                    generation: 1,
                    geometry: Rect {
                        x: geometry.x + 4,
                        width: geometry.width - 8,
                        height: geometry.height.min(24),
                        ..geometry
                    },
                    text,
                    font_size_millis: 12_000,
                    color: if d
                        .is_some_and(|d| d.attention != sophia_protocol::AttentionState::None)
                    {
                        CompositorRgb8 {
                            red: 255,
                            green: 190,
                            blue: 80,
                        }
                    } else if d
                        .is_some_and(|d| d.trust_level == sophia_protocol::TrustLevel::Isolated)
                    {
                        CompositorRgb8 {
                            red: 170,
                            green: 200,
                            blue: 255,
                        }
                    } else {
                        CompositorRgb8 {
                            red: 230,
                            green: 230,
                            blue: 235,
                        }
                    },
                }));
        }
        if let Some(d) = d {
            result.targets.push(PresentedChromeTarget {
                id: PresentedChromeTargetId {
                    authority_session_epoch: d.action.recipient_epoch,
                    slot: d.slot,
                    generation,
                },
                output: group.output,
                geometry,
                action: d.action,
            });
        }
    }
    result
}

pub fn append_tab_bars(
    commands: &mut Vec<CompositorDisplayCommand>,
    groups: &[PolicyTabGroup],
    generation: u64,
    bars: &[TabBarProjection],
    output: OutputId,
) {
    // Bars precede client surfaces so floating clients occlude them naturally.
    let mut chrome = Vec::new();
    for group in groups.iter().filter(|g| g.output == output) {
        let bar = bars
            .iter()
            .find(|b| b.output == output && b.policy == *group);
        chrome.extend(
            bar.cloned()
                .unwrap_or_else(|| tab_bar_projection(group, generation.max(1), None))
                .commands,
        );
    }
    chrome.append(commands);
    *commands = chrome;
}

pub fn tab_rects_overlap(a: Rect, b: Rect) -> bool {
    i64::from(a.x) < i64::from(b.x) + i64::from(b.width)
        && i64::from(b.x) < i64::from(a.x) + i64::from(a.width)
        && i64::from(a.y) < i64::from(b.y) + i64::from(b.height)
        && i64::from(b.y) < i64::from(a.y) + i64::from(a.height)
}
