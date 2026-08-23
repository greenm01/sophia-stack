use crate::{AttentionState, DisplayLabel, OutputId, TrustLevel};

pub const SOPHIA_SHELL_INTERFACE_REVISION: u16 = 1;
pub const SOPHIA_SHELL_CAPABILITY_DESCRIPTOR_SWITCHER: u64 = 1 << 0;
pub const SOPHIA_SHELL_MAX_DESCRIPTORS: usize = 16;
pub const SOPHIA_SHELL_MAX_PENDING_ACTIVATIONS: usize = 16;

/// A broker-issued, shell-recipient-scoped toplevel activation capability.
///
/// The record name fixes the issuer, recipient, and operation class. Its wire
/// integer has no meaning outside this family and exact epoch tuple.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ToplevelActionCapabilityRef {
    pub token: u64,
    pub issuer_epoch: u64,
    pub issuer_revocation_epoch: u64,
    pub recipient_epoch: u64,
    pub target_slot: u16,
    pub target_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShellV1ClientHello {
    pub minimum_revision: u16,
    pub maximum_revision: u16,
    pub required_capabilities: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShellV1ServerWelcome {
    pub selected_revision: u16,
    pub connection_epoch: u64,
    pub capabilities: u64,
    pub max_descriptors: u16,
    pub max_label_bytes: u16,
    pub max_pending_activations: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellV1Descriptor {
    pub slot: u16,
    pub generation: u64,
    pub label: Option<DisplayLabel>,
    pub trust_level: TrustLevel,
    pub attention: AttentionState,
    pub action: ToplevelActionCapabilityRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellV1DescriptorSnapshot {
    pub connection_epoch: u64,
    pub snapshot_generation: u64,
    pub output: OutputId,
    pub output_generation: u64,
    pub broker_epoch: u64,
    pub broker_revocation_epoch: u64,
    pub descriptors: Vec<ShellV1Descriptor>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShellV1CandidateEntry {
    pub slot: u16,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellV1Candidate {
    pub connection_epoch: u64,
    pub snapshot_generation: u64,
    pub candidate_generation: u64,
    pub output: OutputId,
    pub visible: bool,
    pub selected_slot: Option<u16>,
    pub entries: Vec<ShellV1CandidateEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShellV1CandidateOutcomeKind {
    Prepared,
    Presented,
    Rejected,
    Superseded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShellV1CandidateOutcome {
    pub connection_epoch: u64,
    pub candidate_generation: u64,
    pub presentation_epoch: u64,
    pub kind: ShellV1CandidateOutcomeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShellV1Activation {
    pub connection_epoch: u64,
    pub candidate_generation: u64,
    pub presentation_epoch: u64,
    pub activation: u64,
    pub action: ToplevelActionCapabilityRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShellV1ActivationDisposition {
    Consumed,
    RejectedStale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShellV1ActivationAck {
    pub connection_epoch: u64,
    pub activation: u64,
    pub disposition: ShellV1ActivationDisposition,
}
