//! Session accounting for terminal input delivery outcomes.

use sophia_x_authority::{
    XAuthorityClientInputDelivery, XAuthorityInputDeliveryId, XAuthorityInputDeliveryOutcome,
};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

pub struct InputDeliveryState {
    pub fail_on_client_error: bool,
    pub events_failed: usize,
    pub next: u64,
    pub pending: BTreeSet<XAuthorityInputDeliveryId>,
    pub events_expected: usize,
    pub events_flushed: usize,
    pub wait_started_at: Option<Instant>,
    pub source: Option<&'static str>,
    pub flush_latency: Option<Duration>,
}

impl Default for InputDeliveryState {
    fn default() -> Self {
        Self {
            fail_on_client_error: true,
            events_failed: 0,
            next: 1,
            pending: BTreeSet::new(),
            events_expected: 0,
            events_flushed: 0,
            wait_started_at: None,
            source: None,
            flush_latency: None,
        }
    }
}

/// Settle each receipt once. Client failures release their pending barrier but
/// never count as successful flushes. Desktop sessions contain these failures;
/// proof sessions retain strict delivery requirements.
pub fn settle_input_delivery(
    state: &mut InputDeliveryState,
    release_barrier: &mut BTreeSet<XAuthorityInputDeliveryId>,
    delivery: XAuthorityClientInputDelivery,
) -> Result<Option<XAuthorityInputDeliveryOutcome>, XAuthorityClientInputDelivery> {
    if !state.pending.remove(&delivery.delivery) {
        return Ok(None);
    }
    release_barrier.remove(&delivery.delivery);
    match delivery.outcome {
        XAuthorityInputDeliveryOutcome::Flushed => {
            state.events_flushed = state.events_flushed.saturating_add(1);
        }
        XAuthorityInputDeliveryOutcome::TargetGone
        | XAuthorityInputDeliveryOutcome::EpochRevoked => {
            state.events_expected = state.events_expected.saturating_sub(1);
        }
        XAuthorityInputDeliveryOutcome::RouteRejected
        | XAuthorityInputDeliveryOutcome::WriteFailed => {
            state.events_failed = state.events_failed.saturating_add(1);
            if state.fail_on_client_error {
                return Err(delivery);
            }
            state.events_expected = state.events_expected.saturating_sub(1);
        }
    }
    Ok(Some(delivery.outcome))
}
