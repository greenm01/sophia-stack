use crate::prelude::*;
use sophia_protocol::SurfaceConstraints;

/// Protocol-neutral configure request produced while recovering an abandoned
/// layout epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayoutRecoveryConfigure {
    pub transaction: TransactionId,
    pub surface: SurfaceId,
    pub size: Size,
}

/// Passive admission state for a policy-managed surface.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SurfaceAdmissionState {
    #[default]
    Unmanaged,
    PendingLayout,
    Managed,
}

/// Declared and temporary constraints are stored separately so recovery never
/// mutates application truth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceConstraintState {
    pub declared: SurfaceConstraints,
    pub recovery_extent: Option<Size>,
}

impl SurfaceConstraintState {
    pub fn effective(self) -> SurfaceConstraints {
        self.recovery_extent
            .map_or(self.declared, |size| SurfaceConstraints {
                min_size: Some(size),
                max_size: Some(size),
            })
    }

    pub const fn resizable(self) -> bool {
        self.recovery_extent.is_none()
            && !matches!(
                (self.declared.min_size, self.declared.max_size),
                (Some(minimum), Some(maximum)) if minimum.width == maximum.width
                    && minimum.height == maximum.height
            )
    }
}

/// Engine-owned state for joining authority content sizes to blind-WM layout
/// epochs. The record contains no X11 identifiers or application metadata.
#[derive(Debug)]
pub struct LayoutEpochCoordinator {
    committed_sizes: BTreeMap<SurfaceId, Size>,
    rollback_sizes: BTreeMap<SurfaceId, Size>,
    rollback_transactions: BTreeMap<SurfaceId, TransactionId>,
    rejected_sizes: BTreeMap<SurfaceId, Size>,
    constraints: BTreeMap<SurfaceId, SurfaceConstraintState>,
    admission: BTreeMap<SurfaceId, SurfaceAdmissionState>,
    next_transaction: u64,
}

impl Default for LayoutEpochCoordinator {
    fn default() -> Self {
        Self {
            committed_sizes: BTreeMap::new(),
            rollback_sizes: BTreeMap::new(),
            rollback_transactions: BTreeMap::new(),
            rejected_sizes: BTreeMap::new(),
            constraints: BTreeMap::new(),
            admission: BTreeMap::new(),
            next_transaction: 1 << 63,
        }
    }
}

impl LayoutEpochCoordinator {
    pub fn committed_size(&self, surface: SurfaceId) -> Option<Size> {
        self.committed_sizes.get(&surface).copied()
    }

    pub fn record_committed(&mut self, surface: SurfaceId, size: Size) {
        self.committed_sizes.insert(surface, size);
        if self.rejected_sizes.get(&surface) == Some(&size) {
            self.rejected_sizes.remove(&surface);
        }
    }

    pub fn request_allowed(&self, surface: SurfaceId, size: Size) -> bool {
        self.rejected_sizes.get(&surface) != Some(&size)
    }

    pub fn accept_observation(&mut self, surface: SurfaceId, size: Size) -> bool {
        let Some(expected) = self.rollback_sizes.get(&surface).copied() else {
            return true;
        };
        if size != expected {
            return false;
        }
        self.rollback_sizes.remove(&surface);
        self.rollback_transactions.remove(&surface);
        self.rejected_sizes.remove(&surface);
        true
    }

    pub fn begin_recovery(
        &mut self,
        requests: impl IntoIterator<Item = (SurfaceId, Size)>,
        fixed_surfaces: impl IntoIterator<Item = SurfaceId>,
    ) -> Result<Vec<LayoutRecoveryConfigure>, &'static str> {
        let fixed = fixed_surfaces
            .into_iter()
            .map(|surface| {
                self.committed_size(surface)
                    .map(|extent| (surface, extent))
                    .ok_or("layout recovery surface has no safe content extent")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sizes = requests
            .into_iter()
            .map(|(surface, rejected)| {
                self.committed_size(surface)
                    .map(|size| (surface, rejected, size))
                    .ok_or("layout recovery surface has no committed authority size")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let transaction = TransactionId::from_raw(self.next_transaction);
        self.next_transaction = self
            .next_transaction
            .checked_add(1)
            .ok_or("layout recovery transaction ID exhausted")?;
        for (surface, extent) in fixed {
            self.set_recovery_extent(surface, extent);
            self.admission
                .insert(surface, SurfaceAdmissionState::PendingLayout);
        }
        Ok(sizes
            .into_iter()
            .map(|(surface, rejected, size)| {
                self.rejected_sizes.insert(surface, rejected);
                self.rollback_sizes.insert(surface, size);
                self.rollback_transactions.insert(surface, transaction);
                LayoutRecoveryConfigure {
                    transaction,
                    surface,
                    size,
                }
            })
            .collect())
    }

    pub fn begin_rollback(
        &mut self,
        requests: impl IntoIterator<Item = (SurfaceId, Size)>,
    ) -> Result<Vec<LayoutRecoveryConfigure>, &'static str> {
        self.begin_recovery(requests, [])
    }

    pub fn set_declared_constraints(&mut self, surface: SurfaceId, declared: SurfaceConstraints) {
        self.constraints
            .entry(surface)
            .and_modify(|state| state.declared = declared)
            .or_insert(SurfaceConstraintState {
                declared,
                recovery_extent: None,
            });
    }

    /// A successful authority configure acknowledgement completes the
    /// compensating-control fence when safe pixels at that extent are already
    /// retained. A redraw is not required merely to unblock the blind-WM
    /// recovery replan.
    pub fn acknowledge_recovery_configure(
        &mut self,
        transaction: TransactionId,
        surface: SurfaceId,
    ) -> bool {
        if self.rollback_transactions.get(&surface) != Some(&transaction) {
            return false;
        }
        self.rollback_transactions.remove(&surface);
        self.rollback_sizes.remove(&surface);
        self.rejected_sizes.remove(&surface);
        true
    }

    pub fn set_recovery_extent(&mut self, surface: SurfaceId, extent: Size) {
        self.constraints
            .entry(surface)
            .and_modify(|state| state.recovery_extent = Some(extent))
            .or_insert(SurfaceConstraintState {
                declared: SurfaceConstraints {
                    min_size: None,
                    max_size: None,
                },
                recovery_extent: Some(extent),
            });
    }

    pub fn effective_constraints(&self, surface: SurfaceId) -> SurfaceConstraints {
        self.constraints.get(&surface).map_or(
            SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            |state| state.effective(),
        )
    }

    pub fn surface_resizable(&self, surface: SurfaceId) -> bool {
        self.constraints
            .get(&surface)
            .is_none_or(|state| state.resizable())
    }

    pub fn recovery_extent(&self, surface: SurfaceId) -> Option<Size> {
        self.constraints
            .get(&surface)
            .and_then(|state| state.recovery_extent)
    }

    pub fn clear_recovery_extent(&mut self, surface: SurfaceId) -> bool {
        self.constraints.get_mut(&surface).is_some_and(|state| {
            let changed = state.recovery_extent.is_some();
            state.recovery_extent = None;
            changed
        })
    }

    pub fn set_admission(&mut self, surface: SurfaceId, state: SurfaceAdmissionState) {
        self.admission.insert(surface, state);
    }

    pub fn admission(&self, surface: SurfaceId) -> SurfaceAdmissionState {
        self.admission.get(&surface).copied().unwrap_or_default()
    }

    pub fn remove(&mut self, surface: SurfaceId) {
        self.committed_sizes.remove(&surface);
        self.rollback_sizes.remove(&surface);
        self.rollback_transactions.remove(&surface);
        self.rejected_sizes.remove(&surface);
        self.constraints.remove(&surface);
        self.admission.remove(&surface);
    }

    pub fn rollback_pending(&self, surface: SurfaceId) -> bool {
        self.rollback_sizes.contains_key(&surface)
    }

    pub fn rollback_surfaces(&self) -> impl Iterator<Item = SurfaceId> + '_ {
        self.rollback_sizes.keys().copied()
    }
}
