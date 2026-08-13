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
/// Keyed by connector name, which is what configuration speaks and what a card can
/// answer directly from a selection. Keying by DRM connector id looked tidier and
/// was circular: the id-to-name map lives on a capability, capabilities are readable
/// only after sessions are built, and the grouping is needed to build them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeMirrorGrouping {
    groups: Vec<Vec<String>>,
}

/// Why a proposed grouping cannot be used.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeMirrorGroupingError {
    /// A group named no connector, so it identifies no output.
    EmptyGroup,
    /// One connector appeared in two groups. A connector drives one logical output
    /// or the identity of that output is undefined.
    ConnectorInTwoGroups(String),
}

impl NativeMirrorGrouping {
    /// The ordinary desktop: one logical output per connector.
    pub const fn none() -> Self {
        Self { groups: Vec::new() }
    }

    /// Builds a grouping, rejecting one that cannot describe a desktop.
    pub fn new(
        groups: impl IntoIterator<Item = Vec<String>>,
    ) -> Result<Self, NativeMirrorGroupingError> {
        let groups = groups.into_iter().collect::<Vec<_>>();
        let mut claimed = BTreeSet::new();
        for group in &groups {
            if group.is_empty() {
                return Err(NativeMirrorGroupingError::EmptyGroup);
            }
            for connector in group {
                if !claimed.insert(connector.clone()) {
                    return Err(NativeMirrorGroupingError::ConnectorInTwoGroups(
                        connector.clone(),
                    ));
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
    pub fn group_of(&self, connector: &str) -> Option<usize> {
        self.groups
            .iter()
            .position(|group| group.iter().any(|member| member == connector))
    }

    /// Whether this connector shares its logical output with another.
    pub fn is_mirrored(&self, connector: &str) -> bool {
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
