use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerBoundarySide {
    Minimum,
    Maximum,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PointerBoundaryContact {
    pub horizontal: Option<PointerBoundarySide>,
    pub vertical: Option<PointerBoundarySide>,
}

impl PointerBoundaryContact {
    pub const fn is_empty(self) -> bool {
        self.horizontal.is_none() && self.vertical.is_none()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PointerBoundaryMetrics {
    pub clamps: u64,
    pub immediate_reversals: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutputUnionPointerPlacement {
    pub position: Point,
    pub output_index: Option<usize>,
    pub entered: PointerBoundaryContact,
    pub contact: PointerBoundaryContact,
    pub reversed: PointerBoundaryContact,
    pub transition: Option<PointerOutputTransition>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointerOutputTransition {
    pub from: usize,
    pub to: usize,
}

/// Engine-owned state for projecting one physical pointer into an output union.
///
/// Libinput supplies an accumulated device position. Sophia retains one
/// raw-to-logical offset so startup placement and output-edge confinement do
/// not mutate the backend accumulator. Correcting that offset whenever a point
/// is clamped ensures the first physical delta away from an edge moves the
/// logical cursor immediately rather than consuming discarded overshoot.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OutputUnionPointerState {
    raw_position: Option<Point>,
    raw_to_logical_offset: Option<Point>,
    position: Option<Point>,
    output_bounds: Vec<Rect>,
    boundary_contact: PointerBoundaryContact,
    boundary_metrics: PointerBoundaryMetrics,
}

impl OutputUnionPointerState {
    pub fn with_raw_to_logical_offset(offset: Point) -> Self {
        Self {
            raw_to_logical_offset: Some(offset),
            ..Self::default()
        }
    }

    pub const fn position(&self) -> Option<Point> {
        self.position
    }

    pub fn output_index(&self) -> Option<usize> {
        self.position
            .and_then(|position| output_index_for_position(position, &self.output_bounds))
    }

    pub const fn boundary_metrics(&self) -> PointerBoundaryMetrics {
        self.boundary_metrics
    }

    pub fn set_output_bounds(&mut self, output_bounds: Vec<Rect>) {
        self.output_bounds = output_bounds;
        self.boundary_contact = PointerBoundaryContact::default();
        if let Some(position) = self.position {
            self.position = confine_pointer_to_outputs(position, &self.output_bounds);
            if let (Some(raw), Some(position)) = (self.raw_position, self.position) {
                self.raw_to_logical_offset = Some(Point {
                    x: position.x - raw.x,
                    y: position.y - raw.y,
                });
            }
        }
    }

    pub fn center_on_primary_output(&mut self, size: Size) -> Point {
        let center = Point {
            x: f64::from(size.width.max(1)) / 2.0,
            y: f64::from(size.height.max(1)) / 2.0,
        };
        if self.output_bounds.is_empty() {
            self.output_bounds.push(Rect {
                x: 0,
                y: 0,
                width: size.width.max(1),
                height: size.height.max(1),
            });
        }
        self.raw_position = Some(Point::default());
        self.raw_to_logical_offset = Some(center);
        self.position = Some(center);
        self.boundary_contact = PointerBoundaryContact::default();
        center
    }

    pub fn arm_at_geometry_center(&mut self, geometry: Option<Rect>) -> Option<Point> {
        let geometry = geometry?;
        let raw = self.raw_position.unwrap_or_default();
        let offset = pointer_offset_for_geometry(raw, geometry);
        let proposed = Point {
            x: raw.x + offset.x,
            y: raw.y + offset.y,
        };
        let position =
            confine_pointer_to_outputs(proposed, &self.output_bounds).unwrap_or(proposed);
        self.raw_to_logical_offset = Some(Point {
            x: position.x - raw.x,
            y: position.y - raw.y,
        });
        self.position = Some(position);
        self.boundary_contact = boundary_contact(proposed, position);
        Some(position)
    }

    pub fn place(
        &mut self,
        raw: Point,
        initial_geometry: Option<Rect>,
    ) -> OutputUnionPointerPlacement {
        if !raw.x.is_finite() || !raw.y.is_finite() {
            let position = self.position.unwrap_or_default();
            return OutputUnionPointerPlacement {
                position,
                output_index: output_index_for_position(position, &self.output_bounds),
                entered: PointerBoundaryContact::default(),
                contact: self.boundary_contact,
                reversed: PointerBoundaryContact::default(),
                transition: None,
            };
        }
        let previous_raw = self.raw_position;
        let previous_position = self.position;
        let previous_contact = self.boundary_contact;
        let previous_output_index = previous_position
            .and_then(|position| output_index_for_position(position, &self.output_bounds));
        self.raw_position = Some(raw);
        let offset = *self.raw_to_logical_offset.get_or_insert_with(|| {
            initial_geometry.map_or_else(Point::default, |geometry| {
                pointer_offset_for_geometry(raw, geometry)
            })
        });
        let proposed = Point {
            x: raw.x + offset.x,
            y: raw.y + offset.y,
        };
        let position =
            confine_pointer_to_outputs(proposed, &self.output_bounds).unwrap_or(proposed);
        let stationary = previous_raw == Some(raw);
        let contact = retain_boundary_contact(
            previous_contact,
            boundary_contact(proposed, position),
            previous_position,
            position,
        );
        let entered = boundary_entry(previous_contact, contact);
        if !contact.is_empty() {
            self.raw_to_logical_offset = Some(Point {
                x: position.x - raw.x,
                y: position.y - raw.y,
            });
            if !stationary {
                self.boundary_metrics.clamps = self.boundary_metrics.clamps.saturating_add(1);
            }
        }
        let reversed = immediate_boundary_reversal(
            previous_raw,
            raw,
            previous_position,
            position,
            previous_contact,
        );
        if !reversed.is_empty() {
            self.boundary_metrics.immediate_reversals =
                self.boundary_metrics.immediate_reversals.saturating_add(1);
        }
        let output_index = output_index_for_position(position, &self.output_bounds);
        let transition = previous_output_index
            .zip(output_index)
            .filter(|(from, to)| from != to)
            .map(|(from, to)| PointerOutputTransition { from, to });
        self.position = Some(position);
        self.boundary_contact = contact;
        OutputUnionPointerPlacement {
            position,
            output_index,
            entered,
            contact,
            reversed,
            transition,
        }
    }
}

pub fn pointer_offset_for_geometry(raw: Point, geometry: Rect) -> Point {
    Point {
        x: f64::from(geometry.x) + f64::from(geometry.width) / 2.0 - raw.x,
        y: f64::from(geometry.y) + f64::from(geometry.height) / 2.0 - raw.y,
    }
}

/// Projects a physical pointer position onto the nearest visible output.
///
/// Output rectangles are half-open. The returned point is therefore always a
/// valid KMS cursor target when at least one non-empty output is supplied.
pub fn confine_pointer_to_outputs(position: Point, outputs: &[Rect]) -> Option<Point> {
    if !position.x.is_finite() || !position.y.is_finite() {
        return None;
    }

    let mut nearest = None;
    for output in outputs.iter().copied().filter(|output| !output.is_empty()) {
        let left = f64::from(output.x);
        let top = f64::from(output.y);
        let right = f64::from(output.x.saturating_add(output.width));
        let bottom = f64::from(output.y.saturating_add(output.height));
        if position.x >= left && position.x < right && position.y >= top && position.y < bottom {
            return Some(position);
        }

        let candidate = Point {
            x: position.x.clamp(left, right - 1.0),
            y: position.y.clamp(top, bottom - 1.0),
        };
        let dx = position.x - candidate.x;
        let dy = position.y - candidate.y;
        let distance_squared = dx * dx + dy * dy;
        if nearest
            .as_ref()
            .is_none_or(|(nearest_distance, _)| distance_squared < *nearest_distance)
        {
            nearest = Some((distance_squared, candidate));
        }
    }

    nearest.map(|(_, point)| point)
}

fn boundary_contact(proposed: Point, confined: Point) -> PointerBoundaryContact {
    PointerBoundaryContact {
        horizontal: if proposed.x < confined.x {
            Some(PointerBoundarySide::Minimum)
        } else if proposed.x > confined.x {
            Some(PointerBoundarySide::Maximum)
        } else {
            None
        },
        vertical: if proposed.y < confined.y {
            Some(PointerBoundarySide::Minimum)
        } else if proposed.y > confined.y {
            Some(PointerBoundarySide::Maximum)
        } else {
            None
        },
    }
}

fn boundary_entry(
    previous: PointerBoundaryContact,
    current: PointerBoundaryContact,
) -> PointerBoundaryContact {
    PointerBoundaryContact {
        horizontal: (current.horizontal != previous.horizontal)
            .then_some(current.horizontal)
            .flatten(),
        vertical: (current.vertical != previous.vertical)
            .then_some(current.vertical)
            .flatten(),
    }
}

fn retain_boundary_contact(
    previous: PointerBoundaryContact,
    current: PointerBoundaryContact,
    previous_position: Option<Point>,
    position: Point,
) -> PointerBoundaryContact {
    let Some(previous_position) = previous_position else {
        return current;
    };
    PointerBoundaryContact {
        horizontal: current.horizontal.or_else(|| {
            (position.x == previous_position.x)
                .then_some(previous.horizontal)
                .flatten()
        }),
        vertical: current.vertical.or_else(|| {
            (position.y == previous_position.y)
                .then_some(previous.vertical)
                .flatten()
        }),
    }
}

fn output_index_for_position(position: Point, outputs: &[Rect]) -> Option<usize> {
    outputs.iter().position(|output| {
        !output.is_empty()
            && position.x >= f64::from(output.x)
            && position.x < f64::from(output.x.saturating_add(output.width))
            && position.y >= f64::from(output.y)
            && position.y < f64::from(output.y.saturating_add(output.height))
    })
}

fn immediate_boundary_reversal(
    previous_raw: Option<Point>,
    raw: Point,
    previous_position: Option<Point>,
    position: Point,
    previous_contact: PointerBoundaryContact,
) -> PointerBoundaryContact {
    let (Some(previous_raw), Some(previous_position)) = (previous_raw, previous_position) else {
        return PointerBoundaryContact::default();
    };
    let raw_dx = raw.x - previous_raw.x;
    let raw_dy = raw.y - previous_raw.y;
    PointerBoundaryContact {
        horizontal: match previous_contact.horizontal {
            Some(PointerBoundarySide::Minimum)
                if raw_dx > 0.0 && position.x > previous_position.x =>
            {
                Some(PointerBoundarySide::Minimum)
            }
            Some(PointerBoundarySide::Maximum)
                if raw_dx < 0.0 && position.x < previous_position.x =>
            {
                Some(PointerBoundarySide::Maximum)
            }
            _ => None,
        },
        vertical: match previous_contact.vertical {
            Some(PointerBoundarySide::Minimum)
                if raw_dy > 0.0 && position.y > previous_position.y =>
            {
                Some(PointerBoundarySide::Minimum)
            }
            Some(PointerBoundarySide::Maximum)
                if raw_dy < 0.0 && position.y < previous_position.y =>
            {
                Some(PointerBoundarySide::Maximum)
            }
            _ => None,
        },
    }
}
