//! The metadata broker's model: what it knows and what it publishes.
//!
//! TEA-shaped per `docs/style-guide.md`. Events describe what an authority or the
//! session observed; the model reduces them; commands are the rule an authority must
//! apply and the descriptor Engine will store. Nothing here reaches back into
//! another component's state, and nothing here is X-aware — every input names a
//! `SurfaceId` and a `NamespaceId`, both authority-neutral, which is what stops a
//! second authority from needing a second broker.
//!
//! What the broker owns is exactly what an authority cannot decide alone: trust,
//! icon tokens, and disclosure. What it deliberately never holds is raw identity;
//! labels arrive already reduced.

use std::collections::BTreeMap;

use sophia_protocol::{
    AttentionState, IconTokenId, IdAllocator, MetadataDisclosure, MetadataDisclosureRule,
    NamespaceProfile, ReducedMetadataCandidate, SanitizedChromeMetadata, SurfaceId,
};

use crate::trust::{trust_for_namespace_profile, unknown_trust};

/// Most surfaces a broker will track at once.
///
/// Matched to the policy snapshot's surface bound: a broker describing more surfaces
/// than policy can lay out is describing something nobody will draw.
pub const BROKER_MAX_SURFACES: usize = 1024;

/// What the broker learned, from an authority or from the session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetadataBrokerEvent {
    /// A surface entered the desktop in a known namespace.
    SurfaceAdmitted {
        surface: SurfaceId,
        profile: NamespaceProfile,
    },
    /// An authority applied the published rule and produced this.
    CandidateReduced(ReducedMetadataCandidate),
    /// Attention state changed. Separate from the candidate because it is not
    /// identity and does not pass through disclosure.
    AttentionChanged {
        surface: SurfaceId,
        attention: AttentionState,
    },
    /// The surface left the desktop.
    SurfaceRemoved { surface: SurfaceId },
}

/// What the broker decided, for someone else to carry out.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetadataBrokerCommand {
    /// Give this rule to the authority owning the surface.
    PublishRule(MetadataDisclosureRule),
    /// Give this descriptor to Engine.
    EmitDescriptor(SanitizedChromeMetadata),
    /// Drop everything about this surface.
    RetireSurface { surface: SurfaceId },
}

/// Why an event changed nothing.
///
/// Rejections are outcomes rather than errors, per `docs/style-guide.md`: a stale
/// candidate is an ordinary consequence of two components running at once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataBrokerRejection {
    /// No surface by that id has been admitted.
    UnknownSurface,
    /// An older generation than one already applied.
    StaleGeneration,
    /// The broker is full and will not evict a live surface to make room.
    CapacityExhausted,
    /// The authority disclosed more than its rule allowed.
    DisclosureExceeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BrokerSurface {
    profile: Option<NamespaceProfile>,
    disclosure: MetadataDisclosure,
    icon: IconTokenId,
    attention: AttentionState,
    label: Option<String>,
    label_redacted: bool,
    generation: u64,
}

/// Per-surface metadata state and the tokens it owns.
#[derive(Debug)]
pub struct MetadataBroker {
    surfaces: BTreeMap<SurfaceId, BrokerSurface>,
    icons: IdAllocator<IconTokenId>,
}

impl Default for MetadataBroker {
    fn default() -> Self {
        Self::new()
    }
}

impl MetadataBroker {
    pub const fn new() -> Self {
        Self {
            surfaces: BTreeMap::new(),
            icons: IdAllocator::new(),
        }
    }

    /// Reduces one event into commands, or says why it changed nothing.
    pub fn update(
        &mut self,
        event: MetadataBrokerEvent,
    ) -> Result<Vec<MetadataBrokerCommand>, MetadataBrokerRejection> {
        match event {
            MetadataBrokerEvent::SurfaceAdmitted { surface, profile } => {
                self.admit(surface, profile)
            }
            MetadataBrokerEvent::CandidateReduced(candidate) => self.apply_candidate(candidate),
            MetadataBrokerEvent::AttentionChanged { surface, attention } => {
                self.set_attention(surface, attention)
            }
            MetadataBrokerEvent::SurfaceRemoved { surface } => self.retire(surface),
        }
    }

    /// Admits a surface and publishes its first rule.
    ///
    /// The rule is published on admission rather than on first candidate because an
    /// authority cannot reduce without one, and the default of `None` means a
    /// surface discloses nothing during the window before anyone has thought about
    /// it.
    fn admit(
        &mut self,
        surface: SurfaceId,
        profile: NamespaceProfile,
    ) -> Result<Vec<MetadataBrokerCommand>, MetadataBrokerRejection> {
        if !surface.is_valid() {
            return Err(MetadataBrokerRejection::UnknownSurface);
        }
        // A returning surface keeps its token, so a taskbar entry does not change
        // icon because a client reconnected.
        let icon = match self.surfaces.get(&surface) {
            Some(existing) => existing.icon,
            None => {
                if self.surfaces.len() >= BROKER_MAX_SURFACES {
                    return Err(MetadataBrokerRejection::CapacityExhausted);
                }
                self.icons.next_id()
            }
        };
        let entry = self.surfaces.entry(surface).or_insert(BrokerSurface {
            profile: None,
            disclosure: MetadataDisclosure::None,
            icon,
            attention: AttentionState::None,
            label: None,
            label_redacted: false,
            generation: 0,
        });
        entry.profile = Some(profile);
        entry.icon = icon;

        Ok(vec![MetadataBrokerCommand::PublishRule(
            self.rule_for(surface)
                .expect("the surface was just admitted"),
        )])
    }

    /// Accepts a reduced candidate and turns it into a descriptor.
    ///
    /// The candidate's own disclosure level is checked against the published rule.
    /// An authority that disclosed more than it was permitted is refused rather than
    /// trimmed: trimming would hide a broken authority behind a working desktop, and
    /// the whole boundary rests on authorities applying the rule honestly.
    fn apply_candidate(
        &mut self,
        candidate: ReducedMetadataCandidate,
    ) -> Result<Vec<MetadataBrokerCommand>, MetadataBrokerRejection> {
        let Some(entry) = self.surfaces.get_mut(&candidate.surface) else {
            return Err(MetadataBrokerRejection::UnknownSurface);
        };
        if candidate.generation < entry.generation {
            return Err(MetadataBrokerRejection::StaleGeneration);
        }
        if candidate.disclosure > entry.disclosure {
            return Err(MetadataBrokerRejection::DisclosureExceeded);
        }
        entry.generation = candidate.generation;

        let trust = entry
            .profile
            .map_or_else(unknown_trust, trust_for_namespace_profile);
        let (label, label_redacted) = candidate
            .label
            .map_or((None, false), |label| (Some(label.text), label.redacted));
        entry.label.clone_from(&label);
        entry.label_redacted = label_redacted;

        Ok(vec![MetadataBrokerCommand::EmitDescriptor(
            SanitizedChromeMetadata {
                surface: candidate.surface,
                label,
                label_redacted,
                icon: Some(entry.icon),
                trust_level: trust,
                attention: entry.attention,
                generation: candidate.generation,
            },
        )])
    }

    fn set_attention(
        &mut self,
        surface: SurfaceId,
        attention: AttentionState,
    ) -> Result<Vec<MetadataBrokerCommand>, MetadataBrokerRejection> {
        let Some(entry) = self.surfaces.get_mut(&surface) else {
            return Err(MetadataBrokerRejection::UnknownSurface);
        };
        if entry.attention == attention {
            return Ok(Vec::new());
        }
        entry.attention = attention;
        // Attention is not identity, so it needs no candidate and no new rule. The
        // descriptor is reissued at the same generation because nothing about the
        // label changed.
        Ok(vec![MetadataBrokerCommand::EmitDescriptor(
            SanitizedChromeMetadata {
                surface,
                label: entry.label.clone(),
                label_redacted: entry.label_redacted,
                icon: Some(entry.icon),
                trust_level: entry
                    .profile
                    .map_or_else(unknown_trust, trust_for_namespace_profile),
                attention,
                generation: entry.generation,
            },
        )])
    }

    fn retire(
        &mut self,
        surface: SurfaceId,
    ) -> Result<Vec<MetadataBrokerCommand>, MetadataBrokerRejection> {
        if self.surfaces.remove(&surface).is_none() {
            return Err(MetadataBrokerRejection::UnknownSurface);
        }
        // The token is dropped with the surface and never reused, because the
        // allocator only moves forward. A recycled token would let a stale
        // descriptor point at a different window.
        Ok(vec![MetadataBrokerCommand::RetireSurface { surface }])
    }

    /// Raises or lowers what one surface may disclose, publishing the new rule.
    pub fn set_disclosure(
        &mut self,
        surface: SurfaceId,
        disclosure: MetadataDisclosure,
    ) -> Result<Vec<MetadataBrokerCommand>, MetadataBrokerRejection> {
        let Some(entry) = self.surfaces.get_mut(&surface) else {
            return Err(MetadataBrokerRejection::UnknownSurface);
        };
        let replacement = if disclosure < entry.disclosure {
            entry.label = None;
            entry.label_redacted = false;
            Some(SanitizedChromeMetadata {
                surface,
                label: None,
                label_redacted: false,
                icon: Some(entry.icon),
                trust_level: entry
                    .profile
                    .map_or_else(unknown_trust, trust_for_namespace_profile),
                attention: entry.attention,
                generation: entry.generation,
            })
        } else {
            None
        };
        entry.disclosure = disclosure;
        let mut commands = vec![MetadataBrokerCommand::PublishRule(
            self.rule_for(surface).expect("the surface exists"),
        )];
        if let Some(replacement) = replacement {
            commands.push(MetadataBrokerCommand::EmitDescriptor(replacement));
        }
        Ok(commands)
    }

    pub fn rule_for(&self, surface: SurfaceId) -> Option<MetadataDisclosureRule> {
        let entry = self.surfaces.get(&surface)?;
        Some(MetadataDisclosureRule {
            surface,
            disclosure: entry.disclosure,
            trust_level: entry
                .profile
                .map_or_else(unknown_trust, trust_for_namespace_profile),
            icon: Some(entry.icon),
            generation: entry.generation,
        })
    }

    pub fn len(&self) -> usize {
        self.surfaces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }
}
