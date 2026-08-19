//! Shape checks for policy scenes and the projections answering them.
//!
//! Split from `policy_projection.rs` to keep that file inside the reviewed
//! source-length bound; these are the predicates it applies, not a separate
//! stage of the pipeline.

use super::PolicyProjectionError;
use sophia_protocol::{
    OutputId, POLICY_MAX_OUTPUTS, POLICY_MAX_SURFACES, PolicyOutputProjection, PolicyRequestCause,
    PolicySceneSnapshot, PolicySurfacePlacement, PolicySurfaceSnapshot, PolicyTransform, Rect,
    Size, SurfaceId,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn validate_scene(scene: &PolicySceneSnapshot) -> Result<(), PolicyProjectionError> {
    if scene.generation == 0 {
        return Err(PolicyProjectionError::InvalidSceneGeneration);
    }
    if scene.outputs.is_empty() {
        return Err(PolicyProjectionError::InvalidOutput);
    }
    if scene.outputs.len() > POLICY_MAX_OUTPUTS {
        return Err(PolicyProjectionError::ExcessiveOutputs);
    }
    let mut outputs = BTreeSet::new();
    for output in &scene.outputs {
        if !output.output.is_valid() || output.generation == 0 {
            return Err(PolicyProjectionError::InvalidOutput);
        }
        if !outputs.insert(output.output) {
            return Err(PolicyProjectionError::DuplicateOutput);
        }
        if output.bounds.is_empty()
            || output.work_area.is_empty()
            || !rect_contains(output.bounds, output.work_area)
        {
            return Err(PolicyProjectionError::InvalidOutputGeometry);
        }
    }
    if !scene.active_output.is_valid() || !outputs.contains(&scene.active_output) {
        return Err(PolicyProjectionError::InvalidOutput);
    }
    if scene.surfaces.len() > POLICY_MAX_SURFACES {
        return Err(PolicyProjectionError::ExcessiveSurfaces);
    }
    let mut surfaces = BTreeSet::new();
    for surface in &scene.surfaces {
        if !surface.surface.is_valid() || surface.generation == 0 {
            return Err(PolicyProjectionError::InvalidSurface);
        }
        if surface
            .current_output
            .is_some_and(|output| !outputs.contains(&output))
        {
            return Err(PolicyProjectionError::InvalidOutput);
        }
        if !surfaces.insert(surface.surface) {
            return Err(PolicyProjectionError::DuplicateSurface);
        }
        if surface.geometry.is_empty() {
            return Err(PolicyProjectionError::EmptySceneSurfaceGeometry {
                surface: surface.surface,
                geometry: surface.geometry,
            });
        }
        validate_constraints(surface)?;
    }
    for surface in &scene.surfaces {
        if surface
            .transient_owner
            .is_some_and(|owner| owner == surface.surface || !surfaces.contains(&owner))
        {
            return Err(PolicyProjectionError::InvalidTransientOwner);
        }
    }
    let surface_map = scene
        .surfaces
        .iter()
        .map(|surface| (surface.surface, surface))
        .collect::<BTreeMap<_, _>>();
    let committed = committed_from_scene(scene);
    for output in &scene.outputs {
        validate_output_projection(
            committed
                .get(&output.output)
                .expect("current projection has every validated output"),
            output.bounds,
            output.work_area,
            &surface_map,
        )?;
    }
    Ok(())
}

pub(super) fn committed_from_scene(
    scene: &PolicySceneSnapshot,
) -> BTreeMap<OutputId, PolicyOutputProjection> {
    let mut committed = scene
        .outputs
        .iter()
        .map(|output| {
            (
                output.output,
                PolicyOutputProjection {
                    output: output.output,
                    placements: Vec::new(),
                    focus: output.focus,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    for surface in &scene.surfaces {
        if let Some(output) = surface.current_output {
            committed
                .get_mut(&output)
                .expect("validated current output exists")
                .placements
                .push(placement_from_snapshot(surface));
        }
    }
    committed
}

pub(super) fn placement_from_snapshot(surface: &PolicySurfaceSnapshot) -> PolicySurfacePlacement {
    PolicySurfacePlacement {
        surface: surface.surface,
        surface_generation: surface.generation,
        geometry: surface.geometry,
        requested_size: None,
        crop: None,
        transform: PolicyTransform::Identity,
        presentation: surface.current_state,
    }
}

pub(super) fn sync_scene_projection(
    scene: &mut PolicySceneSnapshot,
    committed: &BTreeMap<OutputId, PolicyOutputProjection>,
) {
    let focus = committed
        .iter()
        .map(|(output, projection)| (*output, projection.focus))
        .collect::<BTreeMap<_, _>>();
    let placements = committed
        .iter()
        .flat_map(|(output, projection)| {
            projection.placements.iter().map(|placement| {
                (
                    placement.surface,
                    (*output, placement.geometry, placement.presentation),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    for output in &mut scene.outputs {
        output.focus = focus.get(&output.output).copied().flatten();
    }
    for surface in &mut scene.surfaces {
        let committed = placements.get(&surface.surface).copied();
        surface.current_output = committed.map(|(output, _, _)| output);
        if let Some((_, geometry, presentation)) = committed {
            surface.geometry = geometry;
            surface.current_state = presentation;
        }
    }
}

pub(super) fn validate_constraints(
    surface: &PolicySurfaceSnapshot,
) -> Result<(), PolicyProjectionError> {
    let valid_size = |size: Size| size.width > 0 && size.height > 0;
    if surface
        .constraints
        .min_size
        .is_some_and(|size| !valid_size(size))
        || surface
            .constraints
            .max_size
            .is_some_and(|size| !valid_size(size))
    {
        return Err(PolicyProjectionError::InvalidSurfaceConstraints);
    }
    if let (Some(minimum), Some(maximum)) =
        (surface.constraints.min_size, surface.constraints.max_size)
        && (minimum.width > maximum.width || minimum.height > maximum.height)
    {
        return Err(PolicyProjectionError::InvalidSurfaceConstraints);
    }
    if surface.exact_size.is_some_and(|exact| {
        !valid_size(exact)
            || surface
                .constraints
                .min_size
                .is_some_and(|minimum| exact.width < minimum.width || exact.height < minimum.height)
            || surface
                .constraints
                .max_size
                .is_some_and(|maximum| exact.width > maximum.width || exact.height > maximum.height)
    }) {
        return Err(PolicyProjectionError::InvalidSurfaceConstraints);
    }
    validate_presentation(surface.requested_state, surface)?;
    validate_presentation(surface.current_state, surface)?;
    Ok(())
}

pub(super) fn validate_presentation(
    state: sophia_protocol::PolicyPresentationState,
    surface: &PolicySurfaceSnapshot,
) -> Result<(), PolicyProjectionError> {
    if (state.fullscreen && state.maximized)
        || (state.minimized && (state.fullscreen || state.maximized))
        || (state.fullscreen && !surface.capabilities.fullscreenable)
    {
        return Err(PolicyProjectionError::InvalidPresentationState);
    }
    Ok(())
}

pub(super) fn validate_request_cause(
    cause: PolicyRequestCause,
    surfaces: &[PolicySurfaceSnapshot],
) -> Result<(), PolicyProjectionError> {
    let live = |target: SurfaceId| surfaces.iter().any(|surface| surface.surface == target);
    match cause {
        PolicyRequestCause::SceneChanged => Ok(()),
        PolicyRequestCause::Action {
            activation_serial,
            action,
        } if activation_serial != 0 && action.is_valid() => Ok(()),
        PolicyRequestCause::Focus { target } if live(target) => Ok(()),
        PolicyRequestCause::Interaction {
            phase,
            kind,
            axis,
            target,
            geometry,
        } if live(target)
            && sophia_protocol::valid_policy_interaction_payload(phase, kind, axis, geometry) =>
        {
            Ok(())
        }
        _ => Err(PolicyProjectionError::InvalidRequestCause),
    }
}

pub(super) fn validate_output_projection(
    projection: &PolicyOutputProjection,
    bounds: Rect,
    work_area: Rect,
    surfaces: &BTreeMap<SurfaceId, &PolicySurfaceSnapshot>,
) -> Result<(), PolicyProjectionError> {
    let mut visible = BTreeSet::new();
    for placement in &projection.placements {
        if !visible.insert(placement.surface) {
            return Err(PolicyProjectionError::DuplicateSurface);
        }
        let surface = surfaces
            .get(&placement.surface)
            .ok_or(PolicyProjectionError::InvalidSurface)?;
        if placement.surface_generation != surface.generation {
            return Err(PolicyProjectionError::InvalidSurface);
        }
        let valid_geometry = if placement.presentation.fullscreen {
            placement.geometry == bounds
        } else {
            rect_contains(work_area, placement.geometry)
        };
        if placement.geometry.is_empty()
            || !valid_geometry
            || placement.crop.is_some_and(Rect::is_empty)
        {
            return Err(PolicyProjectionError::InvalidSurfaceGeometry {
                surface: placement.surface,
                geometry: placement.geometry,
                work_area,
                bounds,
                fullscreen: placement.presentation.fullscreen,
            });
        }
        let requested = placement.requested_size.unwrap_or(Size {
            width: placement.geometry.width,
            height: placement.geometry.height,
        });
        if surface.constraints.min_size.is_some_and(|minimum| {
            requested.width < minimum.width || requested.height < minimum.height
        }) || surface.constraints.max_size.is_some_and(|maximum| {
            requested.width > maximum.width || requested.height > maximum.height
        }) {
            return Err(PolicyProjectionError::InvalidSurfaceConstraints);
        }
        if surface.exact_size.is_some_and(|exact| exact != requested) {
            return Err(PolicyProjectionError::InvalidSurfaceConstraints);
        }
        validate_presentation(placement.presentation, surface)?;
    }
    if projection.focus.is_some_and(|focus| {
        !visible.contains(&focus)
            || !surfaces
                .get(&focus)
                .is_some_and(|surface| surface.capabilities.focusable)
            || projection
                .placements
                .iter()
                .find(|placement| placement.surface == focus)
                .is_some_and(|placement| placement.presentation.minimized)
    }) {
        return Err(PolicyProjectionError::InvalidSurface);
    }
    Ok(())
}

pub(super) fn rect_contains(outer: Rect, inner: Rect) -> bool {
    let outer_right = i64::from(outer.x) + i64::from(outer.width);
    let outer_bottom = i64::from(outer.y) + i64::from(outer.height);
    let inner_right = i64::from(inner.x) + i64::from(inner.width);
    let inner_bottom = i64::from(inner.y) + i64::from(inner.height);
    i64::from(inner.x) >= i64::from(outer.x)
        && i64::from(inner.y) >= i64::from(outer.y)
        && inner_right <= outer_right
        && inner_bottom <= outer_bottom
}
