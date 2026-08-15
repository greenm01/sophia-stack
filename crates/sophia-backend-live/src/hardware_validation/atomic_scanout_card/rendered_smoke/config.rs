use super::super::RealAtomicScanoutPageFlipWaitPolicy;
use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealAtomicScanoutSmokeConfig {
    pub slot: LibdrmNativeOutputSlot,
    pub output: OutputId,
    pub head: sophia_engine::RenderHeadId,
    pub authority: LibdrmBackendFdAuthority,
    pub wait_policy: RealAtomicScanoutPageFlipWaitPolicy,
}

impl RealAtomicScanoutSmokeConfig {
    pub fn from_raw(
        slot: u16,
        output: u64,
        authority_generation: u64,
        wait_policy: RealAtomicScanoutPageFlipWaitPolicy,
    ) -> Option<Self> {
        Some(Self {
            slot: LibdrmNativeOutputSlot::new(slot)?,
            output: OutputId::from_raw(output),
            // A single-head smoke session is its own backend: the one head it
            // drives shares the output's raw identity.
            head: sophia_engine::RenderHeadId::from_raw(output),
            authority: LibdrmBackendFdAuthority::new(authority_generation)?,
            wait_policy,
        })
    }

    pub fn default_primary_output() -> Option<Self> {
        Self::from_raw(
            1,
            1,
            1,
            RealAtomicScanoutPageFlipWaitPolicy::hardware_smoke(),
        )
    }
}
