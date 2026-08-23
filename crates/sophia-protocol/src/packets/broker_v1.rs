use crate::{
    AttentionState, MetadataDisclosure, MetadataDisclosureRule, NamespaceProfile,
    ReducedMetadataCandidate, SanitizedChromeMetadata, SurfaceId,
};

pub const SOPHIA_BROKER_INTERFACE_REVISION: u16 = 2;
pub const SOPHIA_BROKER_MAX_SURFACES: u32 = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrokerToplevelActionGrant {
    pub token: u64,
    pub revocation_epoch: u64,
    pub target_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrokerV1ClientHello {
    pub minimum_revision: u16,
    pub maximum_revision: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrokerV1ServerWelcome {
    pub selected_revision: u16,
    pub connection_epoch: u64,
    pub max_surfaces: u32,
    pub max_label_bytes: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrokerV1Request {
    SurfaceAdmitted {
        connection_epoch: u64,
        surface: SurfaceId,
        profile: NamespaceProfile,
    },
    CandidateReduced {
        connection_epoch: u64,
        candidate: ReducedMetadataCandidate,
    },
    AttentionChanged {
        connection_epoch: u64,
        surface: SurfaceId,
        attention: AttentionState,
    },
    SurfaceRemoved {
        connection_epoch: u64,
        surface: SurfaceId,
    },
    SetDisclosure {
        connection_epoch: u64,
        surface: SurfaceId,
        disclosure: MetadataDisclosure,
    },
}

impl BrokerV1Request {
    pub const fn connection_epoch(&self) -> u64 {
        match self {
            Self::SurfaceAdmitted {
                connection_epoch, ..
            }
            | Self::CandidateReduced {
                connection_epoch, ..
            }
            | Self::AttentionChanged {
                connection_epoch, ..
            }
            | Self::SurfaceRemoved {
                connection_epoch, ..
            }
            | Self::SetDisclosure {
                connection_epoch, ..
            } => *connection_epoch,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerV1Rejection {
    UnknownSurface,
    StaleGeneration,
    CapacityExhausted,
    DisclosureExceeded,
    InvalidConnectionEpoch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrokerV1Response {
    PublishRule {
        connection_epoch: u64,
        rule: MetadataDisclosureRule,
    },
    EmitDescriptor {
        connection_epoch: u64,
        descriptor: SanitizedChromeMetadata,
        action: BrokerToplevelActionGrant,
    },
    RetireSurface {
        connection_epoch: u64,
        surface: SurfaceId,
    },
    Rejected {
        connection_epoch: u64,
        rejection: BrokerV1Rejection,
    },
    NoChange {
        connection_epoch: u64,
    },
}

impl BrokerV1Response {
    pub const fn connection_epoch(&self) -> u64 {
        match self {
            Self::PublishRule {
                connection_epoch, ..
            }
            | Self::EmitDescriptor {
                connection_epoch, ..
            }
            | Self::RetireSurface {
                connection_epoch, ..
            }
            | Self::Rejected {
                connection_epoch, ..
            }
            | Self::NoChange {
                connection_epoch, ..
            } => *connection_epoch,
        }
    }
}
