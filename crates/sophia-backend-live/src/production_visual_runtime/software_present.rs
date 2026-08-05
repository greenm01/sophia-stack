use super::*;

pub(super) enum LiveProductionSoftwarePresentFramePayload {
    Cpu(LiveProductionComposedFrame),
    Mixed(LiveOwnedMixedCompositionFrame),
}

pub(super) struct LiveProductionSoftwarePresentFrame {
    pub output: OutputId,
    pub payload: LiveProductionSoftwarePresentFramePayload,
    pub submissions: Vec<LiveProductionSoftwarePresentSubmission>,
}

pub(super) struct LiveProductionSoftwarePresentBinding {
    pub submissions: Vec<LiveProductionSoftwarePresentSubmission>,
    pub phase: LiveProductionSoftwarePresentFramePhase,
}

impl LiveProductionVisualRuntime {
    pub(super) fn frame_unframed_software_presents(
        &mut self,
        scene: &mut LiveProductionCpuScene,
        output_descriptors: &[HeadlessOutput],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let submissions = self
            .software_presents_unframed
            .drain(..)
            .flatten()
            .collect::<Vec<_>>();
        // Every software Present committed in this cycle owns the same
        // immutable composition, so keep the set attached to that frame.
        self.queue_software_present_frame(scene, output_descriptors, submissions)
    }

    pub(super) fn settle_unframed_software_presents_without_native(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let submissions = self
            .software_presents_unframed
            .drain(..)
            .flatten()
            .collect::<Vec<_>>();
        for submission in submissions {
            self.presentation_feedback
                .resources_mut()
                .mark_submitted(submission.transaction)?;
            let outcome = self
                .presentation_feedback
                .complete_copy(submission.transaction, 0, 0)?;
            self.route_present_feedback(outcome);
            if self.retired_software_presents.len() == PRESENT_FEEDBACK_CAPACITY {
                self.retired_software_presents_overflowed = true;
            } else {
                self.retired_software_presents
                    .push_back(LiveProductionRetiredSoftwarePresent {
                        candidate: submission.candidate,
                        source_size: submission.source_size,
                        frame: LiveProductionNativeFrameId::from_raw(0),
                        native_submission: 0,
                        ust_usec: 0,
                        msc: 0,
                    });
            }
            self.finish_surface_content_owner(submission.candidate)?;
        }
        Ok(())
    }

    pub(super) fn queue_software_present_frame(
        &mut self,
        scene: &mut LiveProductionCpuScene,
        output_descriptors: &[HeadlessOutput],
        submissions: Vec<LiveProductionSoftwarePresentSubmission>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if submissions.is_empty() {
            return Ok(());
        }
        if self
            .software_present_frames_waiting
            .len()
            .saturating_add(self.software_present_frames_bound.len())
            >= PRESENT_FEEDBACK_CAPACITY
        {
            return Err("production software Present frame queue overflowed".into());
        }
        let output = self
            .outputs
            .primary_output()
            .ok_or("software Present has no primary output")?;
        let cpu_layers = scene.presentation_layers(
            self.production.committed_surfaces(),
            &self.presentation_order,
        );
        let gpu_projection = self.present_scheduler.has_in_flight()
            || self
                .presentation_order
                .iter()
                .any(|surface| self.displayed_surfaces.contains_key(surface));
        let payload = if gpu_projection {
            let frame = self
                .retained_mixed_frame(&cpu_layers)?
                .ok_or("software Present mixed projection has no client pixels")?;
            LiveProductionSoftwarePresentFramePayload::Mixed(frame)
        } else {
            let primary_index = output_descriptors
                .iter()
                .position(|descriptor| descriptor.id == output)
                .ok_or("software Present primary output descriptor is missing")?;
            let frame = scene
                .frames_for_outputs(output_descriptors)?
                .into_iter()
                .nth(primary_index)
                .ok_or("software Present CPU frame is missing")?;
            LiveProductionSoftwarePresentFramePayload::Cpu(frame)
        };
        self.software_present_frames_waiting
            .push_back(LiveProductionSoftwarePresentFrame {
                output,
                payload,
                submissions,
            });
        Ok(())
    }

    pub(super) fn stage_software_present_frame(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
        output: OutputId,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let Some(index) = native_scanout.output_index(output) else {
            return Err("software Present targeted an unknown native output".into());
        };
        if native_scanout.pending_frame(index) {
            return Ok(false);
        }
        let Some(waiting) = self.software_present_frames_waiting.front() else {
            return Ok(false);
        };
        if waiting.output != output {
            return Ok(false);
        }
        let waiting = self
            .software_present_frames_waiting
            .pop_front()
            .expect("software Present frame front checked above");
        let frame = match waiting.payload {
            LiveProductionSoftwarePresentFramePayload::Cpu(frame) => {
                native_scanout.queue_present_cpu_frame(index, frame)?
            }
            LiveProductionSoftwarePresentFramePayload::Mixed(frame) => {
                native_scanout.queue_retained_mixed_frame(index, frame)
            }
        };
        if self
            .software_present_frames_bound
            .insert(
                frame,
                LiveProductionSoftwarePresentBinding {
                    submissions: waiting.submissions,
                    phase: LiveProductionSoftwarePresentFramePhase::Pending,
                },
            )
            .is_some()
        {
            return Err("native frame ID was reused for software Present".into());
        }
        tracing::debug!(
            output = output.raw(),
            frame = frame.raw(),
            "bound software Present work to an immutable native frame"
        );
        Ok(true)
    }

    pub(super) fn observe_software_present_frame_submitted(
        &mut self,
        frame: LiveProductionNativeFrameId,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(binding) = self.software_present_frames_bound.get_mut(&frame) else {
            return Ok(());
        };
        match reduce_software_present_frame_observation(
            frame,
            binding.phase,
            LiveProductionSoftwarePresentFrameObservation::NativeSubmitted(frame),
        ) {
            LiveProductionSoftwarePresentFrameTransition::Submitted => {
                for submission in &binding.submissions {
                    self.presentation_feedback
                        .resources_mut()
                        .mark_submitted(submission.transaction)?;
                }
                binding.phase = LiveProductionSoftwarePresentFramePhase::Submitted;
            }
            LiveProductionSoftwarePresentFrameTransition::AlreadySubmitted => {}
            _ => return Err("software Present frame submission identity is invalid".into()),
        }
        Ok(())
    }

    pub(super) fn settle_software_present_frame(
        &mut self,
        retirement: LiveProductionNativeFrameRetirement,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(binding) = self.software_present_frames_bound.remove(&retirement.frame) else {
            return Ok(());
        };
        if reduce_software_present_frame_observation(
            retirement.frame,
            binding.phase,
            LiveProductionSoftwarePresentFrameObservation::NativeRetired(retirement.frame),
        ) != LiveProductionSoftwarePresentFrameTransition::Retired
        {
            return Err("software Present frame retired before submission".into());
        }
        for submission in binding.submissions {
            let outcome = self.presentation_feedback.complete_copy(
                submission.transaction,
                retirement.ust,
                retirement.msc,
            )?;
            self.route_present_feedback(outcome);
            if self.retired_software_presents.len() == PRESENT_FEEDBACK_CAPACITY {
                self.retired_software_presents_overflowed = true;
            } else {
                self.retired_software_presents
                    .push_back(LiveProductionRetiredSoftwarePresent {
                        candidate: submission.candidate,
                        source_size: submission.source_size,
                        frame: retirement.frame,
                        native_submission: retirement.submission,
                        ust_usec: retirement.ust,
                        msc: retirement.msc,
                    });
            }
            self.finish_surface_content_owner(submission.candidate)?;
        }
        Ok(())
    }

    pub(super) fn reject_software_present_frames(&mut self) {
        let submissions = self
            .software_presents_unframed
            .drain(..)
            .flatten()
            .chain(
                self.software_present_frames_waiting
                    .drain(..)
                    .flat_map(|frame| frame.submissions),
            )
            .chain(
                std::mem::take(&mut self.software_present_frames_bound)
                    .into_values()
                    .flat_map(|binding| binding.submissions),
            )
            .collect::<Vec<_>>();
        for submission in submissions {
            if let Ok(outcome) = self
                .presentation_feedback
                .reject_skip_at_last_display(submission.transaction)
            {
                self.route_present_feedback(outcome);
            }
            if let Err(error) = self.finish_surface_content_owner(submission.candidate) {
                tracing::error!(
                    transaction = submission.transaction.raw(),
                    %error,
                    "failed to release rejected software Present content owner"
                );
            }
        }
    }
}
