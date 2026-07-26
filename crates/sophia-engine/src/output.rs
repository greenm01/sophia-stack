use crate::DrmKmsOutputRegistry;
use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputLogicalGeometry {
    pub output: OutputId,
    pub logical: Rect,
    pub pixel_size: Size,
    pub scale: u32,
}

impl OutputLogicalGeometry {
    pub fn project_rect(self, rect: Rect) -> Option<Rect> {
        let left = rect.x.max(self.logical.x);
        let top = rect.y.max(self.logical.y);
        let right = rect
            .x
            .saturating_add(rect.width)
            .min(self.logical.x.saturating_add(self.logical.width));
        let bottom = rect
            .y
            .saturating_add(rect.height)
            .min(self.logical.y.saturating_add(self.logical.height));
        let width = right.saturating_sub(left);
        let height = bottom.saturating_sub(top);
        (width > 0 && height > 0).then_some(Rect {
            x: left.saturating_sub(self.logical.x),
            y: top.saturating_sub(self.logical.y),
            width,
            height,
        })
    }

    pub fn project_damage(self, damage: &Region) -> Region {
        Region {
            rects: damage
                .rects
                .iter()
                .filter_map(|rect| self.project_rect(*rect))
                .collect(),
        }
    }

    pub fn project_rect_pixels(self, rect: Rect) -> Option<Rect> {
        let local = self.project_rect(rect)?;
        let scale = i32::try_from(self.scale.max(1)).unwrap_or(i32::MAX);
        Some(Rect {
            x: local.x.saturating_mul(scale),
            y: local.y.saturating_mul(scale),
            width: local.width.saturating_mul(scale).min(self.pixel_size.width),
            height: local
                .height
                .saturating_mul(scale)
                .min(self.pixel_size.height),
        })
    }

    pub fn project_damage_pixels(self, damage: &Region) -> Region {
        Region {
            rects: damage
                .rects
                .iter()
                .filter_map(|rect| self.project_rect_pixels(*rect))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExtendedDesktopTopology {
    outputs: BTreeMap<OutputId, OutputLogicalGeometry>,
    logical_size: Size,
}

impl ExtendedDesktopTopology {
    pub fn from_drm_outputs(outputs: &DrmKmsOutputRegistry) -> Self {
        let mut logical_x = 0i32;
        let mut logical_height = 0i32;
        let mut geometries = BTreeMap::new();
        for output in outputs.outputs() {
            let scale = output.scale.max(1);
            let scale_i32 = i32::try_from(scale).unwrap_or(i32::MAX);
            let logical_size = Size {
                width: output.mode.size.width.saturating_div(scale_i32).max(1),
                height: output.mode.size.height.saturating_div(scale_i32).max(1),
            };
            let logical = Rect {
                x: logical_x,
                y: 0,
                width: logical_size.width,
                height: logical_size.height,
            };
            geometries.insert(
                output.output,
                OutputLogicalGeometry {
                    output: output.output,
                    logical,
                    pixel_size: output.mode.size,
                    scale,
                },
            );
            logical_x = logical_x.saturating_add(logical_size.width);
            logical_height = logical_height.max(logical_size.height);
        }
        Self {
            outputs: geometries,
            logical_size: Size {
                width: logical_x,
                height: logical_height,
            },
        }
    }

    pub fn get(&self, output: OutputId) -> Option<&OutputLogicalGeometry> {
        self.outputs.get(&output)
    }

    pub fn outputs(&self) -> impl Iterator<Item = &OutputLogicalGeometry> {
        self.outputs.values()
    }

    pub const fn logical_size(&self) -> Size {
        self.logical_size
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputWorkArea {
    pub output: OutputId,
    pub full: Rect,
    pub work: Option<Rect>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SurfaceOutputReservationState {
    reservations: BTreeMap<SurfaceId, SurfaceOutputReservations>,
    active_surfaces: BTreeSet<SurfaceId>,
}

impl SurfaceOutputReservationState {
    pub fn observe_presentation(
        &mut self,
        surface: SurfaceId,
        role: SurfacePresentationRole,
        mapped: bool,
    ) -> bool {
        let was_active = self.active_surfaces.contains(&surface);
        let is_active = mapped && role == SurfacePresentationRole::ClientPositioned;
        if is_active {
            self.active_surfaces.insert(surface);
        } else {
            self.active_surfaces.remove(&surface);
        }
        was_active != is_active && self.reservations.contains_key(&surface)
    }

    pub fn observe_reservations(&mut self, snapshot: SurfaceOutputReservations) -> bool {
        if !snapshot.is_valid() {
            return false;
        }
        let surface = snapshot.surface;
        let previous = self.reservations.get(&surface);
        let changed = if snapshot.reservations.is_empty() {
            self.reservations.remove(&surface).is_some()
        } else if previous == Some(&snapshot) {
            false
        } else {
            self.reservations.insert(surface, snapshot);
            true
        };
        changed && self.active_surfaces.contains(&surface)
    }

    pub fn remove_surface(&mut self, surface: SurfaceId) -> bool {
        let was_active = self.active_surfaces.remove(&surface);
        let had_reservation = self.reservations.remove(&surface).is_some();
        was_active && had_reservation
    }

    pub fn active_reservations(&self) -> Vec<SurfaceOutputReservations> {
        self.reservations
            .iter()
            .filter(|(surface, _)| self.active_surfaces.contains(surface))
            .map(|(_, snapshot)| snapshot.clone())
            .collect()
    }

    pub fn len(&self) -> usize {
        self.reservations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.reservations.is_empty()
    }
}

pub fn reduce_output_work_areas(
    root: Rect,
    outputs: impl IntoIterator<Item = (OutputId, Rect)>,
    surface_reservations: &[SurfaceOutputReservations],
) -> Vec<OutputWorkArea> {
    let reservations = surface_reservations
        .iter()
        .filter(|surface| surface.is_valid())
        .flat_map(|surface| surface.reservations.iter().copied())
        .filter(|reservation| reservation_within_root(*reservation, root))
        .collect::<Vec<_>>();
    outputs
        .into_iter()
        .map(|(output, full)| OutputWorkArea {
            output,
            full,
            work: reduce_output_work_area(root, full, &reservations),
        })
        .collect()
}

pub fn reduce_output_work_area(
    root: Rect,
    output: Rect,
    reservations: &[OutputReservation],
) -> Option<Rect> {
    if root.is_empty() || output.is_empty() || !rect_contains(root, output) {
        return None;
    }
    let mut left = 0;
    let mut right = 0;
    let mut top = 0;
    let mut bottom = 0;
    for reservation in reservations
        .iter()
        .copied()
        .filter(|reservation| reservation_within_root(*reservation, root))
    {
        match reservation.edge {
            OutputEdge::Left => {
                let output_span = vertical_span(output)?;
                if reservation.span.intersects(output_span) {
                    let band_right = root.x.checked_add(reservation.depth)?;
                    left = left.max(band_right.saturating_sub(output.x).clamp(0, output.width));
                }
            }
            OutputEdge::Right => {
                let output_span = vertical_span(output)?;
                if reservation.span.intersects(output_span) {
                    let root_right = rect_right(root)?;
                    let band_left = root_right.checked_sub(reservation.depth)?;
                    right = right.max(
                        rect_right(output)?
                            .saturating_sub(band_left)
                            .clamp(0, output.width),
                    );
                }
            }
            OutputEdge::Top => {
                let output_span = horizontal_span(output)?;
                if reservation.span.intersects(output_span) {
                    let band_bottom = root.y.checked_add(reservation.depth)?;
                    top = top.max(band_bottom.saturating_sub(output.y).clamp(0, output.height));
                }
            }
            OutputEdge::Bottom => {
                let output_span = horizontal_span(output)?;
                if reservation.span.intersects(output_span) {
                    let root_bottom = rect_bottom(root)?;
                    let band_top = root_bottom.checked_sub(reservation.depth)?;
                    bottom = bottom.max(
                        rect_bottom(output)?
                            .saturating_sub(band_top)
                            .clamp(0, output.height),
                    );
                }
            }
        }
    }
    let width = output.width.checked_sub(left)?.checked_sub(right)?;
    let height = output.height.checked_sub(top)?.checked_sub(bottom)?;
    let work = Rect {
        x: output.x.checked_add(left)?,
        y: output.y.checked_add(top)?,
        width,
        height,
    };
    (!work.is_empty()).then_some(work)
}

fn reservation_within_root(reservation: OutputReservation, root: Rect) -> bool {
    if !reservation.is_valid() || root.is_empty() {
        return false;
    }
    let (root_span, maximum_depth) = match reservation.edge {
        OutputEdge::Left | OutputEdge::Right => (vertical_span(root), root.width),
        OutputEdge::Top | OutputEdge::Bottom => (horizontal_span(root), root.height),
    };
    root_span.is_some_and(|root_span| {
        reservation.depth <= maximum_depth
            && reservation.span.start >= root_span.start
            && reservation.span.end <= root_span.end
    })
}

fn rect_contains(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && rect_right(inner)
            .zip(rect_right(outer))
            .is_some_and(|(inner_right, outer_right)| inner_right <= outer_right)
        && rect_bottom(inner)
            .zip(rect_bottom(outer))
            .is_some_and(|(inner_bottom, outer_bottom)| inner_bottom <= outer_bottom)
}

fn horizontal_span(rect: Rect) -> Option<AxisSpan> {
    Some(AxisSpan {
        start: rect.x,
        end: rect_right(rect)?,
    })
}

fn vertical_span(rect: Rect) -> Option<AxisSpan> {
    Some(AxisSpan {
        start: rect.y,
        end: rect_bottom(rect)?,
    })
}

fn rect_right(rect: Rect) -> Option<i32> {
    rect.x.checked_add(rect.width)
}

fn rect_bottom(rect: Rect) -> Option<i32> {
    rect.y.checked_add(rect.height)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OutputVrrCapability {
    pub capable: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OutputVrrEligibility {
    pub opaque_fullscreen_surface_count: u8,
    pub unoccluded: bool,
    pub overlays_present: bool,
    pub composition_required: bool,
}

impl OutputVrrEligibility {
    pub const fn fullscreen_eligible(self) -> bool {
        self.opaque_fullscreen_surface_count == 1
            && self.unoccluded
            && !self.overlays_present
            && !self.composition_required
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputVrrDecision {
    Enabled,
    DisabledByPolicy,
    Unsupported,
    Ineligible,
}

pub const fn decide_output_vrr(
    policy_enabled: bool,
    capability: OutputVrrCapability,
    eligibility: OutputVrrEligibility,
) -> OutputVrrDecision {
    if !policy_enabled {
        OutputVrrDecision::DisabledByPolicy
    } else if !capability.capable {
        OutputVrrDecision::Unsupported
    } else if !eligibility.fullscreen_eligible() {
        OutputVrrDecision::Ineligible
    } else {
        OutputVrrDecision::Enabled
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeadlessOutput {
    pub id: OutputId,
    pub size: Size,
    pub scale: u32,
}

impl HeadlessOutput {
    pub const fn deterministic() -> Self {
        Self {
            id: OutputId::from_raw(1),
            size: Size {
                width: 1280,
                height: 720,
            },
            scale: 1,
        }
    }
}

impl Default for HeadlessOutput {
    fn default() -> Self {
        Self::deterministic()
    }
}
