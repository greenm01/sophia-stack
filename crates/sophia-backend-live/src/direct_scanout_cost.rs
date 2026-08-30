//! What a frame costs, kept separately for the two ways one reaches a plane.
//!
//! Direct scanout skips a whole composition pass, so it should cost less --
//! but "should" is the word this module exists to remove. The counters that
//! already exist say how many frames flipped, never what any of them cost,
//! and the maxima that exist are single worst cases across a session with
//! both populations mixed into one number.
//!
//! Two durations, per population:
//!
//!   offer -> submit  the pass direct scanout skips. A composed frame waits
//!                    for the renderer and pays for the draw; a direct frame
//!                    pays only the export path. This is where a difference,
//!                    if there is one, has to appear.
//!   submit -> flip   what the display engine does with it afterwards, which
//!                    should not differ by population and is measured to
//!                    check that rather than to assume it.
//!
//! Bounded by construction: Milestone 14 asks for no steady-state allocation
//! growth, and instrumentation that grows without limit would be answering a
//! question about efficiency by becoming a leak.

use std::time::Duration;

/// How many samples one population keeps before it stops accepting more.
///
/// A twenty-second probe at sixty frames a second offers about twelve
/// hundred, so this holds a full run with room to spare. When a longer run
/// does fill it, the summary says so rather than quietly describing a
/// prefix as though it were the whole.
const SAMPLE_CAPACITY: usize = 4096;

#[derive(Clone, Debug, Default)]
pub struct DirectScanoutCostSamples {
    microseconds: Vec<u32>,
    saturated: bool,
}

impl DirectScanoutCostSamples {
    fn record(&mut self, elapsed: Duration) {
        if self.microseconds.len() >= SAMPLE_CAPACITY {
            self.saturated = true;
            return;
        }
        self.microseconds
            .push(u32::try_from(elapsed.as_micros()).unwrap_or(u32::MAX));
    }

    fn merge(&mut self, other: &Self) {
        for sample in &other.microseconds {
            if self.microseconds.len() >= SAMPLE_CAPACITY {
                self.saturated = true;
                return;
            }
            self.microseconds.push(*sample);
        }
        self.saturated |= other.saturated;
    }

    /// The distribution, or nothing when this population never happened.
    ///
    /// Nothing is a real answer: a session that never opened an overlay has
    /// no composed frames to compare against, and reporting zeros for it
    /// would read as "free" rather than "absent".
    pub fn summary(&self) -> Option<DirectScanoutCostSummary> {
        if self.microseconds.is_empty() {
            return None;
        }
        let mut sorted = self.microseconds.clone();
        sorted.sort_unstable();
        Some(DirectScanoutCostSummary {
            frames: sorted.len(),
            min: sorted[0],
            p50: percentile(&sorted, 50),
            p99: percentile(&sorted, 99),
            max: sorted[sorted.len() - 1],
            saturated: self.saturated,
        })
    }
}

/// Nearest-rank, so every reported value is a measurement rather than an
/// interpolation between two of them.
fn percentile(sorted: &[u32], percent: usize) -> u32 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = percent * sorted.len();
    let index = rank.div_ceil(100).saturating_sub(1);
    sorted[index.min(sorted.len() - 1)]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectScanoutCostSummary {
    pub frames: usize,
    pub min: u32,
    pub p50: u32,
    pub p99: u32,
    pub max: u32,
    pub saturated: bool,
}

/// One population's two measurements.
#[derive(Clone, Debug, Default)]
pub struct DirectScanoutPopulationCost {
    pub offer_to_submit: DirectScanoutCostSamples,
    pub submit_to_flip: DirectScanoutCostSamples,
}

impl DirectScanoutPopulationCost {
    fn merge(&mut self, other: &Self) {
        self.offer_to_submit.merge(&other.offer_to_submit);
        self.submit_to_flip.merge(&other.submit_to_flip);
    }
}

/// Both populations, recorded wherever the frame is and summed at the end.
#[derive(Clone, Debug, Default)]
pub struct DirectScanoutCost {
    pub direct: DirectScanoutPopulationCost,
    pub composed: DirectScanoutPopulationCost,
}

impl DirectScanoutCost {
    fn population(&mut self, direct: bool) -> &mut DirectScanoutPopulationCost {
        if direct {
            &mut self.direct
        } else {
            &mut self.composed
        }
    }

    pub fn record_offer_to_submit(&mut self, direct: bool, elapsed: Duration) {
        self.population(direct).offer_to_submit.record(elapsed);
    }

    pub fn record_submit_to_flip(&mut self, direct: bool, elapsed: Duration) {
        self.population(direct).submit_to_flip.record(elapsed);
    }

    pub fn merge(&mut self, other: &Self) {
        self.direct.merge(&other.direct);
        self.composed.merge(&other.composed);
    }
}
