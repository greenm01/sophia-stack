use crate::prelude::*;

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
