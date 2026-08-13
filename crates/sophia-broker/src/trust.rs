//! Trust assigned from the namespace a client was admitted into.
//!
//! This is the broker's first real decision, and `TrustLevel` had no production
//! assigner before it, so the mapping below is a choice rather than a translation.
//!
//! Trust belongs here because it is a cross-authority fact. Two authorities reading
//! the same namespace must reach the same trust, or a user sees one application
//! badged two ways depending on which frontend admitted it.

use sophia_protocol::{NamespaceProfile, TrustLevel};

/// Maps an admitted namespace profile onto a trust level.
///
/// `ClassicShared` is `Trusted` because the profile means it: these clients
/// deliberately retain ordinary shared-X coordination, which is a decision someone
/// made about them, not an absence of one.
///
/// `Confined` is `Isolated` rather than `Untrusted`. Confinement describes what the
/// namespace *enforces* — discovery and delivery fail closed outside it — and says
/// nothing about whether the client deserves suspicion. A sandboxed application from
/// a trusted vendor is isolated and not untrusted, and badging it as untrusted would
/// teach users to ignore the badge.
///
/// `Untrusted` is therefore deliberately unreachable from a profile alone. It is a
/// judgment about a client, and the broker has no input carrying one yet; inventing
/// a mapping to fill the arm would put a security label on evidence that does not
/// exist. `Unknown` covers a surface whose namespace has not been established.
pub const fn trust_for_namespace_profile(profile: NamespaceProfile) -> TrustLevel {
    match profile {
        NamespaceProfile::ClassicShared => TrustLevel::Trusted,
        NamespaceProfile::Confined => TrustLevel::Isolated,
    }
}

/// Trust for a surface whose namespace is not yet known.
///
/// Separate from the mapping above so the absence of a profile is expressed by
/// calling this, not by passing a placeholder profile into it.
pub const fn unknown_trust() -> TrustLevel {
    TrustLevel::Unknown
}
