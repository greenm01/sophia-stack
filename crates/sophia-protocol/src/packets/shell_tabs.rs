use crate::{OutputId, ShellV1Descriptor};
pub const SOPHIA_SHELL_TAB_REVISION: u16 = 2;
pub const SOPHIA_SHELL_CAPABILITY_TAB_GROUPS: u64 = 1 << 2;
pub const SOPHIA_SHELL_MAX_TAB_GROUPS: usize = 1024;
pub const SOPHIA_SHELL_MAX_TAB_ENTRIES: usize = 2048;

/// Recipient-local presentation handles; no geometry or surface identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellTabGroup {
    pub slot: u64,
    pub output: OutputId,
    pub focused: bool,
    pub selected_slot: Option<u16>,
    pub entries: Vec<ShellV1Descriptor>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellTabSnapshot {
    pub connection_epoch: u64,
    pub generation: u64,
    pub groups: Vec<ShellTabGroup>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellTabCandidate {
    pub connection_epoch: u64,
    pub snapshot_generation: u64,
    pub candidate_generation: u64,
    pub groups: Vec<u64>,
}
