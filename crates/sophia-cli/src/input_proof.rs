#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalTextProofEvent {
    pub keycode: u8,
    pub pressed: bool,
    pub state: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalTextProofProgress {
    Awaiting,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalTextProofMismatch {
    pub event_index: usize,
    pub expected: PhysicalTextProofEvent,
    pub observed: PhysicalTextProofEvent,
}

pub const fn pointer_selection_pending(required: bool, routed_buttons: usize) -> bool {
    required && routed_buttons == 0
}

/// Modifier transitions do not produce application text. They may surround an
/// Engine-owned shortcut before the shortcut's non-modifier key is consumed,
/// so they are outside the exact text-producing sequence.
///
/// The locking modifiers belong here for the same reason, and their absence cost
/// a physical run: caps lock sits against the home row, and clipping it while
/// reaching for a neighbouring letter ended the session on a key that types
/// nothing. Locking one on still fails the proof, and should -- every character
/// after it is genuinely a different one -- but the transition itself is not text.
pub const fn physical_text_proof_ignores_evdev_key(keycode: u32) -> bool {
    matches!(
        keycode,
        29 | 42 | 54 | 56 | 58 | 69 | 70 | 97 | 100 | 125 | 126
    )
}

pub const fn pointer_selection_waiting(
    required: bool,
    text_complete: bool,
    input_pixels_presented: bool,
    cursor_ready: bool,
    routed_buttons: usize,
    pointer_pixels_changed: bool,
) -> bool {
    required
        && text_complete
        && input_pixels_presented
        && cursor_ready
        && (routed_buttons == 0 || !pointer_pixels_changed)
}

pub const fn application_exit_overdue(
    application_proof: bool,
    surface_missing: bool,
    primary_exited: bool,
) -> bool {
    application_proof && surface_missing && !primary_exited
}

pub const fn cursor_repaint_preserves_application(
    layers_composed: usize,
    nonzero_pixel_bytes: usize,
) -> bool {
    const CURSOR_ONLY_MAX_NONZERO_BYTES: usize = 12 * 16 * 4;
    layers_composed > 0 && nonzero_pixel_bytes > CURSOR_ONLY_MAX_NONZERO_BYTES
}
pub fn pointer_proof_suppresses_return(required: bool, keycode: u32, text_complete: bool) -> bool {
    required && text_complete && keycode == 28
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PhysicalTextProofBuildError {
    InvalidText,
    UnsupportedCharacter(u8),
}

impl core::fmt::Display for PhysicalTextProofBuildError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidText => {
                formatter.write_str("physical text proof requires 1-24 lowercase ASCII letters")
            }
            Self::UnsupportedCharacter(byte) => {
                write!(
                    formatter,
                    "physical text proof has no keycode for byte {byte}"
                )
            }
        }
    }
}

impl std::error::Error for PhysicalTextProofBuildError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalTextProof {
    expected: Vec<PhysicalTextProofEvent>,
    matched_presses: usize,
    matched_events: usize,
    pressed_keycodes: Vec<u8>,
}

impl PhysicalTextProof {
    pub fn new(text: &str) -> Result<Self, PhysicalTextProofBuildError> {
        Self::build(text, true)
    }

    pub fn new_without_submit(text: &str) -> Result<Self, PhysicalTextProofBuildError> {
        Self::build(text, false)
    }

    fn build(text: &str, submit: bool) -> Result<Self, PhysicalTextProofBuildError> {
        if text.is_empty() || text.len() > 24 || !text.bytes().all(|byte| byte.is_ascii_lowercase())
        {
            return Err(PhysicalTextProofBuildError::InvalidText);
        }

        let mut expected = Vec::with_capacity((text.len() + 1).saturating_mul(2));
        for byte in text.bytes() {
            let keycode = x11_keycode_for_lowercase_ascii(byte)
                .ok_or(PhysicalTextProofBuildError::UnsupportedCharacter(byte))?;
            push_key_pair(&mut expected, keycode);
        }
        if submit {
            push_key_pair(&mut expected, 36);
        }

        Ok(Self {
            expected,
            matched_presses: 0,
            matched_events: 0,
            pressed_keycodes: Vec::new(),
        })
    }

    pub fn observe(
        &mut self,
        observed: PhysicalTextProofEvent,
    ) -> Result<PhysicalTextProofProgress, PhysicalTextProofMismatch> {
        if self.is_complete() {
            return Ok(PhysicalTextProofProgress::Complete);
        }
        let expected_press_index = self.matched_presses.saturating_mul(2);
        let expected = self
            .expected
            .get(expected_press_index)
            .copied()
            .unwrap_or(observed);
        let matches = if observed.pressed {
            observed == expected && !self.pressed_keycodes.contains(&observed.keycode)
        } else {
            observed.state == 0 && self.pressed_keycodes.contains(&observed.keycode)
        };
        if !matches {
            return Err(PhysicalTextProofMismatch {
                event_index: self.matched_events,
                expected,
                observed,
            });
        }
        if observed.pressed {
            self.matched_presses = self.matched_presses.saturating_add(1);
            self.pressed_keycodes.push(observed.keycode);
        } else {
            self.pressed_keycodes
                .retain(|keycode| *keycode != observed.keycode);
        }
        self.matched_events = self.matched_events.saturating_add(1);
        Ok(if self.is_complete() {
            PhysicalTextProofProgress::Complete
        } else {
            PhysicalTextProofProgress::Awaiting
        })
    }

    pub fn expected_events(&self) -> usize {
        self.expected.len()
    }

    pub fn matched_events(&self) -> usize {
        self.matched_events
    }

    pub fn is_complete(&self) -> bool {
        self.matched_presses.saturating_mul(2) == self.expected.len()
            && self.pressed_keycodes.is_empty()
    }
}

fn push_key_pair(expected: &mut Vec<PhysicalTextProofEvent>, keycode: u8) {
    for pressed in [true, false] {
        expected.push(PhysicalTextProofEvent {
            keycode,
            pressed,
            state: 0,
        });
    }
}

fn x11_keycode_for_lowercase_ascii(byte: u8) -> Option<u8> {
    b"qwertyuiop"
        .iter()
        .position(|candidate| *candidate == byte)
        .map(|index| 24 + index as u8)
        .or_else(|| {
            b"asdfghjkl"
                .iter()
                .position(|candidate| *candidate == byte)
                .map(|index| 38 + index as u8)
        })
        .or_else(|| {
            b"zxcvbnm"
                .iter()
                .position(|candidate| *candidate == byte)
                .map(|index| 52 + index as u8)
        })
}
