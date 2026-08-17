//! Offering a whole batch to one bounded sink.
//!
//! A producer that hands over records one at a time can decide per record. A
//! producer handed a batch cannot: when the sink saturates partway through, the
//! tail has to go somewhere, and the failure this exists to prevent is letting
//! it go nowhere and saying nothing. Every outcome here names how many records
//! were admitted and how many were not, so the caller cannot report a partial
//! batch as a whole one.
//!
//! Shutdown is separated from saturation deliberately. Records lost while the
//! process is tearing down are not a degradation of a running session, and a
//! site that conflates the two reports loss on every clean exit.

use super::channel::{CapacityAttempt, CapacityDrive, CapacityWait, drive_capacity};
use super::policy::{BoundedCapacity, CapacitySaturationCause, CapacitySaturationReport};

/// One attempt to place a batch record, returning the record when it did not
/// fit so the same record is retried rather than a copy of it.
pub enum CapacityBatchAttempt<T> {
    Accepted,
    Full { record: T, depth: usize },
    RecipientGone,
}

/// Outcome of offering a batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapacityBatchOutcome {
    /// Every record was admitted.
    Drained { admitted: usize },
    /// The sink is gone. The remainder was never offered.
    RecipientGone { admitted: usize, abandoned: usize },
    /// The caller asked to stop waiting. Not a degradation: teardown.
    Cancelled { admitted: usize, abandoned: usize },
    /// Deferral was spent. The tail was abandoned and counted.
    Saturated {
        admitted: usize,
        discarded: usize,
        report: CapacitySaturationReport,
    },
}

impl CapacityBatchOutcome {
    /// Records admitted to the sink.
    pub const fn admitted(&self) -> usize {
        match self {
            Self::Drained { admitted }
            | Self::RecipientGone { admitted, .. }
            | Self::Cancelled { admitted, .. }
            | Self::Saturated { admitted, .. } => *admitted,
        }
    }

    /// Records lost to saturation while the session was running. Teardown and a
    /// departed recipient are not counted here, because neither is a
    /// degradation the session could have avoided.
    pub const fn discarded(&self) -> usize {
        match self {
            Self::Saturated { discarded, .. } => *discarded,
            _ => 0,
        }
    }
}

/// Offers each record in turn, giving each its own deferral budget.
///
/// `new_wait` is called per record rather than per batch: a bounded deferral
/// bounds one admission, and sharing one budget across a batch would let an
/// early record spend the whole allowance.
pub fn drive_capacity_batch<T, I, W, MW, A>(
    capacity: &BoundedCapacity,
    records: I,
    mut new_wait: MW,
    mut attempt: A,
) -> CapacityBatchOutcome
where
    I: IntoIterator<Item = T>,
    I::IntoIter: ExactSizeIterator,
    W: CapacityWait,
    MW: FnMut() -> W,
    A: FnMut(T) -> CapacityBatchAttempt<T>,
{
    let mut records = records.into_iter();
    let mut admitted = 0usize;
    while let Some(record) = records.next() {
        let mut wait = new_wait();
        let mut held = Some(record);
        let mut departed = false;
        let drive = drive_capacity(capacity, &mut wait, || {
            let Some(record) = held.take() else {
                departed = true;
                return CapacityAttempt::RecipientGone;
            };
            match attempt(record) {
                CapacityBatchAttempt::Accepted => CapacityAttempt::Accepted,
                CapacityBatchAttempt::Full { record, depth } => {
                    held = Some(record);
                    CapacityAttempt::Full { depth }
                }
                CapacityBatchAttempt::RecipientGone => {
                    departed = true;
                    CapacityAttempt::RecipientGone
                }
            }
        });
        let report = match drive {
            CapacityDrive::Admitted { .. } => {
                admitted = admitted.saturating_add(1);
                continue;
            }
            CapacityDrive::Saturated(report) => report,
        };
        // The held record plus everything after it.
        let abandoned = records.len().saturating_add(1);
        if departed {
            return CapacityBatchOutcome::RecipientGone {
                admitted,
                abandoned,
            };
        }
        if matches!(report.cause, CapacitySaturationCause::RecipientGone) {
            return CapacityBatchOutcome::Cancelled {
                admitted,
                abandoned,
            };
        }
        return CapacityBatchOutcome::Saturated {
            admitted,
            discarded: abandoned,
            report: CapacitySaturationReport {
                discarded: abandoned,
                ..report
            },
        };
    }
    CapacityBatchOutcome::Drained { admitted }
}
