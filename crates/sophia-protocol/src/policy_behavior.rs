/// Stable names for the revision-1 black-box behavior corpus. Every public
/// policy client must accept the same complete snapshots and return a proposal
/// admitted by the canonical reducer for each entry before revision 1 freezes.
pub const SOPHIA_WM_V1_BEHAVIOR_SCENARIOS: [&str; 4] = [
    "single-output-constraints",
    "two-output-partition",
    "output-loss",
    "returned-output-generation",
];
