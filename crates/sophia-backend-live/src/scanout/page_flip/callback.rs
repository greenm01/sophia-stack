use crate::prelude::*;
use std::collections::BTreeMap;

/// One CRTC completed a flip.
///
/// The connector is carried beside the output because a mirror group is several
/// connectors behind one logical output: two flips arrive naming the same output,
/// and only the connector says which head each one is. Dropping it here is what
/// made a sibling's flip look like a stale repeat of the first.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivePageFlipCallback {
    pub output: OutputId,
    pub connector_id: u32,
    pub frame_serial: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivePageFlipCallbackReport {
    pub decision: LivePageFlipCallbackDecision,
    pub event: LivePageFlipEvent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LivePageFlipCallbackDecision {
    Accepted,
    RejectedUnexpectedOutput,
    RejectedStaleFrameSerial,
}

/// Admits page flips for one logical output, one head at a time.
///
/// The monotonic frame-serial guard is per connector, not per output. A mirror
/// group's heads flip independently and carry independent kernel sequences, so a
/// single serial shared across the group would admit whichever head reported
/// first and reject its siblings as stale repeats -- the group would then look
/// presented after one of its screens had updated.
///
/// A head is admitted the first time it reports. There is nothing to register in
/// advance: a head with no prior serial has nothing to be stale against, which is
/// exactly the right answer for its first flip.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LivePageFlipCallbackIntake {
    expected_output: OutputId,
    heads: BTreeMap<u32, u64>,
}

impl LivePageFlipCallbackIntake {
    pub fn new(expected_output: OutputId) -> Self {
        Self {
            expected_output,
            heads: BTreeMap::new(),
        }
    }

    /// The newest frame serial admitted for any head of this output.
    ///
    /// Callers use this as the baseline a later flip must beat before a
    /// submission retires. Taking the newest rather than a per-head value is the
    /// conservative choice for a group: no head's older flip can retire work
    /// submitted after a sibling had already flipped.
    pub fn last_frame_serial(&self) -> Option<u64> {
        self.heads.values().copied().max()
    }

    /// The newest frame serial admitted for one head.
    pub fn head_frame_serial(&self, connector_id: u32) -> Option<u64> {
        self.heads.get(&connector_id).copied()
    }

    /// How many heads of this output have reported a flip.
    pub fn observed_heads(&self) -> usize {
        self.heads.len()
    }

    pub fn observe(&mut self, callback: LivePageFlipCallback) -> LivePageFlipCallbackReport {
        if callback.output != self.expected_output {
            return LivePageFlipCallbackReport {
                decision: LivePageFlipCallbackDecision::RejectedUnexpectedOutput,
                event: LivePageFlipEvent {
                    status: LivePageFlipEventStatus::WaitingForOutput,
                    frame_serial: None,
                },
            };
        }

        if self
            .heads
            .get(&callback.connector_id)
            .is_some_and(|last_frame_serial| callback.frame_serial <= *last_frame_serial)
        {
            return LivePageFlipCallbackReport {
                decision: LivePageFlipCallbackDecision::RejectedStaleFrameSerial,
                event: LivePageFlipEvent {
                    status: LivePageFlipEventStatus::Rejected,
                    frame_serial: Some(callback.frame_serial),
                },
            };
        }

        self.heads
            .insert(callback.connector_id, callback.frame_serial);
        LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::Accepted,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(callback.frame_serial),
            },
        }
    }
}
