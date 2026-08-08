use std::collections::BTreeMap;

use sophia_protocol::{
    ApplicationRouteLeaseId, ApplicationRouteLeaseIdentity, ClientAdmissionId, DeviceId,
    NamespaceId, NamespaceProfile, OutputId, SeatId, SurfaceId,
};

pub const APPLICATION_ROUTE_RELEASE_TIMEOUT_MSEC: u64 = 500;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplicationRouteScope {
    pub profile: NamespaceProfile,
    pub authority: NamespaceId,
}

impl ApplicationRouteScope {
    pub fn covers(self, candidate: Self) -> bool {
        match self.profile {
            NamespaceProfile::ClassicShared => {
                matches!(candidate.profile, NamespaceProfile::ClassicShared)
            }
            NamespaceProfile::Confined => {
                matches!(candidate.profile, NamespaceProfile::Confined)
                    && self.authority == candidate.authority
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationRouteLeasePhase {
    Provisional,
    Active,
    Releasing { deadline_msec: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplicationRouteLease {
    pub identity: ApplicationRouteLeaseIdentity,
    pub phase: ApplicationRouteLeasePhase,
    pub target_surface: SurfaceId,
    pub admission: ClientAdmissionId,
    pub scope: ApplicationRouteScope,
    pub authority_session_epoch: u64,
    pub output: OutputId,
    pub presentation_epoch: u64,
    pub initiating_device: Option<DeviceId>,
    pub initiating_button: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplicationRouteLeaseCandidate {
    pub seat: SeatId,
    pub target_surface: SurfaceId,
    pub admission: ClientAdmissionId,
    pub scope: ApplicationRouteScope,
    pub authority_session_epoch: u64,
    pub output: OutputId,
    pub presentation_epoch: u64,
    pub initiating_device: Option<DeviceId>,
    pub initiating_button: Option<u32>,
}

impl ApplicationRouteLeaseCandidate {
    fn is_valid(self) -> bool {
        self.seat.is_valid()
            && self.target_surface.is_valid()
            && self.admission.is_valid()
            && self.scope.authority.is_valid()
            && self.authority_session_epoch != 0
            && self.output.is_valid()
            && self.presentation_epoch != 0
            && self.initiating_device.is_none_or(DeviceId::is_valid)
            && self.initiating_button.is_none_or(|button| button != 0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationRouteLeaseError {
    InvalidCandidate,
    SeatAlreadyOwned,
    NoLease,
    IdentityMismatch,
    InvalidPhase,
    StaleAuthoritySession,
    StaleControlEpoch,
    StalePresentation,
    OutsideScope,
    WrongDevice,
    IdExhausted,
    SequenceExhausted,
    ControlEpochExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationRouteLeaseTimeout {
    NotReleasing,
    Pending,
    Quarantine(ApplicationRouteLease),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationRouteLeaseState {
    control_epoch: u64,
    next_id: u64,
    frontend_sequence: BTreeMap<SeatId, u64>,
    leases: BTreeMap<SeatId, ApplicationRouteLease>,
}

impl Default for ApplicationRouteLeaseState {
    fn default() -> Self {
        Self {
            control_epoch: 1,
            next_id: 1,
            frontend_sequence: BTreeMap::new(),
            leases: BTreeMap::new(),
        }
    }
}

impl ApplicationRouteLeaseState {
    pub fn control_epoch(&self) -> u64 {
        self.control_epoch
    }

    pub fn lease(&self, seat: SeatId) -> Option<ApplicationRouteLease> {
        self.leases.get(&seat).copied()
    }

    pub fn leases(&self) -> impl Iterator<Item = ApplicationRouteLease> + '_ {
        self.leases.values().copied()
    }

    pub fn begin_provisional(
        &mut self,
        candidate: ApplicationRouteLeaseCandidate,
    ) -> Result<ApplicationRouteLease, ApplicationRouteLeaseError> {
        if !candidate.is_valid() {
            return Err(ApplicationRouteLeaseError::InvalidCandidate);
        }
        if self.leases.contains_key(&candidate.seat) {
            return Err(ApplicationRouteLeaseError::SeatAlreadyOwned);
        }
        let id = ApplicationRouteLeaseId::from_raw(self.next_id);
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(ApplicationRouteLeaseError::IdExhausted)?;
        let sequence = self
            .frontend_sequence
            .get(&candidate.seat)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(ApplicationRouteLeaseError::SequenceExhausted)?;
        self.frontend_sequence.insert(candidate.seat, sequence);
        let lease = ApplicationRouteLease {
            identity: ApplicationRouteLeaseIdentity {
                id,
                seat: candidate.seat,
                frontend_sequence: sequence,
                control_epoch: self.control_epoch,
            },
            phase: ApplicationRouteLeasePhase::Provisional,
            target_surface: candidate.target_surface,
            admission: candidate.admission,
            scope: candidate.scope,
            authority_session_epoch: candidate.authority_session_epoch,
            output: candidate.output,
            presentation_epoch: candidate.presentation_epoch,
            initiating_device: candidate.initiating_device,
            initiating_button: candidate.initiating_button,
        };
        self.leases.insert(candidate.seat, lease);
        Ok(lease)
    }

    pub fn confirm(
        &mut self,
        identity: ApplicationRouteLeaseIdentity,
        target_surface: SurfaceId,
        admission: ClientAdmissionId,
        authority_session_epoch: u64,
    ) -> Result<ApplicationRouteLease, ApplicationRouteLeaseError> {
        let lease = self.exact_mut(identity)?;
        if lease.target_surface != target_surface || lease.admission != admission {
            return Err(ApplicationRouteLeaseError::IdentityMismatch);
        }
        if lease.authority_session_epoch != authority_session_epoch {
            return Err(ApplicationRouteLeaseError::StaleAuthoritySession);
        }
        if lease.phase != ApplicationRouteLeasePhase::Provisional {
            return Err(ApplicationRouteLeaseError::InvalidPhase);
        }
        lease.phase = ApplicationRouteLeasePhase::Active;
        Ok(*lease)
    }

    pub fn reject(
        &mut self,
        identity: ApplicationRouteLeaseIdentity,
    ) -> Result<ApplicationRouteLease, ApplicationRouteLeaseError> {
        self.remove_exact(identity, ApplicationRouteLeasePhase::Provisional)
    }

    pub fn authorize(
        &self,
        seat: SeatId,
        scope: ApplicationRouteScope,
        device: DeviceId,
        output: OutputId,
        presentation_epoch: u64,
        authority_session_epoch: u64,
    ) -> Result<ApplicationRouteLease, ApplicationRouteLeaseError> {
        let lease = self
            .leases
            .get(&seat)
            .copied()
            .ok_or(ApplicationRouteLeaseError::NoLease)?;
        if matches!(lease.phase, ApplicationRouteLeasePhase::Releasing { .. }) {
            return Err(ApplicationRouteLeaseError::InvalidPhase);
        }
        if lease.identity.control_epoch != self.control_epoch {
            return Err(ApplicationRouteLeaseError::StaleControlEpoch);
        }
        if lease.authority_session_epoch != authority_session_epoch {
            return Err(ApplicationRouteLeaseError::StaleAuthoritySession);
        }
        if lease
            .initiating_device
            .is_some_and(|expected| expected != device)
        {
            return Err(ApplicationRouteLeaseError::WrongDevice);
        }
        if lease.output != output || lease.presentation_epoch != presentation_epoch {
            return Err(ApplicationRouteLeaseError::StalePresentation);
        }
        if !lease.scope.covers(scope) {
            return Err(ApplicationRouteLeaseError::OutsideScope);
        }
        Ok(lease)
    }

    pub fn request_release(
        &mut self,
        seat: SeatId,
        now_msec: u64,
    ) -> Result<ApplicationRouteLease, ApplicationRouteLeaseError> {
        let lease = self
            .leases
            .get_mut(&seat)
            .ok_or(ApplicationRouteLeaseError::NoLease)?;
        if matches!(lease.phase, ApplicationRouteLeasePhase::Releasing { .. }) {
            return Err(ApplicationRouteLeaseError::InvalidPhase);
        }
        lease.phase = ApplicationRouteLeasePhase::Releasing {
            deadline_msec: now_msec.saturating_add(APPLICATION_ROUTE_RELEASE_TIMEOUT_MSEC),
        };
        Ok(*lease)
    }

    pub fn acknowledge_release(
        &mut self,
        identity: ApplicationRouteLeaseIdentity,
        admission: ClientAdmissionId,
    ) -> Result<ApplicationRouteLease, ApplicationRouteLeaseError> {
        let lease = self
            .leases
            .get(&identity.seat)
            .copied()
            .ok_or(ApplicationRouteLeaseError::NoLease)?;
        if lease.identity != identity || lease.admission != admission {
            return Err(ApplicationRouteLeaseError::IdentityMismatch);
        }
        if !matches!(lease.phase, ApplicationRouteLeasePhase::Releasing { .. }) {
            return Err(ApplicationRouteLeaseError::InvalidPhase);
        }
        self.leases.remove(&identity.seat);
        Ok(lease)
    }

    pub fn frontend_release(
        &mut self,
        identity: ApplicationRouteLeaseIdentity,
        admission: ClientAdmissionId,
    ) -> Result<ApplicationRouteLease, ApplicationRouteLeaseError> {
        let lease = self
            .leases
            .get(&identity.seat)
            .copied()
            .ok_or(ApplicationRouteLeaseError::NoLease)?;
        if lease.identity != identity || lease.admission != admission {
            return Err(ApplicationRouteLeaseError::IdentityMismatch);
        }
        if identity.control_epoch != self.control_epoch {
            return Err(ApplicationRouteLeaseError::StaleControlEpoch);
        }
        self.leases.remove(&identity.seat);
        Ok(lease)
    }

    pub fn observe_timeout(&mut self, seat: SeatId, now_msec: u64) -> ApplicationRouteLeaseTimeout {
        let Some(lease) = self.leases.get(&seat).copied() else {
            return ApplicationRouteLeaseTimeout::NotReleasing;
        };
        let ApplicationRouteLeasePhase::Releasing { deadline_msec } = lease.phase else {
            return ApplicationRouteLeaseTimeout::NotReleasing;
        };
        if now_msec < deadline_msec {
            return ApplicationRouteLeaseTimeout::Pending;
        }
        self.leases.remove(&seat);
        ApplicationRouteLeaseTimeout::Quarantine(lease)
    }

    pub fn revoke_admission(&mut self, admission: ClientAdmissionId) -> Vec<ApplicationRouteLease> {
        self.retain_collect(|lease| lease.admission != admission)
    }

    pub fn invalidate_output(
        &mut self,
        output: OutputId,
        presentation_epoch: u64,
    ) -> Vec<ApplicationRouteLease> {
        self.retain_collect(|lease| {
            lease.output != output || lease.presentation_epoch == presentation_epoch
        })
    }

    pub fn security_transition(
        &mut self,
    ) -> Result<Vec<ApplicationRouteLease>, ApplicationRouteLeaseError> {
        self.control_epoch = self
            .control_epoch
            .checked_add(1)
            .ok_or(ApplicationRouteLeaseError::ControlEpochExhausted)?;
        Ok(std::mem::take(&mut self.leases).into_values().collect())
    }

    fn exact_mut(
        &mut self,
        identity: ApplicationRouteLeaseIdentity,
    ) -> Result<&mut ApplicationRouteLease, ApplicationRouteLeaseError> {
        if identity.control_epoch != self.control_epoch {
            return Err(ApplicationRouteLeaseError::StaleControlEpoch);
        }
        let lease = self
            .leases
            .get_mut(&identity.seat)
            .ok_or(ApplicationRouteLeaseError::NoLease)?;
        if lease.identity != identity {
            return Err(ApplicationRouteLeaseError::IdentityMismatch);
        }
        Ok(lease)
    }

    fn remove_exact(
        &mut self,
        identity: ApplicationRouteLeaseIdentity,
        phase: ApplicationRouteLeasePhase,
    ) -> Result<ApplicationRouteLease, ApplicationRouteLeaseError> {
        let lease = *self.exact_mut(identity)?;
        if lease.phase != phase {
            return Err(ApplicationRouteLeaseError::InvalidPhase);
        }
        self.leases.remove(&identity.seat);
        Ok(lease)
    }

    fn retain_collect(
        &mut self,
        mut keep: impl FnMut(ApplicationRouteLease) -> bool,
    ) -> Vec<ApplicationRouteLease> {
        let mut removed = Vec::new();
        self.leases.retain(|_, lease| {
            let retain = keep(*lease);
            if !retain {
                removed.push(*lease);
            }
            retain
        });
        removed
    }
}
