use std::collections::BTreeMap;

use sophia_protocol::{DeviceId, InputEventKind, OutputId, Point, SeatId, WmActionId};

use crate::IndicatorChromeHitTarget;

pub const CHROME_PRIMARY_BUTTON: u32 = 0x110;
pub const MAX_CHROME_CAPTURE_SEATS: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChromeCapture {
    pub seat: SeatId,
    pub device: DeviceId,
    pub button: u32,
    pub output: OutputId,
    pub presentation_epoch: u64,
    pub target: IndicatorChromeHitTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChromeCaptureCandidate {
    pub seat: SeatId,
    pub device: DeviceId,
    pub button: u32,
    pub output: OutputId,
    pub presentation_epoch: u64,
    pub target: IndicatorChromeHitTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChromeCaptureError {
    InvalidCandidate,
    SeatAlreadyOwned,
    SeatCapacityExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChromePointerDisposition {
    Pass,
    Consumed,
    Captured,
    Cancelled,
    Activated {
        output: OutputId,
        action: WmActionId,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChromeCaptureState {
    captures: BTreeMap<SeatId, ChromeCapture>,
}

impl ChromeCaptureState {
    pub fn capture(&self, seat: SeatId) -> Option<&ChromeCapture> {
        self.captures.get(&seat)
    }

    pub fn begin(
        &mut self,
        candidate: ChromeCaptureCandidate,
    ) -> Result<ChromeCapture, ChromeCaptureError> {
        if !candidate.seat.is_valid()
            || !candidate.device.is_valid()
            || candidate.button != CHROME_PRIMARY_BUTTON
            || !candidate.output.is_valid()
            || candidate.presentation_epoch == 0
            || candidate.target.output != candidate.output
            || candidate.target.connection_epoch == 0
            || candidate.target.projection_commit_serial == 0
            || candidate.target.action.is_none()
            || candidate.target.geometry.is_empty()
        {
            return Err(ChromeCaptureError::InvalidCandidate);
        }
        if self.captures.contains_key(&candidate.seat) {
            return Err(ChromeCaptureError::SeatAlreadyOwned);
        }
        if self.captures.len() >= MAX_CHROME_CAPTURE_SEATS {
            return Err(ChromeCaptureError::SeatCapacityExceeded);
        }
        let capture = ChromeCapture {
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

    pub fn route_captured(
        &mut self,
        seat: SeatId,
        device: DeviceId,
        kind: InputEventKind,
        position: Option<Point>,
        output: Option<OutputId>,
        presentation_epoch: u64,
        targets: &[IndicatorChromeHitTarget],
    ) -> ChromePointerDisposition {
        let Some(capture) = self.captures.get(&seat).cloned() else {
            return ChromePointerDisposition::Pass;
        };
        let current = targets.iter().find(|target| {
            target.output == capture.target.output && target.indicator == capture.target.indicator
        });
        if output != Some(capture.output)
            || presentation_epoch != capture.presentation_epoch
            || current != Some(&capture.target)
        {
            self.captures.remove(&seat);
            return ChromePointerDisposition::Cancelled;
        }
        let InputEventKind::PointerButton { button, pressed } = kind else {
            return ChromePointerDisposition::Consumed;
        };
        if device != capture.device || button != capture.button {
            return ChromePointerDisposition::Consumed;
        }
        if pressed {
            return ChromePointerDisposition::Consumed;
        }
        self.captures.remove(&seat);
        if !position.is_some_and(|position| contains(capture.target.geometry, position)) {
            return ChromePointerDisposition::Consumed;
        }
        capture
            .target
            .action
            .map_or(ChromePointerDisposition::Consumed, |action| {
                ChromePointerDisposition::Activated {
                    output: capture.output,
                    action,
                }
            })
    }

    pub fn cancel_all(&mut self) -> Vec<ChromeCapture> {
        std::mem::take(&mut self.captures).into_values().collect()
    }
}

pub fn resolve_chrome_pointer_event(
    state: &mut ChromeCaptureState,
    seat: SeatId,
    device: DeviceId,
    kind: InputEventKind,
    position: Option<Point>,
    output: Option<OutputId>,
    presentation_epoch: u64,
    targets: &[IndicatorChromeHitTarget],
    occlusion: Option<sophia_protocol::Rect>,
    application_owned: bool,
) -> Result<ChromePointerDisposition, ChromeCaptureError> {
    let retained = state.route_captured(
        seat,
        device,
        kind,
        position,
        output,
        presentation_epoch,
        targets,
    );
    if retained != ChromePointerDisposition::Pass {
        return Ok(retained);
    }
    if application_owned {
        return Ok(ChromePointerDisposition::Pass);
    }
    let Some(position) = position else {
        return Ok(ChromePointerDisposition::Pass);
    };
    let target = targets
        .iter()
        .find(|target| contains(target.geometry, position));
    let occluded = occlusion.is_some_and(|geometry| contains(geometry, position));
    if target.is_none() && !occluded {
        return Ok(ChromePointerDisposition::Pass);
    }
    let InputEventKind::PointerButton { button, pressed } = kind else {
        return Ok(ChromePointerDisposition::Consumed);
    };
    let Some(target) = target else {
        return Ok(ChromePointerDisposition::Consumed);
    };
    if !pressed || button != CHROME_PRIMARY_BUTTON || target.action.is_none() {
        return Ok(ChromePointerDisposition::Consumed);
    }
    state.begin(ChromeCaptureCandidate {
        seat,
        device,
        button,
        output: target.output,
        presentation_epoch,
        target: target.clone(),
    })?;
    Ok(ChromePointerDisposition::Captured)
}

fn contains(rect: sophia_protocol::Rect, point: Point) -> bool {
    point.x >= f64::from(rect.x)
        && point.y >= f64::from(rect.y)
        && point.x < f64::from(rect.x.saturating_add(rect.width))
        && point.y < f64::from(rect.y.saturating_add(rect.height))
}
