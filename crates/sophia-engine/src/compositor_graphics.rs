use crate::prelude::*;
use crate::{HeadlessOutput, OutputFrameDamageSnapshot, output_frame_damage};

pub const MAX_COMPOSITOR_DISPLAY_COMMANDS: usize = 1_024;
pub const MAX_OUTPUT_DAMAGE_RECTS: usize = MAX_COMPOSITOR_DISPLAY_COMMANDS * 2;

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
pub struct OutputFramePresentation {
    pub snapshot: OutputFrameDamageSnapshot,
    pub compositor_damage: Region,
    pub damage: Region,
    pub repaint: OutputRepaintPlan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputRepaintPolicy {
    pub max_partial_rects: usize,
    pub full_repaint_percent: u8,
}

impl Default for OutputRepaintPolicy {
    fn default() -> Self {
        Self {
            max_partial_rects: 32,
            full_repaint_percent: 60,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFullRepaintReason {
    DamageCapacityExceeded,
    PartialRectLimitExceeded,
    CoverageThresholdReached,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutputRepaintPlan {
    Skip,
    Partial {
        damage: Region,
        damaged_pixels: u64,
    },
    Full {
        damage: Region,
        damaged_pixels: u64,
        reason: OutputFullRepaintReason,
    },
}

impl OutputRepaintPlan {
    pub const fn reduced_name(&self) -> &'static str {
        match self {
            Self::Skip => "skip",
            Self::Partial { .. } => "partial",
            Self::Full { .. } => "full",
        }
    }

    pub fn damage(&self) -> Option<&Region> {
        match self {
            Self::Skip => None,
            Self::Partial { damage, .. } | Self::Full { damage, .. } => Some(damage),
        }
    }

    pub const fn damaged_pixels(&self) -> u64 {
        match self {
            Self::Skip => 0,
            Self::Partial { damaged_pixels, .. } | Self::Full { damaged_pixels, .. } => {
                *damaged_pixels
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputRepaintPlanError {
    InvalidOutputSize,
    InvalidPolicy,
}

impl fmt::Display for OutputRepaintPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for OutputRepaintPlanError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFramePresentationError {
    InvalidOutput,
    InvalidOutputSize,
    InvalidRepaintPolicy,
    InvalidSnapshot,
    OutputMismatch,
    MissingPending,
    SubmissionInFlight,
    MissingSubmitted,
}

impl fmt::Display for OutputFramePresentationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for OutputFramePresentationError {}

/// Tracks immutable output-frame state through the scanout lifecycle.
///
/// A queued snapshot is compared with the state that will precede it on
/// screen: the submitted snapshot when a page flip is in flight, otherwise the
/// presented snapshot. Failed or superseded queue work never advances
/// presented state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputFramePresentationState {
    output: HeadlessOutput,
    repaint_policy: OutputRepaintPolicy,
    pending: Option<OutputFramePresentation>,
    submitted: Option<OutputFramePresentation>,
    presented: Option<OutputFrameDamageSnapshot>,
}

impl OutputFramePresentationState {
    pub fn new(output: HeadlessOutput) -> Result<Self, OutputFramePresentationError> {
        Self::with_repaint_policy(output, OutputRepaintPolicy::default())
    }

    pub fn with_repaint_policy(
        output: HeadlessOutput,
        repaint_policy: OutputRepaintPolicy,
    ) -> Result<Self, OutputFramePresentationError> {
        if !output.id.is_valid() {
            return Err(OutputFramePresentationError::InvalidOutput);
        }
        validate_output_repaint_inputs(output.size, repaint_policy).map_err(
            |error| match error {
                OutputRepaintPlanError::InvalidOutputSize => {
                    OutputFramePresentationError::InvalidOutputSize
                }
                OutputRepaintPlanError::InvalidPolicy => {
                    OutputFramePresentationError::InvalidRepaintPolicy
                }
            },
        )?;
        Ok(Self {
            output,
            repaint_policy,
            pending: None,
            submitted: None,
            presented: None,
        })
    }

    pub const fn output(&self) -> OutputId {
        self.output.id
    }

    pub fn queue(
        &mut self,
        snapshot: OutputFrameDamageSnapshot,
    ) -> Result<&OutputFramePresentation, OutputFramePresentationError> {
        if snapshot.output != self.output {
            return Err(OutputFramePresentationError::OutputMismatch);
        }
        let baseline = self
            .submitted
            .as_ref()
            .map(|submitted| &submitted.snapshot)
            .or(self.presented.as_ref());
        let compositor_baseline = baseline.map(|baseline| &baseline.compositor_display_list);
        let compositor_damage = compositor_baseline.map_or_else(
            || {
                let empty = CompositorDisplayList::empty(self.output.id);
                compositor_display_list_damage(&empty, &snapshot.compositor_display_list)
            },
            |baseline| compositor_display_list_damage(baseline, &snapshot.compositor_display_list),
        );
        let damage = output_frame_damage(baseline, &snapshot).map_err(|error| match error {
            crate::OutputFrameDamageError::OutputMismatch => {
                OutputFramePresentationError::OutputMismatch
            }
            _ => OutputFramePresentationError::InvalidSnapshot,
        })?;
        let repaint = plan_output_repaint(self.output.size, &damage, self.repaint_policy)
            .expect("presentation state validates its output and repaint policy");
        self.pending = Some(OutputFramePresentation {
            snapshot,
            compositor_damage,
            damage,
            repaint,
        });
        Ok(self.pending.as_ref().expect("assigned above"))
    }

    pub fn discard_pending(&mut self) -> Option<OutputFramePresentation> {
        self.pending.take()
    }

    pub fn mark_submitted(
        &mut self,
    ) -> Result<&OutputFramePresentation, OutputFramePresentationError> {
        if self.submitted.is_some() {
            return Err(OutputFramePresentationError::SubmissionInFlight);
        }
        self.submitted = Some(
            self.pending
                .take()
                .ok_or(OutputFramePresentationError::MissingPending)?,
        );
        Ok(self.submitted.as_ref().expect("assigned above"))
    }

    pub fn mark_presented(
        &mut self,
    ) -> Result<OutputFramePresentation, OutputFramePresentationError> {
        let submitted = self
            .submitted
            .take()
            .ok_or(OutputFramePresentationError::MissingSubmitted)?;
        self.presented = Some(submitted.snapshot.clone());
        Ok(submitted)
    }

    pub fn mark_initial_presented(
        &mut self,
    ) -> Result<OutputFramePresentation, OutputFramePresentationError> {
        if self.submitted.is_some() {
            return Err(OutputFramePresentationError::SubmissionInFlight);
        }
        let pending = self
            .pending
            .take()
            .ok_or(OutputFramePresentationError::MissingPending)?;
        self.presented = Some(pending.snapshot.clone());
        Ok(pending)
    }

    pub fn pending(&self) -> Option<&OutputFramePresentation> {
        self.pending.as_ref()
    }

    pub fn submitted(&self) -> Option<&OutputFramePresentation> {
        self.submitted.as_ref()
    }

    pub fn presented(&self) -> Option<&OutputFrameDamageSnapshot> {
        self.presented.as_ref()
    }
}

/// Reduces raw compositor-node damage into bounded output-local repaint work.
///
/// Rectangles are clipped to the output and exact rectangular unions are
/// coalesced deterministically. Excess complexity or coverage falls back to a
/// full repaint; incomplete proof therefore costs performance, never pixels.
pub fn plan_output_repaint(
    output_size: Size,
    damage: &Region,
    policy: OutputRepaintPolicy,
) -> Result<OutputRepaintPlan, OutputRepaintPlanError> {
    validate_output_repaint_inputs(output_size, policy)?;
    let full_output = Rect {
        x: 0,
        y: 0,
        width: output_size.width,
        height: output_size.height,
    };
    let output_pixels = rect_area(full_output);
    if damage.rects.len() > MAX_OUTPUT_DAMAGE_RECTS {
        return Ok(OutputRepaintPlan::Full {
            damage: Region::single(full_output),
            damaged_pixels: output_pixels,
            reason: OutputFullRepaintReason::DamageCapacityExceeded,
        });
    }

    let mut rects = Vec::with_capacity(damage.rects.len());
    for rect in damage.rects.iter().copied() {
        let Some(mut current) = clip_rect(rect, full_output) else {
            continue;
        };
        let mut index = 0;
        while index < rects.len() {
            if rects_form_rectangle(current, rects[index]) {
                current = bounding_rect(current, rects.swap_remove(index));
                index = 0;
            } else {
                index += 1;
            }
        }
        rects.push(current);
    }
    rects.sort_by_key(|rect| (rect.y, rect.x, rect.height, rect.width));
    if rects.is_empty() {
        return Ok(OutputRepaintPlan::Skip);
    }
    if rects.len() > policy.max_partial_rects {
        return Ok(OutputRepaintPlan::Full {
            damage: Region::single(full_output),
            damaged_pixels: output_pixels,
            reason: OutputFullRepaintReason::PartialRectLimitExceeded,
        });
    }

    let damaged_pixels = rects
        .iter()
        .copied()
        .map(rect_area)
        .fold(0_u64, u64::saturating_add);
    if damaged_pixels.saturating_mul(100)
        >= output_pixels.saturating_mul(u64::from(policy.full_repaint_percent))
    {
        return Ok(OutputRepaintPlan::Full {
            damage: Region::single(full_output),
            damaged_pixels: output_pixels,
            reason: OutputFullRepaintReason::CoverageThresholdReached,
        });
    }
    Ok(OutputRepaintPlan::Partial {
        damage: Region { rects },
        damaged_pixels,
    })
}

fn validate_output_repaint_inputs(
    output_size: Size,
    policy: OutputRepaintPolicy,
) -> Result<(), OutputRepaintPlanError> {
    if output_size.width <= 0 || output_size.height <= 0 {
        return Err(OutputRepaintPlanError::InvalidOutputSize);
    }
    if policy.max_partial_rects == 0
        || policy.max_partial_rects > MAX_OUTPUT_DAMAGE_RECTS
        || !(1..=100).contains(&policy.full_repaint_percent)
    {
        return Err(OutputRepaintPlanError::InvalidPolicy);
    }
    Ok(())
}

fn clip_rect(rect: Rect, bounds: Rect) -> Option<Rect> {
    let x = rect.x.max(bounds.x);
    let y = rect.y.max(bounds.y);
    let right = rect
        .x
        .saturating_add(rect.width)
        .min(bounds.x.saturating_add(bounds.width));
    let bottom = rect
        .y
        .saturating_add(rect.height)
        .min(bounds.y.saturating_add(bounds.height));
    let clipped = Rect {
        x,
        y,
        width: right.saturating_sub(x),
        height: bottom.saturating_sub(y),
    };
    (!clipped.is_empty()).then_some(clipped)
}

fn rects_form_rectangle(left: Rect, right: Rect) -> bool {
    let bounds = bounding_rect(left, right);
    let intersection = clip_rect(left, right).map_or(0, rect_area);
    rect_area(bounds)
        == rect_area(left)
            .saturating_add(rect_area(right))
            .saturating_sub(intersection)
}

fn bounding_rect(left: Rect, right: Rect) -> Rect {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    Rect {
        x,
        y,
        width: left
            .x
            .saturating_add(left.width)
            .max(right.x.saturating_add(right.width))
            .saturating_sub(x),
        height: left
            .y
            .saturating_add(left.height)
            .max(right.y.saturating_add(right.height))
            .saturating_sub(y),
    }
}

fn rect_area(rect: Rect) -> u64 {
    u64::try_from(rect.width)
        .unwrap_or_default()
        .saturating_mul(u64::try_from(rect.height).unwrap_or_default())
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
