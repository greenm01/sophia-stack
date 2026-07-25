const EVDEV_KEY_LEFTCTRL: u32 = 29;
const EVDEV_KEY_LEFTALT: u32 = 56;
const EVDEV_KEY_RIGHTCTRL: u32 = 97;
const EVDEV_KEY_RIGHTALT: u32 = 100;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VirtualTerminalChordState {
    left_control: bool,
    right_control: bool,
    left_alt: bool,
    right_alt: bool,
    function_keys_down: u16,
    consumed_function_keys: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtualTerminalChordAction {
    Pass,
    Consume,
    Activate(u8),
}

impl VirtualTerminalChordState {
    pub fn observe(&mut self, keycode: u32, pressed: bool) -> VirtualTerminalChordAction {
        match keycode {
            EVDEV_KEY_LEFTCTRL => self.left_control = pressed,
            EVDEV_KEY_RIGHTCTRL => self.right_control = pressed,
            EVDEV_KEY_LEFTALT => self.left_alt = pressed,
            EVDEV_KEY_RIGHTALT => self.right_alt = pressed,
            _ => {
                let Some(terminal) = virtual_terminal_for_evdev_key(keycode) else {
                    return VirtualTerminalChordAction::Pass;
                };
                let bit = 1u16 << (terminal - 1);
                let was_down = self.function_keys_down & bit != 0;
                if pressed {
                    self.function_keys_down |= bit;
                } else {
                    self.function_keys_down &= !bit;
                }
                if !pressed {
                    let consumed = self.consumed_function_keys & bit != 0;
                    self.consumed_function_keys &= !bit;
                    return if consumed {
                        VirtualTerminalChordAction::Consume
                    } else {
                        VirtualTerminalChordAction::Pass
                    };
                }
                if self.consumed_function_keys & bit != 0 {
                    return VirtualTerminalChordAction::Consume;
                }
                if !was_down && self.control() && self.alt() {
                    self.consumed_function_keys |= bit;
                    return VirtualTerminalChordAction::Activate(terminal);
                }
                return VirtualTerminalChordAction::Pass;
            }
        }
        VirtualTerminalChordAction::Pass
    }

    pub const fn pressed_modifier_keycodes(self) -> [Option<u32>; 4] {
        [
            if self.left_control {
                Some(EVDEV_KEY_LEFTCTRL)
            } else {
                None
            },
            if self.right_control {
                Some(EVDEV_KEY_RIGHTCTRL)
            } else {
                None
            },
            if self.left_alt {
                Some(EVDEV_KEY_LEFTALT)
            } else {
                None
            },
            if self.right_alt {
                Some(EVDEV_KEY_RIGHTALT)
            } else {
                None
            },
        ]
    }

    const fn control(self) -> bool {
        self.left_control || self.right_control
    }

    const fn alt(self) -> bool {
        self.left_alt || self.right_alt
    }
}

const fn virtual_terminal_for_evdev_key(keycode: u32) -> Option<u8> {
    match keycode {
        59..=68 => Some((keycode - 58) as u8),
        87 => Some(11),
        88 => Some(12),
        _ => None,
    }
}
