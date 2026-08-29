//! Drives an overlay over a directly scanned frame, so the return to
//! composition can be proven rather than asserted.
//!
//! `PresentFlipOwnership.tla` models the transition: an activation ends the
//! eligibility episode, the frame the plane is still scanning is retired by a
//! *composed* successor rather than evicted, and a later withdrawal restores
//! eligibility only through a fresh proof and a fresh atomic test. Every part
//! of that is implemented and none of it had run on hardware, because the
//! session that can scan out directly has no shell -- and the shell is what
//! opens the descriptor overlay in a product session.
//!
//! So this opens one. It is a development control, off in every product
//! session, and it uses the same `set_descriptor_overlay` entry the shell
//! uses rather than a private path: a proof that exercises different code
//! than the product proves nothing about the product.

use sophia_engine::{
    CompositorDisplayCommand, CompositorNodeId, CompositorRect, CompositorRgb8,
    DescriptorOverlayNodeRole, DescriptorOverlayProjection,
};
use sophia_protocol::{OutputId, Rect};

/// How many direct flips must reach glass before the overlay opens.
///
/// Counted rather than timed: the transition being proven starts from a frame
/// the plane is actually scanning, and a deadline can fire before one is.
pub(crate) const FLIPS_BEFORE_ACTIVATION: usize = 10;

/// How many ticks the overlay stays up.
///
/// Long enough for composed frames to reach glass and for the displaced
/// direct buffer to be retired by one of them; short enough that a bounded
/// session still ends on its own.
pub(crate) const ACTIVATION_TICKS: u32 = 240;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stage {
    /// Waiting for enough direct flips to have reached glass.
    Waiting,
    /// The overlay is up and every frame should be composed.
    Activated { remaining: u32 },
    /// Withdrawn; eligibility must be re-proven and re-tested before a flip.
    Withdrawn,
}

/// The control's own state. Inert unless the session asked for it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DirectOverlayProof {
    enabled: bool,
    stage: Stage,
}

/// What the caller should do to the overlay this tick.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DirectOverlayAction {
    /// Nothing to do.
    Idle,
    /// Install the overlay on this output.
    Activate(OutputId),
    /// Take it back down.
    Withdraw,
}

impl DirectOverlayProof {
    pub(crate) const fn new(enabled: bool) -> Self {
        Self {
            enabled,
            stage: Stage::Waiting,
        }
    }

    /// Advance one tick, given how many direct flips have reached glass and
    /// which output is carrying them.
    pub(crate) fn tick(&mut self, flips: usize, output: Option<OutputId>) -> DirectOverlayAction {
        if !self.enabled {
            return DirectOverlayAction::Idle;
        }
        match self.stage {
            Stage::Waiting => {
                let Some(output) = output else {
                    return DirectOverlayAction::Idle;
                };
                if flips < FLIPS_BEFORE_ACTIVATION {
                    return DirectOverlayAction::Idle;
                }
                self.stage = Stage::Activated {
                    remaining: ACTIVATION_TICKS,
                };
                DirectOverlayAction::Activate(output)
            }
            Stage::Activated { remaining } => {
                if let Some(remaining) = remaining.checked_sub(1).filter(|left| *left != 0) {
                    self.stage = Stage::Activated { remaining };
                    return DirectOverlayAction::Idle;
                }
                self.stage = Stage::Withdrawn;
                DirectOverlayAction::Withdraw
            }
            Stage::Withdrawn => DirectOverlayAction::Idle,
        }
    }
}

/// An overlay that paints, positioned where it cannot be mistaken for the
/// client's own content.
///
/// A solid rect is enough: `command_requires_composition` disqualifies a frame
/// on any painting primitive, and the verdict records which one it was. There
/// are no interaction targets -- this proves a composition boundary, not an
/// input path, and a target would claim input the control cannot service.
pub(crate) fn overlay_projection(
    output: OutputId,
    generation: u64,
    head: Rect,
) -> DescriptorOverlayProjection {
    let width = (head.width / 3).max(1);
    let height = (head.height / 6).max(1);
    let geometry = Rect {
        x: head.x + (head.width - width) / 2,
        y: head.y + height,
        width,
        height,
    };
    DescriptorOverlayProjection {
        output,
        generation,
        geometry,
        commands: vec![CompositorDisplayCommand::Rect(CompositorRect {
            // The panel node of this projection: the same identity the shell's
            // own overlay uses, so nothing downstream has to special-case it.
            node: CompositorNodeId::DescriptorOverlay {
                projection: generation,
                slot: 0,
                role: DescriptorOverlayNodeRole::Panel,
            },
            generation,
            geometry,
            color: CompositorRgb8 {
                red: 255,
                green: 96,
                blue: 0,
            },
        })],
        targets: Vec::new(),
    }
}
