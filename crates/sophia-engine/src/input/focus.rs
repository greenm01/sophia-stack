use crate::prelude::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InputFocusState {
    focused_surfaces: BTreeMap<SeatId, SurfaceId>,
}

impl InputFocusState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn focused_surface(&self, seat: SeatId) -> Option<SurfaceId> {
        self.focused_surfaces.get(&seat).copied()
    }

    pub fn focus_surface(
        &mut self,
        seat: SeatId,
        surface: SurfaceId,
        committed_surfaces: &[CommittedSurfaceState],
    ) -> InputFocusDecision {
        if !seat.is_valid() {
            return InputFocusDecision::InvalidSeat;
        }
        if !committed_surfaces
            .iter()
            .any(|committed| committed.surface == surface)
        {
            return InputFocusDecision::UnknownSurface;
        }
        if self.focused_surfaces.get(&seat) == Some(&surface) {
            return InputFocusDecision::AlreadyFocused;
        }
        self.focused_surfaces.insert(seat, surface);
        InputFocusDecision::Focused
    }

    pub fn clear_surface(&mut self, surface: SurfaceId) -> usize {
        let before = self.focused_surfaces.len();
        self.focused_surfaces
            .retain(|_, focused| *focused != surface);
        before.saturating_sub(self.focused_surfaces.len())
    }

    pub fn clear_focus(&mut self, seat: SeatId) -> Option<SurfaceId> {
        self.focused_surfaces.remove(&seat)
    }

    pub fn route_keyboard_event(
        &self,
        mut event: InputEventPacket,
        committed_surfaces: &[CommittedSurfaceState],
    ) -> FocusedInputRoute {
        if !matches!(event.kind, InputEventKind::Key { .. }) {
            return FocusedInputRoute::UnsupportedEvent(event);
        }
        let Some(surface) = self.focused_surface(event.seat) else {
            return FocusedInputRoute::NoFocus(event);
        };
        if !committed_surfaces
            .iter()
            .any(|committed| committed.surface == surface)
        {
            return FocusedInputRoute::StaleFocus(event);
        }
        event.target_surface = Some(surface);
        FocusedInputRoute::Routed(event)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputFocusDecision {
    Focused,
    /// The seat already holds this surface, so nothing moved.
    ///
    /// Distinct from `Focused` because a caller that drives protocol on a focus
    /// change must not drive it again for a change that did not happen. It is
    /// reported after the committed-surface check, so it always implies the
    /// surface is still committed.
    AlreadyFocused,
    InvalidSeat,
    UnknownSurface,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FocusedInputRoute {
    Routed(InputEventPacket),
    NoFocus(InputEventPacket),
    StaleFocus(InputEventPacket),
    UnsupportedEvent(InputEventPacket),
}
