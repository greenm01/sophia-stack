use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LiveProductionCursorPresentation {
    HardwarePlane,
    Software(Option<Point>),
}

impl LiveProductionCursorPresentation {
    pub const fn composition_position(self) -> Option<Point> {
        match self {
            Self::HardwarePlane => None,
            Self::Software(position) => position,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LiveProductionVisualDiagnostics {
    pub present_queued: bool,
    pub live_sources: usize,
    pub live_fences: usize,
    pub live_presentations: usize,
    pub acquire_waits: usize,
    pub controlled_rejections: usize,
}

impl LiveProductionVisualRuntime {
    pub fn diagnostics(&self) -> LiveProductionVisualDiagnostics {
        LiveProductionVisualDiagnostics {
            present_queued: self.present_scheduler.has_queued(),
            live_sources: self.presentation_feedback.resources().source_count(),
            live_fences: self.presentation_feedback.resources().fence_count(),
            live_presentations: self.presentation_feedback.resources().presentation_count(),
            acquire_waits: self.present_scheduler.acquire_waits(),
            controlled_rejections: self.present_scheduler.controlled_rejections(),
        }
    }

    pub fn replace_output_projection(
        &mut self,
        index: usize,
        committed: Vec<CommittedSurfaceState>,
    ) -> bool {
        self.outputs.replace_output_projection(index, committed)
    }

    pub fn output_count(&self) -> usize {
        self.outputs.output_count()
    }

    pub fn output_committed(&self, index: usize) -> Option<&[CommittedSurfaceState]> {
        self.outputs.output_committed(index)
    }
}

#[derive(Debug)]
pub struct LiveProductionRetiredPresent {
    pub transaction: TransactionId,
    pub surface: SurfaceId,
    pub source_size: Size,
    pub target: Rect,
}

#[derive(Debug)]
pub struct LiveProductionNativeServiceReport {
    pub tick: Option<LiveBackendRuntimeTickReport>,
    pub retired_present: Option<LiveProductionRetiredPresent>,
    pub retirement_polled: bool,
    pub present_polled: bool,
    pub pending_frame_polled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveProductionOutputServiceState {
    pub output: OutputId,
    pub primary: bool,
    pub in_flight: bool,
    pub cleanup_pending: bool,
    pub frame_pending: bool,
}

pub fn reduce_live_production_async_service_observation(
    outputs: &[LiveProductionOutputServiceState],
    present_queued: bool,
) -> Result<ProductionAsyncServiceObservation, &'static str> {
    if outputs.iter().filter(|output| output.primary).count() != 1 {
        return Err("production async service requires exactly one primary output");
    }
    let primary = outputs
        .iter()
        .find(|output| output.primary)
        .expect("validated above");
    Ok(ProductionAsyncServiceObservation {
        retirement_required: outputs
            .iter()
            .any(|output| output.in_flight || output.cleanup_pending),
        present_output_blocked: primary.in_flight || primary.cleanup_pending,
        present_queued,
        pending_output_ready: outputs.iter().any(|output| {
            output.frame_pending
                && !output.in_flight
                && !output.cleanup_pending
                && (!present_queued || !output.primary)
        }),
    })
}

impl LiveProductionVisualRuntime {
    pub fn native_output_service_states(
        &self,
        native_scanout: &LiveProductionNativeScanout,
    ) -> Result<Vec<LiveProductionOutputServiceState>, Box<dyn std::error::Error>> {
        let primary = self
            .outputs
            .primary_output()
            .ok_or("persistent backend runtime has no primary output")?;
        (0..self.output_count())
            .map(|index| {
                let output = self
                    .outputs
                    .output_id(index)
                    .ok_or("production output index was not registered")?;
                Ok(LiveProductionOutputServiceState {
                    output,
                    primary: output == primary,
                    in_flight: self
                        .outputs
                        .output_native_scanout_in_flight(index)
                        .ok_or("production output in-flight state was not registered")?,
                    cleanup_pending: self
                        .outputs
                        .output_native_cleanup_pending(index)
                        .ok_or("production output cleanup state was not registered")?,
                    frame_pending: native_scanout.pending_frame(index),
                })
            })
            .collect()
    }

    pub fn service_native(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
    ) -> Result<LiveProductionNativeServiceReport, Box<dyn std::error::Error>> {
        let mut coordinator = ProductionAsyncServiceCoordinator::new();
        let mut tick = None;
        let mut retired_present = None;
        let mut retirement_polled = false;
        let mut present_polled = false;
        let mut pending_frame_polled = false;
        loop {
            let output_states = self.native_output_service_states(native_scanout)?;
            let observation = reduce_live_production_async_service_observation(
                &output_states,
                self.diagnostics().present_queued,
            )?;
            let phase = coordinator.next_phase(observation);
            match phase {
                Some(ProductionAsyncServicePhase::KmsRetire) => {
                    retirement_polled = true;
                    retired_present = self.retire_native_scanout(native_scanout)?;
                }
                Some(ProductionAsyncServicePhase::SchedulePresent) => {
                    present_polled = true;
                    tick = Some(self.drive_gpu_presentation(Some(native_scanout))?);
                }
                Some(ProductionAsyncServicePhase::SubmitPendingFrame) => {
                    pending_frame_polled = true;
                    tick = Some(self.run_native_idle_with_primary_reservation(
                        native_scanout,
                        self.diagnostics().present_queued,
                    )?);
                }
                None => break,
            }
        }
        Ok(LiveProductionNativeServiceReport {
            tick,
            retired_present,
            retirement_polled,
            present_polled,
            pending_frame_polled,
        })
    }
}
