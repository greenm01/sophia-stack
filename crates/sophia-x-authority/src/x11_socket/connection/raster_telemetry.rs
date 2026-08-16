/// Bounded per-surface coalescer for sampled-fallback telemetry.
///
/// A poisoned journal re-reports on every requirement, so an unfiltered warn
/// per event buries the rest of the log. Emission is restricted to the first
/// occurrence and then each power of two, and every emitted line carries the
/// cumulative count, so suppression never hides how often a cause fired.
///
/// Suppression is deliberately transition-sensitive. A run that settles into a
/// steady fallback goes quiet, but a change in cause, or the first success
/// after any failure, always emits. An earlier revision keyed only on repeat
/// count and fell silent across the exact transition a physical run needed to
/// explain, which is the failure mode this guards.
#[derive(Debug, Default)]
pub struct XRasterFallbackCoalescer {
    counts: BTreeMap<(SurfaceId, XRasterFallbackCause), u64>,
    /// The cause last reported per surface, so a change of cause is treated as
    /// news rather than another repeat.
    last_cause: BTreeMap<SurfaceId, XRasterFallbackCause>,
    /// Surfaces currently in a failing run, so the first recovery is emitted.
    failing: BTreeSet<SurfaceId>,
}

/// Distinct (surface, cause) pairs retained before the table resets. The reset
/// re-emits rather than dropping, so a churning surface set stays visible.
pub const X_RASTER_FALLBACK_COALESCER_MAX_ENTRIES: usize = 256;

impl XRasterFallbackCoalescer {
    /// Records one fallback and returns the cumulative count when this
    /// occurrence should be logged.
    pub fn observe(&mut self, surface: SurfaceId, cause: XRasterFallbackCause) -> Option<u64> {
        if self.counts.len() >= X_RASTER_FALLBACK_COALESCER_MAX_ENTRIES
            && !self.counts.contains_key(&(surface, cause))
        {
            self.counts.clear();
            self.last_cause.clear();
        }
        let changed_cause = self.last_cause.insert(surface, cause) != Some(cause);
        self.failing.insert(surface);
        let count = self.counts.entry((surface, cause)).or_insert(0);
        *count = count.saturating_add(1);
        let count = *count;
        (changed_cause || count.is_power_of_two()).then_some(count)
    }

    /// Records that a requirement was satisfied. Returns the number of
    /// consecutive failures this recovery ended, when the recovery is itself
    /// worth reporting.
    pub fn observe_satisfied(&mut self, surface: SurfaceId) -> Option<u64> {
        if !self.failing.remove(&surface) {
            return None;
        }
        let ended = self
            .last_cause
            .remove(&surface)
            .map(|cause| self.occurrences(surface, cause))
            .unwrap_or(0);
        self.counts.retain(|(tracked, _), _| *tracked != surface);
        Some(ended)
    }

    /// Cumulative occurrences observed for one surface and cause.
    pub fn occurrences(&self, surface: SurfaceId, cause: XRasterFallbackCause) -> u64 {
        self.counts.get(&(surface, cause)).copied().unwrap_or(0)
    }

    /// Records one satisfied requirement and reports the recovery when it ends
    /// a failing run. Steady success stays silent.
    pub fn report_satisfied(
        &mut self,
        requirements: &sophia_protocol::SurfaceRasterRequirements,
        produced_content_generation: u64,
    ) {
        let Some(ended_failures) = self.observe_satisfied(requirements.surface) else {
            return;
        };
        tracing::info!(
            "sophia_x11_raster_requirement schema=1 status=satisfied surface={:?} content_generation={} produced_content_generation={} requirement_generation={} classes={} ended_failures={}",
            requirements.surface,
            requirements.committed_content_generation,
            produced_content_generation,
            requirements.requirement_generation,
            requirements.classes.len(),
            ended_failures,
        );
    }

    /// Records one fallback and emits the structured warning when this
    /// occurrence is due.
    pub fn report(
        &mut self,
        requirements: &sophia_protocol::SurfaceRasterRequirements,
        cause: XRasterFallbackCause,
        observed_content_generation: u64,
    ) {
        let Some(occurrences) = self.observe(requirements.surface, cause) else {
            return;
        };
        tracing::warn!(
            "sophia_x11_raster_requirement schema=1 status=sampled_fallback cause={} occurrences={} surface={:?} content_generation={} observed_content_generation={} requirement_generation={} classes={}",
            cause.as_str(),
            occurrences,
            requirements.surface,
            requirements.committed_content_generation,
            observed_content_generation,
            requirements.requirement_generation,
            requirements.classes.len(),
        );
    }
}
