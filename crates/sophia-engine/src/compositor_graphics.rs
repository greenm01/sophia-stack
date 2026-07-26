use crate::prelude::*;

pub const MAX_COMPOSITOR_DISPLAY_COMMANDS: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FocusedSurfaceBorderEdge {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompositorNodeId {
    FocusedSurfaceBorder {
        surface: SurfaceId,
        edge: FocusedSurfaceBorderEdge,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositorRgb8 {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositorSolidRect {
    pub node: CompositorNodeId,
    pub generation: u64,
    pub geometry: Rect,
    pub color: CompositorRgb8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositorDisplayCommand {
    Surface { surface: SurfaceId },
    SolidRect(CompositorSolidRect),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositorDisplayList {
    pub output: OutputId,
    pub commands: Vec<CompositorDisplayCommand>,
}

impl CompositorDisplayList {
    pub fn empty(output: OutputId) -> Self {
        Self {
            output,
            commands: Vec::new(),
        }
    }

    pub fn solid_rects(&self) -> impl Iterator<Item = CompositorSolidRect> + '_ {
        self.commands.iter().filter_map(|command| match command {
            CompositorDisplayCommand::SolidRect(rect) => Some(*rect),
            CompositorDisplayCommand::Surface { .. } => None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FocusedSurfaceBorderStyle {
    pub thickness: i32,
    pub color: CompositorRgb8,
}

impl Default for FocusedSurfaceBorderStyle {
    fn default() -> Self {
        Self {
            thickness: 2,
            color: CompositorRgb8 {
                red: 0x70,
                green: 0xb7,
                blue: 0xff,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositorDisplayListError {
    InvalidOutput,
    InvalidSurface,
    DuplicateSurface,
    CapacityExceeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositorDisplayListPresentation {
    pub display_list: CompositorDisplayList,
    pub damage: Region,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositorDisplayListPresentationError {
    InvalidOutput,
    OutputMismatch,
    MissingPending,
    SubmissionInFlight,
    MissingSubmitted,
}

impl fmt::Display for CompositorDisplayListPresentationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CompositorDisplayListPresentationError {}

/// Tracks compositor display-list state through the scanout lifecycle.
///
/// A queued list is compared with the state that will precede it on screen:
/// the submitted list when a page flip is in flight, otherwise the presented
/// list. Failed or superseded queue work never advances presented state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositorDisplayListPresentationState {
    output: OutputId,
    pending: Option<CompositorDisplayListPresentation>,
    submitted: Option<CompositorDisplayListPresentation>,
    presented: Option<CompositorDisplayList>,
}

impl CompositorDisplayListPresentationState {
    pub fn new(output: OutputId) -> Result<Self, CompositorDisplayListPresentationError> {
        if !output.is_valid() {
            return Err(CompositorDisplayListPresentationError::InvalidOutput);
        }
        Ok(Self {
            output,
            pending: None,
            submitted: None,
            presented: None,
        })
    }

    pub const fn output(&self) -> OutputId {
        self.output
    }

    pub fn queue(
        &mut self,
        display_list: CompositorDisplayList,
    ) -> Result<&CompositorDisplayListPresentation, CompositorDisplayListPresentationError> {
        if display_list.output != self.output {
            return Err(CompositorDisplayListPresentationError::OutputMismatch);
        }
        let baseline = self
            .submitted
            .as_ref()
            .map(|submitted| &submitted.display_list)
            .or(self.presented.as_ref());
        let damage = baseline.map_or_else(
            || {
                let empty = CompositorDisplayList::empty(self.output);
                compositor_display_list_damage(&empty, &display_list)
            },
            |baseline| compositor_display_list_damage(baseline, &display_list),
        );
        self.pending = Some(CompositorDisplayListPresentation {
            display_list,
            damage,
        });
        Ok(self.pending.as_ref().expect("assigned above"))
    }

    pub fn discard_pending(&mut self) -> Option<CompositorDisplayListPresentation> {
        self.pending.take()
    }

    pub fn mark_submitted(
        &mut self,
    ) -> Result<&CompositorDisplayListPresentation, CompositorDisplayListPresentationError> {
        if self.submitted.is_some() {
            return Err(CompositorDisplayListPresentationError::SubmissionInFlight);
        }
        self.submitted = Some(
            self.pending
                .take()
                .ok_or(CompositorDisplayListPresentationError::MissingPending)?,
        );
        Ok(self.submitted.as_ref().expect("assigned above"))
    }

    pub fn mark_presented(
        &mut self,
    ) -> Result<CompositorDisplayListPresentation, CompositorDisplayListPresentationError> {
        let submitted = self
            .submitted
            .take()
            .ok_or(CompositorDisplayListPresentationError::MissingSubmitted)?;
        self.presented = Some(submitted.display_list.clone());
        Ok(submitted)
    }

    pub fn mark_initial_presented(
        &mut self,
    ) -> Result<CompositorDisplayListPresentation, CompositorDisplayListPresentationError> {
        if self.submitted.is_some() {
            return Err(CompositorDisplayListPresentationError::SubmissionInFlight);
        }
        let pending = self
            .pending
            .take()
            .ok_or(CompositorDisplayListPresentationError::MissingPending)?;
        self.presented = Some(pending.display_list.clone());
        Ok(pending)
    }

    pub fn pending(&self) -> Option<&CompositorDisplayListPresentation> {
        self.pending.as_ref()
    }

    pub fn submitted(&self) -> Option<&CompositorDisplayListPresentation> {
        self.submitted.as_ref()
    }

    pub fn presented(&self) -> Option<&CompositorDisplayList> {
        self.presented.as_ref()
    }
}

impl fmt::Display for CompositorDisplayListError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CompositorDisplayListError {}

/// Builds one immutable compositor display list from committed visual state.
///
/// Surface commands preserve Engine presentation order. A focused border is
/// inserted immediately after its surface, so renderer lowering does not need
/// focus policy or a protocol-specific window concept.
pub fn focused_surface_display_list(
    output: OutputId,
    presentation_order: &[SurfaceId],
    committed_surfaces: &[CommittedSurfaceState],
    focused_surface: Option<SurfaceId>,
    style: FocusedSurfaceBorderStyle,
) -> Result<CompositorDisplayList, CompositorDisplayListError> {
    if !output.is_valid() {
        return Err(CompositorDisplayListError::InvalidOutput);
    }
    let mut commands = Vec::with_capacity(
        presentation_order
            .len()
            .saturating_add(4)
            .min(MAX_COMPOSITOR_DISPLAY_COMMANDS),
    );
    let mut seen = BTreeSet::new();
    for surface in presentation_order.iter().copied() {
        if !surface.is_valid() {
            return Err(CompositorDisplayListError::InvalidSurface);
        }
        if !seen.insert(surface) {
            return Err(CompositorDisplayListError::DuplicateSurface);
        }
        push_display_command(&mut commands, CompositorDisplayCommand::Surface { surface })?;
        if focused_surface != Some(surface) {
            continue;
        }
        let Some(committed) = committed_surfaces
            .iter()
            .find(|committed| committed.surface == surface)
        else {
            continue;
        };
        for rect in focused_surface_border_rects(committed, style) {
            push_display_command(&mut commands, CompositorDisplayCommand::SolidRect(rect))?;
        }
    }
    Ok(CompositorDisplayList { output, commands })
}

/// Computes compositor-owned damage between two immutable display lists.
///
/// Stable nodes with an unchanged generation, geometry, and color contribute
/// no damage. Changed and removed nodes damage their old extents; changed and
/// created nodes damage their new extents.
pub fn compositor_display_list_damage(
    previous: &CompositorDisplayList,
    current: &CompositorDisplayList,
) -> Region {
    let previous = previous
        .solid_rects()
        .map(|rect| (rect.node, rect))
        .collect::<BTreeMap<_, _>>();
    let current = current
        .solid_rects()
        .map(|rect| (rect.node, rect))
        .collect::<BTreeMap<_, _>>();
    let mut damage = Region::empty();
    for node in previous
        .keys()
        .chain(current.keys())
        .copied()
        .collect::<BTreeSet<_>>()
    {
        match (previous.get(&node), current.get(&node)) {
            (Some(before), Some(after)) if before == after => {}
            (Some(before), Some(after)) => {
                damage.push(before.geometry);
                damage.push(after.geometry);
            }
            (Some(before), None) => damage.push(before.geometry),
            (None, Some(after)) => damage.push(after.geometry),
            (None, None) => unreachable!("node came from one display list"),
        }
    }
    damage
}

fn push_display_command(
    commands: &mut Vec<CompositorDisplayCommand>,
    command: CompositorDisplayCommand,
) -> Result<(), CompositorDisplayListError> {
    if commands.len() >= MAX_COMPOSITOR_DISPLAY_COMMANDS {
        return Err(CompositorDisplayListError::CapacityExceeded);
    }
    commands.push(command);
    Ok(())
}

fn focused_surface_border_rects(
    committed: &CommittedSurfaceState,
    style: FocusedSurfaceBorderStyle,
) -> Vec<CompositorSolidRect> {
    let geometry = committed.geometry;
    if geometry.is_empty() || style.thickness <= 0 {
        return Vec::new();
    }
    let thickness = style
        .thickness
        .min((geometry.width / 2).max(1))
        .min((geometry.height / 2).max(1));
    let mut rects = Vec::with_capacity(4);
    push_border_rect(
        &mut rects,
        committed,
        style,
        FocusedSurfaceBorderEdge::Top,
        Rect {
            height: thickness,
            ..geometry
        },
    );
    if geometry.height > thickness {
        push_border_rect(
            &mut rects,
            committed,
            style,
            FocusedSurfaceBorderEdge::Bottom,
            Rect {
                y: geometry
                    .y
                    .saturating_add(geometry.height)
                    .saturating_sub(thickness),
                height: thickness,
                ..geometry
            },
        );
    }
    let middle_height = geometry.height.saturating_sub(thickness.saturating_mul(2));
    if middle_height > 0 {
        push_border_rect(
            &mut rects,
            committed,
            style,
            FocusedSurfaceBorderEdge::Left,
            Rect {
                y: geometry.y.saturating_add(thickness),
                width: thickness,
                height: middle_height,
                ..geometry
            },
        );
        if geometry.width > thickness {
            push_border_rect(
                &mut rects,
                committed,
                style,
                FocusedSurfaceBorderEdge::Right,
                Rect {
                    x: geometry
                        .x
                        .saturating_add(geometry.width)
                        .saturating_sub(thickness),
                    y: geometry.y.saturating_add(thickness),
                    width: thickness,
                    height: middle_height,
                },
            );
        }
    }
    rects
}

fn push_border_rect(
    rects: &mut Vec<CompositorSolidRect>,
    committed: &CommittedSurfaceState,
    style: FocusedSurfaceBorderStyle,
    edge: FocusedSurfaceBorderEdge,
    geometry: Rect,
) {
    if geometry.is_empty() {
        return;
    }
    rects.push(CompositorSolidRect {
        node: CompositorNodeId::FocusedSurfaceBorder {
            surface: committed.surface,
            edge,
        },
        generation: focused_surface_border_generation(committed.geometry, style),
        geometry,
        color: style.color,
    });
}

fn focused_surface_border_generation(geometry: Rect, style: FocusedSurfaceBorderStyle) -> u64 {
    let mut generation = 0xcbf2_9ce4_8422_2325u64;
    for byte in geometry
        .x
        .to_le_bytes()
        .into_iter()
        .chain(geometry.y.to_le_bytes())
        .chain(geometry.width.to_le_bytes())
        .chain(geometry.height.to_le_bytes())
        .chain(style.thickness.to_le_bytes())
        .chain([style.color.red, style.color.green, style.color.blue])
    {
        generation = (generation ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
    }
    generation.max(1)
}
