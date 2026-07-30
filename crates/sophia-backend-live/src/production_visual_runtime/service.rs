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
    pub present_scheduling_blocked: bool,
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
            present_scheduling_blocked: self.present_scheduler.has_layout_deferred()
                && !self.present_scheduler.has_eligible(),
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
    pub clip: Option<Rect>,
}

#[derive(Debug)]
pub struct LiveProductionNativeServiceReport {
    pub ticks: Vec<LiveBackendRuntimeTickReport>,
    pub retired_present: Option<LiveProductionRetiredPresent>,
    pub effects: Vec<OutputFrameServiceEffect>,
}

pub const fn reduce_output_native_frame_phase(
    in_flight: bool,
    cleanup_pending: bool,
) -> OutputNativeFramePhase {
    if cleanup_pending {
        OutputNativeFramePhase::CleanupPending
    } else if in_flight {
        OutputNativeFramePhase::InFlight
    } else {
        OutputNativeFramePhase::Idle
    }
}

impl LiveProductionVisualRuntime {
    pub fn native_output_service_request(
        &self,
        native_scanout: &LiveProductionNativeScanout,
    ) -> Result<OutputFrameServiceRequest, Box<dyn std::error::Error>> {
        let primary = self
            .outputs
            .primary_output()
            .ok_or("persistent backend runtime has no primary output")?;
        let outputs = (0..self.output_count())
            .map(|index| {
                let output = self
                    .outputs
                    .output_id(index)
                    .ok_or("production output index was not registered")?;
                let in_flight = self
                    .outputs
                    .output_native_scanout_in_flight(index)
                    .ok_or("production output in-flight state was not registered")?;
                let cleanup_pending = self
                    .outputs
                    .output_native_cleanup_pending(index)
                    .ok_or("production output cleanup state was not registered")?;
                Ok(OutputFrameServiceObservation {
                    output,
                    primary: output == primary,
                    native_phase: reduce_output_native_frame_phase(in_flight, cleanup_pending),
                    pending_frame: native_scanout.pending_frame(index),
                })
            })
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
        Ok(OutputFrameServiceRequest {
            outputs,
            presentation_queued: self.diagnostics().present_queued
                && !self.diagnostics().present_scheduling_blocked,
        })
    }

    pub fn service_native(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
    ) -> Result<LiveProductionNativeServiceReport, Box<dyn std::error::Error>> {
        native_scanout.ensure_page_flip_progress()?;
        let initial = self.native_output_service_request(native_scanout)?;
        let mut reducer = OutputFrameServiceReducer::begin(&initial)
            .map_err(|error| format!("invalid output frame service state: {error:?}"))?;
        let mut ticks = Vec::new();
        let mut retired_present = None;
        let mut effects = Vec::new();
        loop {
            let observation = self.native_output_service_request(native_scanout)?;
            let Some(effect) = reducer
                .next_effect(&observation)
                .map_err(|error| format!("output frame service reduction failed: {error:?}"))?
            else {
                break;
            };
            effects.push(effect);
            match effect {
                OutputFrameServiceEffect::PollRetirement { output } => {
                    if let Some(retired) =
                        self.retire_native_scanout_output(native_scanout, output)?
                    {
                        retired_present = Some(retired);
                    }
                }
                OutputFrameServiceEffect::SubmitQueuedPresentation { output } => {
                    if output != reducer.primary() {
                        return Err("queued presentation targeted a non-primary output".into());
                    }
                    ticks.push(self.drive_gpu_presentation(Some(native_scanout))?);
                }
                OutputFrameServiceEffect::SubmitPendingFrame { output } => {
                    ticks.push(self.run_native_pending_output(native_scanout, output)?);
                }
            }
        }
        Ok(LiveProductionNativeServiceReport {
            ticks,
            retired_present,
            effects,
        })
    }
}
