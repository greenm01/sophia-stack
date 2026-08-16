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

#[derive(Debug)]
pub enum LiveProductionNativeTopologyCandidateResource<Enabled, Disabled> {
    Enabled(Enabled),
    Disabled(Disabled),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionNativeTopologyResourceTransition {
    Accepted,
    Ready,
    Duplicate,
    UnknownHead,
    WrongDisposition,
}

#[derive(Debug)]
pub struct LiveProductionNativeTopologyResourceRejection<Owner> {
    pub transition: LiveProductionNativeTopologyResourceTransition,
    pub owner: Owner,
}

/// Affine prepare-all owner for a topology transaction.
///
/// Every affected head needs a candidate resource (an enabled framebuffer or
/// explicit disabled-head property set) and an enabled rollback framebuffer.
/// `ready()` becomes true only after both complete sets exist. This is the
/// safety boundary that prevents the cross-card coordinator from beginning an
/// irreversible prefix with no resource capable of restoring it.
#[derive(Debug)]
pub struct LiveProductionNativeTopologyResourceCohort<Enabled, Disabled> {
    expected:
        BTreeMap<sophia_engine::RenderHeadId, (usize, LiveProductionNativeTopologyDisposition)>,
    candidate: BTreeMap<
        sophia_engine::RenderHeadId,
        LiveProductionNativeTopologyCandidateResource<Enabled, Disabled>,
    >,
    rollback: BTreeMap<sophia_engine::RenderHeadId, Enabled>,
}

type LiveProductionPreparedTopologyHead =
    crate::LivePreparedRenderedTopologyHead<crate::NativeGbmRenderedScanoutOwner>;

type LiveProductionNativeTopologyResources = LiveProductionNativeTopologyResourceCohort<
    LiveProductionPreparedTopologyHead,
    crate::LibdrmNativePreparedDisabledTopologyHead,
>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionNativeTopologyPreparationPhase {
    PreparingCandidate,
    PreparingRollback,
    Prepared,
    Aborting,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveProductionNativeTopologyPreparationReport {
    pub phase: LiveProductionNativeTopologyPreparationPhase,
    pub candidate_prepared: usize,
    pub rollback_prepared: usize,
    pub affected_heads: usize,
}

#[derive(Debug)]
pub(super) struct LiveProductionNativeTopologyPreparation {
    plan: LiveProductionNativeTopologyPlan,
    resources: LiveProductionNativeTopologyResources,
    rollback_frames:
        BTreeMap<sophia_engine::RenderHeadId, crate::LiveProductionHeadCompositionFrame>,
    phase: LiveProductionNativeTopologyPreparationPhase,
    failure: Option<String>,
}

impl<Enabled, Disabled> LiveProductionNativeTopologyResourceCohort<Enabled, Disabled> {
    pub fn new(plan: &LiveProductionNativeTopologyPlan) -> Option<Self> {
        let expected = plan
            .heads
            .iter()
            .map(|head| (head.head, (head.card_index, head.disposition)))
            .collect::<BTreeMap<_, _>>();
        (expected.len() == plan.heads.len() && !expected.is_empty()).then_some(Self {
            expected,
            candidate: BTreeMap::new(),
            rollback: BTreeMap::new(),
        })
    }

    pub fn prepare_candidate_enabled(
        &mut self,
        head: sophia_engine::RenderHeadId,
        owner: Enabled,
    ) -> Result<
        LiveProductionNativeTopologyResourceTransition,
        LiveProductionNativeTopologyResourceRejection<Enabled>,
    > {
        let Some((_, disposition)) = self.expected.get(&head) else {
            return Err(LiveProductionNativeTopologyResourceRejection {
                transition: LiveProductionNativeTopologyResourceTransition::UnknownHead,
                owner,
            });
        };
        if !matches!(
            disposition,
            LiveProductionNativeTopologyDisposition::Enabled { .. }
        ) {
            return Err(LiveProductionNativeTopologyResourceRejection {
                transition: LiveProductionNativeTopologyResourceTransition::WrongDisposition,
                owner,
            });
        }
        if self.candidate.contains_key(&head) {
            return Err(LiveProductionNativeTopologyResourceRejection {
                transition: LiveProductionNativeTopologyResourceTransition::Duplicate,
                owner,
            });
        }
        self.candidate.insert(
            head,
            LiveProductionNativeTopologyCandidateResource::Enabled(owner),
        );
        Ok(self.accepted_transition())
    }

    pub fn prepare_candidate_disabled(
        &mut self,
        head: sophia_engine::RenderHeadId,
        owner: Disabled,
    ) -> Result<
        LiveProductionNativeTopologyResourceTransition,
        LiveProductionNativeTopologyResourceRejection<Disabled>,
    > {
        let Some((_, disposition)) = self.expected.get(&head) else {
            return Err(LiveProductionNativeTopologyResourceRejection {
                transition: LiveProductionNativeTopologyResourceTransition::UnknownHead,
                owner,
            });
        };
        if !matches!(
            disposition,
            LiveProductionNativeTopologyDisposition::Disabled
        ) {
            return Err(LiveProductionNativeTopologyResourceRejection {
                transition: LiveProductionNativeTopologyResourceTransition::WrongDisposition,
                owner,
            });
        }
        if self.candidate.contains_key(&head) {
            return Err(LiveProductionNativeTopologyResourceRejection {
                transition: LiveProductionNativeTopologyResourceTransition::Duplicate,
                owner,
            });
        }
        self.candidate.insert(
            head,
            LiveProductionNativeTopologyCandidateResource::Disabled(owner),
        );
        Ok(self.accepted_transition())
    }

    pub fn prepare_rollback(
        &mut self,
        head: sophia_engine::RenderHeadId,
        owner: Enabled,
    ) -> Result<
        LiveProductionNativeTopologyResourceTransition,
        LiveProductionNativeTopologyResourceRejection<Enabled>,
    > {
        if !self.expected.contains_key(&head) {
            return Err(LiveProductionNativeTopologyResourceRejection {
                transition: LiveProductionNativeTopologyResourceTransition::UnknownHead,
                owner,
            });
        }
        if self.rollback.contains_key(&head) {
            return Err(LiveProductionNativeTopologyResourceRejection {
                transition: LiveProductionNativeTopologyResourceTransition::Duplicate,
                owner,
            });
        }
        self.rollback.insert(head, owner);
        Ok(self.accepted_transition())
    }

    pub fn ready(&self) -> bool {
        self.candidate.len() == self.expected.len() && self.rollback.len() == self.expected.len()
    }

    pub fn candidate_count(&self) -> usize {
        self.candidate.len()
    }

    pub fn rollback_count(&self) -> usize {
        self.rollback.len()
    }

    pub fn card_heads(&self, card_index: usize) -> Vec<sophia_engine::RenderHeadId> {
        self.expected
            .iter()
            .filter_map(|(head, (card, _))| (*card == card_index).then_some(*head))
            .collect()
    }

    pub fn candidate(
        &self,
        head: sophia_engine::RenderHeadId,
    ) -> Option<&LiveProductionNativeTopologyCandidateResource<Enabled, Disabled>> {
        self.candidate.get(&head)
    }

    pub fn rollback(&self, head: sophia_engine::RenderHeadId) -> Option<&Enabled> {
        self.rollback.get(&head)
    }

    pub fn take_candidate(
        &mut self,
        head: sophia_engine::RenderHeadId,
    ) -> Option<LiveProductionNativeTopologyCandidateResource<Enabled, Disabled>> {
        self.candidate.remove(&head)
    }

    pub fn take_rollback(&mut self, head: sophia_engine::RenderHeadId) -> Option<Enabled> {
        self.rollback.remove(&head)
    }

    pub fn into_remaining(
        self,
    ) -> (
        Vec<LiveProductionNativeTopologyCandidateResource<Enabled, Disabled>>,
        Vec<Enabled>,
    ) {
        (
            self.candidate.into_values().collect(),
            self.rollback.into_values().collect(),
        )
    }

    fn accepted_transition(&self) -> LiveProductionNativeTopologyResourceTransition {
        if self.ready() {
            LiveProductionNativeTopologyResourceTransition::Ready
        } else {
            LiveProductionNativeTopologyResourceTransition::Accepted
        }
    }
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

    pub fn output_topology_preparation_active(&self) -> bool {
        self.output_topology_preparation.is_some()
    }

    pub fn output_topology_cleanup_pending(&self) -> bool {
        !self.output_topology_cleanup.is_empty()
    }

    pub fn request_abort_output_topology_preparation(&mut self, reason: impl Into<String>) -> bool {
        let Some(state) = self.output_topology_preparation.as_mut() else {
            return false;
        };
        if state.phase == LiveProductionNativeTopologyPreparationPhase::Failed {
            return true;
        }
        state.failure.get_or_insert_with(|| reason.into());
        state.phase = LiveProductionNativeTopologyPreparationPhase::Aborting;
        true
    }

    pub fn retry_output_topology_cleanup(&mut self) -> usize {
        let pending = core::mem::take(&mut self.output_topology_cleanup);
        for (head, cleanup) in pending {
            let Some(index) = self.head_index_for_head(head) else {
                self.output_topology_cleanup.push((head, cleanup));
                continue;
            };
            let retried =
                crate::retry_rendered_primary_plane_scanout_cleanup(self.card(index), cleanup);
            if let Some(cleanup) = retried.cleanup {
                self.output_topology_cleanup.push((head, cleanup));
            }
        }
        self.output_topology_cleanup.len()
    }

    /// Starts nonblocking renderer preparation for both sides of one topology
    /// transaction. No KMS request is submitted here.
    pub fn begin_output_topology_preparation(
        &mut self,
        plan: LiveProductionNativeTopologyPlan,
        candidate_frames: Vec<crate::LiveProductionHeadCompositionFrame>,
        rollback_frames: Vec<crate::LiveProductionHeadCompositionFrame>,
    ) -> Result<LiveProductionNativeTopologyPreparationReport, Box<dyn std::error::Error>> {
        if self.output_topology_preparation.is_some() {
            return Err("native output topology preparation is already active".into());
        }
        if !self.queued_mirror_successors.is_empty()
            || !self.output_cohorts.is_empty()
            || self.heads.iter().any(|head| {
                head.pending_content.is_some()
                    || head.rendering_content.is_some()
                    || head.submitted_content.is_some()
                    || head.scanout_submission.is_some()
                    || head.prepared_scanout.is_some()
                    || head.scanout_cleanup.is_some()
            })
            || self
                .exporters
                .iter()
                .any(|exporter| exporter.pending_frame())
            || self
                .output_lifecycles
                .values()
                .any(|lifecycle| lifecycle.active_frame().is_some())
        {
            return Err(
                "native output topology preparation requires quiescent frame ownership".into(),
            );
        }

        let mut candidate_frames =
            validate_live_production_topology_frames(&plan, candidate_frames, true)?;
        let rollback_frames =
            validate_live_production_topology_frames(&plan, rollback_frames, false)?;
        let mut resources = LiveProductionNativeTopologyResources::new(&plan)
            .ok_or("native output topology resource cohort is invalid")?;

        // Disabled heads have no renderer work, but their property handles are
        // still part of prepare-all. Resolve them before mutating exporter queues.
        for head_plan in &plan.heads {
            if head_plan.disposition != LiveProductionNativeTopologyDisposition::Disabled {
                continue;
            }
            let prepared = crate::prepare_native_disabled_topology_head(
                self.groups[head_plan.card_index].session.card(),
                head_plan.previous_selection,
            );
            let owner = prepared.prepared.ok_or_else(|| {
                format!(
                    "native disabled topology head {} property preparation failed: {:?}",
                    head_plan.head.raw(),
                    prepared.status,
                )
            })?;
            resources
                .prepare_candidate_disabled(head_plan.head, owner)
                .map_err(|rejected| {
                    format!(
                        "native disabled topology head {} was rejected: {:?}",
                        head_plan.head.raw(),
                        rejected.transition,
                    )
                })?;
        }

        for head_plan in &plan.heads {
            let LiveProductionNativeTopologyDisposition::Enabled { .. } = head_plan.disposition
            else {
                continue;
            };
            let index = self
                .head_index_for_head(head_plan.head)
                .ok_or("topology preparation lost a live head")?;
            let frame = candidate_frames
                .remove(&head_plan.head)
                .expect("candidate frame coverage was validated");
            self.exporters[index].set_pending_mixed_frame(frame.frame);
        }

        let affected_heads = plan.heads.len();
        self.output_topology_preparation = Some(LiveProductionNativeTopologyPreparation {
            plan,
            resources,
            rollback_frames,
            phase: LiveProductionNativeTopologyPreparationPhase::PreparingCandidate,
            failure: None,
        });
        Ok(LiveProductionNativeTopologyPreparationReport {
            phase: LiveProductionNativeTopologyPreparationPhase::PreparingCandidate,
            candidate_prepared: self
                .output_topology_preparation
                .as_ref()
                .map_or(0, |state| state.resources.candidate_count()),
            rollback_prepared: 0,
            affected_heads,
        })
    }

    /// Advances renderer workers by one owner turn. The method returns
    /// `Prepared` only when the candidate and rollback resource sets are both
    /// complete; it never submits KMS.
    pub fn service_output_topology_preparation(
        &mut self,
    ) -> Result<LiveProductionNativeTopologyPreparationReport, Box<dyn std::error::Error>> {
        let mut state = self
            .output_topology_preparation
            .take()
            .ok_or("native output topology preparation is not active")?;
        let result = self.service_output_topology_preparation_inner(&mut state);
        if let Err(error) = &result {
            state.failure = Some(error.to_string());
            state.phase = LiveProductionNativeTopologyPreparationPhase::Aborting;
        }
        let report = LiveProductionNativeTopologyPreparationReport {
            phase: state.phase,
            candidate_prepared: state.resources.candidate_count(),
            rollback_prepared: state.resources.rollback_count(),
            affected_heads: state.plan.heads.len(),
        };
        self.output_topology_preparation = Some(state);
        if let Err(error) = result {
            tracing::warn!(
                "sophia_live_output_topology schema=1 status=preparation_aborting error={error} kms_submits=0"
            );
        }
        Ok(report)
    }

    /// Cancels a fully prepared transaction without submitting KMS and returns
    /// every affine renderer/native owner to the ordinary cleanup path.
    pub fn cancel_prepared_output_topology(
        &mut self,
    ) -> Result<LiveProductionNativeTopologyPlan, Box<dyn std::error::Error>> {
        let mut state = self
            .output_topology_preparation
            .take()
            .ok_or("native output topology preparation is not active")?;
        if state.phase != LiveProductionNativeTopologyPreparationPhase::Prepared
            || !state.resources.ready()
            || self
                .exporters
                .iter()
                .any(|exporter| exporter.pending_frame())
        {
            self.output_topology_preparation = Some(state);
            return Err("native output topology resources are not ready to cancel".into());
        }
        let mut cleanup_pending = 0usize;
        for head_plan in &state.plan.heads {
            self.head_index_for_head(head_plan.head)
                .ok_or("topology cancellation lost a live head")?;
            if let Some(candidate) = state.resources.take_candidate(head_plan.head) {
                match candidate {
                    LiveProductionNativeTopologyCandidateResource::Enabled(owner) => {
                        let cancelled = crate::cancel_prepared_rendered_topology_head(
                            self.groups[head_plan.card_index].session.card(),
                            owner,
                        );
                        if let Some(cleanup) = cancelled.cleanup {
                            self.output_topology_cleanup.push((
                                head_plan.head,
                                cleanup.map_scanout_buffer(|owner| {
                                    Box::new(owner) as Box<dyn std::any::Any>
                                }),
                            ));
                            cleanup_pending = cleanup_pending.saturating_add(1);
                        }
                    }
                    LiveProductionNativeTopologyCandidateResource::Disabled(_) => {}
                }
            }
            if let Some(owner) = state.resources.take_rollback(head_plan.head) {
                let cancelled = crate::cancel_prepared_rendered_topology_head(
                    self.groups[head_plan.card_index].session.card(),
                    owner,
                );
                if let Some(cleanup) = cancelled.cleanup {
                    self.output_topology_cleanup.push((
                        head_plan.head,
                        cleanup
                            .map_scanout_buffer(|owner| Box::new(owner) as Box<dyn std::any::Any>),
                    ));
                    cleanup_pending = cleanup_pending.saturating_add(1);
                }
            }
        }
        tracing::info!(
            "sophia_live_output_topology schema=1 status=prepared_cancelled heads={} cleanup_pending={} kms_submits=0",
            state.plan.heads.len(),
            cleanup_pending,
        );
        Ok(state.plan)
    }

    pub fn finish_failed_output_topology_preparation(
        &mut self,
    ) -> Result<(LiveProductionNativeTopologyPlan, String), Box<dyn std::error::Error>> {
        let state = self
            .output_topology_preparation
            .take()
            .ok_or("native output topology preparation is not active")?;
        if state.phase != LiveProductionNativeTopologyPreparationPhase::Failed {
            self.output_topology_preparation = Some(state);
            return Err("native output topology preparation has not finished aborting".into());
        }
        Ok((
            state.plan,
            state
                .failure
                .unwrap_or_else(|| "native topology preparation failed".to_owned()),
        ))
    }

    fn service_output_topology_preparation_inner(
        &mut self,
        state: &mut LiveProductionNativeTopologyPreparation,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match state.phase {
            LiveProductionNativeTopologyPreparationPhase::PreparingCandidate => {
                for head_plan in &state.plan.heads {
                    let LiveProductionNativeTopologyDisposition::Enabled { selection, vrr, .. } =
                        head_plan.disposition
                    else {
                        continue;
                    };
                    if state.resources.candidate(head_plan.head).is_some() {
                        continue;
                    }
                    let Some(owner) = self.prepare_output_topology_head(
                        head_plan.head,
                        selection,
                        topology_vrr_enabled(vrr),
                    )?
                    else {
                        continue;
                    };
                    state
                        .resources
                        .prepare_candidate_enabled(head_plan.head, owner)
                        .map_err(|rejected| {
                            format!(
                                "candidate topology owner for head {} was rejected: {:?}",
                                head_plan.head.raw(),
                                rejected.transition,
                            )
                        })?;
                }
                if state.resources.candidate_count() == state.plan.heads.len() {
                    for head_plan in &state.plan.heads {
                        let index = self
                            .head_index_for_head(head_plan.head)
                            .ok_or("rollback topology preparation lost a live head")?;
                        if self.exporters[index].pending_frame() {
                            return Err(
                                "candidate exporter remained occupied after preparation".into()
                            );
                        }
                        let frame = state
                            .rollback_frames
                            .remove(&head_plan.head)
                            .expect("rollback frame coverage was validated");
                        self.exporters[index].set_pending_mixed_frame(frame.frame);
                    }
                    state.phase = LiveProductionNativeTopologyPreparationPhase::PreparingRollback;
                }
            }
            LiveProductionNativeTopologyPreparationPhase::PreparingRollback => {
                for head_plan in &state.plan.heads {
                    if state.resources.rollback(head_plan.head).is_some() {
                        continue;
                    }
                    let Some(owner) = self.prepare_output_topology_head(
                        head_plan.head,
                        head_plan.previous_selection,
                        Some(false),
                    )?
                    else {
                        continue;
                    };
                    state
                        .resources
                        .prepare_rollback(head_plan.head, owner)
                        .map_err(|rejected| {
                            format!(
                                "rollback topology owner for head {} was rejected: {:?}",
                                head_plan.head.raw(),
                                rejected.transition,
                            )
                        })?;
                }
                if state.resources.ready() {
                    state.phase = LiveProductionNativeTopologyPreparationPhase::Prepared;
                }
            }
            LiveProductionNativeTopologyPreparationPhase::Prepared => {}
            LiveProductionNativeTopologyPreparationPhase::Aborting => {
                let mut renderer_drained = true;
                for head_plan in &state.plan.heads {
                    let index = self
                        .head_index_for_head(head_plan.head)
                        .ok_or("topology abort lost a live head")?;
                    if !self.exporters[index].pending_frame() {
                        continue;
                    }
                    if !self.exporters[index].worker_in_flight() {
                        self.exporters[index].discard_pending_frame();
                        continue;
                    }
                    let selection = if state.resources.candidate_count() < state.plan.heads.len() {
                        match head_plan.disposition {
                            LiveProductionNativeTopologyDisposition::Enabled {
                                selection, ..
                            } => selection,
                            LiveProductionNativeTopologyDisposition::Disabled => {
                                head_plan.previous_selection
                            }
                        }
                    } else {
                        head_plan.previous_selection
                    };
                    let export =
                        crate::LiveRenderedScanoutBufferExporter::export_rendered_scanout_buffer(
                            &mut self.exporters[index],
                            crate::LiveGbmEglFrameTargetRecord::new(selection.size()),
                        );
                    if export.status == crate::LiveRendererScanoutBufferExportStatus::Pending {
                        renderer_drained = false;
                    }
                    // Dropping an exported worker owner returns its lease. No
                    // native framebuffer was built for this abort-only poll.
                    drop(export);
                }
                if renderer_drained
                    && self
                        .exporters
                        .iter()
                        .all(|exporter| !exporter.pending_frame())
                {
                    self.cancel_partial_output_topology_resources(state)?;
                    state.phase = LiveProductionNativeTopologyPreparationPhase::Failed;
                }
            }
            LiveProductionNativeTopologyPreparationPhase::Failed => {
                return Err(state
                    .failure
                    .clone()
                    .unwrap_or_else(|| "native topology preparation failed".to_owned())
                    .into());
            }
        }
        Ok(())
    }

    fn cancel_partial_output_topology_resources(
        &mut self,
        state: &mut LiveProductionNativeTopologyPreparation,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for head_plan in &state.plan.heads {
            self.head_index_for_head(head_plan.head)
                .ok_or("topology resource cancellation lost a live head")?;
            if let Some(candidate) = state.resources.take_candidate(head_plan.head) {
                match candidate {
                    LiveProductionNativeTopologyCandidateResource::Enabled(owner) => {
                        let cancelled = crate::cancel_prepared_rendered_topology_head(
                            self.groups[head_plan.card_index].session.card(),
                            owner,
                        );
                        if let Some(cleanup) = cancelled.cleanup {
                            self.output_topology_cleanup.push((
                                head_plan.head,
                                cleanup.map_scanout_buffer(|owner| {
                                    Box::new(owner) as Box<dyn std::any::Any>
                                }),
                            ));
                        }
                    }
                    LiveProductionNativeTopologyCandidateResource::Disabled(_) => {}
                }
            }
            if let Some(owner) = state.resources.take_rollback(head_plan.head) {
                let cancelled = crate::cancel_prepared_rendered_topology_head(
                    self.groups[head_plan.card_index].session.card(),
                    owner,
                );
                if let Some(cleanup) = cancelled.cleanup {
                    self.output_topology_cleanup.push((
                        head_plan.head,
                        cleanup
                            .map_scanout_buffer(|owner| Box::new(owner) as Box<dyn std::any::Any>),
                    ));
                }
            }
        }
        Ok(())
    }

    fn prepare_output_topology_head(
        &mut self,
        head: sophia_engine::RenderHeadId,
        selection: crate::LibdrmNativePrimaryPlaneSelection,
        vrr_enabled: Option<bool>,
    ) -> Result<Option<LiveProductionPreparedTopologyHead>, Box<dyn std::error::Error>> {
        let index = self
            .head_index_for_head(head)
            .ok_or("topology renderer preparation targets an unknown head")?;
        let group = self.heads[index].group;
        let mut prepared =
            crate::prepare_rendered_primary_plane_topology_head_from_target_and_selection_with(
                crate::LiveKmsScanoutTargetStatus::Ready,
                Some(crate::LiveGbmEglFrameTargetRecord::new(selection.size())),
                crate::LibdrmNativePrimaryPlaneSelectionResult {
                    status: crate::LibdrmNativePrimaryPlaneSelectionStatus::Selected,
                    selection: Some(selection),
                },
                vrr_enabled,
                self.groups[group].session.card(),
                &mut self.exporters[index],
            );
        match prepared.status {
            crate::LiveRenderedPrimaryPlaneScanoutPrepareStatus::ScanoutExportPending => Ok(None),
            crate::LiveRenderedPrimaryPlaneScanoutPrepareStatus::Prepared => {
                let owner = prepared
                    .prepared
                    .take()
                    .ok_or("prepared topology renderer omitted its affine owner")?;
                match crate::prepare_rendered_topology_head_from_prepared_scanout(
                    owner,
                    vrr_enabled,
                ) {
                    Ok(owner) => Ok(Some(owner)),
                    Err(owner) => {
                        let cancelled = crate::cancel_prepared_rendered_primary_plane_scanout(
                            self.groups[group].session.card(),
                            owner,
                        );
                        if let Some(cleanup) = cancelled.cleanup {
                            self.output_topology_cleanup.push((
                                head,
                                cleanup.map_scanout_buffer(|owner| {
                                    Box::new(owner) as Box<dyn std::any::Any>
                                }),
                            ));
                        }
                        Err("modeset preparation produced a non-topology resource owner".into())
                    }
                }
            }
            status => {
                if let Some(cleanup) = prepared.cleanup.take() {
                    self.output_topology_cleanup.push((
                        head,
                        cleanup
                            .map_scanout_buffer(|owner| Box::new(owner) as Box<dyn std::any::Any>),
                    ));
                }
                Err(format!(
                    "topology renderer preparation failed for head {}: {status:?}",
                    head.raw(),
                )
                .into())
            }
        }
    }
}

fn topology_vrr_enabled(policy: sophia_protocol::OutputVrrPolicy) -> Option<bool> {
    match policy {
        sophia_protocol::OutputVrrPolicy::Disabled => Some(false),
        sophia_protocol::OutputVrrPolicy::Automatic => None,
        sophia_protocol::OutputVrrPolicy::Always => Some(true),
    }
}

pub fn validate_live_production_topology_frames(
    plan: &LiveProductionNativeTopologyPlan,
    frames: Vec<crate::LiveProductionHeadCompositionFrame>,
    candidate: bool,
) -> Result<
    BTreeMap<sophia_engine::RenderHeadId, crate::LiveProductionHeadCompositionFrame>,
    Box<dyn std::error::Error>,
> {
    let mut by_head = BTreeMap::new();
    for frame in frames {
        let head = frame.head;
        if by_head.insert(head, frame).is_some() {
            return Err("topology composition repeats a physical head".into());
        }
    }
    let expected = plan
        .heads
        .iter()
        .filter(|head| {
            !candidate
                || matches!(
                    head.disposition,
                    LiveProductionNativeTopologyDisposition::Enabled { .. }
                )
        })
        .collect::<Vec<_>>();
    if by_head.len() != expected.len() {
        return Err("topology composition has incomplete physical-head coverage".into());
    }
    for head in expected {
        let frame = by_head
            .get(&head.head)
            .ok_or("topology composition omitted a physical head")?;
        let (output, size) = if candidate {
            let LiveProductionNativeTopologyDisposition::Enabled {
                output, selection, ..
            } = head.disposition
            else {
                unreachable!("candidate expected set excludes disabled heads");
            };
            (output, selection.size())
        } else {
            (head.previous_output, head.previous_selection.size())
        };
        let damage = frame
            .frame
            .output_damage_snapshot
            .as_ref()
            .ok_or("topology composition frame has no damage snapshot")?;
        if damage.output.id != output || damage.output.size != size {
            return Err("topology composition frame targets the wrong output extent".into());
        }
    }
    Ok(by_head)
}
