//! Saturation disposition for bounded resources.
//!
//! A bounded resource's behaviour under pressure is data attached to the
//! resource, not control flow at each call site. Changing what a site does when
//! it fills changes a `BoundedCapacity` value, not its code.
//!
//! The five dispositions are this project's existing documented policy rather
//! than new invention. Each variant cites where it is already stated. They were
//! previously chosen ad hoc per call site, which is how identical resource
//! classes ended up with opposite behaviour: a per-client input queue closes one
//! endpoint while the shared ingress ended the whole session.

/// Compile-time identity of one bounded resource.
///
/// Names a resource the code owns. It is never a client, XID, namespace, PID,
/// title, or payload, so a saturation report can be logged by default.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CapacityResourceId(pub &'static str);

impl CapacityResourceId {
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Where a bounded deferral goes when its deadline expires.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapacityEscalation {
    Terminal,
    EndpointEpochClosed,
    RejectAndConsume,
}

/// What a bounded resource does when admission would exceed its bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapacitySaturationDisposition {
    /// Overflow cannot be reduced to a smaller blast radius without losing
    /// committed state. The owner returns a typed terminal error, never a panic
    /// and never an unbounded allocation.
    Terminal,
    /// The saturating recipient's epoch is revoked and its queued work is
    /// discarded. The session and every other endpoint continue: one peer that
    /// stops reading must not end the frontend.
    EndpointEpochClosed,
    /// Admission waits inside a bounded deadline, then escalates. Deferral is a
    /// state transition rather than a rejection, so the producer stops
    /// producing instead of the queue growing.
    BoundedDeferral {
        deadline_msec: u32,
        retry_interval_msec: u32,
        escalation: CapacityEscalation,
    },
    /// The record is refused and consumed exactly once with a bounded
    /// diagnostic. It is never merged into another record, because repeated
    /// activations of the same opaque token are distinct intents.
    RejectAndConsume,
    /// A named-cause degraded result replaces the exact one. The degraded result
    /// is never labelled as exact.
    DegradeWithCause,
}

/// Whether a record may consume reserved capacity.
///
/// Stream admission reserves room for the terminating boundary that closes it,
/// so ordinary pressure can never make a completion, cancellation, or release
/// undeliverable. A dropped release is a stuck key, not a dropped frame.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CapacityClass {
    /// Ordered, non-idempotent work. Admitted only above the reserve.
    #[default]
    Ordered,
    /// A replaceable hint occupying one keyed slot, which never grows the
    /// resource. Permitted only for replaceable refreshes and continuous
    /// interaction geometry at the same target, kind, axis, capture, and epoch.
    Replaceable,
    /// A terminating boundary. Admitted into the reserve.
    TerminatingBoundary,
}

/// Why a bounded resource refused admission.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CapacitySaturationCause {
    /// Occupancy reached the configured bound.
    DepthExhausted,
    /// Occupancy plus the boundaries still owed reached the bound, so only a
    /// terminating boundary may still be admitted.
    ReserveExhausted,
    /// A bounded deferral crossed its deadline.
    DeadlineExpired,
    /// The recipient closed while work was held for it.
    RecipientGone,
    /// A monotonic identity space ran out.
    IdentityExhausted,
}

impl CapacitySaturationCause {
    /// Stable snake_case token for structured logs.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DepthExhausted => "depth_exhausted",
            Self::ReserveExhausted => "reserve_exhausted",
            Self::DeadlineExpired => "deadline_expired",
            Self::RecipientGone => "recipient_gone",
            Self::IdentityExhausted => "identity_exhausted",
        }
    }
}

/// One admission decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapacityAdmission {
    Admit,
    /// Admission is refused for now and the caller should retain the record.
    Defer {
        cause: CapacitySaturationCause,
    },
    /// Admission is refused and the resource's disposition applies.
    Saturated {
        cause: CapacitySaturationCause,
    },
}

/// Bound and disposition of one resource. Passive: it holds no clock, no sink,
/// and no callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundedCapacity {
    pub resource: CapacityResourceId,
    pub capacity: usize,
    pub disposition: CapacitySaturationDisposition,
}

impl BoundedCapacity {
    pub const fn new(
        resource: CapacityResourceId,
        capacity: usize,
        disposition: CapacitySaturationDisposition,
    ) -> Self {
        Self {
            resource,
            capacity,
            disposition,
        }
    }

    /// Pure admission arithmetic.
    ///
    /// `depth` is current occupancy. `reserved` is how many terminating
    /// boundaries are still owed — outstanding presses, active captures,
    /// in-flight leases. A terminating boundary may use the reserve; ordinary
    /// work may not; a replaceable hint occupies a keyed slot it already owns
    /// and so is always admitted.
    pub const fn admit(
        &self,
        depth: usize,
        reserved: usize,
        class: CapacityClass,
    ) -> CapacityAdmission {
        if matches!(class, CapacityClass::Replaceable) {
            return CapacityAdmission::Admit;
        }
        if depth >= self.capacity {
            return self.refuse(CapacitySaturationCause::DepthExhausted);
        }
        if matches!(class, CapacityClass::TerminatingBoundary) {
            return CapacityAdmission::Admit;
        }
        // Saturating arithmetic keeps a miscounted reserve from wrapping into a
        // permissive answer.
        if depth.saturating_add(reserved) >= self.capacity {
            return self.refuse(CapacitySaturationCause::ReserveExhausted);
        }
        CapacityAdmission::Admit
    }

    const fn refuse(&self, cause: CapacitySaturationCause) -> CapacityAdmission {
        match self.disposition {
            CapacitySaturationDisposition::BoundedDeferral { .. } => {
                CapacityAdmission::Defer { cause }
            }
            _ => CapacityAdmission::Saturated { cause },
        }
    }
}

/// One recorded saturation fact.
///
/// Passive and identity-free: counts, durations, and the resource's own name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapacitySaturationReport {
    pub resource: CapacityResourceId,
    pub cause: CapacitySaturationCause,
    pub disposition: CapacitySaturationDisposition,
    pub depth: usize,
    pub capacity: usize,
    /// Records lost to this saturation. Zero when the disposition retained or
    /// deferred them; a truncation that reports zero would be a silent drop.
    pub discarded: usize,
    pub waited_msec: u32,
}
