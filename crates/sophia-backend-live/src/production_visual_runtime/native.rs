use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LiveProductionNativeSuspendOutcome {
    #[default]
    Drained,
    ForcedDetachTimeout,
    ForcedDetachRevoked,
}

impl LiveProductionNativeSuspendOutcome {
    pub const fn reduced_name(self) -> &'static str {
        match self {
            Self::Drained => "drained",
            Self::ForcedDetachTimeout => "forced_detach_timeout",
            Self::ForcedDetachRevoked => "forced_detach_revoked",
        }
    }

    pub const fn drained(self) -> bool {
        matches!(self, Self::Drained)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LiveProductionNativeSuspendReport {
    pub outcome: LiveProductionNativeSuspendOutcome,
    pub abandoned_scanouts: usize,
    pub skipped_present: Option<TransactionId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionNativeRetirementOwner {
    IndependentFrame,
    SubmittedDmaPresent,
    InvalidDmaOwnership,
}

pub fn reduce_live_production_native_retirement_owner(
    retired_frame: LiveProductionNativeFrameId,
    retired_content: LiveProductionScanoutContent,
    submitted_dma_frame: Option<LiveProductionNativeFrameId>,
) -> LiveProductionNativeRetirementOwner {
    if retired_content.frame() != retired_frame {
        return LiveProductionNativeRetirementOwner::InvalidDmaOwnership;
    }
    match (retired_content, submitted_dma_frame) {
        (LiveProductionScanoutContent::MixedPresent { .. }, Some(submitted))
            if submitted == retired_frame =>
        {
            LiveProductionNativeRetirementOwner::SubmittedDmaPresent
        }
        (LiveProductionScanoutContent::MixedPresent { .. }, _) => {
            LiveProductionNativeRetirementOwner::InvalidDmaOwnership
        }
        (_, _) => LiveProductionNativeRetirementOwner::IndependentFrame,
    }
}

impl LiveProductionVisualRuntime {
    pub fn suspend_native_scanout(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
        outputs: &[sophia_engine::HeadlessOutput],
        timeout: Duration,
    ) -> Result<LiveProductionNativeSuspendReport, Box<dyn std::error::Error>> {
        if self.drain_native_scanout_until(native_scanout, timeout)? {
            self.detach_native_scanout(
                Some(native_scanout),
                outputs,
                LiveProductionNativeSuspendOutcome::Drained,
            )
        } else {
            self.detach_native_scanout(
                Some(native_scanout),
                outputs,
                LiveProductionNativeSuspendOutcome::ForcedDetachTimeout,
            )
        }
    }

    pub fn suspend_revoked_native_scanout(
        &mut self,
        outputs: &[sophia_engine::HeadlessOutput],
    ) -> Result<LiveProductionNativeSuspendReport, Box<dyn std::error::Error>> {
        self.detach_native_scanout(
            None,
            outputs,
            LiveProductionNativeSuspendOutcome::ForcedDetachRevoked,
        )
    }

    fn detach_native_scanout(
        &mut self,
        mut native_scanout: Option<&mut LiveProductionNativeScanout>,
        outputs: &[sophia_engine::HeadlessOutput],
        outcome: LiveProductionNativeSuspendOutcome,
    ) -> Result<LiveProductionNativeSuspendReport, Box<dyn std::error::Error>> {
        let abandoned_scanouts = self.outputs.native_scanout_in_flight_count();
        let skipped_present = self
            .present_scheduler
            .take_submitted()
            .or_else(|| self.present_scheduler.take_rendering());
        self.reject_software_presents();
        if let Some(present) = skipped_present.as_ref() {
            if let Some(native_scanout) = native_scanout.as_deref_mut() {
                let _ = native_scanout.rollback_renderer_image(present.displayed_layer.image_id);
            }
            self.reject_gpu_presentation(present.transaction);
        }
        self.outputs = LiveProductionOutputRuntimeSet::new(
            outputs,
            self.production.committed_surfaces(),
            None,
            None,
        )?;
        Ok(LiveProductionNativeSuspendReport {
            outcome,
            abandoned_scanouts,
            skipped_present: skipped_present.map(|present| present.transaction),
        })
    }

    pub fn resume_native_scanout(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
        outputs: &[sophia_engine::HeadlessOutput],
        frames: Vec<LiveProductionComposedFrame>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.outputs = LiveProductionOutputRuntimeSet::new(
            outputs,
            self.production.committed_surfaces(),
            Some(native_scanout),
            Some(frames),
        )?;
        if let Some(frame) = self.retained_mixed_frame(&[])? {
            let primary = self
                .outputs
                .primary_output()
                .ok_or("persistent backend runtime has no primary output")?;
            let primary_index = self
                .outputs
                .output_index(primary)
                .ok_or("persistent backend primary output was not registered")?;
            native_scanout.queue_retained_mixed_frame(primary_index, frame);
        }
        Ok(())
    }

    pub fn drain_native_scanout(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
        timeout: Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !self.drain_native_scanout_until(native_scanout, timeout)? {
            return Err("persistent native scanout remained in flight during teardown".into());
        }
        Ok(())
    }

    fn drain_native_scanout_until(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
        timeout: Duration,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let deadline = Instant::now() + timeout;
        while self.native_scanout_in_flight() && Instant::now() < deadline {
            self.retire_native_scanout(native_scanout)?;
            std::thread::sleep(Duration::from_millis(5));
        }
        if self.native_scanout_in_flight() {
            return Ok(false);
        }
        let output_count = self.outputs.output_count();
        let production = &self.production;
        let outputs = &mut self.outputs;
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
                native_scanout.release_displayed_output(index, &mut output.runtime)
            },
        );
        let _ = production.run_outputs(&mut adapter)?;
        Ok(true)
    }

    pub(super) fn run_native_pending_output(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
        selected_output: OutputId,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let layer_templates = self.compositor_layer_templates();
        let index = self
            .outputs
            .output_index(selected_output)
            .ok_or("frame service selected an unknown output")?;
        if !native_scanout.pending_frame(index) {
            self.stage_software_present_frame(native_scanout, selected_output)?;
        }
        let committed = self.production.committed_surfaces().to_vec();
        let output = self
            .outputs
            .values_mut()
            .nth(index)
            .ok_or("production output index was not registered")?;
        output
            .runtime
            .assembly_mut()
            .replace_committed_surfaces(committed);
        if output.runtime.rendered_primary_plane_scanout_in_flight()
            || output
                .runtime
                .rendered_primary_plane_scanout_cleanup_pending()
            || !native_scanout.pending_frame(index)
        {
            return Err("frame service selected an output that is not ready".into());
        }
        let report = native_scanout.run_tick(
            index,
            &mut output.runtime,
            compositor_tick_input(&layer_templates, 0, Vec::new(), None),
        )?;
        use crate::LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus as Status;
        match report
            .rendered_primary_plane_scanout_submit
            .map(|submit| submit.status)
        {
            Some(Status::SubmittedWaitingForPageFlip) => {
                if let Some(transaction) = self.present_scheduler.promote_rendering_to_submitted() {
                    native_scanout.discard_presentation_feedback(self.outputs.primary_output());
                    self.presentation_feedback
                        .resources_mut()
                        .mark_submitted(transaction)?;
                }
                let submitted = native_scanout
                    .submitted_content(index)
                    .ok_or("native submit did not retain its frame identity")?;
                self.observe_software_present_frame_submitted(submitted.frame())?;
            }
            Some(Status::ScanoutExportPending) | None => {}
            Some(Status::AlreadyInFlight | Status::CleanupPending) => {}
            Some(_) => {
                if let Some(rendering) = self.present_scheduler.take_rendering() {
                    native_scanout.rollback_renderer_image(rendering.displayed_layer.image_id)?;
                    self.reject_gpu_presentation(rendering.transaction);
                }
            }
        }
        Ok(report)
    }

    pub fn retire_native_scanout(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
    ) -> Result<Option<LiveProductionRetiredPresent>, Box<dyn std::error::Error>> {
        let outputs = (0..self.outputs.output_count())
            .filter_map(|index| self.outputs.output_id(index))
            .collect::<Vec<_>>();
        let mut retired_present = None;
        for output in outputs {
            if let Some(retired) = self.retire_native_scanout_output(native_scanout, output)? {
                retired_present = Some(retired);
            }
        }
        Ok(retired_present)
    }

    pub(super) fn retire_native_scanout_output(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
        selected_output: OutputId,
    ) -> Result<Option<LiveProductionRetiredPresent>, Box<dyn std::error::Error>> {
        let index = self
            .outputs
            .output_index(selected_output)
            .ok_or("frame service selected an unknown retirement output")?;
        // Authority and repaint cycles may submit a staged frame between
        // frame-service passes. The scanout owner retains that exact identity
        // until retirement, so observe it before consuming the callback.
        if let Some(submitted) = native_scanout.submitted_content(index) {
            self.observe_software_present_frame_submitted(submitted.frame())?;
        }
        let committed = self.production.committed_surfaces().to_vec();
        let output = self
            .outputs
            .values_mut()
            .nth(index)
            .ok_or("production output index was not registered")?;
        output
            .runtime
            .assembly_mut()
            .replace_committed_surfaces(committed);
        native_scanout.retire_ready_and_retry_cleanup(index, &mut output.runtime)?;
        if self.outputs.primary_output() == Some(selected_output)
            && let Some(retirement) = native_scanout.take_presentation_feedback(selected_output)
        {
            return self.finalize_gpu_page_flip(native_scanout, retirement);
        }
        Ok(None)
    }

    pub fn finalize_gpu_page_flip(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
        retirement: LiveProductionNativeFrameRetirement,
    ) -> Result<Option<LiveProductionRetiredPresent>, Box<dyn std::error::Error>> {
        let ust = retirement.ust;
        let msc = retirement.msc;
        match reduce_live_production_native_retirement_owner(
            retirement.frame,
            retirement.content,
            self.present_scheduler.submitted_frame(),
        ) {
            LiveProductionNativeRetirementOwner::IndependentFrame => {
                // A callback and the next submission may share one backend
                // tick. Retire only work bound to the callback's frame.
                self.settle_software_present_frame(retirement)?;
                return Ok(None);
            }
            LiveProductionNativeRetirementOwner::SubmittedDmaPresent => {}
            LiveProductionNativeRetirementOwner::InvalidDmaOwnership => {
                return Err(
                    "DMA Present retired on a native frame with different ownership".into(),
                );
            }
        }
        let submitted = self
            .present_scheduler
            .take_submitted()
            .ok_or("native retirement lost its submitted DMA Present")?;
        if !matches!(
            retirement.content,
            LiveProductionScanoutContent::MixedPresent { transaction, .. }
                if transaction == submitted.transaction
        ) {
            return Err("DMA Present retired on a native frame with different ownership".into());
        }
        // The page flip is the commit point for the compositor copy. Promote
        // its staged image before releasing the client source or emitting any
        // protocol feedback.
        if native_scanout.promote_renderer_image(submitted.displayed_layer.image_id)? == 0 {
            return Err("retired Present lost its staged renderer snapshot".into());
        }
        let (production, presentation_feedback) =
            (&mut self.production, &mut self.presentation_feedback);
        let completion = production
            .settle_prepared_retirement(submitted.prepared, |commit| match commit.outcome {
                TransactionOutcome::Committed => {
                    presentation_feedback.complete_copy(submitted.transaction, ust, msc)
                }
                TransactionOutcome::RejectedStaleSurface
                | TransactionOutcome::RejectedInvalidSurface
                | TransactionOutcome::TimedOut => {
                    presentation_feedback.reject_skip(submitted.transaction, ust, msc)
                }
            })
            .map_err(|error| format!("page flip protocol settlement failed: {error:?}"))?;
        self.outputs
            .project_committed(&completion.committed_surfaces);
        self.route_present_feedback(completion.evidence);
        let deferred_groups = self.finish_surface_content_owner(submitted.candidate)?;
        if deferred_groups != 0 {
            tracing::debug!(
                transaction = submitted.transaction.raw(),
                surface = submitted.surface.index(),
                deferred_groups,
                "retired Present released its ordered surface authority backlog"
            );
        }
        if completion.commit.outcome != TransactionOutcome::Committed {
            native_scanout.evict_renderer_image(submitted.displayed_layer.image_id)?;
            self.settle_software_present_frame(retirement)?;
            tracing::warn!(
                transaction = completion.commit.transaction.raw(),
                outcome = ?completion.commit.outcome,
                "settled retired Present without applying its stale Engine candidate"
            );
            return Ok(None);
        }
        self.rebuild_input_layers();
        let source_size = submitted.displayed_layer.size;
        let target = submitted.displayed_layer.placement.target;
        let clip = submitted.displayed_layer.placement.clip;
        let replaced = replace_displayed_surface(
            &mut self.displayed_surfaces,
            submitted.surface,
            submitted.displayed_layer,
        );
        if let Some(replaced) = replaced {
            native_scanout.evict_renderer_image(replaced.layer.image_id)?;
        }
        self.settle_software_present_frame(retirement)?;
        Ok(Some(LiveProductionRetiredPresent {
            candidate: submitted.candidate,
            transaction: submitted.transaction,
            surface: submitted.surface,
            source_size,
            target,
            clip,
            ust_usec: ust,
            msc,
        }))
    }

    pub fn native_scanout_in_flight(&self) -> bool {
        self.outputs.native_scanout_in_flight()
    }

    pub fn native_cleanup_pending(&self) -> bool {
        self.outputs.native_cleanup_pending()
    }

    pub fn native_diagnostic(&self) -> String {
        self.outputs.diagnostic()
    }
}
