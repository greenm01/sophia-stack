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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionNativeTopologyApplyPhase {
    Prepared,
    Applying,
    RollingBack,
    Applied,
    RolledBack,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LiveProductionNativeTopologyCard {
    card_index: usize,
    heads: Vec<sophia_engine::RenderHeadId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveProductionNativeTopologyApplyTransition {
    Accepted,
    Retry,
    CardApplied {
        card_index: usize,
        heads: Vec<sophia_engine::RenderHeadId>,
    },
    Applied {
        card_index: usize,
        heads: Vec<sophia_engine::RenderHeadId>,
    },
    RollbackRequired {
        failed_card_index: usize,
    },
    CardRolledBack {
        card_index: usize,
        heads: Vec<sophia_engine::RenderHeadId>,
    },
    RolledBack {
        card_index: usize,
        heads: Vec<sophia_engine::RenderHeadId>,
    },
    FailedWithoutMutation {
        card_index: usize,
    },
    RollbackFailed {
        card_index: usize,
    },
    OutOfOrder,
    Terminal,
}

/// Orders blocking card commits and reverses the accepted prefix on failure.
///
/// KMS is atomic only within one DRM card. This reducer supplies the missing
/// userspace transaction across cards: cards apply in stable index order, and a
/// later rejection rolls the accepted prefix back in reverse order. It owns no
/// DRM handles; the live executor keeps candidate and rollback resource owners
/// beside it and consumes the card named by `current_card_index()`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveProductionNativeTopologyApplyCoordinator {
    cards: Vec<LiveProductionNativeTopologyCard>,
    phase: LiveProductionNativeTopologyApplyPhase,
    next_apply: usize,
    applied: usize,
    rollback_remaining: usize,
}

impl LiveProductionNativeTopologyApplyCoordinator {
    pub fn new(plan: &LiveProductionNativeTopologyPlan) -> Option<Self> {
        let mut by_card = BTreeMap::<usize, Vec<sophia_engine::RenderHeadId>>::new();
        for head in &plan.heads {
            by_card.entry(head.card_index).or_default().push(head.head);
        }
        if by_card.is_empty() {
            return None;
        }
        let cards = by_card
            .into_iter()
            .map(|(card_index, mut heads)| {
                heads.sort();
                heads.dedup();
                LiveProductionNativeTopologyCard { card_index, heads }
            })
            .collect::<Vec<_>>();
        if cards.iter().any(|card| card.heads.is_empty())
            || cards.iter().map(|card| card.heads.len()).sum::<usize>() != plan.heads.len()
        {
            return None;
        }
        Some(Self {
            cards,
            phase: LiveProductionNativeTopologyApplyPhase::Prepared,
            next_apply: 0,
            applied: 0,
            rollback_remaining: 0,
        })
    }

    pub const fn phase(&self) -> LiveProductionNativeTopologyApplyPhase {
        self.phase
    }

    pub fn current_card_index(&self) -> Option<usize> {
        match self.phase {
            LiveProductionNativeTopologyApplyPhase::Applying => {
                self.cards.get(self.next_apply).map(|card| card.card_index)
            }
            LiveProductionNativeTopologyApplyPhase::RollingBack => self
                .rollback_remaining
                .checked_sub(1)
                .and_then(|index| self.cards.get(index))
                .map(|card| card.card_index),
            _ => None,
        }
    }

    pub fn current_heads(&self) -> &[sophia_engine::RenderHeadId] {
        match self.phase {
            LiveProductionNativeTopologyApplyPhase::Applying => self
                .cards
                .get(self.next_apply)
                .map_or(&[], |card| card.heads.as_slice()),
            LiveProductionNativeTopologyApplyPhase::RollingBack => self
                .rollback_remaining
                .checked_sub(1)
                .and_then(|index| self.cards.get(index))
                .map_or(&[], |card| card.heads.as_slice()),
            _ => &[],
        }
    }

    pub fn begin_apply(&mut self) -> LiveProductionNativeTopologyApplyTransition {
        if self.phase != LiveProductionNativeTopologyApplyPhase::Prepared {
            return self.out_of_order();
        }
        self.phase = LiveProductionNativeTopologyApplyPhase::Applying;
        LiveProductionNativeTopologyApplyTransition::Accepted
    }

    pub fn observe_apply(
        &mut self,
        card_index: usize,
        outcome: crate::NativeTopologySubmitOutcome,
    ) -> LiveProductionNativeTopologyApplyTransition {
        if self.phase != LiveProductionNativeTopologyApplyPhase::Applying
            || self.current_card_index() != Some(card_index)
        {
            return self.out_of_order();
        }
        if outcome == crate::NativeTopologySubmitOutcome::Busy {
            return LiveProductionNativeTopologyApplyTransition::Retry;
        }
        if outcome != crate::NativeTopologySubmitOutcome::Accepted {
            if self.applied == 0 {
                self.phase = LiveProductionNativeTopologyApplyPhase::Failed;
                return LiveProductionNativeTopologyApplyTransition::FailedWithoutMutation {
                    card_index,
                };
            }
            self.phase = LiveProductionNativeTopologyApplyPhase::RollingBack;
            self.rollback_remaining = self.applied;
            return LiveProductionNativeTopologyApplyTransition::RollbackRequired {
                failed_card_index: card_index,
            };
        }

        let card = &self.cards[self.next_apply];
        let transition = if self.next_apply + 1 == self.cards.len() {
            self.phase = LiveProductionNativeTopologyApplyPhase::Applied;
            LiveProductionNativeTopologyApplyTransition::Applied {
                card_index,
                heads: card.heads.clone(),
            }
        } else {
            LiveProductionNativeTopologyApplyTransition::CardApplied {
                card_index,
                heads: card.heads.clone(),
            }
        };
        self.next_apply += 1;
        self.applied += 1;
        transition
    }

    pub fn observe_rollback(
        &mut self,
        card_index: usize,
        outcome: crate::NativeTopologySubmitOutcome,
    ) -> LiveProductionNativeTopologyApplyTransition {
        if self.phase != LiveProductionNativeTopologyApplyPhase::RollingBack
            || self.current_card_index() != Some(card_index)
        {
            return self.out_of_order();
        }
        if outcome == crate::NativeTopologySubmitOutcome::Busy {
            return LiveProductionNativeTopologyApplyTransition::Retry;
        }
        if outcome != crate::NativeTopologySubmitOutcome::Accepted {
            self.phase = LiveProductionNativeTopologyApplyPhase::Failed;
            return LiveProductionNativeTopologyApplyTransition::RollbackFailed { card_index };
        }
        let card = &self.cards[self.rollback_remaining - 1];
        self.rollback_remaining -= 1;
        if self.rollback_remaining == 0 {
            self.phase = LiveProductionNativeTopologyApplyPhase::RolledBack;
            LiveProductionNativeTopologyApplyTransition::RolledBack {
                card_index,
                heads: card.heads.clone(),
            }
        } else {
            LiveProductionNativeTopologyApplyTransition::CardRolledBack {
                card_index,
                heads: card.heads.clone(),
            }
        }
    }

    fn out_of_order(&self) -> LiveProductionNativeTopologyApplyTransition {
        if matches!(
            self.phase,
            LiveProductionNativeTopologyApplyPhase::Applied
                | LiveProductionNativeTopologyApplyPhase::RolledBack
                | LiveProductionNativeTopologyApplyPhase::Failed
        ) {
            LiveProductionNativeTopologyApplyTransition::Terminal
        } else {
            LiveProductionNativeTopologyApplyTransition::OutOfOrder
        }
    }
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
    PublishedSnapshotMismatch,
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

/// Projects the published authority snapshot back into native render targets.
///
/// This is the rollback-side twin of candidate resolution. It deliberately
/// joins logical state from the published snapshot with physical size and
/// generation from the live head owner, preventing a provisional candidate
/// from contaminating the rollback image set.
pub fn project_live_production_published_topology(
    current: &[LiveProductionNativeTopologyCurrentHead],
    snapshot: &sophia_protocol::OutputAuthoritySnapshot,
    mut selected_timing: impl FnMut(
        LiveProductionNativeTopologyCurrentHead,
    ) -> Result<
        crate::LibdrmNativeOutputTiming,
        LiveProductionNativeTopologyPlanError,
    >,
) -> Result<crate::LiveResolvedOutputTopology, LiveProductionNativeTopologyPlanError> {
    snapshot
        .validate()
        .map_err(|_| LiveProductionNativeTopologyPlanError::PublishedSnapshotMismatch)?;
    let mut output_by_head = BTreeMap::new();
    let mut mapping_by_head = BTreeMap::new();
    for group in &snapshot.groups {
        for member in &group.members {
            let head = sophia_engine::RenderHeadId::from_raw(member.head.raw());
            if output_by_head.insert(head, group.output).is_some()
                || mapping_by_head.insert(head, member.mapping).is_some()
            {
                return Err(LiveProductionNativeTopologyPlanError::PublishedSnapshotMismatch);
            }
        }
    }
    let descriptor_by_head = snapshot
        .heads
        .iter()
        .map(|head| (sophia_engine::RenderHeadId::from_raw(head.head.raw()), head))
        .collect::<BTreeMap<_, _>>();
    if descriptor_by_head.len() != snapshot.heads.len() || descriptor_by_head.len() != current.len()
    {
        return Err(LiveProductionNativeTopologyPlanError::PublishedSnapshotMismatch);
    }

    let mut targets = Vec::with_capacity(current.len());
    for native in current.iter().copied() {
        let descriptor = descriptor_by_head
            .get(&native.head)
            .ok_or(LiveProductionNativeTopologyPlanError::PublishedSnapshotMismatch)?;
        let output = output_by_head
            .get(&native.head)
            .copied()
            .ok_or(LiveProductionNativeTopologyPlanError::PublishedSnapshotMismatch)?;
        let timing = selected_timing(native)?;
        if !descriptor.connected
            || !descriptor.enabled
            || descriptor.generation != native.target_generation
            || output != native.output
            || u32::try_from(native.selection.size().width).ok() != Some(timing.width)
            || u32::try_from(native.selection.size().height).ok() != Some(timing.height)
        {
            return Err(LiveProductionNativeTopologyPlanError::PublishedSnapshotMismatch);
        }
        targets.push(crate::LiveOutputAuthorityHeadTarget {
            head: native.head,
            target_generation: native.target_generation,
            output,
            timing,
            native_size: native.selection.size(),
            transform: sophia_protocol::OutputTransform::Normal,
            mapping: mapping_by_head[&native.head],
            vrr: sophia_protocol::OutputVrrPolicy::Disabled,
        });
    }
    let logical_viewports = snapshot
        .groups
        .iter()
        .map(|group| crate::LiveOutputAuthorityLogicalViewport {
            output: group.output,
            logical: group.logical,
        })
        .collect::<Vec<_>>();
    let outputs = snapshot
        .groups
        .iter()
        .map(|group| sophia_engine::HeadlessOutput {
            id: group.output,
            size: sophia_protocol::Size {
                width: group.logical.width,
                height: group.logical.height,
            },
            scale: 1,
        })
        .collect::<Vec<_>>();
    Ok(crate::LiveResolvedOutputTopology {
        primary_output: snapshot.primary_output,
        outputs,
        logical_viewports,
        disabled_heads: Vec::new(),
        targets,
        // Connector grouping is a discovery/configuration input. Rendering
        // rollback targets needs only the already-resolved opaque members.
        mirror_grouping: crate::NativeMirrorGrouping::none(),
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

    /// Reconstructs the still-published topology as render targets for a
    /// rollback pool. This never consults the provisional candidate: logical
    /// positions come from the published authority snapshot, while native sizes
    /// and generations come from the live head owner.
    pub fn published_output_topology(
        &self,
        snapshot: &sophia_protocol::OutputAuthoritySnapshot,
    ) -> Result<crate::LiveResolvedOutputTopology, LiveProductionNativeTopologyPlanError> {
        let capabilities = self
            .output_capabilities()
            .map_err(|error| LiveProductionNativeTopologyPlanError::Native(error.to_string()))?;
        let capability_by_head = capabilities
            .iter()
            .filter_map(|capability| capability.head().map(|head| (head, capability)))
            .collect::<BTreeMap<_, _>>();
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
        project_live_production_published_topology(&current, snapshot, |native| {
            capability_by_head
                .get(&native.head)
                .map(|capability| capability.selected_mode())
                .ok_or(LiveProductionNativeTopologyPlanError::PublishedSnapshotMismatch)
        })
    }
}
