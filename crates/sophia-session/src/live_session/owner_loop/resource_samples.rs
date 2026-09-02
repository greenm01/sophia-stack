// Periodic resource sampling, so "no steady-state growth" is measured rather
// than inferred.
//
// Every resource figure the session reports is emitted once, at completion. A
// single end-of-session record answers whether the session drained -- live
// entries at zero, no slot still leased -- and cannot answer whether anything
// grew while it ran. A session that leaks one buffer a minute for two hours and
// frees them all at teardown produces the same clean completion record as one
// that never held more than three.
//
// Milestone 14 exits on "bounded warmed resource counts, no steady-state
// allocation growth", and until now nothing measured the second clause. This
// samples the same gauges the completion record carries, on a bounded cadence,
// so a verifier can compare the run against itself.
//
// The sampler states facts and draws no conclusion. Whether a population grew
// is decided by the verifier from the samples, because an emitter that graded
// its own health would be the only witness to its own failure.

/// How often a sample is taken.
///
/// Slow enough that sampling is not itself the workload, fast enough that a
/// bounded gate produces a population a halves comparison can use: a
/// twelve-minute run yields well over a hundred samples.
pub(crate) const RESOURCE_SAMPLE_INTERVAL: Duration = Duration::from_secs(5);

/// The most samples one session records.
///
/// At five seconds this covers two hours plus ten minutes of teardown margin.
/// A longer session keeps running and stops sampling, and says so: the
/// alternative is an unbounded record stream in a soak, which is the shape of
/// leak this file exists to detect.
pub(crate) const RESOURCE_SAMPLE_CAPACITY: u64 = 1_560;

/// Bounded periodic sampling of the session's resource gauges.
///
/// Passive: it owns when a sample is due and how many have been taken, and
/// nothing else. The values come from the caller, which is the only thing that
/// can see them.
struct LiveResourceSampler {
    started: Instant,
    next_sample_at: Instant,
    sequence: u64,
    saturated: bool,
}

/// One reading of the gauges a growth check compares.
///
/// Every field is a live count rather than a total, because a total only ever
/// rises and says nothing about whether anything is being held. `rss_kib` is
/// the process's own resident size, which is the only figure here that includes
/// allocations Sophia does not itself account for.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct LiveResourceSample {
    pub cpu_registry_buffers: usize,
    pub cpu_registry_bytes: usize,
    pub cpu_cow_splits: u64,
    pub frame_slots_leased: u32,
    pub snapshot_live_entries: usize,
    pub import_cache_live_entries: usize,
}

impl LiveResourceSampler {
    fn new(started: Instant) -> Self {
        Self {
            started,
            next_sample_at: started + RESOURCE_SAMPLE_INTERVAL,
            sequence: 0,
            saturated: false,
        }
    }

    /// Whether a sample is due, which the caller checks before gathering one.
    ///
    /// Asking first keeps the gauge reads out of the hot path: they walk a map
    /// and read `/proc`, and doing that per loop iteration would make the
    /// measurement part of what it measures.
    fn is_due(&self, now: Instant) -> bool {
        !self.saturated && now >= self.next_sample_at
    }

    /// Record one sample, and say when the next is due.
    fn record(&mut self, now: Instant, sample: LiveResourceSample) {
        if self.saturated {
            return;
        }
        self.sequence = self.sequence.saturating_add(1);
        if self.sequence >= RESOURCE_SAMPLE_CAPACITY {
            self.saturated = true;
        }
        self.next_sample_at = now + RESOURCE_SAMPLE_INTERVAL;
        let uptime_msec = u64::try_from(now.duration_since(self.started).as_millis())
            .unwrap_or(u64::MAX);
        crate::session_println!(
            "sophia_live_resource_sample schema=1 seq={} uptime_msec={uptime_msec} rss_kib={} cpu_registry_buffers={} cpu_registry_bytes={} cpu_cow_splits={} frame_slots_leased={} snapshot_live_entries={} import_cache_live_entries={}",
            self.sequence,
            resident_kib().unwrap_or(0),
            sample.cpu_registry_buffers,
            sample.cpu_registry_bytes,
            sample.cpu_cow_splits,
            sample.frame_slots_leased,
            sample.snapshot_live_entries,
            sample.import_cache_live_entries,
        );
    }

    /// The population this session produced, reported without a verdict.
    ///
    /// `saturated=true` means sampling stopped before the session did, so the
    /// samples describe only the bounded prefix rather than the whole run. A
    /// verifier that reasoned over them as if they covered the session would
    /// be reading a truncated population as a complete one.
    fn report(&self) {
        crate::session_println!(
            "sophia_live_resource_steady_state schema=1 status=complete samples={} saturated={} interval_msec={}",
            self.sequence,
            self.saturated,
            RESOURCE_SAMPLE_INTERVAL.as_millis(),
        );
    }
}

/// The process's resident set size, in kibibytes.
///
/// Read from `/proc/self/status` rather than tracked, because the figure that
/// matters includes every allocation the process made, not only the ones Sophia
/// counts. `None` where the file cannot be read or does not carry the field,
/// which the record reports as zero: a missing reading is not a small one, and
/// the verifier's growth rule treats a flat zero series as nothing to compare.
fn resident_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let rest = line.strip_prefix("VmRSS:")?;
        rest.split_whitespace().next()?.parse().ok()
    })
}
