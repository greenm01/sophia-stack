mod support;

use support::*;

fn output(
    raw: u64,
    primary: bool,
    native_phase: OutputNativeFramePhase,
    pending_frame: bool,
) -> OutputFrameServiceObservation {
    OutputFrameServiceObservation {
        output: OutputId::from_raw(raw),
        primary,
        native_phase,
        pending_frame,
    }
}

fn request(
    outputs: Vec<OutputFrameServiceObservation>,
    presentation_queued: bool,
) -> OutputFrameServiceRequest {
    OutputFrameServiceRequest {
        outputs,
        presentation_queued,
    }
}

#[test]
fn idle_outputs_emit_no_frame_service_effect() {
    let request = request(
        vec![
            output(1, true, OutputNativeFramePhase::Idle, false),
            output(2, false, OutputNativeFramePhase::Idle, false),
        ],
        false,
    );
    let mut reducer = OutputFrameServiceReducer::begin(&request).unwrap();

    assert_eq!(reducer.next_effect(&request), Ok(None));
    assert_eq!(reducer.effects_emitted(), 0);
}

#[test]
fn retirement_precedes_primary_presentation_and_secondary_pending_frame() {
    let mut request = request(
        vec![
            output(1, true, OutputNativeFramePhase::InFlight, true),
            output(2, false, OutputNativeFramePhase::CleanupPending, true),
        ],
        true,
    );
    let mut reducer = OutputFrameServiceReducer::begin(&request).unwrap();

    assert_eq!(
        reducer.next_effect(&request),
        Ok(Some(OutputFrameServiceEffect::PollRetirement {
            output: OutputId::from_raw(1),
        }))
    );
    request.outputs[0].native_phase = OutputNativeFramePhase::Idle;
    assert_eq!(
        reducer.next_effect(&request),
        Ok(Some(OutputFrameServiceEffect::PollRetirement {
            output: OutputId::from_raw(2),
        }))
    );
    request.outputs[1].native_phase = OutputNativeFramePhase::Idle;
    assert_eq!(
        reducer.next_effect(&request),
        Ok(Some(OutputFrameServiceEffect::SubmitQueuedPresentation {
            output: OutputId::from_raw(1),
        }))
    );
    request.outputs[0].native_phase = OutputNativeFramePhase::InFlight;
    assert_eq!(
        reducer.next_effect(&request),
        Ok(Some(OutputFrameServiceEffect::SubmitPendingFrame {
            output: OutputId::from_raw(2),
        }))
    );
    assert_eq!(reducer.next_effect(&request), Ok(None));
}

#[test]
fn secondary_retirement_does_not_starve_idle_primary_presentation() {
    let mut request = request(
        vec![
            output(1, true, OutputNativeFramePhase::Idle, false),
            output(2, false, OutputNativeFramePhase::InFlight, false),
        ],
        true,
    );
    let mut reducer = OutputFrameServiceReducer::begin(&request).unwrap();

    assert_eq!(
        reducer.next_effect(&request),
        Ok(Some(OutputFrameServiceEffect::PollRetirement {
            output: OutputId::from_raw(2),
        }))
    );
    assert_eq!(
        reducer.next_effect(&request),
        Ok(Some(OutputFrameServiceEffect::SubmitQueuedPresentation {
            output: OutputId::from_raw(1),
        }))
    );
    request.outputs[0].native_phase = OutputNativeFramePhase::InFlight;
    assert_eq!(
        reducer.next_effect(&request),
        Ok(Some(OutputFrameServiceEffect::PollRetirement {
            output: OutputId::from_raw(1),
        }))
    );
    assert_eq!(reducer.next_effect(&request), Ok(None));
}

#[test]
fn queued_presentation_reserves_primary_but_not_secondary() {
    let request = request(
        vec![
            output(1, true, OutputNativeFramePhase::Idle, true),
            output(2, false, OutputNativeFramePhase::Idle, true),
        ],
        true,
    );
    let mut reducer = OutputFrameServiceReducer::begin(&request).unwrap();

    assert_eq!(
        reducer.next_effect(&request),
        Ok(Some(OutputFrameServiceEffect::SubmitQueuedPresentation {
            output: OutputId::from_raw(1),
        }))
    );
    assert_eq!(
        reducer.next_effect(&request),
        Ok(Some(OutputFrameServiceEffect::SubmitPendingFrame {
            output: OutputId::from_raw(2),
        }))
    );
    assert_eq!(reducer.next_effect(&request), Ok(None));
}

#[test]
fn effect_is_not_reissued_when_backend_observation_does_not_advance() {
    let request = request(
        vec![
            output(1, true, OutputNativeFramePhase::InFlight, false),
            output(2, false, OutputNativeFramePhase::Idle, false),
        ],
        false,
    );
    let mut reducer = OutputFrameServiceReducer::begin(&request).unwrap();

    assert!(matches!(
        reducer.next_effect(&request),
        Ok(Some(OutputFrameServiceEffect::PollRetirement { .. }))
    ));
    assert_eq!(reducer.next_effect(&request), Ok(None));
}

#[test]
fn frame_service_rejects_invalid_output_identity_sets() {
    assert_eq!(
        OutputFrameServiceReducer::begin(&request(Vec::new(), false)),
        Err(OutputFrameServiceError::EmptyOutputSet)
    );

    let duplicate = request(
        vec![
            output(1, true, OutputNativeFramePhase::Idle, false),
            output(1, false, OutputNativeFramePhase::Idle, false),
        ],
        false,
    );
    assert_eq!(
        OutputFrameServiceReducer::begin(&duplicate),
        Err(OutputFrameServiceError::DuplicateOutput {
            output: OutputId::from_raw(1),
        })
    );

    let missing_primary = request(
        vec![output(1, false, OutputNativeFramePhase::Idle, false)],
        false,
    );
    assert_eq!(
        OutputFrameServiceReducer::begin(&missing_primary),
        Err(OutputFrameServiceError::MissingPrimary)
    );

    let multiple_primary = request(
        vec![
            output(1, true, OutputNativeFramePhase::Idle, false),
            output(2, true, OutputNativeFramePhase::Idle, false),
        ],
        false,
    );
    assert_eq!(
        OutputFrameServiceReducer::begin(&multiple_primary),
        Err(OutputFrameServiceError::MultiplePrimary)
    );

    let over_capacity = request(
        (1..=OUTPUT_FRAME_SERVICE_CAPACITY + 1)
            .map(|raw| {
                output(
                    u64::try_from(raw).unwrap(),
                    raw == 1,
                    OutputNativeFramePhase::Idle,
                    false,
                )
            })
            .collect(),
        false,
    );
    assert_eq!(
        OutputFrameServiceReducer::begin(&over_capacity),
        Err(OutputFrameServiceError::CapacityExceeded {
            capacity: OUTPUT_FRAME_SERVICE_CAPACITY,
            actual: OUTPUT_FRAME_SERVICE_CAPACITY + 1,
        })
    );
}

#[test]
fn frame_service_fails_closed_when_effect_budget_is_exhausted() {
    let request = request(
        vec![output(1, true, OutputNativeFramePhase::InFlight, false)],
        false,
    );
    let mut reducer = OutputFrameServiceReducer::with_effect_budget(&request, 0).unwrap();

    assert_eq!(
        reducer.next_effect(&request),
        Err(OutputFrameServiceError::EffectBudgetExhausted { budget: 0 })
    );
}

#[test]
fn output_set_and_primary_must_remain_stable_during_one_service_pass() {
    let initial = request(
        vec![
            output(1, true, OutputNativeFramePhase::Idle, false),
            output(2, false, OutputNativeFramePhase::Idle, false),
        ],
        false,
    );
    let mut reducer = OutputFrameServiceReducer::begin(&initial).unwrap();
    let changed_set = request(
        vec![output(1, true, OutputNativeFramePhase::Idle, false)],
        false,
    );
    assert_eq!(
        reducer.next_effect(&changed_set),
        Err(OutputFrameServiceError::OutputSetChanged)
    );

    let mut reducer = OutputFrameServiceReducer::begin(&initial).unwrap();
    let changed_primary = request(
        vec![
            output(1, false, OutputNativeFramePhase::Idle, false),
            output(2, true, OutputNativeFramePhase::Idle, false),
        ],
        false,
    );
    assert_eq!(
        reducer.next_effect(&changed_primary),
        Err(OutputFrameServiceError::PrimaryChanged {
            expected: OutputId::from_raw(1),
            actual: OutputId::from_raw(2),
        })
    );
}
