use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveProductionNativeTopologyCurrentHead {
    pub head: sophia_engine::RenderHeadId,
    pub card_index: usize,
    pub output: OutputId,
    pub selection: crate::LibdrmNativePrimaryPlaneSelection,
    pub target_generation: u64,
}

impl LiveProductionNativeTopologyCurrentHead {
    pub const fn new(
        head: sophia_engine::RenderHeadId,
        card_index: usize,
        output: OutputId,
        selection: crate::LibdrmNativePrimaryPlaneSelection,
        target_generation: u64,
    ) -> Self {
        Self {
            head,
            card_index,
            output,
            selection,
            target_generation,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionNativeTopologyDisposition {
    Enabled {
        output: OutputId,
        selection: crate::LibdrmNativePrimaryPlaneSelection,
        transform: sophia_protocol::OutputTransform,
        mapping: sophia_protocol::OutputHeadMapping,
        vrr: sophia_protocol::OutputVrrPolicy,
    },
    Disabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveProductionNativeTopologyHeadPlan {
    pub head: sophia_engine::RenderHeadId,
    pub card_index: usize,
    pub previous_output: OutputId,
    pub previous_selection: crate::LibdrmNativePrimaryPlaneSelection,
    pub previous_target_generation: u64,
    pub candidate_target_generation: u64,
    pub disposition: LiveProductionNativeTopologyDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveProductionNativeTopologyPlan {
    pub primary_output: OutputId,
    pub outputs: Vec<sophia_engine::HeadlessOutput>,
    pub logical_viewports: Vec<crate::LiveOutputAuthorityLogicalViewport>,
    pub heads: Vec<LiveProductionNativeTopologyHeadPlan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveProductionNativeTopologyPlanError {
    Empty,
    DuplicateCurrentHead(sophia_engine::RenderHeadId),
    DuplicateCandidateHead(sophia_engine::RenderHeadId),
    MissingCurrentHead(sophia_engine::RenderHeadId),
    MissingCandidateHead(sophia_engine::RenderHeadId),
    InvalidOutput(OutputId),
    InvalidGeneration(sophia_engine::RenderHeadId),
    ModeUnavailable(sophia_engine::RenderHeadId),
    Native(String),
}

impl core::fmt::Display for LiveProductionNativeTopologyPlanError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LiveProductionNativeTopologyPlanError {}

/// Binds a resolved authority candidate to the live session's existing native
/// objects without changing any output, exporter, runtime, or KMS state.
///
/// `resolve_mode` is the only hardware read. Keeping it injected makes the
/// projection deterministic in tests and makes the ownership boundary explicit:
/// the returned plan contains copied handles, never a second DRM owner.
pub fn plan_live_production_native_topology(
    current: &[LiveProductionNativeTopologyCurrentHead],
    resolved: &crate::LiveResolvedOutputTopology,
    mut resolve_mode: impl FnMut(
        LiveProductionNativeTopologyCurrentHead,
        crate::LibdrmNativeOutputTiming,
    ) -> Result<
        Option<::drm::control::Mode>,
        LiveProductionNativeTopologyPlanError,
    >,
) -> Result<LiveProductionNativeTopologyPlan, LiveProductionNativeTopologyPlanError> {
    if current.is_empty() || resolved.outputs.is_empty() {
        return Err(LiveProductionNativeTopologyPlanError::Empty);
    }
    let mut current_by_head = BTreeMap::new();
    for current in current.iter().copied() {
        if current_by_head.insert(current.head, current).is_some() {
            return Err(LiveProductionNativeTopologyPlanError::DuplicateCurrentHead(
                current.head,
            ));
        }
    }
    let output_ids = resolved
        .outputs
        .iter()
        .map(|output| output.id)
        .collect::<BTreeSet<_>>();
    if output_ids.len() != resolved.outputs.len()
        || resolved.logical_viewports.len() != resolved.outputs.len()
        || !output_ids.contains(&resolved.primary_output)
        || resolved.outputs.iter().any(|output| {
            !output.id.is_valid()
                || output.size.width <= 0
                || output.size.height <= 0
                || output.scale == 0
        })
    {
        return Err(LiveProductionNativeTopologyPlanError::InvalidOutput(
            resolved.primary_output,
        ));
    }
    for (output, viewport) in resolved.outputs.iter().zip(&resolved.logical_viewports) {
        if viewport.output != output.id
            || viewport.logical.width != output.size.width
            || viewport.logical.height != output.size.height
        {
            return Err(LiveProductionNativeTopologyPlanError::InvalidOutput(
                output.id,
            ));
        }
    }

    let enabled = resolved
        .targets
        .iter()
        .map(|target| (target.head, target))
        .collect::<BTreeMap<_, _>>();
    if enabled.len() != resolved.targets.len() {
        let duplicate = resolved
            .targets
            .iter()
            .map(|target| target.head)
            .find(|head| {
                resolved
                    .targets
                    .iter()
                    .filter(|target| target.head == *head)
                    .count()
                    > 1
            })
            .expect("a shorter map proves a duplicate");
        return Err(LiveProductionNativeTopologyPlanError::DuplicateCandidateHead(duplicate));
    }
    let disabled = resolved
        .disabled_heads
        .iter()
        .map(|disabled| (disabled.head, disabled))
        .collect::<BTreeMap<_, _>>();
    if disabled.len() != resolved.disabled_heads.len() {
        let duplicate = resolved
            .disabled_heads
            .iter()
            .map(|disabled| disabled.head)
            .find(|head| {
                resolved
                    .disabled_heads
                    .iter()
                    .filter(|disabled| disabled.head == *head)
                    .count()
                    > 1
            })
            .expect("a shorter map proves a duplicate");
        return Err(LiveProductionNativeTopologyPlanError::DuplicateCandidateHead(duplicate));
    }
    if let Some(head) = enabled.keys().find(|head| disabled.contains_key(head)) {
        return Err(LiveProductionNativeTopologyPlanError::DuplicateCandidateHead(*head));
    }
    for head in enabled.keys().chain(disabled.keys()) {
        if !current_by_head.contains_key(head) {
            return Err(LiveProductionNativeTopologyPlanError::MissingCurrentHead(
                *head,
            ));
        }
    }
    if let Some(head) = current_by_head
        .keys()
        .find(|head| !enabled.contains_key(head) && !disabled.contains_key(head))
    {
        return Err(LiveProductionNativeTopologyPlanError::MissingCandidateHead(
            *head,
        ));
    }

    let mut heads = Vec::with_capacity(current.len());
    for current in current.iter().copied() {
        let (candidate_target_generation, disposition) =
            if let Some(target) = enabled.get(&current.head) {
                if !output_ids.contains(&target.output)
                    || target.native_size.width <= 0
                    || target.native_size.height <= 0
                {
                    return Err(LiveProductionNativeTopologyPlanError::InvalidOutput(
                        target.output,
                    ));
                }
                let expected_generation = current.target_generation.checked_add(1).ok_or(
                    LiveProductionNativeTopologyPlanError::InvalidGeneration(current.head),
                )?;
                if target.target_generation != expected_generation {
                    return Err(LiveProductionNativeTopologyPlanError::InvalidGeneration(
                        current.head,
                    ));
                }
                let mode = resolve_mode(current, target.timing)?.ok_or(
                    LiveProductionNativeTopologyPlanError::ModeUnavailable(current.head),
                )?;
                let selection = crate::LibdrmNativePrimaryPlaneSelection::new(
                    current.selection.connector_handle(),
                    current.selection.crtc_handle(),
                    current.selection.plane_handle(),
                    target.native_size,
                    Some(mode),
                );
                (
                    target.target_generation,
                    LiveProductionNativeTopologyDisposition::Enabled {
                        output: target.output,
                        selection,
                        transform: target.transform,
                        mapping: target.mapping,
                        vrr: target.vrr,
                    },
                )
            } else {
                let disabled = disabled[&current.head];
                let expected_generation = current.target_generation.checked_add(1).ok_or(
                    LiveProductionNativeTopologyPlanError::InvalidGeneration(current.head),
                )?;
                if disabled.target_generation != expected_generation {
                    return Err(LiveProductionNativeTopologyPlanError::InvalidGeneration(
                        current.head,
                    ));
                }
                (
                    disabled.target_generation,
                    LiveProductionNativeTopologyDisposition::Disabled,
                )
            };
        heads.push(LiveProductionNativeTopologyHeadPlan {
            head: current.head,
            card_index: current.card_index,
            previous_output: current.output,
            previous_selection: current.selection,
            previous_target_generation: current.target_generation,
            candidate_target_generation,
            disposition,
        });
    }
    Ok(LiveProductionNativeTopologyPlan {
        primary_output: resolved.primary_output,
        outputs: resolved.outputs.clone(),
        logical_viewports: resolved.logical_viewports.clone(),
        heads,
    })
}

impl LiveProductionNativeScanout {
    /// Resolves a provisional IPC topology against the live DRM master without
    /// mutating the currently published head table or any scanout ownership.
    pub fn plan_output_topology(
        &self,
        resolved: &crate::LiveResolvedOutputTopology,
    ) -> Result<LiveProductionNativeTopologyPlan, LiveProductionNativeTopologyPlanError> {
        let current = self
            .heads
            .iter()
            .map(|head| {
                LiveProductionNativeTopologyCurrentHead::new(
                    head.head,
                    head.group,
                    head.output.id,
                    head.selection,
                    head.target_generation,
                )
            })
            .collect::<Vec<_>>();
        plan_live_production_native_topology(&current, resolved, |current, timing| {
            crate::resolve_native_connector_mode(
                self.groups[current.card_index].session.card(),
                current.selection.connector_handle(),
                timing,
            )
            .map_err(|error| LiveProductionNativeTopologyPlanError::Native(error.to_string()))
        })
    }
}
