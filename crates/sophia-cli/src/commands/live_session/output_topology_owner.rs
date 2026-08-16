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
    /// How many connectors drive each logical output.
    ///
    /// Held beside the output list because losing one head of a mirror group
    /// leaves that list identical: the logical output survives, one of its screens
    /// goes dark, and a comparison on outputs alone calls it no change at all.
    heads: Vec<(sophia_protocol::OutputId, usize)>,
    policy_committed: bool,
    policy_settlement_pending: bool,
    presentation_baseline: usize,
}

impl LiveOutputTopologyOwner {
    fn new_at_generation(
        outputs: Vec<sophia_engine::HeadlessOutput>,
        heads: Vec<(sophia_protocol::OutputId, usize)>,
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
            heads,
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

    /// Quarantines input for an authority-driven topology transaction without
    /// pretending that a DRM hotplug notice occurred. The replacement remains
    /// private until `observe_rebuild` and `mark_published` run after its
    /// physical first-presentation barrier.
    fn begin_policy_change(&mut self) -> Result<bool, &'static str> {
        if self.phase != LiveOutputTopologyPhase::Stable {
            return Err("live policy topology change overlaps another transition");
        }
        self.transition = self
            .transition
            .checked_add(1)
            .ok_or("live topology transition identity exhausted")?;
        self.phase = LiveOutputTopologyPhase::Quarantined;
        Ok(true)
    }

    /// Restores the published topology after a policy candidate was rejected
    /// or physically rolled back. No published fields were mutated while the
    /// owner was quarantined, so cancellation is an explicit phase transition.
    fn cancel_policy_change(&mut self) -> Result<(), &'static str> {
        if self.phase != LiveOutputTopologyPhase::Quarantined {
            return Err("live policy topology cancellation is out of order");
        }
        self.phase = LiveOutputTopologyPhase::Stable;
        Ok(())
    }

    /// Records the monotonic X/RandR generation consumed while reverting a
    /// candidate that reached the frontend but never became Engine authority.
    /// The logical topology epoch and published output set remain unchanged.
    fn observe_policy_transport_rollback(
        &mut self,
        publication_generation: u64,
    ) -> Result<(), &'static str> {
        if self.phase != LiveOutputTopologyPhase::Quarantined
            || publication_generation <= self.publication_generation
        {
            return Err("live policy topology transport rollback is out of order");
        }
        self.publication_generation = publication_generation;
        Ok(())
    }

    fn observe_rebuild(
        &mut self,
        outputs: Vec<sophia_engine::HeadlessOutput>,
        heads: Vec<(sophia_protocol::OutputId, usize)>,
    ) -> Result<LiveOutputTopologyRebuild, &'static str> {
        if self.phase != LiveOutputTopologyPhase::Quarantined {
            return Err("live topology rebuild was observed outside quarantine");
        }
        if outputs.is_empty() {
            return Ok(LiveOutputTopologyRebuild::Unavailable);
        }
        // A surviving-head topology is a new candidate. Comparing the head counts
        // as well as the outputs is what makes that true here rather than only in
        // the model that requires it.
        let changed = outputs != self.outputs || heads != self.heads;
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
            self.heads = heads;
        }
        self.phase = LiveOutputTopologyPhase::Rebuilt;
        Ok(if changed {
            LiveOutputTopologyRebuild::TopologyChanged
        } else {
            LiveOutputTopologyRebuild::TransportReplaced
        })
    }

    /// Installs an authority-approved policy candidate after its physical
    /// first-presentation barrier. Unlike hotplug reduction, this always
    /// advances the topology epoch: mode timing, transform, mapping, and VRR
    /// changes may preserve both logical extents and mirror member counts.
    fn observe_policy_rebuild(
        &mut self,
        outputs: Vec<sophia_engine::HeadlessOutput>,
        heads: Vec<(sophia_protocol::OutputId, usize)>,
        candidate_topology_epoch: u64,
    ) -> Result<(), &'static str> {
        if self.phase != LiveOutputTopologyPhase::Quarantined
            || outputs.is_empty()
            || candidate_topology_epoch != self.topology_epoch.checked_add(1).unwrap_or(0)
        {
            return Err("live policy topology rebuild is invalid or out of order");
        }
        self.topology_epoch = candidate_topology_epoch;
        self.publication_generation = self
            .publication_generation
            .checked_add(1)
            .ok_or("live output publication generation exhausted")?;
        self.outputs = outputs;
        self.heads = heads;
        self.phase = LiveOutputTopologyPhase::Rebuilt;
        Ok(())
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
