use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LiveProductionNativeSuspendOutcome {
    #[default]
    Drained,
    ForcedDetachTimeout,
    ForcedDetachDrainError,
    ForcedDetachRevoked,
}

impl LiveProductionNativeSuspendOutcome {
    pub const fn reduced_name(self) -> &'static str {
        match self {
            Self::Drained => "drained",
            Self::ForcedDetachTimeout => "forced_detach_timeout",
            Self::ForcedDetachDrainError => "forced_detach_drain_error",
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

#[derive(Debug)]
pub struct LiveProductionNativeSuspendError {
    pub drain_error: Box<dyn std::error::Error>,
    pub detach_report: Option<LiveProductionNativeSuspendReport>,
    pub detach_error: Option<Box<dyn std::error::Error>>,
}

impl LiveProductionNativeSuspendError {
    pub const fn forced_detach_established(&self) -> bool {
        self.detach_report.is_some()
    }
}

impl std::fmt::Display for LiveProductionNativeSuspendError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "native scanout drain failed: {}",
            self.drain_error
        )?;
        if let Some(report) = self.detach_report {
            write!(
                formatter,
                "; forced detach completed: outcome={} abandoned_scanouts={} skipped_present={}",
                report.outcome.reduced_name(),
                report.abandoned_scanouts,
                report.skipped_present.map_or_else(
                    || "none".to_owned(),
                    |transaction| transaction.raw().to_string()
                )
            )
        } else if let Some(error) = self.detach_error.as_deref() {
            write!(formatter, "; forced detach failed: {error}")
        } else {
            write!(formatter, "; forced detach outcome is unknown")
        }
    }
}

impl std::error::Error for LiveProductionNativeSuspendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.drain_error.as_ref())
    }
}

/// Complete a native suspension after its bounded drain attempt.
///
/// This seam keeps forced-detach ordering testable without constructing real
/// DRM resources: every drain failure invokes `detach` before the original
/// error is returned.
pub fn finish_live_production_native_suspend(
    drain: Result<bool, Box<dyn std::error::Error>>,
    detach: impl FnOnce(
        LiveProductionNativeSuspendOutcome,
    ) -> Result<LiveProductionNativeSuspendReport, Box<dyn std::error::Error>>,
) -> Result<LiveProductionNativeSuspendReport, Box<dyn std::error::Error>> {
    match drain {
        Ok(true) => detach(LiveProductionNativeSuspendOutcome::Drained),
        Ok(false) => detach(LiveProductionNativeSuspendOutcome::ForcedDetachTimeout),
        Err(drain_error) => {
            match detach(LiveProductionNativeSuspendOutcome::ForcedDetachDrainError) {
                Ok(detach_report) => Err(Box::new(LiveProductionNativeSuspendError {
                    drain_error,
                    detach_report: Some(detach_report),
                    detach_error: None,
                })),
                Err(detach_error) => Err(Box::new(LiveProductionNativeSuspendError {
                    drain_error,
                    detach_report: None,
                    detach_error: Some(detach_error),
                })),
            }
        }
    }
}

fn validate_renderer_image_resume_admission(
    retained: &[LiveRendererImageId],
    handoff: Option<&[LiveRendererImageId]>,
) -> Result<(), &'static str> {
    match crate::reduce_live_renderer_image_handoff_admission(retained, handoff) {
        crate::LiveRendererImageHandoffAdmission::Ready => Ok(()),
        crate::LiveRendererImageHandoffAdmission::Missing => {
            Err("native resume omitted retained renderer images")
        }
        crate::LiveRendererImageHandoffAdmission::InvalidIdentity => {
            Err("native resume contains an invalid renderer-image identity")
        }
        crate::LiveRendererImageHandoffAdmission::DuplicateIdentity => {
            Err("native resume contains a duplicate renderer-image identity")
        }
        crate::LiveRendererImageHandoffAdmission::CoverageMismatch => {
            Err("native resume renderer-image handoff does not match the retained scene")
        }
    }
}

fn advance_renderer_image_resume(
    phase: crate::LiveRendererImageResumePhase,
    observation: crate::LiveRendererImageResumeObservation,
) -> Result<crate::LiveRendererImageResumePhase, &'static str> {
    match crate::reduce_live_renderer_image_resume_observation(phase, observation) {
        crate::LiveRendererImageResumeTransition::Advanced(next) => Ok(next),
        crate::LiveRendererImageResumeTransition::Rejected => {
            Err("native resume renderer-image lifecycle is out of order")
        }
    }
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

pub const fn reduce_live_production_abandoned_scanout_count(
    logical_runtime_owners: usize,
    physical_head_owners: usize,
) -> usize {
    logical_runtime_owners.saturating_add(physical_head_owners)
}

impl LiveProductionVisualRuntime {
    pub fn retained_renderer_image_ids(&self) -> Vec<LiveRendererImageId> {
        let mut images = self
            .displayed_surfaces
            .values()
            .map(|displayed| displayed.layer.image_id)
            .collect::<BTreeSet<_>>();
        if let Some((_, in_flight)) = self.present_scheduler.in_flight_displayed_layer() {
            images.insert(in_flight.image_id);
        }
        images.into_iter().collect()
    }

    pub fn discard_retained_renderer_images(&mut self) -> usize {
        let discarded = self.displayed_surfaces.len();
        self.displayed_surfaces.clear();
        discarded
    }

    pub fn suspend_native_scanout(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
        outputs: &[sophia_engine::HeadlessOutput],
        timeout: Duration,
    ) -> Result<LiveProductionNativeSuspendReport, Box<dyn std::error::Error>> {
        let drain = self.drain_native_scanout_until(native_scanout, timeout);
        finish_live_production_native_suspend(drain, |outcome| {
            self.detach_native_scanout(Some(native_scanout), outputs, outcome)
        })
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
        let abandoned_scanouts = reduce_live_production_abandoned_scanout_count(
            self.outputs.native_scanout_in_flight_count(),
            native_scanout
                .as_deref()
                .map_or(0, LiveProductionNativeScanout::head_scanout_in_flight_count),
        );
        let skipped_present = self
            .present_scheduler
            .take_submitted()
            .or_else(|| self.present_scheduler.take_rendering());
        self.reject_software_presents();
        if let Some(present) = skipped_present.as_ref() {
            if let Some(native_scanout) = native_scanout.as_deref_mut() {
                let _ = native_scanout.rollback_renderer_image(present.displayed_layer.image_id);
            }
            if self.reject_gpu_presentation(present.transaction) {
                self.native_suspend_present_rejections =
                    self.native_suspend_present_rejections.saturating_add(1);
            }
        }
        let invalidation_epoch = self
            .input_projections
            .iter()
            .map(|projection| projection.epoch)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .expect("presented input epoch exhausted");
        self.outputs = LiveProductionOutputRuntimeSet::new(
            outputs,
            self.production.committed_surfaces(),
            None,
            None,
        )?;
        // A suspended/revoked output no longer has a visible native
        // interaction snapshot. Do not retain routes into retired pixels.
        self.input_projections = (0..self.outputs.output_count())
            .filter_map(|index| self.outputs.output_id(index))
            .map(|output| LivePresentedInputProjection {
                output,
                epoch: invalidation_epoch,
                layers: Vec::new(),
            })
            .collect();
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
        renderer_handoff: Option<LiveProductionRendererImageHandoff>,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let retained = self.retained_renderer_image_ids();
        validate_renderer_image_resume_admission(
            &retained,
            renderer_handoff.as_ref().map(|handoff| handoff.image_ids()),
        )?;
        let mut resume_phase = crate::LiveRendererImageResumePhase::default();
        // Native initialization establishes the replacement renderer worker.
        // Its image table cannot accept the retained snapshots before then.
        let resumed_outputs = LiveProductionOutputRuntimeSet::new(
            outputs,
            self.production.committed_surfaces(),
            Some(native_scanout),
            Some(frames),
        )?;
        if !native_scanout.renderer_image_owners_initialized() {
            return Err("native resume did not initialize every renderer image owner".into());
        }
        resume_phase = advance_renderer_image_resume(
            resume_phase,
            crate::LiveRendererImageResumeObservation::OutputOwnerInitialized,
        )?;
        let restored = renderer_handoff.map_or(Ok(0), |handoff| {
            native_scanout.restore_renderer_image_handoff(handoff)
        })?;
        resume_phase = advance_renderer_image_resume(
            resume_phase,
            crate::LiveRendererImageResumeObservation::ImagesRestored,
        )?;
        if resume_phase != crate::LiveRendererImageResumePhase::Ready {
            return Err("native resume renderer-image lifecycle did not become ready".into());
        }
        // Publish the replacement output runtime only after its retained IDs
        // belong to the new renderer generation.
        self.outputs = resumed_outputs;
        self.publish_presented_input_layers(native_scanout);
        if let Some(frame) = self.retained_mixed_frame(&[])? {
            let primary = self
                .outputs
                .primary_output()
                .ok_or("persistent backend runtime has no primary output")?;
            native_scanout.queue_retained_mixed_frame(primary, frame)?;
        }
        Ok(restored)
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
        while (self.native_scanout_in_flight()
            || native_scanout.any_head_scanout_in_flight()
            || native_scanout.any_head_cleanup_pending())
            && Instant::now() < deadline
        {
            self.retire_native_scanout_for_drain(native_scanout)?;
            std::thread::sleep(Duration::from_millis(5));
        }
        if self.native_scanout_in_flight()
            || native_scanout.any_head_scanout_in_flight()
            || native_scanout.any_head_cleanup_pending()
        {
            return Ok(false);
        }
        let output_count = self.outputs.output_count();
        let production = &self.production;
        let outputs = &mut self.outputs;
        let mut adapter = crate::LiveProductionOutputRuntimeAdapter::new(
            output_count,
            |index, committed: &[CommittedSurfaceState]| -> Result<_, Box<dyn std::error::Error>> {
                // The adapter counts logical outputs; the scanout is addressed by
                // output identity.
                let output_id = outputs
                    .output_id(index)
                    .ok_or("production output index was not registered")?;
                let output = outputs
                    .values_mut()
                    .nth(index)
                    .ok_or("production output index was not registered")?;
                output
                    .runtime
                    .assembly_mut()
                    .replace_committed_surfaces(committed.to_vec());
                native_scanout.release_displayed_output(output_id, &mut output.runtime)
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
        let index = self
            .outputs
            .output_index(selected_output)
            .ok_or("frame service selected an unknown output")?;
        if !native_scanout.pending_frame(selected_output) {
            self.stage_software_present_frame(native_scanout, selected_output)?;
        }
        // Both views of the scene are taken after staging, from the same moment.
        // Reading the templates first let staging change the committed set
        // underneath them, and a template whose surface no longer matches its
        // committed state is rejected as an invalid surface -- which is what this
        // tick did the first time a mirror group ever reached it.
        let (layer_templates, committed) = self.scene_views();
        let output = self
            .outputs
            .values_mut()
            .nth(index)
            .ok_or("production output index was not registered")?;
        output
            .runtime
            .assembly_mut()
            .replace_committed_surfaces(committed);
        // Per-head state as well as the runtime's, or this guard is blind to a
        // mirror group and would tick an output whose heads are still in flight.
        if output.runtime.rendered_primary_plane_scanout_in_flight()
            || native_scanout.output_in_flight(selected_output)
            || output
                .runtime
                .rendered_primary_plane_scanout_cleanup_pending()
            || native_scanout.output_cleanup_pending(selected_output)
            || !native_scanout.pending_frame(selected_output)
        {
            return Err("frame service selected an output that is not ready".into());
        }
        let report = native_scanout.run_tick(
            selected_output,
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
                    .submitted_frame(selected_output)
                    .ok_or("native submit did not retain its frame identity")?;
                self.observe_software_present_frame_submitted(submitted)?;
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
            if let Some(retired) =
                self.retire_native_scanout_output_with_mode(native_scanout, output, false)?
            {
                retired_present = Some(retired);
            }
        }
        Ok(retired_present)
    }

    fn retire_native_scanout_for_drain(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
    ) -> Result<Option<LiveProductionRetiredPresent>, Box<dyn std::error::Error>> {
        let outputs = (0..self.outputs.output_count())
            .filter_map(|index| self.outputs.output_id(index))
            .collect::<Vec<_>>();
        let mut retired_present = None;
        for output in outputs {
            if let Some(retired) =
                self.retire_native_scanout_output_with_mode(native_scanout, output, true)?
            {
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
        self.retire_native_scanout_output_with_mode(native_scanout, selected_output, false)
    }

    fn retire_native_scanout_output_with_mode(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
        selected_output: OutputId,
        draining: bool,
    ) -> Result<Option<LiveProductionRetiredPresent>, Box<dyn std::error::Error>> {
        let index = self
            .outputs
            .output_index(selected_output)
            .ok_or("frame service selected an unknown retirement output")?;
        // Authority and repaint cycles may submit a staged frame between
        // frame-service passes. The scanout owner retains that exact identity
        // until retirement, so observe it before consuming the callback.
        if let Some(submitted) = native_scanout.submitted_frame(selected_output) {
            self.observe_software_present_frame_submitted(submitted)?;
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
        if draining {
            native_scanout.retire_ready_for_drain(selected_output, &mut output.runtime)?;
        } else {
            native_scanout.retire_ready_and_retry_cleanup(selected_output, &mut output.runtime)?;
        }
        self.publish_presented_input_layers(native_scanout);
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
        if completion.commit.outcome != TransactionOutcome::Committed {
            self.present_rejections = self.present_rejections.saturating_add(1);
        }
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
