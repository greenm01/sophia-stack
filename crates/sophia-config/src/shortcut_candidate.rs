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

fn positional_string<'a>(node: &'a KdlNode, index: usize) -> Option<&'a str> {
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
                || matches!(byte, b'_' | b'-' | b'=' | b'?' | b',' | b'.' | b'[' | b']')
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
    let trigger = trigger.to_ascii_lowercase();
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
    if node.entries().len() != 2 || node.children().is_some() || node.ty().is_some() {
        return Err(schema_error("binding requires trigger and target strings"));
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
    Ok(DesktopShortcutBinding {
        chord: parse_chord(kind, trigger)?,
        target: parse_target(kind, target)?,
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
