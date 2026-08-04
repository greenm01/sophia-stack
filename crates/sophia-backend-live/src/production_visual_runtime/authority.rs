use super::*;

impl LiveProductionVisualRuntime {
    pub(super) fn finish_surface_content_fence(
        &mut self,
        surface: SurfaceId,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let deferred = self.surface_content_fence.finish(surface)?;
        if deferred.is_empty() {
            return Ok(0);
        }
        let deferred = rebase_authority_groups_to_committed(
            &LiveProductionAuthorityBatch {
                groups: deferred,
                dma_buf_registrations: Vec::new(),
                fence_registrations: Vec::new(),
                released_dma_bufs: Vec::new(),
                released_fences: Vec::new(),
            },
            self.production.committed_surfaces(),
        );
        let count = deferred.len();
        self.enqueue_software_presents(&deferred)?;
        let _ = self.prepare_authority_groups(&deferred)?;
        self.outputs
            .project_committed(self.production.committed_surfaces());
        tracing::debug!(
            surface = surface.index(),
            groups = count,
            "released authority groups behind retired surface Present"
        );
        Ok(count)
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
        self.software_presents_waiting_submit.push_back(submissions);
        Ok(())
    }

    pub(super) fn mark_software_present_frame_submitted(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(submissions) = self.software_presents_waiting_submit.pop_front() else {
            return Ok(());
        };
        for submission in &submissions {
            self.presentation_feedback
                .resources_mut()
                .mark_submitted(submission.transaction)?;
        }
        self.software_presents_submitted.push_back(submissions);
        Ok(())
    }

    pub(super) fn settle_software_present_frame(
        &mut self,
        ust: u64,
        msc: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(submissions) = self.software_presents_submitted.pop_front() else {
            return Ok(());
        };
        for submission in submissions {
            let outcome =
                self.presentation_feedback
                    .complete_copy(submission.transaction, ust, msc)?;
            self.route_present_feedback(outcome);
        }
        Ok(())
    }

    pub(super) fn reject_software_presents(&mut self) {
        let transactions = self
            .software_presents_waiting_submit
            .drain(..)
            .chain(self.software_presents_submitted.drain(..))
            .flatten()
            .map(|submission| submission.transaction)
            .collect::<Vec<_>>();
        for transaction in transactions {
            if let Ok(outcome) = self
                .presentation_feedback
                .reject_skip_at_last_display(transaction)
            {
                self.route_present_feedback(outcome);
            }
        }
    }

    pub fn reject_gpu_presentation(&mut self, transaction: TransactionId) {
        if let Ok(outcome) = self
            .presentation_feedback
            .reject_skip_at_last_display(transaction)
        {
            self.route_present_feedback(outcome);
        }
    }

    pub fn release_layout_deferred_presentations(&mut self) {
        self.present_scheduler.release_layout_deferred();
    }

    pub fn abort_queued_presentations(&mut self) -> usize {
        let transactions = self.present_scheduler.drain_layout_deferred_transactions();
        let rejected = transactions.len();
        for transaction in transactions {
            self.reject_gpu_presentation(transaction);
        }
        rejected
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
        let discarded = self.surface_content_fence.discard();
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
