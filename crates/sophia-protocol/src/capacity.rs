//! Bounded-resource capacity, disposition, and saturation reporting.
//!
//! `policy` holds the passive vocabulary and pure admission arithmetic.
//! `coalescer` bounds repeated reporting and accounts for what saturation
//! cost. `batch` offers a whole batch to one sink so a partial admission is
//! never reported as a whole one. `channel` drives one admission under
//! a resource's disposition without owning a clock or a thread.

mod batch;
mod channel;
mod coalescer;
mod policy;

pub use batch::{CapacityBatchAttempt, CapacityBatchOutcome, drive_capacity_batch};
pub use channel::{CapacityAttempt, CapacityDrive, CapacityWait, drive_capacity, escalation_of};
pub use coalescer::{
    CAPACITY_REPORT_LEDGER_MAX_ENTRIES, CapacityAcquisitionLedger, CapacityReportLedger,
    CapacityStallLedger,
};
pub use policy::{
    BoundedCapacity, CapacityAdmission, CapacityClass, CapacityEscalation, CapacityResourceId,
    CapacitySaturationCause, CapacitySaturationDisposition, CapacitySaturationReport,
};
