use sophia_protocol::{DeviceId, SeatId, SurfaceId};

const EVDEV_KEY_LEFTCTRL: u32 = 29;
const EVDEV_KEY_LEFTALT: u32 = 56;
const EVDEV_KEY_RIGHTCTRL: u32 = 97;
const EVDEV_KEY_RIGHTALT: u32 = 100;
const EVDEV_KEY_LEFTSHIFT: u32 = 42;
const EVDEV_KEY_RIGHTSHIFT: u32 = 54;
const SHIFTED_PRINTABLE_KEYCODES: [u32; 21] = [
    41, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 26, 27, 43, 39, 40, 51, 52, 53,
];
pub const SESSION_CLIENT_PRESSED_KEY_CAPACITY: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionClientPressedKey {
    pub surface: SurfaceId,
    pub seat: SeatId,
    pub device: DeviceId,
    pub keycode: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SessionClientKeyMetrics {
    pub peak_pressed: usize,
    pub synthetic_releases: usize,
    pub state_only_releases: usize,
    pub orphan_releases_suppressed: usize,
    pub removed_surface_keys: usize,
}

#[derive(Debug, Default)]
pub struct SessionClientKeyState {
    pressed: Vec<SessionClientPressedKey>,
    metrics: SessionClientKeyMetrics,
}

impl SessionClientKeyState {
    pub fn is_pressed(&self, key: SessionClientPressedKey) -> bool {
        self.pressed.contains(&key)
    }

    pub fn release_is_routable(&self, key: SessionClientPressedKey) -> bool {
        self.is_pressed(key)
    }

    pub fn record_routed(
        &mut self,
        key: SessionClientPressedKey,
        pressed: bool,
    ) -> Result<(), &'static str> {
        if pressed {
            if self.pressed.contains(&key) {
                return Ok(());
            }
            if self.pressed.len() >= SESSION_CLIENT_PRESSED_KEY_CAPACITY {
                return Err("client pressed-key ledger is full");
            }
            self.pressed.push(key);
            self.metrics.peak_pressed = self.metrics.peak_pressed.max(self.pressed.len());
        } else if let Some(index) = self.pressed.iter().position(|pressed| *pressed == key) {
            self.pressed.swap_remove(index);
        } else {
            self.metrics.orphan_releases_suppressed =
                self.metrics.orphan_releases_suppressed.saturating_add(1);
        }
        Ok(())
    }

    pub fn copy_surface_keys(
        &self,
        surface: SurfaceId,
        destination: &mut Vec<SessionClientPressedKey>,
    ) {
        destination.clear();
        destination.extend(
            self.pressed
                .iter()
                .copied()
                .filter(|pressed| pressed.surface == surface),
        );
    }

    pub fn record_synthetic_release(&mut self, key: SessionClientPressedKey) {
        if let Some(index) = self.pressed.iter().position(|pressed| *pressed == key) {
            self.pressed.swap_remove(index);
            self.metrics.synthetic_releases = self.metrics.synthetic_releases.saturating_add(1);
        }
    }

    pub fn record_state_only_release(&mut self, key: SessionClientPressedKey) {
        if let Some(index) = self.pressed.iter().position(|pressed| *pressed == key) {
            self.pressed.swap_remove(index);
            self.metrics.state_only_releases = self.metrics.state_only_releases.saturating_add(1);
        }
    }

    pub fn clear_surface(&mut self, surface: SurfaceId) -> usize {
        let before = self.pressed.len();
        self.pressed.retain(|pressed| pressed.surface != surface);
        let removed = before.saturating_sub(self.pressed.len());
        self.metrics.removed_surface_keys =
            self.metrics.removed_surface_keys.saturating_add(removed);
        removed
    }

    pub fn pending_len(&self) -> usize {
        self.pressed.len()
    }

    pub fn metrics(&self) -> SessionClientKeyMetrics {
        self.metrics
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PhysicalKeyboardCoverage {
    left_shift: bool,
    right_shift: bool,
    shifted_printable_mask: u32,
    virtual_terminal_mask: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalKeyboardCoverageSnapshot {
    pub shifted_positions: u32,
    pub shifted_positions_required: u32,
    pub virtual_terminals: u32,
    pub virtual_terminals_required: u32,
}

impl PhysicalKeyboardCoverage {
    pub fn observe_key(&mut self, keycode: u32, pressed: bool) {
        match keycode {
            EVDEV_KEY_LEFTSHIFT => self.left_shift = pressed,
            EVDEV_KEY_RIGHTSHIFT => self.right_shift = pressed,
            _ if pressed && (self.left_shift || self.right_shift) => {
                if let Some(index) = SHIFTED_PRINTABLE_KEYCODES
                    .iter()
                    .position(|candidate| *candidate == keycode)
                {
                    self.shifted_printable_mask |= 1 << index;
                }
            }
            _ => {}
        }
    }

    pub fn observe_virtual_terminal(&mut self, terminal: u8) {
        if (1..=12).contains(&terminal) {
            self.virtual_terminal_mask |= 1 << (terminal - 1);
        }
    }

    pub fn snapshot(self) -> PhysicalKeyboardCoverageSnapshot {
        PhysicalKeyboardCoverageSnapshot {
            shifted_positions: self.shifted_printable_mask.count_ones(),
            shifted_positions_required: SHIFTED_PRINTABLE_KEYCODES.len() as u32,
            virtual_terminals: self.virtual_terminal_mask.count_ones(),
            virtual_terminals_required: 12,
        }
    }
}

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
