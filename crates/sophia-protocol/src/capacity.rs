//! Bounded-resource capacity, disposition, and saturation reporting.
//!
//! `policy` holds the passive vocabulary and pure admission arithmetic.
//! `coalescer` bounds repeated reporting and accounts for what saturation
//! cost. `channel` drives one admission under
//! a resource's disposition without owning a clock or a thread.

mod channel;
mod coalescer;
mod policy;

pub use channel::{CapacityAttempt, CapacityDrive, CapacityWait, drive_capacity, escalation_of};
pub use coalescer::{
    CAPACITY_REPORT_LEDGER_MAX_ENTRIES, CapacityAcquisitionLedger, CapacityReportLedger,
    CapacityStallLedger,
};
pub use policy::{
    BoundedCapacity, CapacityAdmission, CapacityClass, CapacityEscalation, CapacityResourceId,
    CapacitySaturationCause, CapacitySaturationDisposition, CapacitySaturationReport,
};
