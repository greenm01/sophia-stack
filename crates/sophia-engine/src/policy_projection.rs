use std::collections::{BTreeMap, BTreeSet};

use sophia_protocol::{
    OutputId, POLICY_INDICATOR_STATE_MASK, POLICY_MAX_INDICATORS, POLICY_MAX_INDICATORS_PER_OUTPUT,
    POLICY_MAX_OUTPUT_STATUSES, POLICY_MAX_OUTPUTS, POLICY_MAX_SURFACES,
    POLICY_OUTPUT_STATUS_FOCUS_MASK, PolicyOutputProjection, PolicyProjectionIndicator,
    PolicyProjectionOutcome, PolicyProjectionOutputStatus, PolicyProjectionProposal,
    PolicyProjectionRequest, PolicyRequestCause, PolicySceneSnapshot, PolicySurfacePlacement,
    PolicySurfaceSnapshot, PolicyTransform, Rect, Size, SurfaceId, Transform,
};

use crate::WmPolicyPlan;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyProjectionError {
    InvalidSceneGeneration,
    InvalidOutput,
    DuplicateOutput,
    InvalidOutputGeometry,
    ExcessiveOutputs,
    InvalidSurface,
    DuplicateSurface,
    InvalidSurfaceGeometry,
    InvalidSurfaceConstraints,
    InvalidPresentationState,
    InvalidTransientOwner,
    ExcessiveSurfaces,
    ConnectionAlreadyActive,
    InvalidConnectionEpoch,
    NoActiveConnection,
    RequestAlreadyPending,
    NoAffectedOutputs,
    UnknownAffectedOutput,
    InvalidRequestCause,
    RequestIdExhausted,
    V7AdapterState,
    InvalidIndicator,
    DuplicateIndicator,
    DuplicateIndicatorSlot,
    ExcessiveIndicators,
    InvalidOutputStatus,
    DuplicateOutputStatus,
}

impl core::fmt::Display for PolicyProjectionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for PolicyProjectionError {}

/// Engine-owned canonical projection state.
///
/// Validation constructs a complete candidate map before replacing committed
/// state. Rejection, timeout, and policy loss therefore cannot expose a partial
/// multi-output update.
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyProjectionReducer {
    scene: PolicySceneSnapshot,
    committed: BTreeMap<OutputId, PolicyOutputProjection>,
    active_epoch: Option<u64>,
    greatest_epoch: u64,
    next_request_id: u64,
    outstanding: Option<PolicyProjectionRequest>,
    commit_serial: u64,
    indicators: BTreeMap<OutputId, Vec<PolicyProjectionIndicator>>,
    output_statuses: BTreeMap<OutputId, PolicyProjectionOutputStatus>,
    indicator_publication_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyIndicatorPublication {
    pub generation: u64,
    pub connection_epoch: Option<u64>,
    pub projection_commit_serial: u64,
    pub indicators: Vec<PolicyProjectionIndicator>,
    pub output_statuses: Vec<PolicyProjectionOutputStatus>,
}

/// A fully validated reducer successor held outside authoritative state until
/// frontend configuration and renderable content have settled.
#[derive(Clone, Debug, PartialEq)]
pub struct StagedPolicyProjection {
    candidate: PolicyProjectionReducer,
    connection_epoch: u64,
    request_id: u64,
    scene_generation: u64,
    commit_serial: u64,
}

impl StagedPolicyProjection {
    pub fn projections(&self) -> Vec<PolicyOutputProjection> {
        self.candidate.committed()
    }

    pub const fn connection_epoch(&self) -> u64 {
        self.connection_epoch
    }

    pub const fn request_id(&self) -> u64 {
        self.request_id
    }

    pub const fn scene_generation(&self) -> u64 {
        self.scene_generation
    }
}

impl PolicyProjectionReducer {
    pub fn new(scene: PolicySceneSnapshot) -> Result<Self, PolicyProjectionError> {
        validate_scene(&scene)?;
        let committed = committed_from_scene(&scene);
        Ok(Self {
            scene,
            committed,
            active_epoch: None,
            greatest_epoch: 0,
            next_request_id: 1,
            outstanding: None,
            commit_serial: 0,
            indicators: BTreeMap::new(),
            output_statuses: BTreeMap::new(),
            indicator_publication_generation: 0,
        })
    }

    pub fn connect(&mut self, connection_epoch: u64) -> Result<(), PolicyProjectionError> {
        if self.active_epoch.is_some() {
            return Err(PolicyProjectionError::ConnectionAlreadyActive);
        }
        if connection_epoch == 0 || connection_epoch <= self.greatest_epoch {
            return Err(PolicyProjectionError::InvalidConnectionEpoch);
        }
        self.active_epoch = Some(connection_epoch);
        self.greatest_epoch = connection_epoch;
        self.clear_indicator_publication();
        Ok(())
    }

    pub fn disconnect(&mut self, connection_epoch: u64) -> PolicyProjectionOutcome {
        if self.active_epoch != Some(connection_epoch) {
            return PolicyProjectionOutcome::Disconnected;
        }
        self.active_epoch = None;
        self.outstanding = None;
        self.clear_indicator_publication();
        PolicyProjectionOutcome::Disconnected
    }

    pub fn issue_request(
        &mut self,
        affected_outputs: Vec<OutputId>,
    ) -> Result<PolicyProjectionRequest, PolicyProjectionError> {
        self.issue_request_with_cause(affected_outputs, PolicyRequestCause::SceneChanged)
    }

    /// Issues a request without collapsing its initiating event. Equal action
    /// tokens with distinct activation serials remain separate ordered work.
    pub fn issue_request_with_cause(
        &mut self,
        affected_outputs: Vec<OutputId>,
        cause: PolicyRequestCause,
    ) -> Result<PolicyProjectionRequest, PolicyProjectionError> {
        let Some(connection_epoch) = self.active_epoch else {
            return Err(PolicyProjectionError::NoActiveConnection);
        };
        if self.outstanding.is_some() {
            return Err(PolicyProjectionError::RequestAlreadyPending);
        }
        if affected_outputs.is_empty() {
            return Err(PolicyProjectionError::NoAffectedOutputs);
        }
        let live_outputs = self
            .scene
            .outputs
            .iter()
            .map(|output| output.output)
            .collect::<BTreeSet<_>>();
        let unique = affected_outputs.iter().copied().collect::<BTreeSet<_>>();
        if unique.len() != affected_outputs.len() {
            return Err(PolicyProjectionError::DuplicateOutput);
        }
        if !unique.is_subset(&live_outputs) {
            return Err(PolicyProjectionError::UnknownAffectedOutput);
        }
        validate_request_cause(cause, &self.scene.surfaces)?;
        let request_id = self.next_request_id;
        self.next_request_id = self
            .next_request_id
            .checked_add(1)
            .ok_or(PolicyProjectionError::RequestIdExhausted)?;
        let request = PolicyProjectionRequest {
            connection_epoch,
            request_id,
            scene_generation: self.scene.generation,
            affected_outputs,
            cause,
        };
        self.outstanding = Some(request.clone());
        Ok(request)
    }

    pub fn apply_proposal(
        &mut self,
        proposal: &PolicyProjectionProposal,
    ) -> PolicyProjectionOutcome {
        let Some(request) = self.outstanding.take() else {
            return if self.active_epoch == Some(proposal.connection_epoch) {
                PolicyProjectionOutcome::RejectedStale
            } else {
                PolicyProjectionOutcome::Disconnected
            };
        };
        if self.active_epoch != Some(proposal.connection_epoch)
            || proposal.connection_epoch != request.connection_epoch
        {
            return PolicyProjectionOutcome::Disconnected;
        }
        if proposal.request_id != request.request_id
            || proposal.base_generation != request.scene_generation
            || request.scene_generation != self.scene.generation
        {
            return PolicyProjectionOutcome::RejectedStale;
        }
        if !proposal.transaction.is_valid() {
            return PolicyProjectionOutcome::RejectedInvalid;
        }
        let Ok(candidate) = self.validated_candidate(&request, proposal) else {
            return PolicyProjectionOutcome::RejectedInvalid;
        };
        let Ok((indicators, output_statuses)) = self.validated_descriptors(&request, proposal)
        else {
            return PolicyProjectionOutcome::RejectedInvalid;
        };
        self.committed = candidate;
        self.indicators = indicators;
        self.output_statuses = output_statuses;
        sync_scene_projection(&mut self.scene, &self.committed);
        self.commit_serial = self.commit_serial.saturating_add(1);
        self.indicator_publication_generation =
            self.indicator_publication_generation.saturating_add(1);
        PolicyProjectionOutcome::Committed
    }

    /// Validates a proposal against a clone, preserving the authoritative
    /// reducer and its outstanding request until frontend settlement.
    pub fn stage_proposal(
        &self,
        proposal: &PolicyProjectionProposal,
    ) -> Result<StagedPolicyProjection, PolicyProjectionOutcome> {
        let mut candidate = self.clone();
        let outcome = candidate.apply_proposal(proposal);
        if outcome != PolicyProjectionOutcome::Committed {
            return Err(outcome);
        }
        Ok(StagedPolicyProjection {
            candidate,
            connection_epoch: proposal.connection_epoch,
            request_id: proposal.request_id,
            scene_generation: proposal.base_generation,
            commit_serial: self.commit_serial,
        })
    }

    /// Promotes a staged successor only if no connection, scene, request, or
    /// earlier frontend settlement has superseded its validation base.
    pub fn commit_staged(&mut self, staged: StagedPolicyProjection) -> PolicyProjectionOutcome {
        if self.active_epoch != Some(staged.connection_epoch) {
            return PolicyProjectionOutcome::Disconnected;
        }
        let exact_request = self
            .outstanding
            .as_ref()
            .is_some_and(|request| request.request_id == staged.request_id);
        if !exact_request {
            return PolicyProjectionOutcome::RejectedStale;
        }
        if self.scene.generation != staged.scene_generation
            || self.commit_serial != staged.commit_serial
        {
            self.outstanding = None;
            return PolicyProjectionOutcome::RejectedStale;
        }
        *self = staged.candidate;
        PolicyProjectionOutcome::Committed
    }

    pub fn timeout(&mut self, request_id: u64) -> PolicyProjectionOutcome {
        if self
            .outstanding
            .as_ref()
            .is_some_and(|request| request.request_id == request_id)
        {
            self.outstanding = None;
            PolicyProjectionOutcome::TimedOut
        } else {
            PolicyProjectionOutcome::RejectedStale
        }
    }

    pub fn observe_scene(
        &mut self,
        scene: PolicySceneSnapshot,
    ) -> Result<(), PolicyProjectionError> {
        validate_scene(&scene)?;
        if scene.generation <= self.scene.generation {
            return Err(PolicyProjectionError::InvalidSceneGeneration);
        }
        let surfaces = scene
            .surfaces
            .iter()
            .map(|surface| (surface.surface, surface))
            .collect::<BTreeMap<_, _>>();
        let output_ids = scene
            .outputs
            .iter()
            .map(|output| output.output)
            .collect::<BTreeSet<_>>();
        let mut committed = BTreeMap::new();
        for output in &scene.outputs {
            let mut projection =
                self.committed
                    .get(&output.output)
                    .cloned()
                    .unwrap_or(PolicyOutputProjection {
                        output: output.output,
                        placements: Vec::new(),
                        focus: output.focus,
                    });
            projection.placements.retain_mut(|placement| {
                let Some(surface) = surfaces.get(&placement.surface) else {
                    return false;
                };
                placement.surface_generation = surface.generation;
                true
            });
            let visible = projection
                .placements
                .iter()
                .map(|placement| placement.surface)
                .collect::<BTreeSet<_>>();
            if projection.focus.is_some_and(|focus| {
                !visible.contains(&focus)
                    || !surfaces
                        .get(&focus)
                        .is_some_and(|surface| surface.capabilities.focusable)
            }) {
                projection.focus = None;
            }
            committed.insert(output.output, projection);
        }
        let mut placed = committed
            .values()
            .flat_map(|projection| projection.placements.iter())
            .map(|placement| placement.surface)
            .collect::<BTreeSet<_>>();
        for surface in &scene.surfaces {
            let Some(output) = surface.current_output else {
                continue;
            };
            if placed.insert(surface.surface) {
                committed
                    .get_mut(&output)
                    .expect("validated current output exists")
                    .placements
                    .push(placement_from_snapshot(surface));
            }
        }
        debug_assert_eq!(
            committed.keys().copied().collect::<BTreeSet<_>>(),
            output_ids
        );
        self.scene = scene;
        self.committed = committed;
        let before = (self.indicators.len(), self.output_statuses.len());
        self.indicators
            .retain(|output, _| output_ids.contains(output));
        self.output_statuses
            .retain(|output, _| output_ids.contains(output));
        if before != (self.indicators.len(), self.output_statuses.len()) {
            self.indicator_publication_generation =
                self.indicator_publication_generation.saturating_add(1);
        }
        sync_scene_projection(&mut self.scene, &self.committed);
        Ok(())
    }

    pub const fn scene(&self) -> &PolicySceneSnapshot {
        &self.scene
    }

    pub fn committed(&self) -> Vec<PolicyOutputProjection> {
        self.committed.values().cloned().collect()
    }

    pub const fn outstanding(&self) -> Option<&PolicyProjectionRequest> {
        self.outstanding.as_ref()
    }

    pub const fn commit_serial(&self) -> u64 {
        self.commit_serial
    }

    pub fn indicator_publication(&self) -> PolicyIndicatorPublication {
        PolicyIndicatorPublication {
            generation: self.indicator_publication_generation,
            connection_epoch: self.active_epoch,
            projection_commit_serial: self.commit_serial,
            indicators: self.indicators.values().flatten().cloned().collect(),
            output_statuses: self.output_statuses.values().cloned().collect(),
        }
    }

    fn clear_indicator_publication(&mut self) {
        self.indicators.clear();
        self.output_statuses.clear();
        self.indicator_publication_generation =
            self.indicator_publication_generation.saturating_add(1);
    }

    fn validated_candidate(
        &self,
        request: &PolicyProjectionRequest,
        proposal: &PolicyProjectionProposal,
    ) -> Result<BTreeMap<OutputId, PolicyOutputProjection>, PolicyProjectionError> {
        let affected = request
            .affected_outputs
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let proposed = proposal
            .outputs
            .iter()
            .map(|output| output.output)
            .collect::<BTreeSet<_>>();
        if proposed.len() != proposal.outputs.len() || proposed != affected {
            return Err(PolicyProjectionError::DuplicateOutput);
        }
        let live_outputs = self
            .scene
            .outputs
            .iter()
            .map(|output| (output.output, output.bounds))
            .collect::<BTreeMap<_, _>>();
        let live_surfaces = self
            .scene
            .surfaces
            .iter()
            .map(|surface| (surface.surface, surface))
            .collect::<BTreeMap<_, _>>();
        let mut candidate = self.committed.clone();
        for output in &proposal.outputs {
            let bounds = live_outputs
                .get(&output.output)
                .copied()
                .ok_or(PolicyProjectionError::InvalidOutput)?;
            validate_output_projection(output, bounds, &live_surfaces)?;
            candidate.insert(output.output, output.clone());
        }
        let mut surfaces = BTreeSet::new();
        let mut count = 0usize;
        for output in candidate.values() {
            count = count
                .checked_add(output.placements.len())
                .ok_or(PolicyProjectionError::ExcessiveSurfaces)?;
            if count > POLICY_MAX_SURFACES {
                return Err(PolicyProjectionError::ExcessiveSurfaces);
            }
            for placement in &output.placements {
                if !surfaces.insert(placement.surface) {
                    return Err(PolicyProjectionError::DuplicateSurface);
                }
            }
        }
        Ok(candidate)
    }

    fn validated_descriptors(
        &self,
        request: &PolicyProjectionRequest,
        proposal: &PolicyProjectionProposal,
    ) -> Result<
        (
            BTreeMap<OutputId, Vec<PolicyProjectionIndicator>>,
            BTreeMap<OutputId, PolicyProjectionOutputStatus>,
        ),
        PolicyProjectionError,
    > {
        if proposal.indicators.len() > POLICY_MAX_INDICATORS
            || proposal.output_statuses.len() > POLICY_MAX_OUTPUT_STATUSES
        {
            return Err(PolicyProjectionError::ExcessiveIndicators);
        }
        let affected = request
            .affected_outputs
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut indicators = self.indicators.clone();
        let mut statuses = self.output_statuses.clone();
        for output in &affected {
            indicators.remove(output);
            statuses.remove(output);
        }

        let mut identities = BTreeSet::new();
        let mut slots = BTreeSet::new();
        for indicator in &proposal.indicators {
            if !affected.contains(&indicator.output)
                || indicator.indicator == 0
                || indicator.state_bits & !POLICY_INDICATOR_STATE_MASK != 0
                || indicator.label.is_empty()
                || indicator.label.len() > 32
                || indicator.label.chars().any(char::is_control)
                || indicator.action.is_some_and(|action| !action.is_valid())
            {
                return Err(PolicyProjectionError::InvalidIndicator);
            }
            if !identities.insert((indicator.output, indicator.indicator)) {
                return Err(PolicyProjectionError::DuplicateIndicator);
            }
            if !slots.insert((indicator.output, indicator.slot)) {
                return Err(PolicyProjectionError::DuplicateIndicatorSlot);
            }
            let output_indicators = indicators.entry(indicator.output).or_default();
            if output_indicators.len() == POLICY_MAX_INDICATORS_PER_OUTPUT {
                return Err(PolicyProjectionError::ExcessiveIndicators);
            }
            output_indicators.push(indicator.clone());
        }
        for output_indicators in indicators.values_mut() {
            output_indicators.sort_by_key(|indicator| indicator.slot);
        }

        for status in &proposal.output_statuses {
            if !affected.contains(&status.output)
                || status.focus_bits & !POLICY_OUTPUT_STATUS_FOCUS_MASK != 0
                || status.layout.is_empty()
                || status.layout.len() > 32
                || status.layout.chars().any(char::is_control)
            {
                return Err(PolicyProjectionError::InvalidOutputStatus);
            }
            if statuses.insert(status.output, status.clone()).is_some() {
                return Err(PolicyProjectionError::DuplicateOutputStatus);
            }
        }
        Ok((indicators, statuses))
    }
}

/// Adapts one API v7 candidate through the canonical output-projection shape.
/// The adapter owns workspace interpretation; the reducer never stores it.
pub fn adapt_v7_policy_plan(
    request: &PolicyProjectionRequest,
    scene: &PolicySceneSnapshot,
    plan: &WmPolicyPlan,
) -> Result<PolicyProjectionProposal, PolicyProjectionError> {
    let surfaces = scene
        .surfaces
        .iter()
        .map(|surface| (surface.surface, surface))
        .collect::<BTreeMap<_, _>>();
    let rendered = plan
        .layout
        .render_positions
        .iter()
        .map(|placement| (placement.surface, placement))
        .collect::<BTreeMap<_, _>>();
    let requested_sizes = plan
        .layout
        .requested_sizes
        .iter()
        .map(|request| (request.surface, request.size))
        .collect::<BTreeMap<_, _>>();
    let mut outputs = Vec::with_capacity(request.affected_outputs.len());
    for output in &request.affected_outputs {
        let output_state = plan
            .candidate
            .output(*output)
            .ok_or(PolicyProjectionError::V7AdapterState)?;
        let visible = plan
            .candidate
            .visible_surfaces(*output)
            .map_err(|_| PolicyProjectionError::V7AdapterState)?;
        let mut placements = visible
            .into_iter()
            .map(|surface| {
                let snapshot = surfaces
                    .get(&surface)
                    .ok_or(PolicyProjectionError::V7AdapterState)?;
                let rendered = rendered.get(&surface).copied();
                Ok(PolicySurfacePlacement {
                    surface,
                    surface_generation: snapshot.generation,
                    geometry: rendered.map_or(snapshot.geometry, |placement| placement.geometry),
                    requested_size: requested_sizes.get(&surface).copied(),
                    crop: rendered.and_then(|placement| placement.crop),
                    transform: match rendered.map(|placement| placement.transform) {
                        None | Some(Transform::IDENTITY) => PolicyTransform::Identity,
                        Some(_) => return Err(PolicyProjectionError::V7AdapterState),
                    },
                    presentation: snapshot.current_state,
                })
            })
            .collect::<Result<Vec<_>, PolicyProjectionError>>()?;
        placements.sort_by_key(|placement| {
            rendered
                .get(&placement.surface)
                .map_or(0, |rendered| rendered.z_index)
        });
        outputs.push(PolicyOutputProjection {
            output: *output,
            placements,
            focus: output_state.focus,
        });
    }
    Ok(PolicyProjectionProposal {
        transaction: plan.transaction,
        connection_epoch: request.connection_epoch,
        request_id: request.request_id,
        base_generation: request.scene_generation,
        outputs,
        indicators: Vec::new(),
        output_statuses: Vec::new(),
    })
}

fn validate_scene(scene: &PolicySceneSnapshot) -> Result<(), PolicyProjectionError> {
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
            return Err(PolicyProjectionError::InvalidSurfaceGeometry);
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
            &surface_map,
        )?;
    }
    Ok(())
}

fn committed_from_scene(scene: &PolicySceneSnapshot) -> BTreeMap<OutputId, PolicyOutputProjection> {
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

fn placement_from_snapshot(surface: &PolicySurfaceSnapshot) -> PolicySurfacePlacement {
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

fn sync_scene_projection(
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

fn validate_constraints(surface: &PolicySurfaceSnapshot) -> Result<(), PolicyProjectionError> {
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

fn validate_presentation(
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

fn validate_request_cause(
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
            target, geometry, ..
        } if live(target) && !geometry.is_empty() => Ok(()),
        _ => Err(PolicyProjectionError::InvalidRequestCause),
    }
}

fn validate_output_projection(
    projection: &PolicyOutputProjection,
    bounds: Rect,
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
        if placement.geometry.is_empty()
            || !rect_contains(bounds, placement.geometry)
            || placement.crop.is_some_and(Rect::is_empty)
        {
            return Err(PolicyProjectionError::InvalidSurfaceGeometry);
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
    }) {
        return Err(PolicyProjectionError::InvalidSurface);
    }
    Ok(())
}

fn rect_contains(outer: Rect, inner: Rect) -> bool {
    let outer_right = i64::from(outer.x) + i64::from(outer.width);
    let outer_bottom = i64::from(outer.y) + i64::from(outer.height);
    let inner_right = i64::from(inner.x) + i64::from(inner.width);
    let inner_bottom = i64::from(inner.y) + i64::from(inner.height);
    i64::from(inner.x) >= i64::from(outer.x)
        && i64::from(inner.y) >= i64::from(outer.y)
        && inner_right <= outer_right
        && inner_bottom <= outer_bottom
}
