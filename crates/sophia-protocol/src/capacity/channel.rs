//! Driving one bounded admission under its resource's disposition.
//!
//! This module holds no clock, no thread, and no channel type. The caller
//! supplies both the attempt and the waiting strategy, which keeps the policy
//! testable without sleeping and lets one driver serve channels, ring buffers,
//! and bounded collections alike.

use super::policy::{
    BoundedCapacity, CapacityEscalation, CapacitySaturationCause, CapacitySaturationDisposition,
    CapacitySaturationReport,
};

/// One attempt to place a record into a bounded resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapacityAttempt {
    Accepted,
    /// The resource is full right now.
    Full {
        depth: usize,
    },
    /// The recipient is gone; retrying cannot succeed.
    RecipientGone,
}

/// Caller-supplied waiting strategy.
///
/// Keeping the clock outside makes a bounded deferral deterministic in tests: a
/// fake wait can advance time without sleeping.
pub trait CapacityWait {
    /// Milliseconds elapsed since this wait began.
    fn elapsed_msec(&self) -> u32;
    /// Pause for approximately one retry interval.
    fn pause(&mut self, interval_msec: u32);
    /// Whether the caller has been asked to stop waiting, so a deferral cannot
    /// outlive a shutdown.
    fn cancelled(&self) -> bool {
        false
    }
}

/// Outcome of driving one admission to completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapacityDrive {
    Admitted { waited_msec: u32 },
    Saturated(CapacitySaturationReport),
}

/// Drives one admission under the resource's disposition.
///
/// Only `BoundedDeferral` retries. Every other disposition refuses immediately,
/// because retrying a terminal, endpoint-closing, reject-and-consume, or
/// degrading resource would convert a decision the owner already made into an
/// unbounded wait.
pub fn drive_capacity<W, A>(
    capacity: &BoundedCapacity,
    wait: &mut W,
    mut attempt: A,
) -> CapacityDrive
where
    W: CapacityWait,
    A: FnMut() -> CapacityAttempt,
{
    let (deadline_msec, retry_interval_msec) = match capacity.disposition {
        CapacitySaturationDisposition::BoundedDeferral {
            deadline_msec,
            retry_interval_msec,
            ..
        } => (deadline_msec, retry_interval_msec),
        _ => (0, 0),
    };
    loop {
        match attempt() {
            CapacityAttempt::Accepted => {
                return CapacityDrive::Admitted {
                    waited_msec: wait.elapsed_msec(),
                };
            }
            CapacityAttempt::RecipientGone => {
                return CapacityDrive::Saturated(report(
                    capacity,
                    CapacitySaturationCause::RecipientGone,
                    capacity.capacity,
                    wait.elapsed_msec(),
                ));
            }
            CapacityAttempt::Full { depth } => {
                if deadline_msec == 0 {
                    return CapacityDrive::Saturated(report(
                        capacity,
                        CapacitySaturationCause::DepthExhausted,
                        depth,
                        wait.elapsed_msec(),
                    ));
                }
                if wait.cancelled() {
                    return CapacityDrive::Saturated(report(
                        capacity,
                        CapacitySaturationCause::RecipientGone,
                        depth,
                        wait.elapsed_msec(),
                    ));
                }
                if wait.elapsed_msec() >= deadline_msec {
                    return CapacityDrive::Saturated(report(
                        capacity,
                        CapacitySaturationCause::DeadlineExpired,
                        depth,
                        wait.elapsed_msec(),
                    ));
                }
                wait.pause(retry_interval_msec);
            }
        }
    }
}

/// Where a saturated bounded deferral goes next.
pub const fn escalation_of(
    disposition: CapacitySaturationDisposition,
) -> Option<CapacityEscalation> {
    match disposition {
        CapacitySaturationDisposition::BoundedDeferral { escalation, .. } => Some(escalation),
        _ => None,
    }
}

const fn report(
    capacity: &BoundedCapacity,
    cause: CapacitySaturationCause,
    depth: usize,
    waited_msec: u32,
) -> CapacitySaturationReport {
    CapacitySaturationReport {
        resource: capacity.resource,
        cause,
        disposition: capacity.disposition,
        depth,
        capacity: capacity.capacity,
        discarded: 0,
        waited_msec,
    }
}
