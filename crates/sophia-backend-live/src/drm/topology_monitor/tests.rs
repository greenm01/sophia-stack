#![cfg(test)]

use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct LiveDrmTopologyNoticeReducer {
    latest_sequence: u64,
    pending: bool,
    observed: u64,
    coalesced: u64,
    delivered: u64,
}

impl LiveDrmTopologyNoticeReducer {
    fn observe(&mut self) -> Result<bool, &'static str> {
        self.latest_sequence = self
            .latest_sequence
            .checked_add(1)
            .ok_or("DRM topology notice sequence exhausted")?;
        self.observed = self.observed.saturating_add(1);
        if self.pending {
            self.coalesced = self.coalesced.saturating_add(1);
            return Ok(false);
        }
        self.pending = true;
        Ok(true)
    }

    fn take(&mut self) -> Option<LiveDrmTopologyRescanNotice> {
        if !self.pending {
            return None;
        }
        self.pending = false;
        self.delivered = self.delivered.saturating_add(1);
        Some(LiveDrmTopologyRescanNotice {
            sequence: self.latest_sequence,
        })
    }

    fn stats(self) -> LiveDrmTopologyMonitorStats {
        LiveDrmTopologyMonitorStats {
            observed: self.observed,
            coalesced: self.coalesced,
            delivered: self.delivered,
        }
    }
}

#[test]
fn capacity_one_notice_reducer_delivers_latest_sequence() {
    let mut reducer = LiveDrmTopologyNoticeReducer::default();
    assert_eq!(reducer.observe(), Ok(true));
    assert_eq!(reducer.observe(), Ok(false));
    assert_eq!(reducer.observe(), Ok(false));
    assert_eq!(
        reducer.take(),
        Some(LiveDrmTopologyRescanNotice { sequence: 3 })
    );
    assert_eq!(reducer.take(), None);
    assert_eq!(
        reducer.stats(),
        LiveDrmTopologyMonitorStats {
            observed: 3,
            coalesced: 2,
            delivered: 1,
        }
    );
}

#[test]
fn notice_reducer_rearms_after_delivery() {
    let mut reducer = LiveDrmTopologyNoticeReducer::default();
    assert_eq!(reducer.observe(), Ok(true));
    assert_eq!(reducer.take().map(|notice| notice.sequence), Some(1));
    assert_eq!(reducer.observe(), Ok(true));
    assert_eq!(reducer.take().map(|notice| notice.sequence), Some(2));
}
