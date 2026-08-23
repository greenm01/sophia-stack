use std::collections::BTreeMap;

use sophia_protocol::{DeviceId, InputEventKind, OutputId, Point, SeatId};

use crate::{PresentedChromeTarget, ToplevelActionCapabilityRef};

pub const MAX_PRESENTED_CHROME_CAPTURE_SEATS: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentedChromeCapture {
    pub seat: SeatId,
    pub device: DeviceId,
    pub button: u32,
    pub output: OutputId,
    pub presentation_epoch: u64,
    pub target: PresentedChromeTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentedChromeCaptureCandidate {
    pub seat: SeatId,
    pub device: DeviceId,
    pub button: u32,
    pub output: OutputId,
    pub presentation_epoch: u64,
    pub target: PresentedChromeTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentedChromeCaptureError {
    InvalidCandidate,
    SeatAlreadyOwned,
    SeatCapacityExceeded,
    ActivationIdentityExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentedChromePointerDisposition {
    Pass,
    Consumed,
    Captured,
    Cancelled,
    Activated {
        action: ToplevelActionCapabilityRef,
        activation: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentedChromeCaptureState {
    captures: BTreeMap<SeatId, PresentedChromeCapture>,
    next_activation: u64,
}

impl Default for PresentedChromeCaptureState {
    fn default() -> Self {
        Self {
            captures: BTreeMap::new(),
            next_activation: 1,
        }
    }
}

impl PresentedChromeCaptureState {
    pub fn capture(&self, seat: SeatId) -> Option<&PresentedChromeCapture> {
        self.captures.get(&seat)
    }

    pub fn begin(
        &mut self,
        candidate: PresentedChromeCaptureCandidate,
    ) -> Result<PresentedChromeCapture, PresentedChromeCaptureError> {
        if !candidate.seat.is_valid()
            || !candidate.device.is_valid()
            || candidate.button != crate::CHROME_PRIMARY_BUTTON
            || !candidate.output.is_valid()
            || candidate.presentation_epoch == 0
            || candidate.target.output != candidate.output
            || candidate.target.id.authority_session_epoch == 0
            || candidate.target.id.generation == 0
            || candidate.target.geometry.is_empty()
            || candidate.target.action.token == 0
            || candidate.target.action.target_slot != candidate.target.id.slot
            || candidate.target.action.target_generation != candidate.target.id.generation
            || candidate.target.action.recipient_epoch
                != candidate.target.id.authority_session_epoch
        {
            return Err(PresentedChromeCaptureError::InvalidCandidate);
        }
        if self.captures.contains_key(&candidate.seat) {
            return Err(PresentedChromeCaptureError::SeatAlreadyOwned);
        }
        if self.captures.len() >= MAX_PRESENTED_CHROME_CAPTURE_SEATS {
            return Err(PresentedChromeCaptureError::SeatCapacityExceeded);
        }
        let capture = PresentedChromeCapture {
            seat: candidate.seat,
            device: candidate.device,
            button: candidate.button,
            output: candidate.output,
            presentation_epoch: candidate.presentation_epoch,
            target: candidate.target,
        };
        self.captures.insert(capture.seat, capture.clone());
        Ok(capture)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn route_captured(
        &mut self,
        seat: SeatId,
        device: DeviceId,
        kind: InputEventKind,
        position: Option<Point>,
        output: Option<OutputId>,
        presentation_epoch: u64,
        targets: &[PresentedChromeTarget],
    ) -> Result<PresentedChromePointerDisposition, PresentedChromeCaptureError> {
        let Some(capture) = self.captures.get(&seat).cloned() else {
            return Ok(PresentedChromePointerDisposition::Pass);
        };
        let current = targets.iter().find(|target| target.id == capture.target.id);
        if output != Some(capture.output)
            || presentation_epoch != capture.presentation_epoch
            || current != Some(&capture.target)
        {
            self.captures.remove(&seat);
            return Ok(PresentedChromePointerDisposition::Cancelled);
        }
        let InputEventKind::PointerButton { button, pressed } = kind else {
            return Ok(PresentedChromePointerDisposition::Consumed);
        };
        if device != capture.device || button != capture.button {
            return Ok(PresentedChromePointerDisposition::Consumed);
        }
        if pressed {
            return Ok(PresentedChromePointerDisposition::Consumed);
        }
        self.captures.remove(&seat);
        if !position.is_some_and(|position| contains(capture.target.geometry, position)) {
            return Ok(PresentedChromePointerDisposition::Consumed);
        }
        let activation = self.next_activation;
        self.next_activation = self
            .next_activation
            .checked_add(1)
            .ok_or(PresentedChromeCaptureError::ActivationIdentityExhausted)?;
        Ok(PresentedChromePointerDisposition::Activated {
            action: capture.target.action,
            activation,
        })
    }

    pub fn cancel_all(&mut self) -> Vec<PresentedChromeCapture> {
        std::mem::take(&mut self.captures).into_values().collect()
    }
}

#[allow(clippy::too_many_arguments)]
pub fn resolve_presented_chrome_pointer_event(
    state: &mut PresentedChromeCaptureState,
    seat: SeatId,
    device: DeviceId,
    kind: InputEventKind,
    position: Option<Point>,
    output: Option<OutputId>,
    presentation_epoch: u64,
    targets: &[PresentedChromeTarget],
    occlusion: Option<sophia_protocol::Rect>,
    application_owned: bool,
) -> Result<PresentedChromePointerDisposition, PresentedChromeCaptureError> {
    let retained = state.route_captured(
        seat,
        device,
        kind,
        position,
        output,
        presentation_epoch,
        targets,
    )?;
    if retained != PresentedChromePointerDisposition::Pass {
        return Ok(retained);
    }
    if application_owned {
        return Ok(PresentedChromePointerDisposition::Pass);
    }
    let Some(position) = position else {
        return Ok(PresentedChromePointerDisposition::Pass);
    };
    let target = targets
        .iter()
        .find(|target| contains(target.geometry, position));
    let occluded = occlusion.is_some_and(|geometry| contains(geometry, position));
    if target.is_none() && !occluded {
        return Ok(PresentedChromePointerDisposition::Pass);
    }
    let InputEventKind::PointerButton { button, pressed } = kind else {
        return Ok(PresentedChromePointerDisposition::Consumed);
    };
    let Some(target) = target else {
        return Ok(PresentedChromePointerDisposition::Consumed);
    };
    if !pressed || button != crate::CHROME_PRIMARY_BUTTON {
        return Ok(PresentedChromePointerDisposition::Consumed);
    }
    state.begin(PresentedChromeCaptureCandidate {
        seat,
        device,
        button,
        output: target.output,
        presentation_epoch,
        target: target.clone(),
    })?;
    Ok(PresentedChromePointerDisposition::Captured)
}

fn contains(rect: sophia_protocol::Rect, point: Point) -> bool {
    point.x >= f64::from(rect.x)
        && point.y >= f64::from(rect.y)
        && point.x < f64::from(rect.x.saturating_add(rect.width))
        && point.y < f64::from(rect.y.saturating_add(rect.height))
}
