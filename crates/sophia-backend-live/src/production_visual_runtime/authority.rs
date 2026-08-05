use super::*;

impl LiveProductionVisualRuntime {
    /// Whether retirement released ordered content that needs another cycle.
    pub fn has_released_surface_content(&self) -> bool {
        !self.released_surface_content.is_empty()
    }

    pub fn released_surface_content_requires_gpu(&self) -> bool {
        self.released_surface_content
            .iter()
            .any(|group| !group.present_submissions.is_empty())
    }

    pub fn released_surface_content_transaction(&self) -> Option<TransactionId> {
        self.released_surface_content
            .front()
            .map(|group| group.transaction)
    }

    pub(super) fn ready_surface_content_batch(
        &mut self,
        batch: &LiveProductionAuthorityBatch,
    ) -> Result<LiveProductionAuthorityBatch, Box<dyn std::error::Error>> {
        batch.validate()?;
        let mut ordered = self.released_surface_content.drain(..).collect::<Vec<_>>();
        ordered.extend(batch.groups.iter().cloned());
        let ordered =
            rebase_authority_groups_to_committed(ordered, self.production.committed_surfaces());
        let mut groups = Vec::with_capacity(ordered.len());
        for group in ordered {
            let touched = group
                .transactions
                .iter()
                .map(|transaction| transaction.surface)
                .collect::<Vec<_>>();
            let removed = group.removed_surfaces.clone();
            match self.surface_content_stream.admit(group, touched, removed)? {
                SurfaceContentAdmission::Ready(group) => {
                    for owner in authority_group_present_owners(&group)? {
                        self.surface_content_stream.begin(owner)?;
                    }
                    groups.push(group);
                }
                SurfaceContentAdmission::Deferred => {}
            }
        }
        Ok(LiveProductionAuthorityBatch {
            groups,
            dma_buf_registrations: batch.dma_buf_registrations.clone(),
            fence_registrations: batch.fence_registrations.clone(),
            released_dma_bufs: batch.released_dma_bufs.clone(),
            released_fences: batch.released_fences.clone(),
        })
    }

    pub(super) fn finish_surface_content_owner(
        &mut self,
        owner: SurfaceTransactionKey,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let ready = self.surface_content_stream.finish(owner)?;
        let count = ready.len();
        self.released_surface_content.extend(ready);
        Ok(count)
    }

    fn finish_surface_content_transaction(
        &mut self,
        transaction: TransactionId,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let Some(owner) = self
            .surface_content_stream
            .owner_for_transaction(transaction)
        else {
            return Ok(0);
        };
        self.finish_surface_content_owner(owner)
    }

    pub(super) fn observe_content_ordered_resource_releases(
        &mut self,
        batch: &LiveProductionAuthorityBatch,
    ) {
        for handle in &batch.released_dma_bufs {
            if self.pending_surface_content_references_dma_buf(*handle) {
                self.deferred_content_dma_buf_releases.insert(*handle);
            } else {
                let _ = self
                    .presentation_feedback
                    .resources_mut()
                    .release_source(*handle);
            }
        }
        for handle in &batch.released_fences {
            if self.pending_surface_content_references_fence(*handle) {
                self.deferred_content_fence_releases.insert(*handle);
            } else {
                let _ = self
                    .presentation_feedback
                    .resources_mut()
                    .release_fence(*handle);
            }
        }

        let dma_bufs = self
            .deferred_content_dma_buf_releases
            .iter()
            .filter(|handle| !self.pending_surface_content_references_dma_buf(**handle))
            .copied()
            .collect::<Vec<_>>();
        for handle in dma_bufs {
            self.deferred_content_dma_buf_releases.remove(&handle);
            let _ = self
                .presentation_feedback
                .resources_mut()
                .release_source(handle);
        }
        let fences = self
            .deferred_content_fence_releases
            .iter()
            .filter(|handle| !self.pending_surface_content_references_fence(**handle))
            .copied()
            .collect::<Vec<_>>();
        for handle in fences {
            self.deferred_content_fence_releases.remove(&handle);
            let _ = self
                .presentation_feedback
                .resources_mut()
                .release_fence(handle);
        }
    }

    fn pending_surface_content_references_dma_buf(&self, handle: BufferHandle) -> bool {
        self.surface_content_stream
            .deferred_items()
            .chain(self.released_surface_content.iter())
            .flat_map(|group| group.present_submissions.iter())
            .any(|submission| submission.buffer == handle)
    }

    fn pending_surface_content_references_fence(&self, handle: FenceHandle) -> bool {
        self.surface_content_stream
            .deferred_items()
            .chain(self.released_surface_content.iter())
            .any(|group| {
                group.present_submissions.iter().any(|submission| {
                    submission.acquire_fence == Some(handle)
                        || submission.idle_fence == Some(handle)
                }) || group.software_present_submissions.iter().any(|submission| {
                    submission.acquire_fence == Some(handle)
                        || submission.idle_fence == Some(handle)
                })
            })
    }

    pub(super) fn enqueue_software_presents(
        &mut self,
        groups: &[LiveProductionAuthorityGroup],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let submissions = groups
            .iter()
            .flat_map(|group| group.software_present_submissions.iter().copied())
            .collect::<Vec<_>>();
        if submissions.is_empty() {
            return Ok(());
        }
        for submission in &submissions {
            self.presentation_feedback.resources_mut().begin_software(
                submission.transaction,
                submission.acquire_fence,
                submission.idle_fence,
            )?;
            if !self
                .presentation_feedback
                .resources_mut()
                .poll_acquire_fence(submission.transaction)?
            {
                return Err("software Present acquire fence is not ready".into());
            }
        }
        self.software_presents_unframed.push_back(submissions);
        Ok(())
    }

    pub fn drain_retired_software_presents_into(
        &mut self,
        retired: &mut Vec<LiveProductionRetiredSoftwarePresent>,
    ) -> Result<(), &'static str> {
        if self.retired_software_presents_overflowed {
            return Err("production software Present retirement queue overflowed");
        }
        retired.extend(self.retired_software_presents.drain(..));
        Ok(())
    }

    pub(super) fn reject_software_presents(&mut self) {
        self.reject_software_present_frames();
    }

    pub fn reject_gpu_presentation(&mut self, transaction: TransactionId) {
        if let Ok(outcome) = self
            .presentation_feedback
            .reject_skip_at_last_display(transaction)
        {
            self.route_present_feedback(outcome);
        }
        match self.finish_surface_content_transaction(transaction) {
            Ok(released) if released != 0 => tracing::debug!(
                transaction = transaction.raw(),
                groups = released,
                "rejected Present released ordered surface content"
            ),
            Ok(_) => {}
            Err(error) => tracing::error!(
                transaction = transaction.raw(),
                %error,
                "failed to release rejected Present content owner"
            ),
        }
    }

    pub fn release_layout_deferred_presentations(&mut self) {
        // `presentation_order` is the last projection applied at the Engine
        // boundary. It deliberately lags pre-admission recovery state until
        // the CPU snapshot transaction makes the surface scene-visible.
        self.present_scheduler.release_layout_deferred_for_surfaces(
            &self.presentation_order,
            self.production.committed_surfaces(),
        );
    }

    pub fn commit_layout_epoch(&mut self, epoch: TransactionId) -> usize {
        self.present_scheduler.commit_layout_epoch(epoch)
    }

    pub fn abort_layout_epoch(
        &mut self,
        epoch: TransactionId,
    ) -> crate::LiveProductionLayoutRollbackReport {
        let report = self.present_scheduler.abort_layout_epoch(epoch);
        for transaction in &report.rejected {
            let transaction = *transaction;
            self.reject_gpu_presentation(transaction);
        }
        report
    }

    pub fn route_present_feedback(&mut self, outcome: crate::LivePresentFeedbackOutcome) {
        if self.present_feedback.len() == PRESENT_FEEDBACK_CAPACITY {
            self.present_feedback_overflowed = true;
            return;
        }
        self.present_feedback.push_back(outcome);
    }

    pub(super) fn release_removed_presentations(
        &mut self,
        removed_surfaces: &[SurfaceId],
        mut native_scanout: Option<&mut LiveProductionNativeScanout>,
    ) {
        for surface in removed_surfaces {
            if let Some(displayed) = self.displayed_surfaces.remove(surface) {
                let renderer_released = native_scanout.as_deref_mut().map_or(true, |native| {
                    native
                        .evict_renderer_image(displayed.layer.image_id)
                        .is_ok()
                });
                if renderer_released
                    && let Some(transaction) = displayed.retained_transaction
                    && let Ok(outcome) = self.presentation_feedback.idle_displayed(transaction)
                {
                    self.route_present_feedback(outcome);
                }
            }
        }
    }

    pub fn drain_present_feedback_into(
        &mut self,
        outcomes: &mut Vec<crate::LivePresentFeedbackOutcome>,
    ) -> Result<(), &'static str> {
        if self.present_feedback_overflowed {
            return Err("production Present feedback queue overflowed");
        }
        outcomes.extend(self.present_feedback.drain(..));
        Ok(())
    }

    pub fn shutdown_presentations(&mut self) -> crate::LivePresentationDisconnectReport {
        self.reject_software_presents();
        let queued = self.present_scheduler.drain_transactions();
        for transaction in queued {
            self.reject_gpu_presentation(transaction);
        }
        if let Some(submitted) = self.present_scheduler.take_submitted() {
            self.reject_gpu_presentation(submitted.transaction);
        }
        if let Some(rendering) = self.present_scheduler.take_rendering() {
            self.reject_gpu_presentation(rendering.transaction);
        }
        let discarded = self
            .surface_content_stream
            .discard()
            .saturating_add(self.released_surface_content.len());
        self.released_surface_content.clear();
        self.deferred_content_dma_buf_releases.clear();
        self.deferred_content_fence_releases.clear();
        if discarded != 0 {
            tracing::debug!(
                deferred_groups = discarded,
                "discarded fenced surface authority during presentation shutdown"
            );
        }
        let displayed = std::mem::take(&mut self.displayed_surfaces);
        for transaction in displayed
            .into_values()
            .filter_map(|displayed| displayed.retained_transaction)
        {
            if let Ok(outcome) = self.presentation_feedback.idle_displayed(transaction) {
                self.route_present_feedback(outcome);
            }
        }

        self.presentation_feedback.disconnect()
    }

    pub fn prepare_authority_transactions(
        &mut self,
        transaction_id: TransactionId,
        transactions: &[SurfaceTransaction],
        removed_surfaces: &[SurfaceId],
    ) -> Result<LiveProductionPreparedAuthorityBatch, Box<dyn std::error::Error>> {
        self.observe_surface_metadata(transactions, removed_surfaces);
        let intake = AuthorityTransactionIntake::new(transaction_id, transactions.to_vec())
            .with_surface_removals(removed_surfaces.to_vec());
        let authority_commits = self
            .production
            .commit_authority_batches(std::slice::from_ref(&intake));
        self.rebuild_input_layers();
        Ok(LiveProductionPreparedAuthorityBatch {
            authority_commits,
            layer_templates: self.compositor_layer_templates(),
        })
    }

    pub fn prepare_authority_groups(
        &mut self,
        groups: &[LiveProductionAuthorityGroup],
    ) -> Result<LiveProductionPreparedAuthorityBatch, Box<dyn std::error::Error>> {
        let mut intakes = Vec::with_capacity(groups.len());
        for group in groups {
            group.validate()?;
            self.observe_surface_metadata(&group.transactions, &group.removed_surfaces);
            intakes.push(
                AuthorityTransactionIntake::new(group.transaction, group.transactions.clone())
                    .with_surface_removals(group.removed_surfaces.clone()),
            );
        }
        let authority_commits = self.production.commit_authority_batches(&intakes);
        self.rebuild_input_layers();
        Ok(LiveProductionPreparedAuthorityBatch {
            authority_commits,
            layer_templates: self.compositor_layer_templates(),
        })
    }

    pub fn run_prepared_authority_transactions(
        &mut self,
        prepared: LiveProductionPreparedAuthorityBatch,
        event_count: usize,
        mut native_scanout: Option<&mut LiveProductionNativeScanout>,
        native_frames: Option<Vec<LiveProductionComposedFrame>>,
        wm_update: Option<WmTransactionUpdate>,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let output_count = self.outputs.output_count();
        let production = &self.production;
        let outputs = &mut self.outputs;
        let mut native_frames = native_frames.unwrap_or_default().into_iter();
        let mut adapter = crate::LiveProductionOutputRuntimeAdapter::new(
            output_count,
            |index, committed: &[CommittedSurfaceState]| -> Result<_, Box<dyn std::error::Error>> {
                let output = outputs
                    .values_mut()
                    .nth(index)
                    .ok_or("production output index was not registered")?;
                output
                    .runtime
                    .assembly_mut()
                    .replace_committed_surfaces(committed.to_vec());
                let input = compositor_tick_input(
                    &prepared.layer_templates,
                    event_count,
                    prepared.authority_commits.clone(),
                    wm_update.clone(),
                );
                Ok(match native_scanout.as_deref_mut() {
                    Some(native_scanout) => {
                        if let Some(frame) = native_frames.next() {
                            native_scanout.queue_frame(index, frame);
                        }
                        if output.runtime.rendered_primary_plane_scanout_in_flight() {
                            output.runtime.run_tick(input)?
                        } else {
                            native_scanout.run_tick(index, &mut output.runtime, input)?
                        }
                    }
                    None => output.runtime.run_tick(input)?,
                })
            },
        );
        production
            .run_outputs(&mut adapter)?
            .into_iter()
            .next()
            .ok_or_else(|| "persistent backend runtime has no outputs".into())
    }

    pub fn run_authority_transactions(
        &mut self,
        run: LiveAuthorityTransactionRun<'_>,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let LiveAuthorityTransactionRun {
            groups,
            event_count,
            native_scanout,
            native_frames,
            wm_update,
        } = run;
        let prepared = self.prepare_authority_groups(groups)?;
        self.run_prepared_authority_transactions(
            prepared,
            event_count,
            native_scanout,
            native_frames,
            wm_update,
        )
    }

    pub fn committed_surfaces(&self) -> &[CommittedSurfaceState] {
        self.production.committed_surfaces()
    }

    pub fn input_layers(&self) -> &[LayerSnapshot] {
        &self.input_layers
    }
}
