use crate::prelude::*;

/// Engine-owned join for one visual transaction whose pixels intersect more
/// than one logical output.
///
/// Each output keeps its independent physical-head cohort and page-flip time.
/// This reducer carries no native owner; it only prevents transaction feedback
/// from becoming visible until every applicable logical output has retired.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionPresentationCohort {
    transaction: TransactionId,
    required_outputs: BTreeSet<OutputId>,
    submitted_outputs: BTreeSet<OutputId>,
    retired_outputs: BTreeMap<OutputId, u64>,
    terminal: Option<TransactionPresentationTerminal>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionPresentationTerminal {
    Presented {
        logical_sequence: u64,
        ust_usec: u64,
    },
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionPresentationTransition {
    Accepted,
    PhaseReady,
    Duplicate,
    UnknownOutput,
    RetiredBeforeSubmission,
    Terminal,
}

impl TransactionPresentationCohort {
    pub fn new(
        transaction: TransactionId,
        outputs: impl IntoIterator<Item = OutputId>,
    ) -> Option<Self> {
        let required_outputs = outputs.into_iter().collect::<BTreeSet<_>>();
        (transaction.is_valid()
            && !required_outputs.is_empty()
            && required_outputs.len() <= crate::MAX_DRM_KMS_OUTPUTS
            && required_outputs.iter().all(|output| output.is_valid()))
        .then_some(Self {
            transaction,
            required_outputs,
            submitted_outputs: BTreeSet::new(),
            retired_outputs: BTreeMap::new(),
            terminal: None,
        })
    }

    pub const fn transaction(&self) -> TransactionId {
        self.transaction
    }

    pub fn required_outputs(&self) -> impl Iterator<Item = OutputId> + '_ {
        self.required_outputs.iter().copied()
    }

    pub fn is_required(&self, output: OutputId) -> bool {
        self.required_outputs.contains(&output)
    }

    pub fn output_submitted(&self, output: OutputId) -> bool {
        self.submitted_outputs.contains(&output)
    }

    pub fn output_retired(&self, output: OutputId) -> bool {
        self.retired_outputs.contains_key(&output)
    }

    pub fn all_submitted(&self) -> bool {
        self.submitted_outputs == self.required_outputs
    }

    pub fn all_retired(&self) -> bool {
        self.retired_outputs.len() == self.required_outputs.len()
            && self
                .required_outputs
                .iter()
                .all(|output| self.retired_outputs.contains_key(output))
    }

    pub const fn terminal(&self) -> Option<TransactionPresentationTerminal> {
        self.terminal
    }

    pub fn mark_submitted(&mut self, output: OutputId) -> TransactionPresentationTransition {
        if self.terminal.is_some() {
            return TransactionPresentationTransition::Terminal;
        }
        if !self.required_outputs.contains(&output) {
            return TransactionPresentationTransition::UnknownOutput;
        }
        if !self.submitted_outputs.insert(output) {
            return TransactionPresentationTransition::Duplicate;
        }
        if self.all_submitted() {
            TransactionPresentationTransition::PhaseReady
        } else {
            TransactionPresentationTransition::Accepted
        }
    }

    pub fn mark_retired(
        &mut self,
        output: OutputId,
        ust_usec: u64,
    ) -> TransactionPresentationTransition {
        if self.terminal.is_some() {
            return TransactionPresentationTransition::Terminal;
        }
        if !self.required_outputs.contains(&output) {
            return TransactionPresentationTransition::UnknownOutput;
        }
        if !self.submitted_outputs.contains(&output) {
            return TransactionPresentationTransition::RetiredBeforeSubmission;
        }
        if self.retired_outputs.insert(output, ust_usec).is_some() {
            return TransactionPresentationTransition::Duplicate;
        }
        if self.all_retired() {
            let ust_usec = self.retired_outputs.values().copied().max().unwrap_or(0);
            self.terminal = Some(TransactionPresentationTerminal::Presented {
                logical_sequence: self.transaction.raw(),
                ust_usec,
            });
            TransactionPresentationTransition::PhaseReady
        } else {
            TransactionPresentationTransition::Accepted
        }
    }

    pub fn fail(&mut self) -> TransactionPresentationTransition {
        if self.terminal.is_some() {
            return TransactionPresentationTransition::Terminal;
        }
        self.terminal = Some(TransactionPresentationTerminal::Failed);
        TransactionPresentationTransition::PhaseReady
    }
}
