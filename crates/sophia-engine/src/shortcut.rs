//! Physical shortcut matching, owed to no protocol revision.
//!
//! Engine matches physical input against registered chords and emits opaque action
//! tokens. That behavior predates API v7 and outlives it: the public
//! `sophia_wm_v1` path resolves its bindings from configuration, and the v7 bridge
//! resolves the same bindings from a `WmHello`. Both arrive here.
//!
//! Nothing in this module mentions an API version, a hello, or a wire frame. The
//! v7 adapters that do live in `wm.rs`, which is what lets that module be deleted
//! when v7 goes without taking shortcut matching with it.

use crate::prelude::*;
use sophia_protocol::{
    WM_MAX_BINDINGS, WmActionId, WmBindingRegistration, WmCapabilities, WmChromePolicy,
    WmModifierMask,
};

/// Why a set of bindings could not become a registry.
///
/// A `&'static str` rather than an enum because the only consumers format it: the
/// v7 path wraps it in `WmIpcError::Negotiation`, and the public path reduces every
/// cause to one message. A parallel enum would duplicate this vocabulary without
/// giving anyone a decision to make on it.
pub type WmShortcutRegistryError = &'static str;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmShortcutRegistry {
    bindings: BTreeMap<(u32, u32), WmActionId>,
    held: BTreeMap<u32, WmActionId>,
    pub(crate) capabilities: WmCapabilities,
    policy_generation: u64,
    chrome: WmChromePolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WmShortcutDecision {
    pub action: Option<WmActionId>,
    pub consumed: bool,
}

impl WmShortcutRegistry {
    /// Builds a registry from bindings a caller already resolved.
    ///
    /// Every rejection here is about the bindings themselves, not about who asked:
    /// an invalid action or keycode, an unsupported modifier, the reserved
    /// emergency chord, a duplicate chord, or more bindings than the wire admits.
    /// A caller that speaks a protocol revision checks its own version first and
    /// then calls this.
    pub fn new(
        bindings: &[WmBindingRegistration],
        capabilities: WmCapabilities,
        policy_generation: u64,
        chrome: WmChromePolicy,
    ) -> Result<Self, WmShortcutRegistryError> {
        if capabilities.bits & !WmCapabilities::SUPPORTED != 0 {
            return Err("unsupported WM capability");
        }
        if policy_generation == 0 {
            return Err("invalid WM policy generation");
        }
        if !valid_chrome_policy(chrome) {
            return Err("invalid WM chrome policy");
        }
        if bindings.len() > WM_MAX_BINDINGS {
            return Err("too many WM bindings");
        }

        let mut resolved = BTreeMap::new();
        for binding in bindings {
            if !binding.action.is_valid() || binding.keycode == 0 || binding.keycode > 0x2ff {
                return Err("invalid WM binding");
            }
            if binding.modifiers.bits & !WmModifierMask::SUPPORTED != 0 {
                return Err("unsupported WM modifier");
            }
            // Ctrl-Alt-Backspace belongs to emergency recovery and is never
            // available to a policy client, whatever it registers.
            if binding.keycode == 14
                && binding.modifiers.bits & (WmModifierMask::CONTROL | WmModifierMask::ALT)
                    == WmModifierMask::CONTROL | WmModifierMask::ALT
            {
                return Err("reserved emergency chord");
            }
            if resolved
                .insert((binding.keycode, binding.modifiers.bits), binding.action)
                .is_some()
            {
                return Err("duplicate WM chord");
            }
        }

        Ok(Self {
            bindings: resolved,
            held: BTreeMap::new(),
            capabilities,
            policy_generation,
            chrome,
        })
    }

    pub fn handle_key(
        &mut self,
        keycode: u32,
        modifiers: WmModifierMask,
        pressed: bool,
    ) -> WmShortcutDecision {
        if !pressed {
            return WmShortcutDecision {
                action: None,
                consumed: self.held.remove(&keycode).is_some(),
            };
        }
        let Some(action) = self.bindings.get(&(keycode, modifiers.bits)).copied() else {
            return WmShortcutDecision {
                action: None,
                consumed: false,
            };
        };
        let first_press = self.held.insert(keycode, action).is_none();
        WmShortcutDecision {
            action: first_press.then_some(action),
            consumed: true,
        }
    }

    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }

    pub const fn policy_generation(&self) -> u64 {
        self.policy_generation
    }

    pub const fn chrome(&self) -> WmChromePolicy {
        self.chrome
    }

    pub const fn supports_chrome_policy(&self) -> bool {
        self.capabilities.bits & WmCapabilities::POLICY_CHROME_V2 != 0
    }

    pub fn is_idle(&self) -> bool {
        self.held.is_empty()
    }
}

pub const WM_MAX_SHORTCUT_SEATS: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmShortcutRouter {
    pub(crate) registry: WmShortcutRegistry,
    seats: BTreeMap<SeatId, WmSeatShortcutState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WmSeatShortcutState {
    shortcuts: WmShortcutRegistry,
    modifiers: WmPhysicalModifierState,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct WmPhysicalModifierState {
    left_shift: bool,
    right_shift: bool,
    left_control: bool,
    right_control: bool,
    left_alt: bool,
    right_alt: bool,
    left_super: bool,
    right_super: bool,
}

impl WmShortcutRouter {
    pub fn new(registry: WmShortcutRegistry) -> Self {
        Self {
            registry,
            seats: BTreeMap::new(),
        }
    }

    pub fn replace_registry(&mut self, registry: WmShortcutRegistry) {
        self.registry = registry;
        self.seats.clear();
    }

    pub fn route_key(&mut self, seat: SeatId, keycode: u32, pressed: bool) -> WmShortcutDecision {
        if !seat.is_valid() {
            return WmShortcutDecision {
                action: None,
                consumed: false,
            };
        }
        if !self.seats.contains_key(&seat) {
            if self.seats.len() >= WM_MAX_SHORTCUT_SEATS {
                return WmShortcutDecision {
                    action: None,
                    consumed: false,
                };
            }
            self.seats.insert(
                seat,
                WmSeatShortcutState {
                    shortcuts: self.registry.clone(),
                    modifiers: WmPhysicalModifierState::default(),
                },
            );
        }
        let state = self.seats.get_mut(&seat).expect("seat was inserted");
        let decision = state
            .shortcuts
            .handle_key(keycode, state.modifiers.mask(), pressed);
        state.modifiers.update(keycode, pressed);
        decision
    }

    pub fn clear_seat(&mut self, seat: SeatId) -> bool {
        self.seats.remove(&seat).is_some()
    }

    pub fn modifier_mask(&self, seat: SeatId) -> WmModifierMask {
        self.seats
            .get(&seat)
            .map(|state| state.modifiers.mask())
            .unwrap_or(WmModifierMask { bits: 0 })
    }

    pub const fn policy_generation(&self) -> u64 {
        self.registry.policy_generation()
    }

    pub fn binding_count(&self) -> usize {
        self.registry.binding_count()
    }

    pub const fn chrome(&self) -> WmChromePolicy {
        self.registry.chrome()
    }

    pub const fn supports_chrome_policy(&self) -> bool {
        self.registry.supports_chrome_policy()
    }

    pub fn shortcut_idle(&self) -> bool {
        self.seats
            .values()
            .all(|state| state.shortcuts.is_idle() && state.modifiers.is_idle())
    }
}

pub(crate) fn valid_chrome_policy(chrome: WmChromePolicy) -> bool {
    let valid_style = |enabled: bool, width: u32| {
        width <= 64 && ((enabled && width > 0) || (!enabled && width == 0))
    };
    valid_style(chrome.focus_ring.enabled, chrome.focus_ring.width)
        && valid_style(chrome.frame.enabled, chrome.frame.width)
}

impl WmPhysicalModifierState {
    fn is_idle(self) -> bool {
        !self.left_shift
            && !self.right_shift
            && !self.left_control
            && !self.right_control
            && !self.left_alt
            && !self.right_alt
            && !self.left_super
            && !self.right_super
    }

    fn mask(self) -> WmModifierMask {
        let mut bits = 0;
        if self.left_shift || self.right_shift {
            bits |= WmModifierMask::SHIFT;
        }
        if self.left_control || self.right_control {
            bits |= WmModifierMask::CONTROL;
        }
        if self.left_alt || self.right_alt {
            bits |= WmModifierMask::ALT;
        }
        if self.left_super || self.right_super {
            bits |= WmModifierMask::SUPER;
        }
        WmModifierMask { bits }
    }

    fn update(&mut self, keycode: u32, pressed: bool) {
        match keycode {
            42 => self.left_shift = pressed,
            54 => self.right_shift = pressed,
            29 => self.left_control = pressed,
            97 => self.right_control = pressed,
            56 => self.left_alt = pressed,
            100 => self.right_alt = pressed,
            125 => self.left_super = pressed,
            126 => self.right_super = pressed,
            _ => {}
        }
    }
}
