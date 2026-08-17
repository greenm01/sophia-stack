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
