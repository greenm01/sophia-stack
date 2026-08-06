use crate::prelude::*;
use std::collections::VecDeque;

/// Maximum authority groups retained behind asynchronous surface content.
pub const SURFACE_CONTENT_STREAM_CAPACITY: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SurfaceContentAdmission<T> {
    Ready(T),
    Deferred { superseded: Option<T> },
}

#[derive(Debug)]
struct DeferredSurfaceContent<T> {
    item: T,
    touched_surfaces: BTreeSet<SurfaceId>,
    blockers: Vec<SurfaceTransactionKey>,
}

/// Bounded ordering for logical-window content around asynchronous candidates.
///
/// The payload remains opaque so protocol frontends retain ownership of their
/// resources. Engine owns only the surface order and exact candidate identity.
#[derive(Debug)]
pub struct SurfaceContentStream<T> {
    active: BTreeMap<SurfaceId, SurfaceTransactionKey>,
    deferred: VecDeque<DeferredSurfaceContent<T>>,
    capacity: usize,
    supersessions: usize,
    max_deferred: usize,
    max_latest_deferred_per_surface: usize,
}

impl<T> Default for SurfaceContentStream<T> {
    fn default() -> Self {
        Self::with_capacity(SURFACE_CONTENT_STREAM_CAPACITY)
    }
}

impl<T> SurfaceContentStream<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            active: BTreeMap::new(),
            deferred: VecDeque::new(),
            capacity,
            supersessions: 0,
            max_deferred: 0,
            max_latest_deferred_per_surface: 0,
        }
    }

    pub fn begin(&mut self, owner: SurfaceTransactionKey) -> Result<(), &'static str> {
        if !owner.transaction.is_valid() || !owner.surface.is_valid() {
            return Err("surface content stream has an invalid owner");
        }
        if self.active.contains_key(&owner.surface) {
            return Err("surface content stream already owns this surface");
        }
        self.active.insert(owner.surface, owner);
        Ok(())
    }

    pub fn admit(
        &mut self,
        item: T,
        touched_surfaces: impl IntoIterator<Item = SurfaceId>,
        removed_surfaces: impl IntoIterator<Item = SurfaceId>,
    ) -> Result<SurfaceContentAdmission<T>, &'static str> {
        self.admit_inner(item, touched_surfaces, removed_surfaces, false, |_| false)
    }

    /// Admits replaceable content while retaining only its newest deferred
    /// candidate for one surface.
    ///
    /// The caller owns the replacement policy because the stream deliberately
    /// does not understand protocol payloads. Replacement never crosses later
    /// work for the same surface, so ordinary mutations remain ordered.
    pub fn admit_latest_deferred(
        &mut self,
        item: T,
        touched_surfaces: impl IntoIterator<Item = SurfaceId>,
        removed_surfaces: impl IntoIterator<Item = SurfaceId>,
        replaceable: impl FnOnce(&T) -> bool,
    ) -> Result<SurfaceContentAdmission<T>, &'static str> {
        self.admit_inner(item, touched_surfaces, removed_surfaces, true, replaceable)
    }

    fn admit_inner(
        &mut self,
        item: T,
        touched_surfaces: impl IntoIterator<Item = SurfaceId>,
        removed_surfaces: impl IntoIterator<Item = SurfaceId>,
        latest_deferred: bool,
        replaceable: impl FnOnce(&T) -> bool,
    ) -> Result<SurfaceContentAdmission<T>, &'static str> {
        let removed_surfaces = removed_surfaces.into_iter().collect::<BTreeSet<_>>();
        let touched_surfaces = touched_surfaces
            .into_iter()
            .filter(|surface| !removed_surfaces.contains(surface))
            .collect::<BTreeSet<_>>();
        if touched_surfaces.iter().any(|surface| !surface.is_valid())
            || removed_surfaces.iter().any(|surface| !surface.is_valid())
        {
            return Err("surface content stream item has an invalid surface");
        }
        let blockers = touched_surfaces
            .iter()
            .filter_map(|surface| self.active.get(surface).copied())
            .collect::<Vec<_>>();
        let follows_deferred_surface = self.deferred.iter().any(|deferred| {
            deferred
                .touched_surfaces
                .iter()
                .any(|surface| touched_surfaces.contains(surface))
        });
        if blockers.is_empty() && !follows_deferred_surface {
            return Ok(SurfaceContentAdmission::Ready(item));
        }
        // A replaceable candidate is safe only when it is the most recent
        // deferred work touching this one surface. This permits coalescing
        // frames across unrelated surfaces without crossing a same-surface
        // mutation or a multi-surface transaction.
        if removed_surfaces.is_empty()
            && touched_surfaces.len() == 1
            && let Some(index) = self
                .deferred
                .iter()
                .rposition(|deferred| !deferred.touched_surfaces.is_disjoint(&touched_surfaces))
            && self.deferred[index].touched_surfaces == touched_surfaces
            && replaceable(&self.deferred[index].item)
        {
            let superseded = std::mem::replace(&mut self.deferred[index].item, item);
            self.supersessions = self.supersessions.saturating_add(1);
            self.max_latest_deferred_per_surface = 1;
            return Ok(SurfaceContentAdmission::Deferred {
                superseded: Some(superseded),
            });
        }
        if self.deferred.len() >= self.capacity {
            return Err("surface content stream capacity exceeded");
        }
        self.deferred.push_back(DeferredSurfaceContent {
            item,
            touched_surfaces,
            blockers,
        });
        self.max_deferred = self.max_deferred.max(self.deferred.len());
        // `admit_latest_deferred` reaches this branch only when its surface
        // has no replaceable candidate in the current ordering segment.
        if latest_deferred {
            self.max_latest_deferred_per_surface = 1;
        }
        Ok(SurfaceContentAdmission::Deferred { superseded: None })
    }

    pub fn finish(&mut self, owner: SurfaceTransactionKey) -> Result<Vec<T>, &'static str> {
        if self.active.get(&owner.surface) != Some(&owner) {
            return Err("surface content stream completion does not match its owner");
        }
        self.active.remove(&owner.surface);

        let mut ready = Vec::new();
        let mut retained = VecDeque::with_capacity(self.deferred.len());
        let mut retained_surfaces = BTreeSet::new();
        while let Some(mut deferred) = self.deferred.pop_front() {
            deferred.blockers.retain(|blocker| *blocker != owner);
            let follows_retained_surface = deferred
                .touched_surfaces
                .iter()
                .any(|surface| retained_surfaces.contains(surface));
            if deferred.blockers.is_empty() && !follows_retained_surface {
                ready.push(deferred.item);
            } else {
                retained_surfaces.extend(deferred.touched_surfaces.iter().copied());
                retained.push_back(deferred);
            }
        }
        self.deferred = retained;
        Ok(ready)
    }

    /// Drops all ownership and queued work during runtime shutdown.
    pub fn discard(&mut self) -> usize {
        self.active.clear();
        let discarded = self.deferred.len();
        self.deferred.clear();
        discarded
    }

    /// Transfers deferred payloads without disturbing active ownership.
    pub fn drain_deferred(&mut self) -> Vec<T> {
        self.deferred
            .drain(..)
            .map(|deferred| deferred.item)
            .collect()
    }

    pub fn owner(&self, surface: SurfaceId) -> Option<SurfaceTransactionKey> {
        self.active.get(&surface).copied()
    }

    pub fn owner_for_transaction(
        &self,
        transaction: TransactionId,
    ) -> Option<SurfaceTransactionKey> {
        let mut owners = self
            .active
            .values()
            .filter(|owner| owner.transaction == transaction)
            .copied();
        let owner = owners.next()?;
        owners.next().is_none().then_some(owner)
    }

    pub fn active_len(&self) -> usize {
        self.active.len()
    }

    pub fn deferred_len(&self) -> usize {
        self.deferred.len()
    }

    pub const fn supersessions(&self) -> usize {
        self.supersessions
    }

    pub const fn max_deferred_len(&self) -> usize {
        self.max_deferred
    }

    pub const fn max_latest_deferred_per_surface(&self) -> usize {
        self.max_latest_deferred_per_surface
    }

    pub fn deferred_items(&self) -> impl Iterator<Item = &T> {
        self.deferred.iter().map(|deferred| &deferred.item)
    }
}
