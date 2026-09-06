use std::collections::BTreeSet;

use kdl::{KdlDocument, KdlNode};

use crate::{
    ConfigDigest, ConfigGeneration, DesktopAuthority, DesktopAuthorityCandidate,
    DesktopProfileError,
};

pub const DESKTOP_SHORTCUT_MAX_BINDINGS: usize = 256;
pub const DESKTOP_SHORTCUT_MAX_TRIGGER_BYTES: usize = 64;
pub const DESKTOP_SHORTCUT_MAX_TARGET_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DesktopShortcutModifiers(u8);

impl DesktopShortcutModifiers {
    pub const SHIFT: Self = Self(1 << 0);
    pub const CONTROL: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);
    pub const SUPER: Self = Self(1 << 3);

    pub const fn bits(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DesktopShortcutBindingKind {
    Key,
    Pointer,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DesktopShortcutChord {
    pub kind: DesktopShortcutBindingKind,
    pub modifiers: DesktopShortcutModifiers,
    pub trigger: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopSessionShortcut {
    CloseFocused,
    Logout,
    LaunchTerminal,
    LaunchBrowser,
    WindowSwitcher,
    ShortcutHelp,
    /// Re-read the desktop profile and put it into effect, the way a window
    /// manager that ships its config in a file has to offer.
    ReloadProfile,
    /// Replace the policy client with a fresh process, keeping the windows.
    RestartWm,
}

impl DesktopSessionShortcut {
    /// The name this shortcut is written as in a profile, and the name it is
    /// reported by. One vocabulary, so a record naming a dropped shortcut can
    /// be matched against the profile line that asked for it.
    pub const fn profile_name(self) -> &'static str {
        match self {
            Self::CloseFocused => "close-window",
            Self::Logout => "logout",
            Self::LaunchTerminal => "spawn-terminal",
            Self::LaunchBrowser => "spawn-browser",
            Self::WindowSwitcher => "window-switcher",
            Self::ShortcutHelp => "shortcut-help",
            Self::ReloadProfile => "reload-profile",
            Self::RestartWm => "restart-wm",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DesktopShortcutTarget {
    PolicyAction(String),
    Session(DesktopSessionShortcut),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopShortcutBinding {
    pub chord: DesktopShortcutChord,
    pub target: DesktopShortcutTarget,
    pub label: Option<String>,
    pub group: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopShortcutCandidate {
    pub generation: ConfigGeneration,
    pub digest: ConfigDigest,
    pub profile: String,
    pub bindings: Vec<DesktopShortcutBinding>,
}

fn schema_error(message: impl Into<String>) -> DesktopProfileError {
    DesktopProfileError::Schema(format!("shortcut candidate: {}", message.into()))
}

fn single_node(encoded: &str) -> Result<KdlNode, DesktopProfileError> {
    let document = KdlDocument::parse_v2(encoded)
        .map_err(|error| schema_error(format!("invalid staged value: {error}")))?;
    if document.nodes().len() != 1 {
        return Err(schema_error("staged value must contain exactly one node"));
    }
    Ok(document.nodes()[0].clone())
}

fn positional_string(node: &KdlNode, index: usize) -> Option<&str> {
    node.get(index).and_then(|value| value.as_string())
}

fn exact_profile(node: &KdlNode) -> Result<String, DesktopProfileError> {
    if node.entries().len() != 1 || node.children().is_some() || node.ty().is_some() {
        return Err(schema_error("profile requires one string argument"));
    }
    let profile = positional_string(node, 0)
        .filter(|profile| !profile.is_empty() && profile.len() <= 64)
        .ok_or_else(|| schema_error("profile identity is invalid"))?;
    if !profile
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(schema_error(
            "profile identity contains unsupported characters",
        ));
    }
    Ok(profile.to_owned())
}

fn parse_chord(
    kind: DesktopShortcutBindingKind,
    source: &str,
) -> Result<DesktopShortcutChord, DesktopProfileError> {
    if source.is_empty() || source.len() > DESKTOP_SHORTCUT_MAX_TRIGGER_BYTES {
        return Err(schema_error("trigger length is invalid"));
    }
    let parts = source.split('+').collect::<Vec<_>>();
    let (trigger, modifiers) = parts
        .split_last()
        .ok_or_else(|| schema_error("trigger is empty"))?;
    if trigger.is_empty()
        || !trigger.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'_' | b'-' | b'=' | b'?' | b'/' | b',' | b'.' | b'[' | b']'
                )
        })
    {
        return Err(schema_error("trigger key contains unsupported characters"));
    }
    let mut modifier_bits = 0_u8;
    for modifier in modifiers {
        let bit = match modifier.to_ascii_lowercase().as_str() {
            "shift" => DesktopShortcutModifiers::SHIFT.bits(),
            "ctrl" | "control" => DesktopShortcutModifiers::CONTROL.bits(),
            "alt" => DesktopShortcutModifiers::ALT.bits(),
            "super" => DesktopShortcutModifiers::SUPER.bits(),
            _ => return Err(schema_error(format!("unsupported modifier {modifier:?}"))),
        };
        if modifier_bits & bit != 0 {
            return Err(schema_error(format!("duplicate modifier {modifier:?}")));
        }
        modifier_bits |= bit;
    }
    let trigger = match trigger.to_ascii_lowercase().as_str() {
        "?" | "question" => {
            modifier_bits |= DesktopShortcutModifiers::SHIFT.bits();
            "slash".to_owned()
        }
        "/" | "slash" => "slash".to_owned(),
        _ => trigger.to_ascii_lowercase(),
    };
    if kind == DesktopShortcutBindingKind::Pointer
        && !["left", "middle", "right"].contains(&trigger.as_str())
    {
        return Err(schema_error(
            "pointer trigger must name left, middle, or right",
        ));
    }
    if kind == DesktopShortcutBindingKind::Key
        && trigger == "backspace"
        && modifier_bits
            & (DesktopShortcutModifiers::CONTROL.bits() | DesktopShortcutModifiers::ALT.bits())
            == DesktopShortcutModifiers::CONTROL.bits() | DesktopShortcutModifiers::ALT.bits()
    {
        return Err(schema_error(
            "reserved emergency chord cannot be overridden",
        ));
    }
    Ok(DesktopShortcutChord {
        kind,
        modifiers: DesktopShortcutModifiers(modifier_bits),
        trigger,
    })
}

/// Resolve normalized profile key names to the evdev key identity consumed by
/// the session input authority. This table is deliberately independent from
/// policy action semantics.
pub fn desktop_shortcut_evdev_keycode(trigger: &str) -> Option<u32> {
    Some(match trigger {
        "escape" => 1,
        // Function keys. F1..F10 are contiguous from 59; F11 and F12 sit
        // after the numeric block rather than continuing it, which is an
        // accident of the original keyboard and not a mistake here.
        "f1" => 59,
        "f2" => 60,
        "f3" => 61,
        "f4" => 62,
        "f5" => 63,
        "f6" => 64,
        "f7" => 65,
        "f8" => 66,
        "f9" => 67,
        "f10" => 68,
        "f11" => 87,
        "f12" => 88,
        "1" => 2,
        "2" => 3,
        "3" => 4,
        "4" => 5,
        "5" => 6,
        "6" => 7,
        "7" => 8,
        "8" => 9,
        "9" => 10,
        "0" => 11,
        "-" => 12,
        "=" => 13,
        "backspace" => 14,
        "tab" => 15,
        "q" => 16,
        "w" => 17,
        "e" => 18,
        "r" => 19,
        "t" => 20,
        "y" => 21,
        "u" => 22,
        "i" => 23,
        "o" => 24,
        "p" => 25,
        "[" => 26,
        "]" => 27,
        "return" | "enter" => 28,
        "a" => 30,
        "s" => 31,
        "d" => 32,
        "f" => 33,
        "g" => 34,
        "h" => 35,
        "j" => 36,
        "k" => 37,
        "l" => 38,
        "grave" => 41,
        "z" => 44,
        "x" => 45,
        "c" => 46,
        "v" => 47,
        "b" => 48,
        "n" => 49,
        "m" => 50,
        "," => 51,
        "." => 52,
        "?" | "/" | "slash" | "question" => 53,
        "space" => 57,
        "print" => 99,
        "up" => 103,
        "left" => 105,
        "right" => 106,
        "down" => 108,
        "home" => 102,
        "page_up" => 104,
        "end" => 107,
        "page_down" => 109,
        "insert" => 110,
        "delete" => 111,
        _ => return None,
    })
}

fn policy_action(target: &str) -> Result<String, DesktopProfileError> {
    if target.is_empty()
        || target.trim() != target
        || target.len() > DESKTOP_SHORTCUT_MAX_TARGET_BYTES
    {
        return Err(schema_error("policy action length is invalid"));
    }
    if !target
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b' ' | b'.'))
    {
        return Err(schema_error(
            "policy action contains unsupported characters",
        ));
    }
    Ok(target.to_owned())
}

fn parse_target(
    kind: DesktopShortcutBindingKind,
    source: &str,
) -> Result<DesktopShortcutTarget, DesktopProfileError> {
    if source.len() > DESKTOP_SHORTCUT_MAX_TARGET_BYTES {
        return Err(schema_error("target length is invalid"));
    }
    let (authority, target) = source
        .split_once(':')
        .ok_or_else(|| schema_error("target must have an explicit authority prefix"))?;
    match authority {
        "policy" => Ok(DesktopShortcutTarget::PolicyAction(policy_action(target)?)),
        "session" if kind == DesktopShortcutBindingKind::Pointer => Err(schema_error(
            "pointer bindings cannot invoke session capabilities",
        )),
        "session" => {
            let shortcut = match target {
                "close-window" => DesktopSessionShortcut::CloseFocused,
                "logout" => DesktopSessionShortcut::Logout,
                "spawn-terminal" => DesktopSessionShortcut::LaunchTerminal,
                "spawn-browser" => DesktopSessionShortcut::LaunchBrowser,
                "window-switcher" => DesktopSessionShortcut::WindowSwitcher,
                "shortcut-help" => DesktopSessionShortcut::ShortcutHelp,
                "reload-profile" => DesktopSessionShortcut::ReloadProfile,
                "restart-wm" => DesktopSessionShortcut::RestartWm,
                _ => return Err(schema_error("unknown session shortcut capability")),
            };
            Ok(DesktopShortcutTarget::Session(shortcut))
        }
        _ => Err(schema_error(format!(
            "unsupported shortcut target authority {authority:?}"
        ))),
    }
}

fn parse_binding(node: &KdlNode) -> Result<DesktopShortcutBinding, DesktopProfileError> {
    if node.entries().iter().filter(|e| e.name().is_none()).count() != 2
        || node.children().is_some()
        || node.ty().is_some()
        || node
            .entries()
            .iter()
            .filter_map(|e| e.name())
            .any(|n| !matches!(n.value(), "label" | "group"))
    {
        return Err(schema_error(
            "binding requires trigger and target strings with optional label/group",
        ));
    }
    let kind = match node.name().value() {
        "bind" => DesktopShortcutBindingKind::Key,
        "pointer-bind" => DesktopShortcutBindingKind::Pointer,
        _ => return Err(schema_error("unsupported binding kind")),
    };
    let trigger = positional_string(node, 0)
        .ok_or_else(|| schema_error("binding trigger must be a string"))?;
    let target = positional_string(node, 1)
        .ok_or_else(|| schema_error("binding target must be a string"))?;
    let metadata =
        |name: &str, limit: usize| -> Result<Option<String>, DesktopProfileError> {
            if node
                .entries()
                .iter()
                .filter(|e| e.name().is_some_and(|n| n.value() == name))
                .count()
                > 1
            {
                return Err(schema_error("duplicate binding metadata"));
            }
            node.get(name)
                .map(|v| {
                    v.as_string()
                        .filter(|s| {
                            !s.is_empty() && s.len() <= limit && !s.chars().any(|c| {
                                c.is_control()
                                    || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
                            })
                        })
                        .map(str::to_owned)
                        .ok_or_else(|| schema_error("invalid binding metadata"))
                })
                .transpose()
        };
    Ok(DesktopShortcutBinding {
        chord: parse_chord(kind, trigger)?,
        target: parse_target(kind, target)?,
        label: metadata("label", 128)?,
        group: metadata("group", 64)?,
    })
}

pub fn prepare_desktop_shortcut_candidate(
    candidate: &DesktopAuthorityCandidate,
) -> Result<DesktopShortcutCandidate, DesktopProfileError> {
    if candidate.authority != DesktopAuthority::Shortcut {
        return Err(schema_error("candidate crossed its authority boundary"));
    }
    let mut prepared = DesktopShortcutCandidate {
        generation: candidate.generation,
        digest: candidate.digest,
        profile: String::new(),
        bindings: Vec::new(),
    };
    let mut chords = BTreeSet::new();
    for value in &candidate.values {
        let node = single_node(&value.encoded)?;
        match node.name().value() {
            "profile" => {
                if !prepared.profile.is_empty() {
                    return Err(schema_error("duplicate profile identity"));
                }
                prepared.profile = exact_profile(&node)?;
            }
            "bind" | "pointer-bind" => {
                if prepared.bindings.len() >= DESKTOP_SHORTCUT_MAX_BINDINGS {
                    return Err(schema_error("binding count exceeds 256"));
                }
                let binding = parse_binding(&node)?;
                if !chords.insert(binding.chord.clone()) {
                    return Err(schema_error("duplicate physical chord"));
                }
                prepared.bindings.push(binding);
            }
            _ => return Err(schema_error("candidate contains a non-shortcut setting")),
        }
    }
    if prepared.profile.is_empty() && !prepared.bindings.is_empty() {
        return Err(schema_error("profile identity is required"));
    }
    Ok(prepared)
}
