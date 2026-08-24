use sophia_protocol::{OutputId, Point, Rect, WmActionId};

use crate::PolicyIndicatorPublication;
use crate::{CompositorDisplayCommand, CompositorIndicatorStrip, CompositorNodeId};

pub const INDICATOR_STRIP_HEIGHT: i32 = 14;
pub const INDICATOR_SLOT_MAX_WIDTH: i32 = 96;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndicatorChromeHitTarget {
    pub publication_generation: u64,
    pub connection_epoch: u64,
    pub output: OutputId,
    pub indicator: u64,
    pub action: Option<WmActionId>,
    pub geometry: Rect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndicatorChromeStrip {
    pub output: OutputId,
    pub geometry: Rect,
    pub labels: Vec<(Rect, String, u16)>,
    pub status: Option<(Rect, String, u16)>,
    pub hit_targets: Vec<IndicatorChromeHitTarget>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndicatorChromeAction {
    Activated {
        output: OutputId,
        action: WmActionId,
    },
    Consumed,
    Stale,
    Missed,
}

pub fn reserve_indicator_strip(bounds: Rect) -> Option<Rect> {
    if bounds.width <= 0 || bounds.height <= INDICATOR_STRIP_HEIGHT {
        return None;
    }
    Some(Rect {
        x: bounds.x,
        y: bounds.y.checked_add(INDICATOR_STRIP_HEIGHT)?,
        width: bounds.width,
        height: bounds.height.checked_sub(INDICATOR_STRIP_HEIGHT)?,
    })
}

/// Produces bounded, deterministic chrome geometry without interpreting slot
/// labels. Rendering may clip glyphs, but every committed slot retains a hit
/// rectangle and its opaque action token.
pub fn layout_indicator_strip(
    publication: &PolicyIndicatorPublication,
    output: OutputId,
    bounds: Rect,
) -> Option<IndicatorChromeStrip> {
    let epoch = publication.connection_epoch?;
    if bounds.width <= 0 || bounds.height < INDICATOR_STRIP_HEIGHT {
        return None;
    }
    let geometry = Rect {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: INDICATOR_STRIP_HEIGHT,
    };
    let status_record = publication
        .output_statuses
        .iter()
        .find(|status| status.output == output);
    let status_width = status_record.map_or(0, |_| (bounds.width / 3).clamp(0, 160));
    let indicators = publication
        .indicators
        .iter()
        .filter(|indicator| indicator.output == output)
        .collect::<Vec<_>>();
    let available = bounds.width.saturating_sub(status_width);
    let slot_width = if indicators.is_empty() {
        0
    } else {
        (available / i32::try_from(indicators.len()).unwrap_or(i32::MAX))
            .clamp(1, INDICATOR_SLOT_MAX_WIDTH)
    };
    let mut labels = Vec::with_capacity(indicators.len());
    let mut hit_targets = Vec::with_capacity(indicators.len());
    for (index, indicator) in indicators.into_iter().enumerate() {
        let x = bounds.x.saturating_add(
            i32::try_from(index)
                .unwrap_or(i32::MAX)
                .saturating_mul(slot_width),
        );
        let slot = Rect {
            x,
            y: bounds.y,
            width: slot_width.min(bounds.x.saturating_add(available).saturating_sub(x)),
            height: INDICATOR_STRIP_HEIGHT,
        };
        labels.push((slot, indicator.label.clone(), indicator.state_bits));
        hit_targets.push(IndicatorChromeHitTarget {
            publication_generation: publication.generation,
            connection_epoch: epoch,
            output,
            indicator: indicator.indicator,
            action: indicator.action,
            geometry: slot,
        });
    }
    let status = status_record.map(|status| {
        (
            Rect {
                x: bounds
                    .x
                    .saturating_add(bounds.width.saturating_sub(status_width)),
                y: bounds.y,
                width: status_width,
                height: INDICATOR_STRIP_HEIGHT,
            },
            status.layout.clone(),
            status.focus_bits,
        )
    });
    Some(IndicatorChromeStrip {
        output,
        geometry,
        labels,
        status,
        hit_targets,
    })
}

/// Builds the private Tier-0 display command for one output.
///
/// A disconnected publication retains the opaque reserved strip but has no
/// labels or targets. This keeps policy loss fail closed without making work
/// geometry depend on the replacement policy's first response.
pub fn indicator_strip_display_command(
    publication: &PolicyIndicatorPublication,
    output: OutputId,
    bounds: Rect,
) -> Option<CompositorDisplayCommand> {
    if bounds.width <= 0 || bounds.height < INDICATOR_STRIP_HEIGHT {
        return None;
    }
    let strip = layout_indicator_strip(publication, output, bounds).unwrap_or_else(|| {
        IndicatorChromeStrip {
            output,
            geometry: Rect {
                x: bounds.x,
                y: bounds.y,
                width: bounds.width,
                height: INDICATOR_STRIP_HEIGHT,
            },
            labels: Vec::new(),
            status: None,
            hit_targets: Vec::new(),
        }
    });
    Some(CompositorDisplayCommand::IndicatorStrip(
        CompositorIndicatorStrip {
            node: CompositorNodeId::IndicatorStrip { output },
            generation: publication.generation,
            strip,
        },
    ))
}

pub fn activate_indicator_at(
    publication: &PolicyIndicatorPublication,
    targets: &[IndicatorChromeHitTarget],
    position: Point,
) -> IndicatorChromeAction {
    let Some(target) = targets
        .iter()
        .find(|target| contains(target.geometry, position))
    else {
        return IndicatorChromeAction::Missed;
    };
    // The epoch and the publication generation together identify the indicators
    // this target was measured against. An unrelated policy commit no longer
    // moves either, so a click survives a layout change under the pointer.
    if publication.connection_epoch != Some(target.connection_epoch)
        || publication.generation != target.publication_generation
    {
        return IndicatorChromeAction::Stale;
    }
    target
        .action
        .map_or(IndicatorChromeAction::Consumed, |action| {
            IndicatorChromeAction::Activated {
                output: target.output,
                action,
            }
        })
}

/// Projects retained hit targets between two exact strip extents.
///
/// The backend supplies a head-native source only after that frame presents;
/// the destination is the corresponding logical strip used for physical input.
pub fn project_indicator_chrome_targets(
    targets: &[IndicatorChromeHitTarget],
    source: Rect,
    destination: Rect,
) -> Vec<IndicatorChromeHitTarget> {
    if source.is_empty() || destination.is_empty() {
        return Vec::new();
    }
    targets
        .iter()
        .cloned()
        .map(|mut target| {
            target.geometry = project_between_rects(target.geometry, source, destination);
            target
        })
        .collect()
}

fn project_between_rects(rect: Rect, source: Rect, destination: Rect) -> Rect {
    let edge = |value: i32,
                source_origin: i32,
                source_extent: i32,
                target_origin: i32,
                target_extent: i32| {
        i64::from(target_origin)
            .saturating_add(
                i64::from(value.saturating_sub(source_origin))
                    .saturating_mul(i64::from(target_extent))
                    / i64::from(source_extent.max(1)),
            )
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    };
    let left = edge(
        rect.x,
        source.x,
        source.width,
        destination.x,
        destination.width,
    );
    let right = edge(
        rect.x.saturating_add(rect.width),
        source.x,
        source.width,
        destination.x,
        destination.width,
    );
    let top = edge(
        rect.y,
        source.y,
        source.height,
        destination.y,
        destination.height,
    );
    let bottom = edge(
        rect.y.saturating_add(rect.height),
        source.y,
        source.height,
        destination.y,
        destination.height,
    );
    Rect {
        x: left.min(right),
        y: top.min(bottom),
        width: right.saturating_sub(left).abs(),
        height: bottom.saturating_sub(top).abs(),
    }
}

fn contains(rect: Rect, point: Point) -> bool {
    point.x >= f64::from(rect.x)
        && point.y >= f64::from(rect.y)
        && point.x < f64::from(rect.x.saturating_add(rect.width))
        && point.y < f64::from(rect.y.saturating_add(rect.height))
}
