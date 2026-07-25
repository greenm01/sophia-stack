use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LiveProductionNativeSuspendReport {
    pub drained: bool,
    pub abandoned_scanouts: usize,
    pub skipped_present: Option<TransactionId>,
}

impl LiveProductionVisualRuntime {
    pub fn suspend_native_scanout(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
        outputs: &[sophia_engine::HeadlessOutput],
        timeout: Duration,
    ) -> Result<LiveProductionNativeSuspendReport, Box<dyn std::error::Error>> {
        if self.drain_native_scanout_until(native_scanout, timeout)? {
            self.detach_native_scanout(outputs, true)
        } else {
            self.detach_native_scanout(outputs, false)
        }
    }

    pub fn suspend_revoked_native_scanout(
        &mut self,
        outputs: &[sophia_engine::HeadlessOutput],
    ) -> Result<LiveProductionNativeSuspendReport, Box<dyn std::error::Error>> {
        self.detach_native_scanout(outputs, false)
    }

    fn detach_native_scanout(
        &mut self,
        outputs: &[sophia_engine::HeadlessOutput],
        drained: bool,
    ) -> Result<LiveProductionNativeSuspendReport, Box<dyn std::error::Error>> {
        let abandoned_scanouts = self.outputs.native_scanout_in_flight_count();
        let skipped_present = self
            .present_scheduler
            .take_submitted()
            .map(|submitted| submitted.transaction);
        if let Some(transaction) = skipped_present {
            self.reject_gpu_presentation(transaction, 0, 0);
        }
        self.outputs = LiveProductionOutputRuntimeSet::new(
            outputs,
            self.production.committed_surfaces(),
            None,
            None,
        )?;
        Ok(LiveProductionNativeSuspendReport {
            drained,
            abandoned_scanouts,
            skipped_present,
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
        if let Some((transaction, frame)) = self.retained_mixed_frame()? {
            let output_count = self.outputs.output_count();
            for index in 0..output_count {
                if index + 1 == output_count {
                    native_scanout.queue_mixed_frame(index, transaction, frame);
                    break;
                }
                native_scanout.queue_mixed_frame(
                    index,
                    transaction,
                    crate::try_clone_mixed_frame(&frame)?,
                );
            }
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

    pub fn run_native_idle(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        self.run_native_idle_with_primary_reservation(native_scanout, false)
    }

    pub(super) fn run_native_idle_with_primary_reservation(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
        reserve_primary: bool,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let transactions = self.layers.values().cloned().collect::<Vec<_>>();
        let output_count = self.outputs.output_count();
        let primary_index = self
            .outputs
            .primary_output()
            .and_then(|output| self.outputs.output_index(output))
            .ok_or("persistent backend runtime has no primary output")?;
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
                if (reserve_primary && index == primary_index)
                    || output.runtime.rendered_primary_plane_scanout_in_flight()
                    || output
                        .runtime
                        .rendered_primary_plane_scanout_cleanup_pending()
                    || !native_scanout.pending_frame(index)
                {
                    return Ok(None);
                }
                Ok(Some(native_scanout.run_tick(
                    index,
                    &mut output.runtime,
                    compositor_tick_input(&transactions, 0, Vec::new(), None),
                )?))
            },
        );
        production
            .run_outputs(&mut adapter)?
            .into_iter()
            .flatten()
            .next()
            .ok_or_else(|| "persistent native idle tick had no pending output".into())
    }

    pub fn retire_native_scanout(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
    ) -> Result<Option<LiveProductionRetiredPresent>, Box<dyn std::error::Error>> {
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
                native_scanout.retire_ready_and_retry_cleanup(index, &mut output.runtime)
            },
        );
        let _ = production.run_outputs(&mut adapter)?;
        if let Some(primary) = self.outputs.primary_output()
            && let Some((ust, msc)) = native_scanout.take_presentation_feedback(primary)
        {
            return self.finalize_gpu_page_flip(ust, msc);
        }
        Ok(None)
    }

    pub fn finalize_gpu_page_flip(
        &mut self,
        ust: u64,
        msc: u64,
    ) -> Result<Option<LiveProductionRetiredPresent>, Box<dyn std::error::Error>> {
        let Some(submitted) = self.present_scheduler.take_submitted() else {
            return Ok(None);
        };
        let (production, presentation_feedback) =
            (&mut self.production, &mut self.presentation_feedback);
        let completion =
            production
                .settle_prepared_retirement(submitted.prepared, |commit| match commit.outcome {
                    TransactionOutcome::Committed => presentation_feedback
                        .complete_flip_without_idle(submitted.transaction, ust, msc),
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
            tracing::warn!(
                transaction = completion.commit.transaction.raw(),
                outcome = ?completion.commit.outcome,
                "settled retired Present without applying its stale Engine candidate"
            );
            return Ok(None);
        }
        let source_size = Size {
            width: i32::try_from(submitted.displayed_layer.frame.width).unwrap_or(i32::MAX),
            height: i32::try_from(submitted.displayed_layer.frame.height).unwrap_or(i32::MAX),
        };
        let target = submitted.displayed_layer.placement.target;
        let clip = submitted.displayed_layer.placement.clip;
        let transaction_to_idle = replace_displayed_surface(
            &mut self.displayed_surfaces,
            submitted.surface,
            submitted.transaction,
            submitted.displayed_layer,
        );
        if let Some(transaction) = transaction_to_idle
            && let Ok(outcome) = self.presentation_feedback.idle_displayed(transaction)
        {
            self.route_present_feedback(outcome);
        }
        Ok(Some(LiveProductionRetiredPresent {
            transaction: submitted.transaction,
            surface: submitted.surface,
            source_size,
            target,
            clip,
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
