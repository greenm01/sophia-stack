use crate::prelude::*;
use crate::render::should_render;

pub fn hit_test_scene_for_input(event: &InputEventPacket, layers: &[LayerSnapshot]) -> InputRoute {
    hit_test_layers(event, layers)
}

/// Hit-tests Engine visual truth when the authority must retain protocol-native
/// window identity. A routed result names only the committed Sophia surface.
pub fn hit_test_scene_surface_for_input(
    event: &InputEventPacket,
    layers: &[LayerSnapshot],
) -> InputRoute {
    hit_test_layers(event, layers)
}

/// Routes an event to an already-selected surface, including positions outside
/// its geometry. This preserves pointer-grab ordering after a button press.
pub fn route_scene_surface_for_input(
    event: &InputEventPacket,
    layers: &[LayerSnapshot],
    target: SurfaceId,
) -> InputRoute {
    let Some(global_position) = event.global_position else {
        return missed_input_route(event, Point::default());
    };
    let Some(layer) = layers
        .iter()
        .find(|layer| layer.surface == target && should_render(layer))
    else {
        return missed_input_route(event, global_position);
    };
    let Some(untransformed_position) = inverse_transform_point(layer.transform, global_position)
    else {
        return missed_input_route(event, global_position);
    };
    InputRoute {
        input_serial: event.serial,
        target_surface: Some(target),
        global_position,
        local_position: Some(Point {
            x: untransformed_position.x - f64::from(layer.geometry.x),
            y: untransformed_position.y - f64::from(layer.geometry.y),
        }),
        transform: layer.transform,
        outcome: InputRouteOutcome::Routed,
    }
}

fn hit_test_layers(event: &InputEventPacket, layers: &[LayerSnapshot]) -> InputRoute {
    let Some(global_position) = event.global_position else {
        return missed_input_route(event, Point::default());
    };

    let mut topmost = None;
    for layer in layers {
        if !layer.surface.is_valid() || !should_render(layer) {
            continue;
        }

        let Some(untransformed_position) =
            inverse_transform_point(layer.transform, global_position)
        else {
            continue;
        };
        if !rect_contains_point(layer.geometry, untransformed_position) {
            continue;
        }

        let route = InputRoute {
            input_serial: event.serial,
            target_surface: Some(layer.surface),
            global_position,
            local_position: Some(Point {
                x: untransformed_position.x - f64::from(layer.geometry.x),
                y: untransformed_position.y - f64::from(layer.geometry.y),
            }),
            transform: layer.transform,
            outcome: InputRouteOutcome::Routed,
        };
        if topmost
            .as_ref()
            .is_none_or(|(rank, _): &(u32, InputRoute)| layer.stack_rank >= *rank)
        {
            topmost = Some((layer.stack_rank, route));
        }
    }

    topmost.map_or_else(
        || missed_input_route(event, global_position),
        |(_, route)| route,
    )
}

fn missed_input_route(event: &InputEventPacket, global_position: Point) -> InputRoute {
    InputRoute {
        input_serial: event.serial,
        target_surface: None,
        global_position,
        local_position: None,
        transform: sophia_protocol::Transform::IDENTITY,
        outcome: InputRouteOutcome::NoTarget,
    }
}

fn rect_contains_point(rect: Rect, point: Point) -> bool {
    let left = f64::from(rect.x);
    let top = f64::from(rect.y);
    let right = left + f64::from(rect.width);
    let bottom = top + f64::from(rect.height);

    point.x >= left && point.x < right && point.y >= top && point.y < bottom
}

fn inverse_transform_point(transform: sophia_protocol::Transform, point: Point) -> Option<Point> {
    let m = transform.matrix.map(f64::from);
    let determinant = m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6])
        + m[2] * (m[3] * m[7] - m[4] * m[6]);
    if !determinant.is_finite() || determinant.abs() < f64::EPSILON {
        return None;
    }

    let inv_det = 1.0 / determinant;
    let inverse = [
        (m[4] * m[8] - m[5] * m[7]) * inv_det,
        (m[2] * m[7] - m[1] * m[8]) * inv_det,
        (m[1] * m[5] - m[2] * m[4]) * inv_det,
        (m[5] * m[6] - m[3] * m[8]) * inv_det,
        (m[0] * m[8] - m[2] * m[6]) * inv_det,
        (m[2] * m[3] - m[0] * m[5]) * inv_det,
        (m[3] * m[7] - m[4] * m[6]) * inv_det,
        (m[1] * m[6] - m[0] * m[7]) * inv_det,
        (m[0] * m[4] - m[1] * m[3]) * inv_det,
    ];

    let x = inverse[0] * point.x + inverse[1] * point.y + inverse[2];
    let y = inverse[3] * point.x + inverse[4] * point.y + inverse[5];
    let w = inverse[6] * point.x + inverse[7] * point.y + inverse[8];
    if !x.is_finite() || !y.is_finite() || !w.is_finite() || w.abs() < f64::EPSILON {
        return None;
    }

    Some(Point { x: x / w, y: y / w })
}
