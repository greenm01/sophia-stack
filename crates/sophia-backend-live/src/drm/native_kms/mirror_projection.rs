//! Where a mirror group's frame lands on each of its heads.
//!
//! A group is one logical output backed by connectors that need not share a mode,
//! so the scene composed once for the group has to be placed into each head's own
//! buffer. That placement is the whole of mirroring's visual behaviour, and it is
//! pure geometry: a source size, a destination size, and a policy.
//!
//! Keeping it separate from the draw call is deliberate. The rect is the part that
//! can be wrong in ways a screenshot would not show -- an off-by-one centring, a
//! rounding that loses a row -- and it is the part worth testing exhaustively
//! without a GPU.

use crate::prelude::*;
use sophia_protocol::Rect;

/// How a group's frame is placed on a head whose mode differs from the scene.
///
/// Named policies rather than one hardcoded behaviour, so letterboxing is an
/// operator's choice instead of a surprise. The vocabulary is `wl-mirror`'s, which
/// is the one users of other mirroring tools will already know.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NativeMirrorFit {
    /// Scale until the whole frame fits, preserving aspect. Unused space becomes
    /// black bars. Nothing is cropped, which is why it is the default: a mirror
    /// that hides part of the screen is worse than one with bars.
    #[default]
    Fit,
    /// Scale until the frame covers the head, preserving aspect. No bars, and
    /// whatever falls outside is cropped.
    Cover,
    /// No scaling. The frame is centred at its own size, cropped if the head is
    /// smaller. For an operator who would rather see exact pixels than a resample.
    Exact,
}

/// Places a `source`-sized frame into a `destination`-sized buffer.
///
/// The returned rect may extend outside the destination under `Cover` and
/// `Exact`; that is the crop, and the caller clips it. Degenerate sizes produce an
/// empty rect at the origin rather than a division by zero, because a head with no
/// area is a state to skip rather than a reason to fail a frame.
pub fn project_mirror_rect(source: Size, destination: Size, fit: NativeMirrorFit) -> Rect {
    if source.width <= 0 || source.height <= 0 || destination.width <= 0 || destination.height <= 0
    {
        return Rect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        };
    }

    let (width, height) = match fit {
        NativeMirrorFit::Exact => (source.width, source.height),
        NativeMirrorFit::Fit | NativeMirrorFit::Cover => {
            // Scaled in one rational step rather than through a float ratio, so a
            // head whose mode is an exact multiple of the scene lands on exact
            // pixels instead of one short.
            let by_width = i64::from(destination.width) * i64::from(source.height);
            let by_height = i64::from(destination.height) * i64::from(source.width);
            let use_width = if fit == NativeMirrorFit::Fit {
                by_width <= by_height
            } else {
                by_width >= by_height
            };
            if use_width {
                let height = i64::from(destination.width) * i64::from(source.height)
                    / i64::from(source.width);
                (destination.width, i32::try_from(height).unwrap_or(i32::MAX))
            } else {
                let width = i64::from(destination.height) * i64::from(source.width)
                    / i64::from(source.height);
                (i32::try_from(width).unwrap_or(i32::MAX), destination.height)
            }
        }
    };

    Rect {
        x: (destination.width - width) / 2,
        y: (destination.height - height) / 2,
        width,
        height,
    }
}
