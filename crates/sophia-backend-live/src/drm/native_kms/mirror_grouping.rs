//! Which connectors share one logical output.
//!
//! Mirroring is one logical output backed by N connectors, so something has to
//! decide which connectors those are before output identities are handed out. That
//! decision comes from configuration and is expressed here as passive data, so the
//! session construction that consumes it stays a loop over selections rather than a
//! second place where mirroring policy lives.
//!
//! An empty grouping is the ordinary desktop: every connector is its own logical
//! output. That is deliberately the default, because a grouping mistake should cost
//! a missing mirror rather than two screens unexpectedly showing the same thing.

use std::collections::BTreeSet;

/// Connector sets that each drive one logical output.
///
/// Connectors are named by their DRM connector id rather than by name, because this
/// layer sees ids and the configuration layer that speaks names has already
/// resolved them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeMirrorGrouping {
    groups: Vec<Vec<u32>>,
}

/// Why a proposed grouping cannot be used.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeMirrorGroupingError {
    /// A group named no connector, so it identifies no output.
    EmptyGroup,
    /// One connector appeared in two groups. A connector drives one logical output
    /// or the identity of that output is undefined.
    ConnectorInTwoGroups(u32),
}

impl NativeMirrorGrouping {
    /// The ordinary desktop: one logical output per connector.
    pub const fn none() -> Self {
        Self { groups: Vec::new() }
    }

    /// Builds a grouping, rejecting one that cannot describe a desktop.
    pub fn new(
        groups: impl IntoIterator<Item = Vec<u32>>,
    ) -> Result<Self, NativeMirrorGroupingError> {
        let groups = groups.into_iter().collect::<Vec<_>>();
        let mut claimed = BTreeSet::new();
        for group in &groups {
            if group.is_empty() {
                return Err(NativeMirrorGroupingError::EmptyGroup);
            }
            for connector in group {
                if !claimed.insert(*connector) {
                    return Err(NativeMirrorGroupingError::ConnectorInTwoGroups(*connector));
                }
            }
        }
        Ok(Self { groups })
    }

    /// The group a connector belongs to, if any.
    ///
    /// A connector in no group is its own logical output, which is why this returns
    /// `None` rather than inventing a single-member group: the caller allocates a
    /// fresh identity for it, and a group index would imply a shared one.
    pub fn group_of(&self, connector: u32) -> Option<usize> {
        self.groups
            .iter()
            .position(|group| group.contains(&connector))
    }

    /// Whether this connector shares its logical output with another.
    pub fn is_mirrored(&self, connector: u32) -> bool {
        self.group_of(connector)
            .is_some_and(|index| self.groups[index].len() > 1)
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    pub fn len(&self) -> usize {
        self.groups.len()
    }
}
