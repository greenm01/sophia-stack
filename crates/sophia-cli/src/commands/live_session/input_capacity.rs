// Bounds and dispositions for the session's input-side resources.
//
// These live beside the loop rather than inside it so that changing what a
// resource does under pressure is a change to a value, not to control flow at
// the site that happens to notice the pressure first.

/// One line, one schema, for every bounded resource that degraded.
///
/// Writing it once is the point: ten sites each inventing a field name is how a
/// log reader ends up unable to answer "what did this session drop" without
/// knowing which subsystem dropped it. `discarded` is cumulative and zero means
/// nothing was lost, never that nobody counted.
fn print_capacity_saturation(report: &sophia_protocol::CapacitySaturationReport) {
    eprintln!(
        "sophia_live_capacity schema=1 status=saturated resource={} cause={} disposition={} depth={} capacity={} discarded={} waited_msec={}",
        report.resource.as_str(),
        report.cause.as_str(),
        report.disposition.as_str(),
        report.depth,
        report.capacity,
        report.discarded,
        report.waited_msec,
    );
}

/// Timing sidecars for keys deferred behind an outstanding focus handoff.
///
/// A latency sidecar is a diagnostic. Losing one costs a number in a log line;
/// failing the session costs the user their desktop, which is what this site
/// did before. The bound is the focus-handoff capacity because a sidecar only
/// outlives its key while that handoff is outstanding.
const PHYSICAL_KEY_TIMING: sophia_protocol::CapacityResourceId =
    sophia_protocol::CapacityResourceId("cli.live_session.physical_key_timing");

fn physical_key_timing_capacity() -> sophia_protocol::BoundedCapacity {
    sophia_protocol::BoundedCapacity::new(
        PHYSICAL_KEY_TIMING,
        sophia_engine::KEYBOARD_FOCUS_HANDOFF_CAPACITY,
        sophia_protocol::CapacitySaturationDisposition::RejectAndConsume,
    )
}

/// Sidecars dropped this tick, kept apart by cause so the report can say
/// whether the measurement never arrived or the map had no room for it.
#[derive(Clone, Copy, Debug, Default)]
struct PhysicalKeyTimingRejects {
    absent: usize,
    overflow: usize,
}

impl PhysicalKeyTimingRejects {
    const fn is_empty(self) -> bool {
        self.absent == 0 && self.overflow == 0
    }

    /// Reports both causes if both occurred, so a tick that hit the bound *and*
    /// lost a measurement does not disguise one as the other.
    fn report(self, depth: usize) {
        let capacity = physical_key_timing_capacity();
        for (discarded, cause) in [
            (
                self.absent,
                sophia_protocol::CapacitySaturationCause::RecipientGone,
            ),
            (
                self.overflow,
                sophia_protocol::CapacitySaturationCause::DepthExhausted,
            ),
        ] {
            if discarded == 0 {
                continue;
            }
            print_capacity_saturation(&sophia_protocol::CapacitySaturationReport {
                resource: capacity.resource,
                cause,
                disposition: capacity.disposition,
                depth,
                capacity: capacity.capacity,
                discarded,
                waited_msec: 0,
            });
        }
    }
}

/// The shared queue carrying routed input to the X authority.
///
/// `docs/architecture.md` is explicit that bounded-queue exhaustion "fails the
/// recipient epoch closed rather than coalescing an ordered boundary or failing
/// every frontend client". Propagating the send error did the third thing: one
/// full queue ended the session for every client on it.
///
/// The epoch advance is a compare-exchange on an atomic rather than a queued
/// record, so the close is deliverable no matter how full the queue is. That is
/// what makes the epoch itself the terminating boundary, and applying it clears
/// active grabs and frozen input.
const ROUTED_INPUT_INGRESS: sophia_protocol::CapacityResourceId =
    sophia_protocol::CapacityResourceId("cli.live_session.routed_input_ingress");

/// Keys one client is believed to be holding.
///
/// Reaching this bound means releases were already lost upstream, since no
/// hand holds 256 keys. The epoch close both reports that and flushes what is
/// held, which is the only thing that lets the ledger drain again.
const PRESSED_KEY_LEDGER: sophia_protocol::CapacityResourceId =
    sophia_protocol::CapacityResourceId("cli.live_session.pressed_key_ledger");

/// How long a terminating boundary waits for room before it is abandoned.
///
/// Ordered input is refused immediately, because a queue this full is already
/// delivering late and holding the owner loop would make that worse. A release
/// is different: it is rare, it is small, and losing one leaves a client
/// believing a key is still down, so it is worth a short wait that ordinary
/// input does not get. This is the terminating-boundary class expressed against
/// a channel that has no reserve of its own.
const ROUTED_INPUT_BOUNDARY_DEFERRAL_MSEC: u32 = 20;
const ROUTED_INPUT_BOUNDARY_RETRY_MSEC: u32 = 1;

fn routed_input_ingress_capacity(
    capacity: usize,
    class: sophia_protocol::CapacityClass,
) -> sophia_protocol::BoundedCapacity {
    let disposition = match class {
        sophia_protocol::CapacityClass::TerminatingBoundary => {
            sophia_protocol::CapacitySaturationDisposition::BoundedDeferral {
                deadline_msec: ROUTED_INPUT_BOUNDARY_DEFERRAL_MSEC,
                retry_interval_msec: ROUTED_INPUT_BOUNDARY_RETRY_MSEC,
                escalation: sophia_protocol::CapacityEscalation::EndpointEpochClosed,
            }
        }
        _ => sophia_protocol::CapacitySaturationDisposition::EndpointEpochClosed,
    };
    sophia_protocol::BoundedCapacity::new(ROUTED_INPUT_INGRESS, capacity, disposition)
}

/// The owner loop's own waiting strategy, on the real clock.
struct OwnerLoopWait {
    started: std::time::Instant,
}

impl OwnerLoopWait {
    fn new() -> Self {
        Self {
            started: std::time::Instant::now(),
        }
    }
}

impl sophia_protocol::CapacityWait for OwnerLoopWait {
    fn elapsed_msec(&self) -> u32 {
        u32::try_from(self.started.elapsed().as_millis()).unwrap_or(u32::MAX)
    }

    fn pause(&mut self, interval_msec: u32) {
        std::thread::sleep(std::time::Duration::from_millis(u64::from(
            interval_msec.max(1),
        )));
    }
}

/// What the routed-input ingress dropped this tick, kept apart by class so the
/// report can say whether an ordinary event or a release was lost.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct RoutedInputIngressSaturation {
    ordered_discarded: usize,
    boundary_discarded: usize,
    ledger_discarded: usize,
}

impl RoutedInputIngressSaturation {
    const fn is_empty(self) -> bool {
        self.ordered_discarded == 0 && self.boundary_discarded == 0 && self.ledger_discarded == 0
    }

    fn merge(&mut self, other: Self) {
        self.ordered_discarded = self.ordered_discarded.saturating_add(other.ordered_discarded);
        self.boundary_discarded = self
            .boundary_discarded
            .saturating_add(other.boundary_discarded);
        self.ledger_discarded = self.ledger_discarded.saturating_add(other.ledger_discarded);
    }

    /// Reports through the shared coalescer so sustained pressure does not
    /// bury the rest of the log. Every emitted line carries the cumulative
    /// count, which is what keeps suppression from hiding volume.
    fn report(
        self,
        capacity: usize,
        ledger: &mut sophia_protocol::CapacityReportLedger,
    ) {
        if self.ledger_discarded != 0
            && let Some(occurrences) = ledger.observe(
                PRESSED_KEY_LEDGER,
                sophia_protocol::CapacitySaturationCause::DepthExhausted,
            )
        {
            print_capacity_saturation(&sophia_protocol::CapacitySaturationReport {
                resource: PRESSED_KEY_LEDGER,
                cause: sophia_protocol::CapacitySaturationCause::DepthExhausted,
                disposition: sophia_protocol::CapacitySaturationDisposition::EndpointEpochClosed,
                depth: SESSION_CLIENT_PRESSED_KEY_CAPACITY,
                capacity: SESSION_CLIENT_PRESSED_KEY_CAPACITY,
                discarded: usize::try_from(occurrences).unwrap_or(usize::MAX),
                waited_msec: 0,
            });
        }
        for (discarded, class) in [
            (
                self.ordered_discarded,
                sophia_protocol::CapacityClass::Ordered,
            ),
            (
                self.boundary_discarded,
                sophia_protocol::CapacityClass::TerminatingBoundary,
            ),
        ] {
            if discarded == 0 {
                continue;
            }
            let bound = routed_input_ingress_capacity(capacity, class);
            let cause = sophia_protocol::CapacitySaturationCause::DepthExhausted;
            let Some(occurrences) = ledger.observe(bound.resource, cause) else {
                continue;
            };
            print_capacity_saturation(&sophia_protocol::CapacitySaturationReport {
                resource: bound.resource,
                cause,
                disposition: bound.disposition,
                depth: capacity,
                capacity,
                discarded: usize::try_from(occurrences).unwrap_or(usize::MAX),
                waited_msec: 0,
            });
        }
    }
}

/// Offers one routed-input record, costing the record rather than the session
/// when the queue is full.
///
/// Returns whether the record was delivered. A `false` answer means the caller
/// must not record the record as routed: the client never saw it, and the epoch
/// close that follows is what keeps that from leaving latched state behind.
fn route_bounded_input<S: RoutedInputIngress>(
    sender: &S,
    route: XAuthorityRoutedInput,
    class: sophia_protocol::CapacityClass,
    saturation: &mut RoutedInputIngressSaturation,
) -> Result<bool, Box<dyn std::error::Error>> {
    let bound = routed_input_ingress_capacity(sender.capacity(), class);
    let outcome = sophia_protocol::drive_capacity_batch(
        &bound,
        [route],
        OwnerLoopWait::new,
        |route| match sender.try_send(route) {
            Ok(()) => sophia_protocol::CapacityBatchAttempt::Accepted,
            Err(std::sync::mpsc::TrySendError::Full(route)) => {
                sophia_protocol::CapacityBatchAttempt::Full {
                    record: route,
                    depth: bound.capacity,
                }
            }
            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                sophia_protocol::CapacityBatchAttempt::RecipientGone
            }
        },
    );
    match outcome {
        sophia_protocol::CapacityBatchOutcome::Drained { .. } => Ok(true),
        // A departed authority is not backpressure. Nothing downstream can
        // route input again, so this stays fatal.
        sophia_protocol::CapacityBatchOutcome::RecipientGone { .. } => {
            Err("routed input recipient departed".into())
        }
        sophia_protocol::CapacityBatchOutcome::Cancelled { .. }
        | sophia_protocol::CapacityBatchOutcome::Saturated { .. } => {
            match class {
                sophia_protocol::CapacityClass::TerminatingBoundary => {
                    saturation.boundary_discarded = saturation.boundary_discarded.saturating_add(1);
                }
                _ => saturation.ordered_discarded = saturation.ordered_discarded.saturating_add(1),
            }
            Ok(false)
        }
    }
}
