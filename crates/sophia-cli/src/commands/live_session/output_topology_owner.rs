#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveOutputTopologyPhase {
    Stable,
    Quarantined,
    Rebuilt,
    Published,
    AwaitingPresentation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveOutputTopologyRebuild {
    Unavailable,
    TransportReplaced,
    TopologyChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LiveOutputTopologyOwner {
    phase: LiveOutputTopologyPhase,
    transition: u64,
    notice_sequence: u64,
    topology_epoch: u64,
    publication_generation: u64,
    outputs: Vec<sophia_engine::HeadlessOutput>,
    policy_committed: bool,
    policy_settlement_pending: bool,
    presentation_baseline: usize,
}

impl LiveOutputTopologyOwner {
    fn new_at_generation(
        outputs: Vec<sophia_engine::HeadlessOutput>,
        publication_generation: u64,
    ) -> Result<Self, &'static str> {
        if outputs.is_empty() {
            return Err("live topology owner requires an initial output");
        }
        if publication_generation == 0 {
            return Err("live topology owner requires a valid publication generation");
        }
        Ok(Self {
            phase: LiveOutputTopologyPhase::Stable,
            transition: 0,
            notice_sequence: 0,
            topology_epoch: 1,
            publication_generation,
            outputs,
            policy_committed: true,
            policy_settlement_pending: false,
            presentation_baseline: 0,
        })
    }

    /// Returns true only when the caller must advance the application-input
    /// security epoch. Retries while one transition is quarantined retain the
    /// already-advanced epoch.
    fn begin_rescan(&mut self, notice_sequence: u64) -> Result<bool, &'static str> {
        if notice_sequence == 0 || notice_sequence < self.notice_sequence {
            return Err("live topology notice is invalid or stale");
        }
        if notice_sequence == self.notice_sequence {
            return Ok(false);
        }
        self.notice_sequence = notice_sequence;
        if self.phase == LiveOutputTopologyPhase::Quarantined {
            return Ok(false);
        }
        if self.phase != LiveOutputTopologyPhase::Stable {
            self.transition = self
                .transition
                .checked_add(1)
                .ok_or("live topology transition identity exhausted")?;
            self.phase = LiveOutputTopologyPhase::Quarantined;
            return Ok(false);
        }
        self.transition = self
            .transition
            .checked_add(1)
            .ok_or("live topology transition identity exhausted")?;
        self.phase = LiveOutputTopologyPhase::Quarantined;
        Ok(true)
    }

    fn observe_rebuild(
        &mut self,
        outputs: Vec<sophia_engine::HeadlessOutput>,
    ) -> Result<LiveOutputTopologyRebuild, &'static str> {
        if self.phase != LiveOutputTopologyPhase::Quarantined {
            return Err("live topology rebuild was observed outside quarantine");
        }
        if outputs.is_empty() {
            return Ok(LiveOutputTopologyRebuild::Unavailable);
        }
        let changed = outputs != self.outputs;
        if changed {
            self.topology_epoch = self
                .topology_epoch
                .checked_add(1)
                .ok_or("live topology epoch exhausted")?;
            self.publication_generation = self
                .publication_generation
                .checked_add(1)
                .ok_or("live output publication generation exhausted")?;
            self.outputs = outputs;
        }
        self.phase = LiveOutputTopologyPhase::Rebuilt;
        Ok(if changed {
            LiveOutputTopologyRebuild::TopologyChanged
        } else {
            LiveOutputTopologyRebuild::TransportReplaced
        })
    }

    fn mark_published(
        &mut self,
        presentation_baseline: usize,
        policy_required: bool,
    ) -> Result<(), &'static str> {
        if self.phase != LiveOutputTopologyPhase::Rebuilt {
            return Err("live topology publication is out of order");
        }
        self.presentation_baseline = presentation_baseline;
        self.policy_settlement_pending |= policy_required;
        self.policy_committed = !self.policy_settlement_pending;
        self.phase = if self.policy_committed {
            LiveOutputTopologyPhase::AwaitingPresentation
        } else {
            LiveOutputTopologyPhase::Published
        };
        Ok(())
    }

    fn mark_policy_committed(
        &mut self,
        presentation_baseline: usize,
    ) -> Result<(), &'static str> {
        if self.phase != LiveOutputTopologyPhase::Published {
            return Err("live topology policy settlement is out of order");
        }
        self.policy_committed = true;
        self.policy_settlement_pending = false;
        // A presentation which retired before the policy projection committed
        // cannot release the input quarantine.
        self.presentation_baseline = presentation_baseline;
        self.phase = LiveOutputTopologyPhase::AwaitingPresentation;
        Ok(())
    }

    fn observe_presentation(&mut self, retirements: usize) -> bool {
        if self.phase != LiveOutputTopologyPhase::AwaitingPresentation
            || !self.policy_committed
            || retirements <= self.presentation_baseline
        {
            return false;
        }
        self.phase = LiveOutputTopologyPhase::Stable;
        true
    }

    const fn input_quarantined(&self) -> bool {
        !matches!(self.phase, LiveOutputTopologyPhase::Stable)
    }
}
