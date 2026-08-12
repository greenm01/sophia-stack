//! Whether a prepared topology may be applied yet.
//!
//! Applying a topology needs plane state, and plane state needs a framebuffer at
//! the mode being applied. The tempting shortcut is to allocate a scratch buffer at
//! the new size so apply can run whenever it likes. `docs/renderer-import-boundary.md`
//! forecloses it: native KMS initialization waits for the first committed-state
//! frame rather than requiring a speculative or blank visual bootstrap. A scratch
//! buffer is exactly that bootstrap, and it would also put a frame on screen that
//! no committed state produced.
//!
//! So apply is fed by the renderer, not by the output authority: the frame target
//! is resized to the new mode, a frame is composed at that size from committed
//! state, and only then can the topology naming that frame be applied. This module
//! is the precondition — it answers whether that has happened, and says precisely
//! what is missing when it has not.
//!
//! Checking it here rather than at head-composition time is what turns "this output
//! has no framebuffer at the requested size" from a discovery mid-apply into a
//! refusal before anything is submitted.

use std::fmt;

use sophia_backend_live::{LiveGbmEglFrameTargetRecord, LiveGbmEglFrameTargetStatus};
use sophia_protocol::OutputId;

use crate::desktop_output_topology::NativeOutputActivationPlan;

/// One output's frame target, as the renderer reduced it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeOutputFrameTarget {
    pub output: OutputId,
    pub target: LiveGbmEglFrameTargetRecord,
}

/// Why a topology cannot be applied yet, or that it can.
///
/// Every rejection names the output and both sizes where it has them, because the
/// operator question is always "which screen, and how far off" and a bare refusal
/// answers neither.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOutputApplyAdmission {
    /// Every enabled output has a valid frame target at its requested mode.
    Ready,
    /// The output has no frame target at all, so nothing has been composed for it.
    NoFrameTarget { output: u64 },
    /// A frame target exists but is not usable for scanout.
    InvalidFrameTarget { output: u64 },
    /// A frame target exists at the wrong size. This is the ordinary state during a
    /// mode change: the target has not been resized and recomposed yet.
    SizeMismatch {
        output: u64,
        target_width: i32,
        target_height: i32,
        requested_width: u32,
        requested_height: u32,
    },
}

impl NativeOutputApplyAdmission {
    pub const fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }
}

impl fmt::Display for NativeOutputApplyAdmission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready => formatter.write_str("every enabled output has a frame at its mode"),
            Self::NoFrameTarget { output } => write!(
                formatter,
                "native output {output} has composed no frame, so there is nothing to scan out"
            ),
            Self::InvalidFrameTarget { output } => write!(
                formatter,
                "native output {output} has a frame target that cannot drive scanout"
            ),
            Self::SizeMismatch {
                output,
                target_width,
                target_height,
                requested_width,
                requested_height,
            } => write!(
                formatter,
                "native output {output} has a {target_width}x{target_height} frame but the \
requested mode is {requested_width}x{requested_height}"
            ),
        }
    }
}

/// Decides whether every enabled output in a plan has a frame at its requested mode.
///
/// Disabled targets are skipped for the same reason they contribute no head: a
/// disabled output is expressed by omission, and asking it for a frame would invent
/// work for a screen that is leaving the desktop.
///
/// The first failing output wins. Reporting one precise cause beats a list, because
/// a mode change resizes targets one at a time and the rest of the list would just
/// restate the same transition.
pub fn native_output_apply_admission(
    plan: &NativeOutputActivationPlan,
    frame_targets: &[NativeOutputFrameTarget],
) -> NativeOutputApplyAdmission {
    for target in plan.targets() {
        let requested = target.requested();
        if !requested.enabled {
            continue;
        }
        let output = target.output();
        let raw = output.raw();

        let Some(frame) = frame_targets
            .iter()
            .find(|frame| frame.output == output)
            .map(|frame| frame.target)
        else {
            return NativeOutputApplyAdmission::NoFrameTarget { output: raw };
        };
        if !matches!(frame.status, LiveGbmEglFrameTargetStatus::Ready)
            || !frame.is_valid_scanout_target()
        {
            return NativeOutputApplyAdmission::InvalidFrameTarget { output: raw };
        }
        if !size_matches(frame.size.width, requested.mode.width)
            || !size_matches(frame.size.height, requested.mode.height)
        {
            return NativeOutputApplyAdmission::SizeMismatch {
                output: raw,
                target_width: frame.size.width,
                target_height: frame.size.height,
                requested_width: requested.mode.width,
                requested_height: requested.mode.height,
            };
        }
    }
    NativeOutputApplyAdmission::Ready
}

/// Compares a signed target dimension with an unsigned mode dimension without
/// letting either conversion wrap a mismatch into a match.
const fn size_matches(target: i32, requested: u32) -> bool {
    target >= 0 && requested <= i32::MAX as u32 && target == requested as i32
}
