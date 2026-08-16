/// Bounded per-surface coalescer for sampled-fallback telemetry.
///
/// A poisoned journal re-reports on every requirement, so an unfiltered warn
/// per event buries the rest of the log. Emission is restricted to the first
/// occurrence and then each power of two, and every emitted line carries the
/// cumulative count, so suppression never hides how often a cause fired.
#[derive(Debug, Default)]
pub struct XRasterFallbackCoalescer {
    counts: BTreeMap<(SurfaceId, XRasterFallbackCause), u64>,
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
        }
        let count = self.counts.entry((surface, cause)).or_insert(0);
        *count = count.saturating_add(1);
        let count = *count;
        count.is_power_of_two().then_some(count)
    }

    /// Cumulative occurrences observed for one surface and cause.
    pub fn occurrences(&self, surface: SurfaceId, cause: XRasterFallbackCause) -> u64 {
        self.counts.get(&(surface, cause)).copied().unwrap_or(0)
    }

    /// Records one fallback and emits the structured warning when this
    /// occurrence is due.
    pub fn report(
        &mut self,
        requirements: &sophia_protocol::SurfaceRasterRequirements,
        cause: XRasterFallbackCause,
    ) {
        let Some(occurrences) = self.observe(requirements.surface, cause) else {
            return;
        };
        tracing::warn!(
            "sophia_x11_raster_requirement schema=1 status=sampled_fallback cause={} occurrences={} surface={:?} content_generation={} requirement_generation={} classes={}",
            cause.as_str(),
            occurrences,
            requirements.surface,
            requirements.committed_content_generation,
            requirements.requirement_generation,
            requirements.classes.len(),
        );
    }
}
