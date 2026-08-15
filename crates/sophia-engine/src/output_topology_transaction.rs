use crate::RenderHeadId;
use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputTopologyTransactionPhase {
    Preparing,
    Prepared,
    Applying,
    AwaitingFirstPresentation,
    RollingBack,
    Committed,
    RolledBack,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputTopologyTransactionFailure {
    Stale,
    Preparation,
    Apply,
    HeadLost(RenderHeadId),
    FirstPresentation,
    Rollback,
    Invariant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputTopologyTransactionTransition {
    Accepted,
    PhaseReady,
    Duplicate,
    UnknownHead,
    UnknownOutput,
    OutOfOrder,
    Terminal,
}

/// Engine's passive settlement record for one complete topology candidate.
///
/// The old topology remains published while this record prepares and applies.
/// The owner replaces it only after `Committed`, which requires one first
/// presentation from every new logical output. A partial physical apply can
/// move only into rollback; it can never be relabelled as committed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputTopologyTransaction {
    base_topology_epoch: u64,
    candidate_topology_epoch: u64,
    required_heads: BTreeSet<RenderHeadId>,
    required_outputs: BTreeSet<OutputId>,
    prepared_heads: BTreeSet<RenderHeadId>,
    applied_heads: BTreeSet<RenderHeadId>,
    first_presented_outputs: BTreeSet<OutputId>,
    rollback_heads: BTreeSet<RenderHeadId>,
    phase: OutputTopologyTransactionPhase,
    failure: Option<OutputTopologyTransactionFailure>,
}

impl OutputTopologyTransaction {
    pub fn new(
        base_topology_epoch: u64,
        candidate_topology_epoch: u64,
        heads: impl IntoIterator<Item = RenderHeadId>,
        outputs: impl IntoIterator<Item = OutputId>,
    ) -> Option<Self> {
        let required_heads = heads.into_iter().collect::<BTreeSet<_>>();
        let required_outputs = outputs.into_iter().collect::<BTreeSet<_>>();
        (base_topology_epoch != 0
            && candidate_topology_epoch > base_topology_epoch
            && !required_heads.is_empty()
            && required_heads.len() <= crate::MAX_DRM_KMS_OUTPUTS * crate::MAX_HEADS_PER_OUTPUT
            && required_heads.iter().all(|head| head.is_valid())
            && !required_outputs.is_empty()
            && required_outputs.len() <= crate::MAX_DRM_KMS_OUTPUTS
            && required_outputs.iter().all(|output| output.is_valid()))
        .then_some(Self {
            base_topology_epoch,
            candidate_topology_epoch,
            required_heads,
            required_outputs,
            prepared_heads: BTreeSet::new(),
            applied_heads: BTreeSet::new(),
            first_presented_outputs: BTreeSet::new(),
            rollback_heads: BTreeSet::new(),
            phase: OutputTopologyTransactionPhase::Preparing,
            failure: None,
        })
    }

    pub const fn base_topology_epoch(&self) -> u64 {
        self.base_topology_epoch
    }

    pub const fn candidate_topology_epoch(&self) -> u64 {
        self.candidate_topology_epoch
    }

    pub const fn phase(&self) -> OutputTopologyTransactionPhase {
        self.phase
    }

    pub const fn failure(&self) -> Option<OutputTopologyTransactionFailure> {
        self.failure
    }

    pub fn mark_prepared(&mut self, head: RenderHeadId) -> OutputTopologyTransactionTransition {
        if self.phase != OutputTopologyTransactionPhase::Preparing {
            return self.out_of_order();
        }
        if !self.required_heads.contains(&head) {
            return OutputTopologyTransactionTransition::UnknownHead;
        }
        if !self.prepared_heads.insert(head) {
            return OutputTopologyTransactionTransition::Duplicate;
        }
        if self.prepared_heads == self.required_heads {
            self.phase = OutputTopologyTransactionPhase::Prepared;
            OutputTopologyTransactionTransition::PhaseReady
        } else {
            OutputTopologyTransactionTransition::Accepted
        }
    }

    pub fn begin_apply(&mut self) -> OutputTopologyTransactionTransition {
        if self.phase != OutputTopologyTransactionPhase::Prepared {
            return self.out_of_order();
        }
        self.phase = OutputTopologyTransactionPhase::Applying;
        OutputTopologyTransactionTransition::PhaseReady
    }

    pub fn mark_applied(&mut self, head: RenderHeadId) -> OutputTopologyTransactionTransition {
        if self.phase != OutputTopologyTransactionPhase::Applying {
            return self.out_of_order();
        }
        if !self.required_heads.contains(&head) {
            return OutputTopologyTransactionTransition::UnknownHead;
        }
        if !self.applied_heads.insert(head) {
            return OutputTopologyTransactionTransition::Duplicate;
        }
        if self.applied_heads == self.required_heads {
            self.phase = OutputTopologyTransactionPhase::AwaitingFirstPresentation;
            OutputTopologyTransactionTransition::PhaseReady
        } else {
            OutputTopologyTransactionTransition::Accepted
        }
    }

    pub fn mark_first_presented(
        &mut self,
        output: OutputId,
    ) -> OutputTopologyTransactionTransition {
        if self.phase != OutputTopologyTransactionPhase::AwaitingFirstPresentation {
            return self.out_of_order();
        }
        if !self.required_outputs.contains(&output) {
            return OutputTopologyTransactionTransition::UnknownOutput;
        }
        if !self.first_presented_outputs.insert(output) {
            return OutputTopologyTransactionTransition::Duplicate;
        }
        if self.first_presented_outputs == self.required_outputs {
            self.phase = OutputTopologyTransactionPhase::Committed;
            OutputTopologyTransactionTransition::PhaseReady
        } else {
            OutputTopologyTransactionTransition::Accepted
        }
    }

    pub fn fail(
        &mut self,
        failure: OutputTopologyTransactionFailure,
    ) -> OutputTopologyTransactionTransition {
        if matches!(
            self.phase,
            OutputTopologyTransactionPhase::Committed
                | OutputTopologyTransactionPhase::RolledBack
                | OutputTopologyTransactionPhase::Failed
        ) {
            return OutputTopologyTransactionTransition::Terminal;
        }
        self.failure = Some(failure);
        if self.applied_heads.is_empty() {
            self.phase = OutputTopologyTransactionPhase::Failed;
        } else {
            self.phase = OutputTopologyTransactionPhase::RollingBack;
        }
        OutputTopologyTransactionTransition::PhaseReady
    }

    pub fn mark_rolled_back(&mut self, head: RenderHeadId) -> OutputTopologyTransactionTransition {
        if self.phase != OutputTopologyTransactionPhase::RollingBack {
            return self.out_of_order();
        }
        if !self.applied_heads.contains(&head) {
            return OutputTopologyTransactionTransition::UnknownHead;
        }
        if !self.rollback_heads.insert(head) {
            return OutputTopologyTransactionTransition::Duplicate;
        }
        if self.rollback_heads == self.applied_heads {
            self.phase = OutputTopologyTransactionPhase::RolledBack;
            OutputTopologyTransactionTransition::PhaseReady
        } else {
            OutputTopologyTransactionTransition::Accepted
        }
    }

    pub fn rollback_failed(&mut self) -> OutputTopologyTransactionTransition {
        if self.phase != OutputTopologyTransactionPhase::RollingBack {
            return self.out_of_order();
        }
        self.failure = Some(OutputTopologyTransactionFailure::Rollback);
        self.phase = OutputTopologyTransactionPhase::Failed;
        OutputTopologyTransactionTransition::PhaseReady
    }

    fn out_of_order(&self) -> OutputTopologyTransactionTransition {
        if matches!(
            self.phase,
            OutputTopologyTransactionPhase::Committed
                | OutputTopologyTransactionPhase::RolledBack
                | OutputTopologyTransactionPhase::Failed
        ) {
            OutputTopologyTransactionTransition::Terminal
        } else {
            OutputTopologyTransactionTransition::OutOfOrder
        }
    }
}
