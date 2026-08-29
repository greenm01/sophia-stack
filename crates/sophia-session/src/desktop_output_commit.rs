use sophia_backend_live::{
    LibdrmNativeAtomicCommitDevice, LibdrmNativeAtomicHead, LibdrmNativeAtomicTopologyHead,
    NativeTopologySubmitIntent, NativeTopologySubmitOutcome,
    submit_native_multi_head_topology_on_device, validate_native_multi_head_topology_on_device,
};

use crate::desktop_output_activation::{
    NativeOutputActivationEffectExecutor, NativeOutputActivationFailure, NativeOutputActivationKey,
    NativeOutputEffectCompletion,
};
use crate::desktop_output_topology::NativeOutputActivationPlan;

/// The heads a candidate needs, already resolved to KMS objects.
///
/// Resolving a plan into heads means naming connectors, CRTCs, planes, mode
/// blobs, and framebuffers, which is hardware state this type deliberately does
/// not reach for. The caller resolves; this carries the result. `rollback` heads
/// describe the topology to restore, which is not the inverse of `apply`:
/// restoring a previous mode needs that mode's own blobs and correctly sized
/// framebuffers.
#[derive(Clone, Debug, Default)]
pub struct NativeOutputHeadSet {
    pub apply: Vec<LibdrmNativeAtomicHead>,
    pub rollback: Vec<LibdrmNativeAtomicHead>,
}

/// Validates a candidate against real hardware and never mutates anything.
///
/// This is the executor a session runs before it owns framebuffers. Its heads carry
/// no plane state, so the kernel judges the topology itself: whether these
/// connectors can be driven by these CRTCs at these modes, together, in one commit.
/// That is the question startup needs answered, and it can be answered before a
/// single pixel exists.
///
/// Apply is not gated here, it is absent. The type cannot mutate output state
/// because a topology request has nothing to scan out; a caller who wants to apply
/// needs `NativeOutputCommitExecutor` and real framebuffers. Rollback succeeds
/// trivially for the same reason: nothing was applied, so nothing needs undoing.
pub struct NativeOutputTopologyValidationExecutor<'a, D> {
    device: &'a D,
    heads: &'a [LibdrmNativeAtomicTopologyHead],
    validation: Option<NativeTopologySubmitOutcome>,
}

impl<'a, D> NativeOutputTopologyValidationExecutor<'a, D> {
    pub const fn new(device: &'a D, heads: &'a [LibdrmNativeAtomicTopologyHead]) -> Self {
        Self {
            device,
            heads,
            validation: None,
        }
    }

    /// What the kernel said about the topology, as one word for evidence.
    ///
    /// The settlement alone cannot carry this. A validation that succeeds still
    /// settles as rejected, because apply then refuses, and it refuses with the
    /// same `WouldBlock` a busy device produces. Without this, an accepted topology
    /// and a busy card are indistinguishable in the log, which would make the
    /// interesting outcome invisible.
    pub const fn validation(&self) -> &'static str {
        match self.validation {
            None => "not_attempted",
            Some(NativeTopologySubmitOutcome::Accepted) => "accepted",
            Some(NativeTopologySubmitOutcome::Busy) => "busy",
            Some(NativeTopologySubmitOutcome::Rejected) => "rejected",
            Some(NativeTopologySubmitOutcome::Unbuildable(_)) => "unbuildable",
        }
    }
}

impl<D> NativeOutputActivationEffectExecutor for NativeOutputTopologyValidationExecutor<'_, D>
where
    D: LibdrmNativeAtomicCommitDevice,
{
    fn test(
        &mut self,
        _key: NativeOutputActivationKey,
        _plan: &NativeOutputActivationPlan,
    ) -> NativeOutputEffectCompletion {
        let outcome = validate_native_multi_head_topology_on_device(self.device, self.heads);
        self.validation = Some(outcome);
        completion(outcome)
    }

    fn apply(
        &mut self,
        _key: NativeOutputActivationKey,
        _plan: &NativeOutputActivationPlan,
    ) -> NativeOutputEffectCompletion {
        // Unreachable behind a declined test, and refused if it is ever reached.
        NativeOutputEffectCompletion::Failed(NativeOutputActivationFailure::WouldBlock)
    }

    fn rollback(
        &mut self,
        _key: NativeOutputActivationKey,
        _plan: &NativeOutputActivationPlan,
    ) -> NativeOutputEffectCompletion {
        NativeOutputEffectCompletion::Succeeded
    }
}

/// Adapts topology submission to the activation reducer's effect executor.
///
/// The submission itself, including the `TEST_ONLY` validation pass and the
/// mapping from kernel result to outcome, lives in `sophia-backend-live` beside
/// the request builder and is tested there. This type only chooses which heads
/// each phase submits and translates the outcome, so the drm-facing decisions
/// stay in one place.
///
/// Apply is gated. With `apply_enabled` false the executor validates against real
/// hardware and then declines, which is the safe configuration for a session that
/// must not change output state.
pub struct NativeOutputCommitExecutor<'a, D> {
    device: &'a D,
    heads: &'a NativeOutputHeadSet,
    apply_enabled: bool,
}

impl<'a, D> NativeOutputCommitExecutor<'a, D> {
    /// Validates without ever applying.
    pub const fn validating(device: &'a D, heads: &'a NativeOutputHeadSet) -> Self {
        Self {
            device,
            heads,
            apply_enabled: false,
        }
    }

    /// Validates and then applies. Only for a caller authorized to change output
    /// state, behind its own gate and with rollback heads populated.
    pub const fn activating(device: &'a D, heads: &'a NativeOutputHeadSet) -> Self {
        Self {
            device,
            heads,
            apply_enabled: true,
        }
    }
}

#[cfg(feature = "native-session")]
const fn completion(outcome: NativeTopologySubmitOutcome) -> NativeOutputEffectCompletion {
    match outcome {
        NativeTopologySubmitOutcome::Accepted => NativeOutputEffectCompletion::Succeeded,
        NativeTopologySubmitOutcome::Busy => {
            NativeOutputEffectCompletion::Failed(NativeOutputActivationFailure::WouldBlock)
        }
        // An unbuildable head set and a kernel refusal both mean this candidate
        // cannot be activated. The reducer discards either the same way.
        NativeTopologySubmitOutcome::Rejected | NativeTopologySubmitOutcome::Unbuildable(_) => {
            NativeOutputEffectCompletion::Failed(NativeOutputActivationFailure::Rejected)
        }
    }
}

impl<D> NativeOutputActivationEffectExecutor for NativeOutputCommitExecutor<'_, D>
where
    D: LibdrmNativeAtomicCommitDevice,
{
    fn test(
        &mut self,
        _key: NativeOutputActivationKey,
        _plan: &NativeOutputActivationPlan,
    ) -> NativeOutputEffectCompletion {
        completion(submit_native_multi_head_topology_on_device(
            self.device,
            &self.heads.apply,
            NativeTopologySubmitIntent::Validate,
        ))
    }

    fn apply(
        &mut self,
        _key: NativeOutputActivationKey,
        _plan: &NativeOutputActivationPlan,
    ) -> NativeOutputEffectCompletion {
        if !self.apply_enabled {
            return NativeOutputEffectCompletion::Failed(NativeOutputActivationFailure::WouldBlock);
        }
        completion(submit_native_multi_head_topology_on_device(
            self.device,
            &self.heads.apply,
            NativeTopologySubmitIntent::Activate,
        ))
    }

    fn rollback(
        &mut self,
        _key: NativeOutputActivationKey,
        _plan: &NativeOutputActivationPlan,
    ) -> NativeOutputEffectCompletion {
        // Restoring nothing is a successful restore: an apply that never reached
        // the kernel left no state to undo.
        if self.heads.rollback.is_empty() {
            return NativeOutputEffectCompletion::Succeeded;
        }
        completion(submit_native_multi_head_topology_on_device(
            self.device,
            &self.heads.rollback,
            NativeTopologySubmitIntent::Activate,
        ))
    }
}
