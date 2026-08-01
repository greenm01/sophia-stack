/// XI2 reserves valuators 0 and 1 for relative pointer X and Y.
pub const X_POINTER_HORIZONTAL_SCROLL_VALUATOR: u16 = 2;
/// The vertical scroll valuator follows pointer X/Y and horizontal scrolling.
pub const X_POINTER_VERTICAL_SCROLL_VALUATOR: u16 = 3;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XCorePointerMapper {
    button_state: u16,
    horizontal_scroll_v120: i32,
    vertical_scroll_v120: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XScrollAxisUpdate {
    pub button: u8,
    pub horizontal_position_v120: Option<i32>,
    pub vertical_position_v120: Option<i32>,
}

impl XCorePointerMapper {
    const fn core_button_mask(button: u8) -> u16 {
        if button <= 5 { 1u16 << (button + 7) } else { 0 }
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub const fn state(self) -> u16 {
        self.button_state
    }

    pub const fn horizontal_scroll_position_v120(self) -> i32 {
        self.horizontal_scroll_v120
    }

    pub const fn vertical_scroll_position_v120(self) -> i32 {
        self.vertical_scroll_v120
    }

    pub fn map_evdev_button(&mut self, evdev_button: u32, pressed: bool) -> Option<(u8, u16)> {
        let (button, mask) = match evdev_button {
            272 => (1, 1 << 8),
            274 => (2, 1 << 9),
            273 => (3, 1 << 10),
            275 => (8, 0),
            276 => (9, 0),
            _ => return None,
        };
        let state = self.button_state;
        if pressed {
            self.button_state |= mask;
        } else {
            self.button_state &= !mask;
        }
        Some((button, state))
    }

    pub const fn map_axis_to_button(horizontal_v120: i32, vertical_v120: i32) -> Option<u8> {
        if vertical_v120 < 0 {
            Some(4)
        } else if vertical_v120 > 0 {
            Some(5)
        } else if horizontal_v120 < 0 {
            Some(6)
        } else if horizontal_v120 > 0 {
            Some(7)
        } else {
            None
        }
    }

    pub fn map_axis(
        &mut self,
        horizontal_v120: i32,
        vertical_v120: i32,
    ) -> Option<XScrollAxisUpdate> {
        let button = Self::map_axis_to_button(horizontal_v120, vertical_v120)?;
        let horizontal_position_v120 = (horizontal_v120 != 0).then(|| {
            self.horizontal_scroll_v120 =
                self.horizontal_scroll_v120.saturating_add(horizontal_v120);
            self.horizontal_scroll_v120
        });
        let vertical_position_v120 = (vertical_v120 != 0).then(|| {
            self.vertical_scroll_v120 = self.vertical_scroll_v120.saturating_add(vertical_v120);
            self.vertical_scroll_v120
        });
        Some(XScrollAxisUpdate {
            button,
            horizontal_position_v120,
            vertical_position_v120,
        })
    }

    pub const fn axis_release_state(self, button: u8) -> u16 {
        self.button_state | Self::core_button_mask(button)
    }
}
