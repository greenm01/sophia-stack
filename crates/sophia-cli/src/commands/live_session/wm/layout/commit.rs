impl PersistentLiveLayout {
    fn commit_proposal(&mut self, proposal: LiveWmProposal) -> LiveWmCommitResult {
        let pending = PendingLiveWmLayout {
            transaction: proposal.transaction,
            layers: proposal.layers,
            requested_sizes: proposal.requested_sizes,
            presentation_states: proposal.presentation_states,
            presentation_settlements: BTreeSet::new(),
            configure_deliveries: proposal.configure_deliveries,
            focus: proposal.focus,
            deadline: Instant::now(),
            update: proposal.update,
            moved_surfaces: proposal.moved_surfaces,
            staged_transactions: BTreeMap::new(),
            admission_surfaces: BTreeSet::new(),
            source: proposal.source,
            effects: proposal.effects,
            policy_settlement: proposal.policy_settlement,
        };
        self.commit_pending(pending)
    }

    fn commit_pending(&mut self, pending: PendingLiveWmLayout) -> LiveWmCommitResult {
        let matched_surfaces = pending.staged_transactions.len();
        for surface in &pending.admission_surfaces {
            let Some(transaction) = pending.staged_transactions.get(surface) else {
                continue;
            };
            if self.visual_candidate_requires_retirement(transaction) {
                // Present semantics do not depend on whether the authority
                // could export the pixmap as a DMA-BUF. A materialized CPU
                // Present remains fenced until its composed frame retires.
                if self
                    .admissions
                    .begin_retirement(*surface, transaction.key())
                {
                    println!(
                        "sophia_live_visual_admission schema=1 status=armed transaction={} surface={}",
                        transaction.transaction.raw(),
                        surface.index(),
                    );
                }
            } else {
                match transaction.target_buffer() {
                    BufferSource::CpuBuffer { .. } => {
                        if self.admissions.mark_managed(*surface) {
                            self.planning_surfaces.remove(surface);
                            self.release_recovery_extent(
                                *surface,
                                "cpu_admission_committed",
                            );
                            println!(
                                "sophia_live_visual_admission schema=1 status=committed transaction={} surface={} source=cpu_backing_snapshot",
                                transaction.transaction.raw(),
                                surface.index(),
                            );
                            if let (Some(source), Some(target)) = (
                                live_transaction_pixel_size(
                                    transaction.target_buffer(),
                                    &self.dma_buf_sizes,
                                    &self.cpu_buffer_sizes,
                                ),
                                pending
                                    .layers
                                    .iter()
                                    .find(|layer| layer.surface == *surface)
                                    .map(|layer| layer.geometry),
                            ) {
                                // Backing snapshots bypass Present retirement, so
                                // record the atomic pixel-and-geometry commit here.
                                println!(
                                    "sophia_live_visual_admission_geometry schema=1 status=committed transaction={} surface={} source={}x{} target={}x{}_{}_{} unit_scale={}",
                                    transaction.transaction.raw(),
                                    surface.index(),
                                    source.width,
                                    source.height,
                                    target.width,
                                    target.height,
                                    target.x,
                                    target.y,
                                    source.width == target.width
                                        && source.height == target.height,
                                );
                            }
                        }
                    }
                    BufferSource::XPixmap { .. } | BufferSource::None => {
                        if self.admissions.mark_managed(*surface) {
                            self.planning_surfaces.remove(surface);
                            self.release_recovery_extent(*surface, "admission_committed");
                        }
                    }
                    BufferSource::DmaBuf { .. } => {}
                }
            }
        }
        if !pending.staged_transactions.is_empty() {
            for transaction in pending.staged_transactions.values() {
                if let Some(size) = live_transaction_pixel_size(
                    transaction.target_buffer(),
                    &self.dma_buf_sizes,
                    &self.cpu_buffer_sizes,
                ) {
                    let layout_size = live_transaction_observed_size(
                        transaction,
                        &self.dma_buf_sizes,
                        &self.cpu_buffer_sizes,
                    );
                    if self.visual_candidate_requires_retirement(transaction) {
                        let candidate = ResizeVisualCommit {
                            candidate: transaction.key(),
                            size,
                            layout_size,
                        };
                        if !self.awaiting_visual_commits.surface_layout_awaiting(
                            transaction.surface,
                            layout_size,
                        ) {
                            self.awaiting_visual_commits
                                .arm(candidate)
                                .expect("a staged layout owns one bounded visual candidate");
                            println!(
                                "sophia_live_resize_epoch schema=3 status=visual_armed epoch={} transaction={} surface={} width={} height={}",
                                pending.transaction.raw(),
                                transaction.transaction.raw(),
                                transaction.surface.index(),
                                layout_size.width,
                                layout_size.height,
                            );
                        }
                    } else {
                        self.layout_epochs
                            .record_committed(transaction.surface, layout_size);
                    }
                }
            }
        }
        let committed_public_admission = (pending.update.commit.outcome
            == TransactionOutcome::Committed
            && pending.policy_settlement.is_some())
        .then_some(pending.source)
        .flatten()
        .and_then(|source| match source {
            LiveWmProposalSource::Manage(surface) => Some(surface),
            _ => None,
        })
        .filter(|surface| {
            pending
                .layers
                .iter()
                .any(|layer| layer.surface == *surface)
        });
        self.layers = pending
            .layers
            .into_iter()
            .map(|layer| (layer.surface, layer))
            .collect();
        for (surface, state) in pending.presentation_states {
            self.committed_policy_presentations.insert(surface, state);
        }
        let selected_admission_transactions = pending
            .admission_surfaces
            .iter()
            .filter_map(|surface| {
                pending
                    .staged_transactions
                    .get(surface)
                    .map(|transaction| (*surface, transaction.transaction))
            })
            .collect();
        self.release_admission_groups(&selected_admission_transactions);
        // A committed relayout can intentionally exclude admissions queued
        // behind it. Keep those surfaces eligible for their own ManageSurface
        // response while they remain in the planning table. The legacy path
        // consumes ownership through its candidate workspace assignment; the
        // public reducer has no private workspace effect, so its exact
        // committed Manage source plus retained placement is the equivalent
        // ownership boundary. The surface can remain visually fenced in
        // AwaitingRetirement without being planned again.
        self.unmanaged_surfaces.retain(|surface| {
            Some(*surface) != committed_public_admission
                && self.planning_surfaces.contains_key(surface)
                && pending.effects.as_ref().is_none_or(|effects| {
                    effects.workspace_state.surface_workspace(*surface).is_none()
                })
        });
        self.admission_retries
            .retain(|surface, _| self.unmanaged_surfaces.contains(surface));
        for surface in self.layers.keys().copied() {
            if !self.unmanaged_surfaces.contains(&surface)
                && matches!(
                    self.admissions.state(surface),
                    sophia_engine::SurfacePresentationAdmissionState::Inactive
                        | sophia_engine::SurfacePresentationAdmissionState::Managed
                )
            {
                self.layout_epochs
                    .set_admission(surface, sophia_engine::SurfaceAdmissionState::Managed);
            }
        }
        // A presented admission can defer positive focus until retirement.
        // Newer committed policy replaces that intent even while the older
        // visual candidate remains independently entitled to retire.
        self.retirement_focus
            .retain(|surface, _| Some(*surface) == pending.focus);
        if let Some(surface) = pending.focus {
            match self.admissions.state(surface) {
                sophia_engine::SurfacePresentationAdmissionState::AwaitingRetirement {
                    visual_candidate,
                    ..
                } => {
                    self.retirement_focus
                        .insert(surface, (visual_candidate, pending.transaction));
                }
                _ => self.queue_focus_handoff(pending.transaction, surface),
            }
        }
        println!(
            "sophia_live_wm schema=1 status=layout_committed transaction={} surfaces={} moved_surfaces={} configure_deliveries={} outcome={:?}",
            pending.transaction.raw(),
            self.layers.len(),
            pending.moved_surfaces,
            pending.configure_deliveries,
            pending.update.commit.outcome
        );
        println!(
            "sophia_live_resize_epoch schema=1 status=committed transaction={} matched_surfaces={}",
            pending.transaction.raw(),
            matched_surfaces,
        );
        LiveWmCommitResult {
            update: pending.update,
            source: pending.source,
            effects: pending.effects,
            policy_settlement: pending.policy_settlement,
        }
    }

    fn visual_candidate_requires_retirement(&self, transaction: &SurfaceTransaction) -> bool {
        self.layout_epochs
            .safe_observation(transaction.surface)
            .is_some_and(|observation| {
                observation.candidate == Some(transaction.key())
                    && observation.evidence
                        == sophia_engine::SurfaceVisualEvidence::PresentedBuffer
            })
    }

    fn projected_batch(
        &mut self,
        batch: &XAuthorityObservedTransactionBatch,
    ) -> (
        XAuthorityObservedTransactionBatch,
        Vec<LiveAdmissionAuthorityGroup>,
    ) {
        // Layout projection preserves normal authority generation order.
        // Unmapped managed surfaces are the exception: their latest drawing
        // transaction remains in the bounded admission quarantine until it is
        // released once at generation zero and the accepted geometry.
        let quarantined_transactions = self
            .pre_admission_groups
            .iter()
            .chain(self.released_admission_groups.iter())
            .map(|group| group.transaction)
            .collect::<BTreeSet<_>>();
        let mut projected = project_authority_batch_onto_layout(batch.clone(), &self.layers);
        projected.transactions.retain(|transaction| {
            !quarantined_transactions.contains(&transaction.transaction)
        });
        projected.present_submissions.retain(|submission| {
            !quarantined_transactions.contains(&submission.transaction)
        });
        projected.software_present_submissions.retain(|submission| {
            !quarantined_transactions.contains(&submission.transaction)
        });
        let projected_cpu_handles = projected
            .transactions
            .iter()
            .flat_map(|transaction| transaction.content.variants())
            .filter_map(|variant| match variant.source {
                BufferSource::CpuBuffer { handle } => Some(handle),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        projected
            .cpu_buffer_updates
            .retain(|update| projected_cpu_handles.contains(&update.handle()));
        let referenced_dma_bufs = self.admission_group_dma_bufs();
        let referenced_fences = self.admission_group_fences();
        projected
            .released_dma_bufs
            .retain(|handle| !referenced_dma_bufs.contains(handle));
        projected
            .released_fences
            .retain(|handle| !referenced_fences.contains(handle));
        let releasable_dma_bufs = self
            .deferred_dma_buf_releases
            .iter()
            .filter(|handle| !referenced_dma_bufs.contains(handle))
            .copied()
            .collect::<Vec<_>>();
        for handle in releasable_dma_bufs {
            self.deferred_dma_buf_releases.remove(&handle);
            self.dma_buf_sizes.remove(&handle);
            if !projected.released_dma_bufs.contains(&handle) {
                projected.released_dma_bufs.push(handle);
            }
        }
        let releasable_fences = self
            .deferred_fence_releases
            .iter()
            .filter(|handle| !referenced_fences.contains(handle))
            .copied()
            .collect::<Vec<_>>();
        for handle in releasable_fences {
            self.deferred_fence_releases.remove(&handle);
            if !projected.released_fences.contains(&handle) {
                projected.released_fences.push(handle);
            }
        }
        let mut released = Vec::new();
        let mut retained = VecDeque::new();
        while let Some(group) = self.released_admission_groups.pop_front() {
            if group.contains_any_surface(&self.unmanaged_surfaces) {
                retained.push_back(group);
            } else {
                released.push(group);
            }
        }
        self.released_admission_groups = retained;
        (projected, released)
    }
}
