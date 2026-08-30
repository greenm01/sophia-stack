//! Moving a cursor over a frame the plane is scanning directly.
//!
//! The roadmap says the legacy cursor continues over directly scanned frames
//! on its own ioctl, and every direct-scanout archive agrees -- but only
//! about a cursor that never moved. `moves_coalesced=0`,
//! `max_motion_to_submit_msec=0`, `hardware_updates=1`: initialized once,
//! visible, and still. The interesting case is the one none of them ran,
//! where the cursor ioctl fires repeatedly while page flips are handing the
//! client's own buffer to the plane.
//!
//! That case is worth proving before the atomic cursor plane replaces it,
//! for two reasons. It is the baseline the replacement has to match, and it
//! is the current claim -- so if it does not hold, the row that replaces it
//! is fixing a bug rather than removing a legacy path, which is a different
//! piece of work.
//!
//! Motion is driven through `Pointer::place`, the same entry physical input
//! uses. A proof that moved the cursor by some private route would prove
//! something about that route.

use sophia_protocol::{OutputId, Point, Rect};

/// How many direct flips must reach glass before the cursor starts moving.
///
/// Counted rather than timed, for the same reason the overlay proof counts:
/// the interaction being proven is with a frame the plane is *actually*
/// scanning, and a deadline can fire before one is.
pub(crate) const FLIPS_BEFORE_MOTION: usize = 10;

/// How many positions the cursor visits.
pub(crate) const MOVES: usize = 12;

/// Ticks between moves.
///
/// Spread rather than burst: a dozen updates in a dozen consecutive ticks
/// would coalesce into one ioctl and prove nothing about a cursor moving
/// across many flips. This spaces them so flips land between them.
pub(crate) const TICKS_PER_MOVE: u32 = 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stage {
    /// Waiting for enough direct flips to have reached glass.
    Waiting,
    /// Moving; `moved` positions visited, `countdown` ticks until the next.
    Moving { moved: usize, countdown: u32 },
    /// Every position visited. The cursor stays where it finished.
    Finished,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DirectCursorProof {
    enabled: bool,
    stage: Stage,
}

/// What the caller should do this tick.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DirectCursorAction {
    Idle,
    /// Place the cursor at this step of its path on this output.
    Move {
        output: OutputId,
        step: usize,
    },
    /// The last position has been visited; report and stop.
    Finished {
        moves: usize,
    },
}

impl DirectCursorProof {
    pub(crate) const fn new(enabled: bool) -> Self {
        Self {
            enabled,
            stage: Stage::Waiting,
        }
    }

    pub(crate) fn tick(&mut self, flips: usize, output: Option<OutputId>) -> DirectCursorAction {
        if !self.enabled {
            return DirectCursorAction::Idle;
        }
        match self.stage {
            Stage::Waiting => {
                let Some(output) = output else {
                    return DirectCursorAction::Idle;
                };
                if flips < FLIPS_BEFORE_MOTION {
                    return DirectCursorAction::Idle;
                }
                self.stage = Stage::Moving {
                    moved: 1,
                    countdown: TICKS_PER_MOVE,
                };
                DirectCursorAction::Move { output, step: 0 }
            }
            Stage::Moving { moved, countdown } => {
                if let Some(countdown) = countdown.checked_sub(1).filter(|left| *left != 0) {
                    self.stage = Stage::Moving { moved, countdown };
                    return DirectCursorAction::Idle;
                }
                if moved >= MOVES {
                    self.stage = Stage::Finished;
                    return DirectCursorAction::Finished { moves: moved };
                }
                // The output is re-read every move rather than remembered:
                // if direct scanout stopped, there is nothing to prove a
                // cursor is riding over, and the proof stalls instead of
                // reporting motion across frames that were composed.
                let Some(output) = output else {
                    return DirectCursorAction::Idle;
                };
                self.stage = Stage::Moving {
                    moved: moved + 1,
                    countdown: TICKS_PER_MOVE,
                };
                DirectCursorAction::Move {
                    output,
                    step: moved,
                }
            }
            Stage::Finished => DirectCursorAction::Idle,
        }
    }
}

/// Where the cursor sits at one step of its path.
///
/// A horizontal sweep across the middle of the head, inset from both edges.
/// Inset because a cursor crossing an output boundary is a different
/// question -- pointer confinement and output transition own it -- and this
/// proof is about the ioctl firing while the plane scans a client's buffer.
pub(crate) fn cursor_position(head: Rect, step: usize) -> Point {
    let steps = MOVES.max(2) - 1;
    let inset = head.width / 8;
    let span = head.width.saturating_sub(inset.saturating_mul(2)).max(1);
    let offset = i32::try_from(step.min(steps))
        .unwrap_or(0)
        .saturating_mul(span)
        / i32::try_from(steps).unwrap_or(1).max(1);
    Point {
        x: f64::from(head.x.saturating_add(inset).saturating_add(offset)),
        y: f64::from(head.y.saturating_add(head.height / 2)),
    }
}
