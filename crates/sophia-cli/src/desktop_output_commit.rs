use sophia_backend_live::{
    LibdrmNativeAtomicCommitDevice, LibdrmNativeAtomicHead, NativeLibdrmAtomicScanoutCommitter,
    NativeTopologySubmitIntent, NativeTopologySubmitOutcome, submit_native_multi_head_topology,
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
    committer: &'a mut NativeLibdrmAtomicScanoutCommitter<D>,
    heads: &'a NativeOutputHeadSet,
    apply_enabled: bool,
}

impl<'a, D> NativeOutputCommitExecutor<'a, D> {
    /// Validates without ever applying.
    pub fn validating(
        committer: &'a mut NativeLibdrmAtomicScanoutCommitter<D>,
        heads: &'a NativeOutputHeadSet,
    ) -> Self {
        Self {
            committer,
            heads,
            apply_enabled: false,
        }
    }

    /// Validates and then applies. Only for a caller authorized to change output
    /// state, behind its own gate and with rollback heads populated.
    pub fn activating(
        committer: &'a mut NativeLibdrmAtomicScanoutCommitter<D>,
        heads: &'a NativeOutputHeadSet,
    ) -> Self {
        Self {
            committer,
            heads,
            apply_enabled: true,
        }
    }
}

#[cfg(feature = "atomic-scanout-live")]
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
        completion(submit_native_multi_head_topology(
            self.committer,
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
        completion(submit_native_multi_head_topology(
            self.committer,
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
        completion(submit_native_multi_head_topology(
            self.committer,
            &self.heads.rollback,
            NativeTopologySubmitIntent::Activate,
        ))
    }
}
