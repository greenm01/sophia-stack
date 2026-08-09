use std::collections::VecDeque;

use sophia_protocol::{InputEventPacket, RoutedInputRequest, SeatId, SurfaceId};

pub const POINTER_FOCUS_HANDOFF_CAPACITY: usize = 256;
pub const POINTER_FOCUS_HANDOFF_TIMEOUT_MSEC: u64 = 4_000;
pub const KEYBOARD_FOCUS_HANDOFF_CAPACITY: usize = 256;
pub const KEYBOARD_FOCUS_HANDOFF_TIMEOUT_MSEC: u64 = 4_000;

/// Ordered physical keys held while Engine focus and frontend focus converge
/// on the same exact surface identity.
///
/// Control-plane shortcuts are resolved before events enter this queue. The
/// queued events are therefore client-bound input, not commands to replay
/// through the Engine control plane. A target, seat, timeout, or capacity
/// mismatch discards the whole sequence so a partial chord cannot escape.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeyboardFocusHandoffState {
    target: Option<SurfaceId>,
    seat: Option<SeatId>,
    started_msec: u64,
    pending: VecDeque<InputEventPacket>,
}

impl KeyboardFocusHandoffState {
    pub fn target(&self) -> Option<SurfaceId> {
        self.target
    }

    pub fn defer(
        &mut self,
        target: SurfaceId,
        started_msec: u64,
        event: InputEventPacket,
    ) -> Result<(), &'static str> {
        if event.target_surface != Some(target) {
            self.clear();
            return Err("keyboard focus handoff event target differs from queue target");
        }
        match (self.target, self.seat) {
            (None, None) => {
                self.target = Some(target);
                self.seat = Some(event.seat);
                self.started_msec = started_msec;
            }
            (Some(held_target), Some(held_seat))
                if held_target == target && held_seat == event.seat => {}
            _ => {
                self.clear();
                return Err("keyboard focus handoff authority changed");
            }
        }
        if self.pending.len() >= KEYBOARD_FOCUS_HANDOFF_CAPACITY {
            self.clear();
            return Err("keyboard focus handoff capacity exhausted");
        }
        self.pending.push_back(event);
        Ok(())
    }

    pub fn expire(&mut self, now_msec: u64) -> bool {
        if self.target.is_some()
            && now_msec.saturating_sub(self.started_msec) >= KEYBOARD_FOCUS_HANDOFF_TIMEOUT_MSEC
        {
            self.clear();
            true
        } else {
            false
        }
    }

    pub fn cancel_if_target_stale(
        &mut self,
        mut target_is_current: impl FnMut(SurfaceId) -> bool,
    ) -> bool {
        let stale = self.target.is_some_and(|target| !target_is_current(target))
            || self.pending.iter().any(|event| {
                event
                    .target_surface
                    .is_none_or(|target| !target_is_current(target))
            });
        if stale {
            self.clear();
        }
        stale
    }

    pub fn take_ready(
        &mut self,
        applied_focus: Option<SurfaceId>,
    ) -> Option<VecDeque<InputEventPacket>> {
        (self.target == applied_focus && self.target.is_some()).then(|| {
            self.target = None;
            self.seat = None;
            std::mem::take(&mut self.pending)
        })
    }

    fn clear(&mut self) {
        self.target = None;
        self.seat = None;
        self.pending.clear();
    }
}

/// Protocol-neutral ordered pointer input held while a requested focus change
/// crosses the WM and frontend authority boundaries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointerFocusHandoffState {
    target: Option<SurfaceId>,
    started_msec: u64,
    pending: VecDeque<RoutedInputRequest>,
}

impl PointerFocusHandoffState {
    pub fn target(&self) -> Option<SurfaceId> {
        self.target
    }

    pub fn begin(
        &mut self,
        target: SurfaceId,
        started_msec: u64,
        request: RoutedInputRequest,
    ) -> Result<(), &'static str> {
        if self.target.is_some() {
            return Err("pointer focus handoff is already active");
        }
        self.target = Some(target);
        self.started_msec = started_msec;
        self.defer(request)
    }

    pub fn defer(&mut self, request: RoutedInputRequest) -> Result<(), &'static str> {
        if matches!(request.kind, sophia_protocol::InputEventKind::PointerMotion)
            && self.pending.back().is_some_and(|pending| {
                matches!(pending.kind, sophia_protocol::InputEventKind::PointerMotion)
                    && pending.seat == request.seat
                    && pending.device == request.device
                    && pending.target_surface == request.target_surface
            })
        {
            *self
                .pending
                .back_mut()
                .expect("adjacent pointer motion was present") = request;
            return Ok(());
        }
        if self.pending.len() >= POINTER_FOCUS_HANDOFF_CAPACITY {
            // Never leave a partial button/axis sequence available for later
            // delivery. The caller may continue the session after reporting
            // the bounded handoff drop.
            self.clear();
            return Err("pointer focus handoff capacity exhausted");
        }
        self.pending.push_back(request);
        Ok(())
    }

    pub fn expire(&mut self, now_msec: u64) -> bool {
        if self.target.is_some()
            && now_msec.saturating_sub(self.started_msec) >= POINTER_FOCUS_HANDOFF_TIMEOUT_MSEC
        {
            self.clear();
            true
        } else {
            false
        }
    }

    /// Cancels an active handoff when its focus target or any buffered route
    /// no longer names an exact current surface identity.
    ///
    /// The caller supplies the authoritative membership check because the
    /// Engine state is intentionally protocol-neutral. Cancellation discards
    /// the whole sequence so no prefix can escape after target replacement.
    pub fn cancel_if_target_stale(
        &mut self,
        mut target_is_current: impl FnMut(SurfaceId) -> bool,
    ) -> bool {
        let stale = self.target.is_some_and(|target| !target_is_current(target))
            || self
                .pending
                .iter()
                .any(|request| !target_is_current(request.target_surface));
        if stale {
            self.clear();
        }
        stale
    }

    pub fn take_ready(
        &mut self,
        applied_focus: Option<SurfaceId>,
    ) -> Option<VecDeque<RoutedInputRequest>> {
        (self.target == applied_focus && self.target.is_some()).then(|| {
            self.target = None;
            std::mem::take(&mut self.pending)
        })
    }

    fn clear(&mut self) {
        self.target = None;
        self.pending.clear();
    }
}
