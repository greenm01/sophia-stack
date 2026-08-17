//! Bounded reporting for repeated saturation.
//!
//! A resource that stays saturated reports on every attempt, which buries the
//! rest of the log. Suppression is deliberately transition-sensitive: the first
//! occurrence, each subsequent power of two, any change of cause, and the first
//! recovery always emit, and every emitted line carries the cumulative count.
//! An earlier revision of this pattern keyed only on repeat count and fell
//! silent across the exact transition a physical run needed to explain.

use super::policy::{CapacityResourceId, CapacitySaturationCause};
use std::collections::{BTreeMap, BTreeSet};

/// Distinct (resource, cause) pairs retained before the table resets. The reset
/// re-emits rather than dropping, so a churning resource set stays visible.
pub const CAPACITY_REPORT_LEDGER_MAX_ENTRIES: usize = 256;

/// Tracks repeated saturation so reporting stays bounded without hiding volume.
#[derive(Debug, Default)]
pub struct CapacityReportLedger {
    counts: BTreeMap<(CapacityResourceId, CapacitySaturationCause), u64>,
    last_cause: BTreeMap<CapacityResourceId, CapacitySaturationCause>,
    saturated: BTreeSet<CapacityResourceId>,
}

impl CapacityReportLedger {
    /// Records one saturation and returns the cumulative count when this
    /// occurrence should be reported.
    pub fn observe(
        &mut self,
        resource: CapacityResourceId,
        cause: CapacitySaturationCause,
    ) -> Option<u64> {
        if self.counts.len() >= CAPACITY_REPORT_LEDGER_MAX_ENTRIES
            && !self.counts.contains_key(&(resource, cause))
        {
            self.counts.clear();
            self.last_cause.clear();
        }
        let changed_cause = self.last_cause.insert(resource, cause) != Some(cause);
        self.saturated.insert(resource);
        let count = self.counts.entry((resource, cause)).or_insert(0);
        *count = count.saturating_add(1);
        let count = *count;
        (changed_cause || count.is_power_of_two()).then_some(count)
    }

    /// Records that a resource admitted again, returning the number of
    /// consecutive saturations that recovery ended when it is worth reporting.
    pub fn observe_admitted(&mut self, resource: CapacityResourceId) -> Option<u64> {
        if !self.saturated.remove(&resource) {
            return None;
        }
        let ended = self
            .last_cause
            .remove(&resource)
            .map(|cause| self.occurrences(resource, cause))
            .unwrap_or(0);
        self.counts.retain(|(tracked, _), _| *tracked != resource);
        Some(ended)
    }

    /// Cumulative saturations observed for one resource and cause.
    pub fn occurrences(&self, resource: CapacityResourceId, cause: CapacitySaturationCause) -> u64 {
        self.counts.get(&(resource, cause)).copied().unwrap_or(0)
    }

    /// Whether this resource is currently in a saturated run.
    pub fn is_saturated(&self, resource: CapacityResourceId) -> bool {
        self.saturated.contains(&resource)
    }
}

/// Tracks which keyed items are currently stalled behind a full resource, so a
/// stall reports once rather than once per retry, and so work queued behind a
/// stalled boundary can be reported as causally waiting.
#[derive(Debug)]
pub struct CapacityStallLedger<K: Ord> {
    stalled: BTreeSet<K>,
}

impl<K: Ord> Default for CapacityStallLedger<K> {
    fn default() -> Self {
        Self {
            stalled: BTreeSet::new(),
        }
    }
}

impl<K: Ord> CapacityStallLedger<K> {
    /// Marks one item stalled. Returns true only the first time, so the caller
    /// reports a stall once per stall rather than once per attempt.
    pub fn begin_stall(&mut self, key: K) -> bool {
        self.stalled.insert(key)
    }

    /// Clears one item's stall. Returns true if it had been stalled.
    pub fn end_stall(&mut self, key: &K) -> bool {
        self.stalled.remove(key)
    }

    pub fn is_stalled(&self, key: &K) -> bool {
        self.stalled.contains(key)
    }

    pub fn len(&self) -> usize {
        self.stalled.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stalled.is_empty()
    }
}

/// Conservation at an acquisition boundary.
///
/// A device packet exists before capacity is examined, so a refused arrival is
/// held, admitted, or discarded — never merely gone. Counting the third case is
/// what separates backpressure from silent loss, and it is the executable form
/// of `AcquisitionIsConserved` in `validation/tla/TargetInputPacing.tla`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CapacityAcquisitionLedger {
    produced: u64,
    admitted: u64,
    discarded: u64,
}

impl CapacityAcquisitionLedger {
    /// `count` arrivals exist and have not yet been examined.
    pub fn arrived(&mut self, count: u64) {
        self.produced = self.produced.saturating_add(count);
    }

    pub fn admitted(&mut self, count: u64) {
        self.admitted = self.admitted.saturating_add(count);
    }

    pub fn discarded(&mut self, count: u64) {
        self.discarded = self.discarded.saturating_add(count);
    }

    pub const fn produced_total(&self) -> u64 {
        self.produced
    }

    pub const fn admitted_total(&self) -> u64 {
        self.admitted
    }

    pub const fn discarded_total(&self) -> u64 {
        self.discarded
    }

    /// Arrivals examined but neither admitted nor discarded: still upstream,
    /// which is what a bounded deferral leaves behind.
    pub const fn held(&self) -> u64 {
        self.produced
            .saturating_sub(self.admitted.saturating_add(self.discarded))
    }

    /// Nothing was accounted for twice. A false answer means an arrival was
    /// counted as both admitted and discarded, or discarded without arriving.
    pub const fn is_conserved(&self) -> bool {
        self.admitted.saturating_add(self.discarded) <= self.produced
    }
}
