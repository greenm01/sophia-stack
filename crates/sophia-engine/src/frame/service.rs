use crate::prelude::*;

pub const OUTPUT_FRAME_SERVICE_CAPACITY: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputNativeFramePhase {
    Idle,
    InFlight,
    CleanupPending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputFrameServiceObservation {
    pub output: OutputId,
    pub primary: bool,
    pub native_phase: OutputNativeFramePhase,
    pub pending_frame: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OutputFrameServiceRequest {
    pub outputs: Vec<OutputFrameServiceObservation>,
    pub presentation_queued: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFrameServiceEffect {
    PollRetirement { output: OutputId },
    SubmitQueuedPresentation { output: OutputId },
    SubmitPendingFrame { output: OutputId },
}

impl OutputFrameServiceEffect {
    pub const fn output(self) -> OutputId {
        match self {
            Self::PollRetirement { output }
            | Self::SubmitQueuedPresentation { output }
            | Self::SubmitPendingFrame { output } => output,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFrameServiceError {
    EmptyOutputSet,
    CapacityExceeded {
        capacity: usize,
        actual: usize,
    },
    DuplicateOutput {
        output: OutputId,
    },
    MissingPrimary,
    MultiplePrimary,
    OutputSetChanged,
    PrimaryChanged {
        expected: OutputId,
        actual: OutputId,
    },
    EffectBudgetExhausted {
        budget: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputFrameServiceReducer {
    outputs: BTreeSet<OutputId>,
    primary: OutputId,
    retirement_attempted: BTreeSet<OutputId>,
    pending_frame_attempted: BTreeSet<OutputId>,
    presentation_attempted: bool,
    effect_budget: usize,
    effects_emitted: usize,
}

impl OutputFrameServiceReducer {
    pub fn begin(request: &OutputFrameServiceRequest) -> Result<Self, OutputFrameServiceError> {
        let effect_budget = request.outputs.len().saturating_mul(2).saturating_add(1);
        Self::with_effect_budget(request, effect_budget)
    }

    pub fn with_effect_budget(
        request: &OutputFrameServiceRequest,
        effect_budget: usize,
    ) -> Result<Self, OutputFrameServiceError> {
        let (outputs, primary) = validate_output_frame_service_request(request)?;
        Ok(Self {
            outputs,
            primary,
            retirement_attempted: BTreeSet::new(),
            pending_frame_attempted: BTreeSet::new(),
            presentation_attempted: false,
            effect_budget,
            effects_emitted: 0,
        })
    }

    pub const fn primary(&self) -> OutputId {
        self.primary
    }

    pub const fn effects_emitted(&self) -> usize {
        self.effects_emitted
    }

    pub fn next_effect(
        &mut self,
        request: &OutputFrameServiceRequest,
    ) -> Result<Option<OutputFrameServiceEffect>, OutputFrameServiceError> {
        self.validate_stable_output_set(request)?;

        if let Some(output) = request
            .outputs
            .iter()
            .filter(|output| output.native_phase != OutputNativeFramePhase::Idle)
            .map(|output| output.output)
            .filter(|output| !self.retirement_attempted.contains(output))
            .min()
        {
            self.retirement_attempted.insert(output);
            return self.emit(OutputFrameServiceEffect::PollRetirement { output });
        }

        let primary = request
            .outputs
            .iter()
            .find(|output| output.output == self.primary)
            .expect("stable validated output set contains its primary");
        if request.presentation_queued
            && primary.native_phase == OutputNativeFramePhase::Idle
            && !self.presentation_attempted
        {
            self.presentation_attempted = true;
            return self.emit(OutputFrameServiceEffect::SubmitQueuedPresentation {
                output: self.primary,
            });
        }

        if let Some(output) = request
            .outputs
            .iter()
            .filter(|output| {
                output.native_phase == OutputNativeFramePhase::Idle
                    && output.pending_frame
                    && !(request.presentation_queued && output.primary)
            })
            .map(|output| output.output)
            .filter(|output| !self.pending_frame_attempted.contains(output))
            .min()
        {
            self.pending_frame_attempted.insert(output);
            return self.emit(OutputFrameServiceEffect::SubmitPendingFrame { output });
        }

        Ok(None)
    }

    fn emit(
        &mut self,
        effect: OutputFrameServiceEffect,
    ) -> Result<Option<OutputFrameServiceEffect>, OutputFrameServiceError> {
        if self.effects_emitted >= self.effect_budget {
            return Err(OutputFrameServiceError::EffectBudgetExhausted {
                budget: self.effect_budget,
            });
        }
        self.effects_emitted = self.effects_emitted.saturating_add(1);
        Ok(Some(effect))
    }

    fn validate_stable_output_set(
        &self,
        request: &OutputFrameServiceRequest,
    ) -> Result<(), OutputFrameServiceError> {
        let (outputs, primary) = validate_output_frame_service_request(request)?;
        if outputs != self.outputs {
            return Err(OutputFrameServiceError::OutputSetChanged);
        }
        if primary != self.primary {
            return Err(OutputFrameServiceError::PrimaryChanged {
                expected: self.primary,
                actual: primary,
            });
        }
        Ok(())
    }
}

fn validate_output_frame_service_request(
    request: &OutputFrameServiceRequest,
) -> Result<(BTreeSet<OutputId>, OutputId), OutputFrameServiceError> {
    if request.outputs.is_empty() {
        return Err(OutputFrameServiceError::EmptyOutputSet);
    }
    if request.outputs.len() > OUTPUT_FRAME_SERVICE_CAPACITY {
        return Err(OutputFrameServiceError::CapacityExceeded {
            capacity: OUTPUT_FRAME_SERVICE_CAPACITY,
            actual: request.outputs.len(),
        });
    }

    let mut outputs = BTreeSet::new();
    let mut primary = None;
    for observation in &request.outputs {
        if !outputs.insert(observation.output) {
            return Err(OutputFrameServiceError::DuplicateOutput {
                output: observation.output,
            });
        }
        if observation.primary && primary.replace(observation.output).is_some() {
            return Err(OutputFrameServiceError::MultiplePrimary);
        }
    }
    let primary = primary.ok_or(OutputFrameServiceError::MissingPrimary)?;
    Ok((outputs, primary))
}
