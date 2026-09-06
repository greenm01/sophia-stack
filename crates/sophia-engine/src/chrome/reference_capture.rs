use sophia_protocol::{
    DeviceId, InputEventKind, InputEventPacket, OutputId, SeatId, ShellReferenceOperation,
};
use std::collections::BTreeSet;

/// A bounded modal capture attached only to a retired reference presentation.
/// Swallowed presses retain their releases even after the sheet is withdrawn.
#[derive(Default)]
pub struct ReferenceSheetCapture {
    presented: Option<(OutputId, u64)>,
    swallowed: BTreeSet<(SeatId, DeviceId, bool, u32)>,
    wheel_v120: i32,
    dismissing: bool,
}
impl ReferenceSheetCapture {
    pub fn present(&mut self, identity: Option<(OutputId, u64)>) {
        if self.presented != identity {
            self.wheel_v120 = 0;
            self.dismissing = false;
        }
        self.presented = identity;
    }
    pub fn route(
        &mut self,
        event: &InputEventPacket,
    ) -> (bool, Option<(OutputId, u64, ShellReferenceOperation)>) {
        let key = match event.kind {
            InputEventKind::Key { keycode, pressed } => {
                if matches!(keycode, 29 | 42 | 54 | 56 | 97 | 100 | 125 | 126) {
                    return (false, None);
                }
                Some((false, keycode, pressed))
            }
            InputEventKind::PointerButton { button, pressed } => Some((true, button, pressed)),
            _ => None,
        };
        if let Some((button, code, pressed)) = key {
            let id = (event.seat, event.device, button, code);
            if !pressed {
                return (self.swallowed.remove(&id), None);
            }
            if self.swallowed.contains(&id) {
                return (true, None);
            }
            let Some((output, epoch)) = self.presented else {
                return (false, None);
            };
            // Physical input is bounded by evdev's code space. On saturation
            // keep the modal barrier without allowing the set to grow.
            if self.swallowed.len() < 1024 {
                self.swallowed.insert(id);
            }
            if button || self.dismissing {
                return (true, None);
            }
            let op = match code {
                104 => ShellReferenceOperation::Previous,
                109 => ShellReferenceOperation::Next,
                _ => {
                    self.dismissing = true;
                    ShellReferenceOperation::Dismiss
                }
            };
            return (true, Some((output, epoch, op)));
        }
        if let (Some((output, epoch)), InputEventKind::PointerAxis { vertical_v120, .. }) =
            (self.presented, event.kind)
        {
            if self.dismissing {
                return (true, None);
            }
            self.wheel_v120 = self.wheel_v120.saturating_add(vertical_v120);
            let op = if self.wheel_v120 >= 120 {
                Some(ShellReferenceOperation::Next)
            } else if self.wheel_v120 <= -120 {
                Some(ShellReferenceOperation::Previous)
            } else {
                None
            };
            if op.is_some() {
                self.wheel_v120 = 0;
            }
            return (true, op.map(|op| (output, epoch, op)));
        }
        (
            self.presented.is_some() && matches!(event.kind, InputEventKind::PointerMotion),
            None,
        )
    }
}
