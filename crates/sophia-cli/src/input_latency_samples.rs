//! Repeatable input-to-photon sampling.
//!
//! The one-shot correlation beside this one answers "did physical input reach
//! a page flip", which a proof needs once. A percentile needs a population,
//! and a session that yields one sample can only ever report its own maximum.
//!
//! Samples are held in a pre-allocated ring rather than a growing vector: the
//! owner loop records into this on the input path, where the style guide
//! forbids casual allocation. Sorting happens once at completion, which is
//! where `XPresentCadence` already accepts the same cost.

use std::collections::VecDeque;

/// Samples retained before the oldest is evicted. A session presenting at
/// 60 Hz for a minute produces a few thousand key-to-photon opportunities at
/// most; this keeps the recent population without unbounded growth, and the
/// eviction count says when the window moved.
pub const INPUT_LATENCY_SAMPLE_CAPACITY: usize = 1_024;

/// A key press waiting for the page flip that will show it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PendingInputLatencySample {
    pub serial: u64,
    pub ingress_ust_usec: u64,
    /// Submissions the head had presented when the press was routed. The flip
    /// that shows this press is the first one past it.
    pub baseline_submission: usize,
    /// The newest composition the head held anywhere in its pipeline when the
    /// press was routed -- pending, rendering, submitted, or presented.
    ///
    /// A later submission is not by itself a later picture. A render already
    /// under way when the press arrives carries content composed before it,
    /// and finishes into a flip that satisfies every ordering test while
    /// showing none of the press. Requiring the presented composition to be
    /// newer than this is what makes the measurement input-to-photon rather
    /// than input-to-next-flip.
    pub baseline_frame: u64,
    pub queue_dwell_usec: u64,
}

/// One settled input-to-photon measurement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputLatencySample {
    pub full_chain_usec: u64,
    pub queue_dwell_usec: u64,
    pub submit_to_page_flip_usec: u64,
}

#[derive(Debug, Default)]
pub struct InputLatencySamples {
    /// Presses routed but not yet shown, oldest first.
    pending: VecDeque<PendingInputLatencySample>,
    settled: VecDeque<InputLatencySample>,
    evicted: usize,
    /// Presses that never found a flip, dropped when the pending queue filled.
    abandoned: usize,
}

impl InputLatencySamples {
    pub fn new() -> Self {
        Self {
            pending: VecDeque::with_capacity(INPUT_LATENCY_SAMPLE_CAPACITY),
            settled: VecDeque::with_capacity(INPUT_LATENCY_SAMPLE_CAPACITY),
            evicted: 0,
            abandoned: 0,
        }
    }

    /// Record a routed press awaiting its flip. A press that arrives while the
    /// queue is full displaces the oldest unshown one, which is counted rather
    /// than silently lost: an abandoned press means input outran presentation,
    /// which is itself worth seeing.
    pub fn observe_press(&mut self, sample: PendingInputLatencySample) {
        if self.pending.len() >= INPUT_LATENCY_SAMPLE_CAPACITY {
            self.pending.pop_front();
            self.abandoned = self.abandoned.saturating_add(1);
        }
        self.pending.push_back(sample);
    }

    /// Settle every press that this page flip showed.
    ///
    /// A flip presents whatever was submitted before it, so one flip can close
    /// several presses that arrived within the same frame. Each is a real
    /// sample: they waited different lengths for the same photon.
    pub fn observe_page_flip(
        &mut self,
        presented_submission: usize,
        presented_frame: u64,
        submission_ust_usec: u64,
        page_flip_ust_usec: u64,
    ) {
        while let Some(pending) = self.pending.front().copied() {
            if presented_submission <= pending.baseline_submission
                || presented_frame <= pending.baseline_frame
                || submission_ust_usec < pending.ingress_ust_usec
                || page_flip_ust_usec < submission_ust_usec
            {
                // Either this flip predates the press, carries a composition
                // built before it, or the clocks disagree about their order.
                // None of those is a measurement.
                break;
            }
            self.pending.pop_front();
            self.push_settled(InputLatencySample {
                full_chain_usec: page_flip_ust_usec.saturating_sub(pending.ingress_ust_usec),
                queue_dwell_usec: pending.queue_dwell_usec,
                submit_to_page_flip_usec: page_flip_ust_usec.saturating_sub(submission_ust_usec),
            });
        }
    }

    fn push_settled(&mut self, sample: InputLatencySample) {
        if self.settled.len() >= INPUT_LATENCY_SAMPLE_CAPACITY {
            self.settled.pop_front();
            self.evicted = self.evicted.saturating_add(1);
        }
        self.settled.push_back(sample);
    }

    pub fn summary(&self) -> Option<InputLatencySummary> {
        if self.settled.is_empty() {
            return None;
        }
        // Allocating once at completion, outside the owner loop's hot path.
        let mut full_chain: Vec<u64> = self
            .settled
            .iter()
            .map(|sample| sample.full_chain_usec)
            .collect();
        full_chain.sort_unstable();
        Some(InputLatencySummary {
            samples: full_chain.len(),
            evicted: self.evicted,
            abandoned: self.abandoned,
            pending: self.pending.len(),
            min_usec: *full_chain.first().expect("nonempty"),
            max_usec: *full_chain.last().expect("nonempty"),
            p50_usec: percentile_usec(&full_chain, 50),
            p95_usec: percentile_usec(&full_chain, 95),
            p99_usec: percentile_usec(&full_chain, 99),
            max_queue_dwell_usec: self
                .settled
                .iter()
                .map(|sample| sample.queue_dwell_usec)
                .max()
                .unwrap_or(0),
            max_submit_to_page_flip_usec: self
                .settled
                .iter()
                .map(|sample| sample.submit_to_page_flip_usec)
                .max()
                .unwrap_or(0),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputLatencySummary {
    pub samples: usize,
    pub evicted: usize,
    pub abandoned: usize,
    pub pending: usize,
    pub min_usec: u64,
    pub max_usec: u64,
    pub p50_usec: u64,
    pub p95_usec: u64,
    pub p99_usec: u64,
    pub max_queue_dwell_usec: u64,
    pub max_submit_to_page_flip_usec: u64,
}

/// Nearest-rank percentile over an ascending slice, matching the ceil-rank
/// formula the shell reporters and `XPresentCadence` already use so a number
/// means the same thing wherever it is computed.
pub fn percentile_usec(ascending: &[u64], percentile: usize) -> u64 {
    if ascending.is_empty() {
        return 0;
    }
    let rank = (percentile * ascending.len()).div_ceil(100).max(1);
    ascending[rank.min(ascending.len()) - 1]
}
