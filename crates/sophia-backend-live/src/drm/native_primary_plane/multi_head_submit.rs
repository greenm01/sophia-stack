use crate::prelude::*;

/// What a caller intends a topology submission to do.
#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeTopologySubmitIntent {
    /// Ask the kernel whether the topology is valid and change nothing.
    Validate,
    /// Apply the topology for real.
    Activate,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeTopologySubmitOutcome {
    Accepted,
    /// The device could not take the request now. Retrying later is legitimate.
    Busy,
    /// The kernel refused the topology.
    Rejected,
    /// The heads could not be composed into a request, so nothing was submitted.
    Unbuildable(LibdrmNativeMultiHeadRequestBuildStatus),
}

/// Submits one topology as a single atomic request.
///
/// `Validate` sets `TEST_ONLY`, which is the whole reason a caller can check a
/// topology against real hardware without risking the desktop. Both intents set
/// `ALLOW_MODESET`, because changing a topology is by definition a modeset.
///
/// An unbuildable head set never reaches the device. Reporting that separately
/// from a kernel rejection matters: one is a mistake in what was asked for, the
/// other is hardware declining something well-formed.
#[cfg(feature = "libdrm-events")]
pub fn submit_native_multi_head_topology<D>(
    committer: &mut NativeLibdrmAtomicScanoutCommitter<D>,
    heads: &[LibdrmNativeAtomicHead],
    intent: NativeTopologySubmitIntent,
) -> NativeTopologySubmitOutcome
where
    D: LibdrmNativeAtomicCommitDevice,
{
    let build = build_native_multi_head_atomic_request(
        heads,
        LibdrmNativeAtomicCommitRequestScope::Modeset,
    );
    if build.status != LibdrmNativeMultiHeadRequestBuildStatus::Built {
        return NativeTopologySubmitOutcome::Unbuildable(build.status);
    }
    let Some(request) = build.request else {
        return NativeTopologySubmitOutcome::Unbuildable(build.status);
    };
    let request = match intent {
        NativeTopologySubmitIntent::Validate => request.test_only().allow_modeset(),
        NativeTopologySubmitIntent::Activate => request.allow_modeset(),
    };
    submit(committer, request)
}

/// Validates one topology against real hardware without naming a framebuffer.
///
/// There is no `Activate` counterpart on purpose. A topology request carries no
/// plane state, so applying it would leave CRTCs active with nothing to scan out.
/// Validation is the only thing this shape is for, which is why the intent is
/// fixed here rather than passed in.
#[cfg(feature = "libdrm-events")]
pub fn validate_native_multi_head_topology<D>(
    committer: &mut NativeLibdrmAtomicScanoutCommitter<D>,
    heads: &[LibdrmNativeAtomicTopologyHead],
) -> NativeTopologySubmitOutcome
where
    D: LibdrmNativeAtomicCommitDevice,
{
    let build = build_native_multi_head_topology_request(heads);
    if build.status != LibdrmNativeMultiHeadRequestBuildStatus::Built {
        return NativeTopologySubmitOutcome::Unbuildable(build.status);
    }
    let Some(request) = build.request else {
        return NativeTopologySubmitOutcome::Unbuildable(build.status);
    };
    submit(committer, request.test_only().allow_modeset())
}

#[cfg(feature = "libdrm-events")]
fn submit<D>(
    committer: &mut NativeLibdrmAtomicScanoutCommitter<D>,
    request: LibdrmNativeAtomicCommitRequest,
) -> NativeTopologySubmitOutcome
where
    D: LibdrmNativeAtomicCommitDevice,
{
    match committer.submit_native_atomic_commit(request).status {
        LibdrmNativeAtomicCommitSubmitStatus::Submitted => NativeTopologySubmitOutcome::Accepted,
        LibdrmNativeAtomicCommitSubmitStatus::WouldBlock => NativeTopologySubmitOutcome::Busy,
        LibdrmNativeAtomicCommitSubmitStatus::Rejected => NativeTopologySubmitOutcome::Rejected,
    }
}
