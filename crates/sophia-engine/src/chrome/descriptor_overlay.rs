use sophia_protocol::{AttentionState, OutputId, Rect, SurfaceId, TrustLevel};

use crate::{
    ChromeDescriptorTable, CompositorDisplayCommand, CompositorNodeId, CompositorRect,
    CompositorRgb8, CompositorText, DescriptorOverlayNodeRole,
};

pub const MAX_DESCRIPTOR_OVERLAY_ENTRIES: usize = 16;
pub const DESCRIPTOR_OVERLAY_MARGIN: i32 = 16;
pub const DESCRIPTOR_OVERLAY_MAX_WIDTH: i32 = 480;
pub const DESCRIPTOR_OVERLAY_MIN_WIDTH: i32 = 160;
pub const DESCRIPTOR_OVERLAY_ROW_HEIGHT: i32 = 32;
pub const DESCRIPTOR_OVERLAY_PADDING: i32 = 8;
pub const DESCRIPTOR_OVERLAY_FONT_SIZE_MILLIS: u32 = 12_000;

const PANEL_SLOT: u16 = u16::MAX;

/// Opaque authority-scoped action reference attached to one toplevel target.
///
/// The type implies the broker-to-shell action family. Input routing may return
/// this value, but only its later issuer-owned resolver can recover an operation
/// or surface. No resolver or wire representation is part of this reference
/// slice.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ToplevelActionCapabilityRef {
    pub token: u64,
    pub issuer_epoch: u64,
    pub issuer_revocation_epoch: u64,
    pub recipient_epoch: u64,
    pub target_slot: u16,
    pub target_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PresentedChromeTargetId {
    pub authority_session_epoch: u64,
    pub slot: u16,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentedChromeTarget {
    pub id: PresentedChromeTargetId,
    pub output: OutputId,
    pub geometry: Rect,
    pub action: ToplevelActionCapabilityRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DescriptorOverlayEntry {
    pub slot: u16,
    pub surface: SurfaceId,
    pub descriptor_generation: u64,
    pub action: ToplevelActionCapabilityRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DescriptorOverlayCandidate {
    pub projection: u64,
    pub generation: u64,
    pub output: OutputId,
    pub broker_epoch: u64,
    pub broker_revocation_epoch: u64,
    pub shell_session_epoch: u64,
    pub selected_slot: Option<u16>,
    pub entries: Vec<DescriptorOverlayEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DescriptorOverlayProjection {
    pub output: OutputId,
    pub generation: u64,
    pub geometry: Rect,
    pub commands: Vec<CompositorDisplayCommand>,
    pub targets: Vec<PresentedChromeTarget>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DescriptorOverlayError {
    InvalidCandidate,
    InvalidBounds,
    CapacityExceeded,
    DuplicateEntry,
    MissingDescriptor,
    StaleDescriptor,
    InvalidDescriptor,
    InvalidAction,
    SelectedEntryMissing,
}

impl core::fmt::Display for DescriptorOverlayError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DescriptorOverlayError {}

/// Reduces an externally authored ordered candidate into Engine visual intent.
///
/// Ordering and selection are inputs on purpose. Deriving either from Engine
/// focus or descriptor insertion order would make Engine the switcher policy.
pub fn descriptor_overlay_projection(
    candidate: &DescriptorOverlayCandidate,
    descriptors: &ChromeDescriptorTable,
    bounds: Rect,
) -> Result<DescriptorOverlayProjection, DescriptorOverlayError> {
    validate_candidate(candidate)?;
    let geometry = overlay_geometry(bounds, candidate.entries.len())?;
    let mut seen_slots = std::collections::BTreeSet::new();
    let mut seen_surfaces = std::collections::BTreeSet::new();
    let mut seen_actions = std::collections::BTreeSet::new();
    let mut commands = Vec::with_capacity(candidate.entries.len().saturating_mul(5) + 1);
    let mut targets = Vec::with_capacity(candidate.entries.len());

    commands.push(CompositorDisplayCommand::Rect(CompositorRect {
        node: overlay_node(
            candidate.projection,
            PANEL_SLOT,
            DescriptorOverlayNodeRole::Panel,
        ),
        generation: candidate.projection,
        geometry,
        color: rgb(0x11, 0x13, 0x18),
    }));

    for (index, entry) in candidate.entries.iter().copied().enumerate() {
        if entry.slot == 0
            || entry.slot == PANEL_SLOT
            || !seen_slots.insert(entry.slot)
            || !seen_surfaces.insert(entry.surface)
            || !seen_actions.insert(entry.action.token)
        {
            return Err(DescriptorOverlayError::DuplicateEntry);
        }
        validate_action(candidate, entry)?;
        let descriptor = descriptors
            .get(entry.surface)
            .ok_or(DescriptorOverlayError::MissingDescriptor)?;
        if descriptor.generation != entry.descriptor_generation {
            return Err(DescriptorOverlayError::StaleDescriptor);
        }
        let label = descriptor
            .label
            .as_ref()
            .map_or("Window", |label| label.text.as_str());
        if label.is_empty()
            || label.len() > sophia_protocol::MAX_CHROME_LABEL_LEN
            || label.chars().any(char::is_control)
        {
            return Err(DescriptorOverlayError::InvalidDescriptor);
        }
        let row = Rect {
            x: geometry.x.saturating_add(DESCRIPTOR_OVERLAY_PADDING),
            y: geometry
                .y
                .saturating_add(DESCRIPTOR_OVERLAY_PADDING)
                .saturating_add(
                    i32::try_from(index)
                        .unwrap_or(i32::MAX)
                        .saturating_mul(DESCRIPTOR_OVERLAY_ROW_HEIGHT),
                ),
            width: geometry
                .width
                .saturating_sub(DESCRIPTOR_OVERLAY_PADDING.saturating_mul(2)),
            height: DESCRIPTOR_OVERLAY_ROW_HEIGHT,
        };
        commands.push(CompositorDisplayCommand::Rect(CompositorRect {
            node: overlay_node(
                candidate.projection,
                entry.slot,
                DescriptorOverlayNodeRole::Row,
            ),
            generation: descriptor.generation.max(1),
            geometry: row,
            color: rgb(0x18, 0x1b, 0x22),
        }));
        if candidate.selected_slot == Some(entry.slot) {
            commands.push(CompositorDisplayCommand::Rect(CompositorRect {
                node: overlay_node(
                    candidate.projection,
                    entry.slot,
                    DescriptorOverlayNodeRole::Selection,
                ),
                generation: candidate.generation,
                geometry: Rect {
                    width: 3.min(row.width),
                    ..row
                },
                color: rgb(0x70, 0xb7, 0xff),
            }));
        }
        commands.push(CompositorDisplayCommand::Rect(CompositorRect {
            node: overlay_node(
                candidate.projection,
                entry.slot,
                DescriptorOverlayNodeRole::Trust,
            ),
            generation: descriptor.generation.max(1),
            geometry: Rect {
                x: row.x.saturating_add(6),
                y: row.y.saturating_add(10),
                width: 4.min(row.width),
                height: 12.min(row.height),
            },
            color: trust_color(descriptor.trust_level),
        }));
        if descriptor.attention != AttentionState::None {
            commands.push(CompositorDisplayCommand::Rect(CompositorRect {
                node: overlay_node(
                    candidate.projection,
                    entry.slot,
                    DescriptorOverlayNodeRole::Attention,
                ),
                generation: descriptor.generation.max(1),
                geometry: Rect {
                    x: row.x.saturating_add(row.width.saturating_sub(4)),
                    y: row.y,
                    width: 4.min(row.width),
                    height: row.height,
                },
                color: attention_color(descriptor.attention),
            }));
        }
        commands.push(CompositorDisplayCommand::Text(CompositorText {
            node: overlay_node(
                candidate.projection,
                entry.slot,
                DescriptorOverlayNodeRole::Label,
            ),
            generation: descriptor.generation.max(1),
            geometry: Rect {
                x: row.x.saturating_add(18),
                y: row.y,
                width: row.width.saturating_sub(30).max(1),
                height: row.height,
            },
            text: label.to_owned(),
            font_size_millis: DESCRIPTOR_OVERLAY_FONT_SIZE_MILLIS,
            color: rgb(0xee, 0xee, 0xee),
        }));
        targets.push(PresentedChromeTarget {
            id: PresentedChromeTargetId {
                authority_session_epoch: candidate.shell_session_epoch,
                slot: entry.slot,
                generation: entry.descriptor_generation,
            },
            output: candidate.output,
            geometry: row,
            action: entry.action,
        });
    }

    if candidate
        .selected_slot
        .is_some_and(|slot| !seen_slots.contains(&slot))
    {
        return Err(DescriptorOverlayError::SelectedEntryMissing);
    }
    if commands.len() > crate::MAX_COMPOSITOR_DISPLAY_COMMANDS {
        return Err(DescriptorOverlayError::CapacityExceeded);
    }
    Ok(DescriptorOverlayProjection {
        output: candidate.output,
        generation: candidate.generation,
        geometry,
        commands,
        targets,
    })
}

fn validate_candidate(
    candidate: &DescriptorOverlayCandidate,
) -> Result<(), DescriptorOverlayError> {
    if candidate.projection == 0
        || candidate.generation == 0
        || !candidate.output.is_valid()
        || candidate.broker_epoch == 0
        || candidate.broker_revocation_epoch == 0
        || candidate.shell_session_epoch == 0
        || candidate.entries.is_empty()
    {
        return Err(DescriptorOverlayError::InvalidCandidate);
    }
    if candidate.entries.len() > MAX_DESCRIPTOR_OVERLAY_ENTRIES {
        return Err(DescriptorOverlayError::CapacityExceeded);
    }
    Ok(())
}

fn validate_action(
    candidate: &DescriptorOverlayCandidate,
    entry: DescriptorOverlayEntry,
) -> Result<(), DescriptorOverlayError> {
    let action = entry.action;
    if entry.slot == 0
        || action.token == 0
        || action.issuer_epoch != candidate.broker_epoch
        || action.issuer_revocation_epoch != candidate.broker_revocation_epoch
        || action.recipient_epoch != candidate.shell_session_epoch
        || action.target_slot != entry.slot
        || action.target_generation != entry.descriptor_generation
        || entry.descriptor_generation == 0
        || !entry.surface.is_valid()
    {
        return Err(DescriptorOverlayError::InvalidAction);
    }
    Ok(())
}

fn overlay_geometry(bounds: Rect, entries: usize) -> Result<Rect, DescriptorOverlayError> {
    if bounds.is_empty() {
        return Err(DescriptorOverlayError::InvalidBounds);
    }
    let available_width = bounds
        .width
        .checked_sub(DESCRIPTOR_OVERLAY_MARGIN.saturating_mul(2))
        .ok_or(DescriptorOverlayError::InvalidBounds)?;
    let available_height = bounds
        .height
        .checked_sub(DESCRIPTOR_OVERLAY_MARGIN.saturating_mul(2))
        .ok_or(DescriptorOverlayError::InvalidBounds)?;
    let width = available_width.min(DESCRIPTOR_OVERLAY_MAX_WIDTH);
    let height = i32::try_from(entries)
        .ok()
        .and_then(|entries| entries.checked_mul(DESCRIPTOR_OVERLAY_ROW_HEIGHT))
        .and_then(|height| height.checked_add(DESCRIPTOR_OVERLAY_PADDING.saturating_mul(2)))
        .ok_or(DescriptorOverlayError::InvalidBounds)?;
    if width < DESCRIPTOR_OVERLAY_MIN_WIDTH || height > available_height {
        return Err(DescriptorOverlayError::InvalidBounds);
    }
    Ok(Rect {
        x: bounds
            .x
            .saturating_add((bounds.width.saturating_sub(width)) / 2),
        y: bounds
            .y
            .saturating_add((bounds.height.saturating_sub(height)) / 2),
        width,
        height,
    })
}

const fn overlay_node(
    projection: u64,
    slot: u16,
    role: DescriptorOverlayNodeRole,
) -> CompositorNodeId {
    CompositorNodeId::DescriptorOverlay {
        projection,
        slot,
        role,
    }
}

const fn rgb(red: u8, green: u8, blue: u8) -> CompositorRgb8 {
    CompositorRgb8 { red, green, blue }
}

const fn trust_color(trust: TrustLevel) -> CompositorRgb8 {
    match trust {
        TrustLevel::Unknown => rgb(0x7c, 0x7c, 0x7c),
        TrustLevel::Trusted => rgb(0x8f, 0xd6, 0x94),
        TrustLevel::Untrusted => rgb(0xff, 0xd1, 0x66),
        TrustLevel::Isolated => rgb(0x70, 0xb7, 0xff),
    }
}

const fn attention_color(attention: AttentionState) -> CompositorRgb8 {
    match attention {
        AttentionState::None => rgb(0x7c, 0x7c, 0x7c),
        AttentionState::Notice => rgb(0xff, 0xd1, 0x66),
        AttentionState::Critical => rgb(0xff, 0xb6, 0xb0),
    }
}
