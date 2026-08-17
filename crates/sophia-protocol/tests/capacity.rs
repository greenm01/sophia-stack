use sophia_protocol::*;

const RESOURCE: CapacityResourceId = CapacityResourceId("test.resource");

fn bounded(capacity: usize, disposition: CapacitySaturationDisposition) -> BoundedCapacity {
    BoundedCapacity::new(RESOURCE, capacity, disposition)
}

fn deferral(deadline_msec: u32) -> CapacitySaturationDisposition {
    CapacitySaturationDisposition::BoundedDeferral {
        deadline_msec,
        retry_interval_msec: 1,
        escalation: CapacityEscalation::EndpointEpochClosed,
    }
}

/// A wait with no clock of its own, so a bounded deferral is provable without
/// sleeping.
#[derive(Debug, Default)]
struct FakeWait {
    elapsed_msec: u32,
    pauses: u32,
    cancelled: bool,
}

impl CapacityWait for FakeWait {
    fn elapsed_msec(&self) -> u32 {
        self.elapsed_msec
    }
    fn pause(&mut self, interval_msec: u32) {
        self.pauses = self.pauses.saturating_add(1);
        self.elapsed_msec = self.elapsed_msec.saturating_add(interval_msec.max(1));
    }
    fn cancelled(&self) -> bool {
        self.cancelled
    }
}

#[test]
fn ordinary_work_may_not_consume_the_boundary_reserve() {
    // The reserve is what keeps a completion, cancellation, or release
    // deliverable under ordinary pressure. A dropped release is a stuck key.
    let capacity = bounded(8, CapacitySaturationDisposition::Terminal);
    assert_eq!(
        capacity.admit(5, 2, CapacityClass::Ordered),
        CapacityAdmission::Admit
    );
    assert_eq!(
        capacity.admit(6, 2, CapacityClass::Ordered),
        CapacityAdmission::Saturated {
            cause: CapacitySaturationCause::ReserveExhausted
        }
    );
    assert_eq!(
        capacity.admit(6, 2, CapacityClass::TerminatingBoundary),
        CapacityAdmission::Admit,
        "a terminating boundary is exactly what the reserve is held for"
    );
}

#[test]
fn a_full_resource_refuses_even_a_terminating_boundary() {
    let capacity = bounded(4, CapacitySaturationDisposition::Terminal);
    assert_eq!(
        capacity.admit(4, 0, CapacityClass::TerminatingBoundary),
        CapacityAdmission::Saturated {
            cause: CapacitySaturationCause::DepthExhausted
        },
        "the reserve creates room in advance; it cannot conjure a slot"
    );
}

#[test]
fn a_replaceable_hint_never_grows_the_resource() {
    let capacity = bounded(4, CapacitySaturationDisposition::Terminal);
    assert_eq!(
        capacity.admit(4, 4, CapacityClass::Replaceable),
        CapacityAdmission::Admit,
        "a replaceable hint occupies a keyed slot it already owns"
    );
}

#[test]
fn a_miscounted_reserve_cannot_wrap_into_permission() {
    let capacity = bounded(8, CapacitySaturationDisposition::Terminal);
    assert_eq!(
        capacity.admit(1, usize::MAX, CapacityClass::Ordered),
        CapacityAdmission::Saturated {
            cause: CapacitySaturationCause::ReserveExhausted
        }
    );
}

#[test]
fn only_a_bounded_deferral_defers() {
    // Retrying a terminal, endpoint-closing, or reject-and-consume resource
    // would turn a decision the owner already made into an unbounded wait.
    for disposition in [
        CapacitySaturationDisposition::Terminal,
        CapacitySaturationDisposition::EndpointEpochClosed,
        CapacitySaturationDisposition::RejectAndConsume,
        CapacitySaturationDisposition::DegradeWithCause,
    ] {
        assert_eq!(
            bounded(2, disposition).admit(2, 0, CapacityClass::Ordered),
            CapacityAdmission::Saturated {
                cause: CapacitySaturationCause::DepthExhausted
            }
        );
    }
    assert_eq!(
        bounded(2, deferral(50)).admit(2, 0, CapacityClass::Ordered),
        CapacityAdmission::Defer {
            cause: CapacitySaturationCause::DepthExhausted
        }
    );
}

#[test]
fn a_bounded_deferral_admits_once_the_resource_drains() {
    let capacity = bounded(4, deferral(50));
    let mut wait = FakeWait::default();
    let mut attempts = 0;
    let outcome = drive_capacity(&capacity, &mut wait, || {
        attempts += 1;
        if attempts < 3 {
            CapacityAttempt::Full { depth: 4 }
        } else {
            CapacityAttempt::Accepted
        }
    });
    assert!(matches!(outcome, CapacityDrive::Admitted { .. }));
    assert_eq!(attempts, 3);
    assert_eq!(wait.pauses, 2);
}

#[test]
fn a_bounded_deferral_stops_at_its_deadline() {
    // This is what distinguishes a bounded deferral from an unbounded retry.
    let capacity = bounded(4, deferral(5));
    let mut wait = FakeWait::default();
    let outcome = drive_capacity(&capacity, &mut wait, || CapacityAttempt::Full { depth: 4 });
    let CapacityDrive::Saturated(report) = outcome else {
        panic!("an unsatisfiable deferral must saturate rather than wait forever");
    };
    assert_eq!(report.cause, CapacitySaturationCause::DeadlineExpired);
    assert_eq!(report.resource, RESOURCE);
    assert!(wait.elapsed_msec >= 5);
    assert_eq!(
        escalation_of(capacity.disposition),
        Some(CapacityEscalation::EndpointEpochClosed)
    );
}

#[test]
fn a_deferral_does_not_outlive_a_cancellation() {
    let capacity = bounded(4, deferral(u32::MAX));
    let mut wait = FakeWait {
        cancelled: true,
        ..FakeWait::default()
    };
    let outcome = drive_capacity(&capacity, &mut wait, || CapacityAttempt::Full { depth: 4 });
    let CapacityDrive::Saturated(report) = outcome else {
        panic!("a cancelled deferral must stop");
    };
    assert_eq!(report.cause, CapacitySaturationCause::RecipientGone);
}

#[test]
fn a_departed_recipient_is_not_retried() {
    let capacity = bounded(4, deferral(u32::MAX));
    let mut wait = FakeWait::default();
    let mut attempts = 0;
    let outcome = drive_capacity(&capacity, &mut wait, || {
        attempts += 1;
        CapacityAttempt::RecipientGone
    });
    assert!(matches!(outcome, CapacityDrive::Saturated(_)));
    assert_eq!(attempts, 1, "retrying a departed recipient cannot succeed");
}

#[test]
fn repeated_saturation_reports_transitions_and_never_hides_its_volume() {
    let mut ledger = CapacityReportLedger::default();
    let emitted = (0..10)
        .filter_map(|_| ledger.observe(RESOURCE, CapacitySaturationCause::DepthExhausted))
        .collect::<Vec<_>>();
    assert_eq!(emitted, vec![1, 2, 4, 8]);
    assert_eq!(
        ledger.occurrences(RESOURCE, CapacitySaturationCause::DepthExhausted),
        10,
        "suppressed occurrences are still counted"
    );

    // A change of cause is news, not another repeat.
    assert_eq!(
        ledger.observe(RESOURCE, CapacitySaturationCause::DeadlineExpired),
        Some(1)
    );
    // And the first recovery reports what it ended.
    assert_eq!(ledger.observe_admitted(RESOURCE), Some(1));
    assert_eq!(ledger.observe_admitted(RESOURCE), None);
    assert!(!ledger.is_saturated(RESOURCE));
}

#[test]
fn a_stall_is_reported_once_rather_than_once_per_retry() {
    let mut ledger = CapacityStallLedger::default();
    assert!(ledger.begin_stall(7_u64));
    assert!(!ledger.begin_stall(7_u64));
    assert!(ledger.is_stalled(&7));
    // Work queued behind the stalled boundary is causally waiting too.
    assert!(ledger.begin_stall(8_u64));
    assert_eq!(ledger.len(), 2);
    assert!(ledger.end_stall(&7));
    assert!(!ledger.end_stall(&7));
    assert!(!ledger.is_empty());
}

/// `validation/tla/TargetInputPacing.tla`, `AcquisitionIsConserved`. Deleting
/// the discard accounting from `EscalateEndpoint` violates it in TLC.
#[test]
fn a_refused_batch_counts_its_tail_rather_than_dropping_it_silently() {
    let resource = bounded(4, CapacitySaturationDisposition::EndpointEpochClosed);
    let mut ledger = CapacityAcquisitionLedger::default();

    // A device batch arrives whole; capacity is examined afterwards.
    ledger.arrived(10);
    let mut depth = 2;
    let mut refused = 0;
    for _ in 0..10 {
        match resource.admit(depth, 0, CapacityClass::Ordered) {
            CapacityAdmission::Admit => {
                depth += 1;
                ledger.admitted(1);
            }
            _ => refused += 1,
        }
    }
    ledger.discarded(refused);

    assert!(ledger.is_conserved());
    assert_eq!(ledger.admitted_total(), 2);
    assert_eq!(ledger.discarded_total(), 8);
    assert_eq!(ledger.held(), 0);
    // The count is what makes the loss visible; a report of zero here would be
    // the silent drop this exists to forbid.
    assert_ne!(ledger.discarded_total(), 0);
}

/// A bounded deferral leaves the arrival upstream, so it is neither admitted
/// nor lost while the wait is in progress.
#[test]
fn a_deferred_arrival_is_held_rather_than_admitted_or_discarded() {
    let resource = bounded(2, deferral(20));
    let mut ledger = CapacityAcquisitionLedger::default();
    ledger.arrived(3);

    assert!(matches!(
        resource.admit(2, 0, CapacityClass::Ordered),
        CapacityAdmission::Defer { .. }
    ));

    assert_eq!(ledger.held(), 3);
    assert_eq!(ledger.discarded_total(), 0);
    assert!(ledger.is_conserved());
}

/// `DeliveryPreservesOrder` and the ordered capacity class. Each ordered record
/// takes its own slot, so two arrivals can never become one.
#[test]
fn ordered_work_takes_its_own_slot_rather_than_sharing_one() {
    let resource = bounded(8, CapacitySaturationDisposition::EndpointEpochClosed);
    let mut depth = 0;
    for expected in 0..8 {
        assert_eq!(depth, expected);
        assert_eq!(
            resource.admit(depth, 0, CapacityClass::Ordered),
            CapacityAdmission::Admit
        );
        depth += 1;
    }
    assert!(matches!(
        resource.admit(depth, 0, CapacityClass::Ordered),
        CapacityAdmission::Saturated { .. }
    ));

    // Only the replaceable class may be admitted without the resource growing.
    assert_eq!(
        resource.admit(depth, 0, CapacityClass::Replaceable),
        CapacityAdmission::Admit
    );
}

/// `validation/tla/TargetInputPacing.tla`, `EndpointCloseIsScoped`. Removing
/// the boundary flush from `EscalateEndpoint` violates it in TLC: a discarded
/// release is a key held down forever.
#[test]
fn a_saturated_endpoint_can_still_release_every_key_it_holds() {
    let held = 3usize;
    let resource = bounded(8, CapacitySaturationDisposition::EndpointEpochClosed);
    let mut depth = resource.capacity - held;

    // Ordinary input is refused at exactly the point the reserve is needed.
    assert!(matches!(
        resource.admit(depth, held, CapacityClass::Ordered),
        CapacityAdmission::Saturated {
            cause: CapacitySaturationCause::ReserveExhausted
        }
    ));

    // Closing the endpoint still emits every release it owes.
    for _ in 0..held {
        assert_eq!(
            resource.admit(depth, held, CapacityClass::TerminatingBoundary),
            CapacityAdmission::Admit
        );
        depth += 1;
    }
    assert_eq!(depth, resource.capacity);
}
