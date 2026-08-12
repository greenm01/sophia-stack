//! Output power, kept separate from mode and enablement.
//!
//! Blanking a screen and removing a monitor are different facts, and conflating
//! them costs more than it saves. Enablement is expressed by omission from the
//! complete policy snapshot: a disabled output is not part of the desktop, its
//! surfaces must go somewhere else, and policy relays out. A powered-down output is
//! still part of the desktop — its bounds, its work area, and its surfaces all
//! survive — and policy must not see the transition at all.
//!
//! The distinction is easy to lose at the KMS layer, where atomic modesetting
//! powers a head down by clearing the CRTC's `ACTIVE`, the same property that
//! disables one. That is an implementation detail of the commit, not a licence to
//! merge the two above it. This authority holds the difference so the layers above
//! never have to reconstruct it.
//!
//! Power transitions therefore do not travel through topology activation. A
//! topology change is a candidate that is validated, applied, and rolled back as a
//! unit; a power change alters no geometry, invalidates no candidate, and needs no
//! rollback beyond restoring the previous level.

use crate::prelude::*;

/// What an output is currently doing with its light.
///
/// The intermediate levels exist because hardware distinguishes them and because a
/// policy for idling is entitled to choose. Sophia's own transitions use `On` and
/// `Off`; the middle two are carried so a configured idle policy can request them
/// without widening this vocabulary later.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum OutputPowerLevel {
    #[default]
    On,
    Standby,
    Suspend,
    Off,
}

impl OutputPowerLevel {
    /// Whether this level scans out. Only `On` does, which is what makes power a
    /// presentation concern and never a layout one.
    pub const fn scans_out(self) -> bool {
        matches!(self, Self::On)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Standby => "standby",
            Self::Suspend => "suspend",
            Self::Off => "off",
        }
    }
}

/// What a requested power change amounts to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputPowerTransition {
    /// The output already sits at the requested level.
    Unchanged,
    /// The level changed and the backend must be told.
    Changed {
        from: OutputPowerLevel,
        to: OutputPowerLevel,
    },
    /// The output is not part of the desktop, so it has no power state to set.
    /// Refused rather than recorded: a level held for an output nobody presents
    /// would be applied to whatever later claimed that id.
    UnknownOutput,
}

/// Per-output power levels for the outputs the desktop currently has.
///
/// Admission is explicit. An output enters at `On` when the topology admits it and
/// leaves when the topology drops it, so this state can never describe an output
/// that is not part of the desktop.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OutputPowerState {
    levels: BTreeMap<OutputId, OutputPowerLevel>,
}

impl OutputPowerState {
    /// Replaces the admitted output set, keeping the level of every output that
    /// survives.
    ///
    /// A topology change must not silently relight an output the operator powered
    /// down, and must not retain a level for one that went away. Both follow from
    /// reconciling against the new set rather than rebuilding from it.
    pub fn admit_outputs(&mut self, outputs: impl IntoIterator<Item = OutputId>) {
        let mut next = BTreeMap::new();
        for output in outputs {
            if !output.is_valid() {
                continue;
            }
            let level = self.levels.get(&output).copied().unwrap_or_default();
            next.insert(output, level);
        }
        self.levels = next;
    }

    pub fn request(&mut self, output: OutputId, level: OutputPowerLevel) -> OutputPowerTransition {
        let Some(current) = self.levels.get_mut(&output) else {
            return OutputPowerTransition::UnknownOutput;
        };
        if *current == level {
            return OutputPowerTransition::Unchanged;
        }
        let from = *current;
        *current = level;
        OutputPowerTransition::Changed { from, to: level }
    }

    pub fn level(&self, output: OutputId) -> Option<OutputPowerLevel> {
        self.levels.get(&output).copied()
    }

    /// Outputs that should be scanning out. A frame produced for an output not in
    /// this set is work nobody sees.
    pub fn scanning_out(&self) -> Vec<OutputId> {
        self.levels
            .iter()
            .filter(|(_, level)| level.scans_out())
            .map(|(output, _)| *output)
            .collect()
    }

    /// True when every admitted output is dark. The session is still running and the
    /// desktop still exists; this is the condition for skipping frame production,
    /// not for tearing anything down.
    pub fn fully_dark(&self) -> bool {
        !self.levels.is_empty() && self.levels.values().all(|level| !level.scans_out())
    }

    pub fn len(&self) -> usize {
        self.levels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }
}
