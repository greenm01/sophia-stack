use kdl::{KdlDocument, KdlNode};

use crate::{
    ConfigDigest, ConfigGeneration, DesktopAuthority, DesktopAuthorityCandidate,
    DesktopProfileError,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DesktopXkbCandidate {
    pub rules: Option<String>,
    pub model: Option<String>,
    pub layout: Option<String>,
    pub variant: Option<String>,
    pub options: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DesktopKeyboardCandidate {
    pub repeat_rate: Option<u32>,
    pub repeat_delay_msec: Option<u64>,
    pub num_lock: Option<bool>,
    pub caps_lock: Option<bool>,
    pub xkb: Option<DesktopXkbCandidate>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopPointerAccelProfile {
    Flat,
    Adaptive,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DesktopPointerCandidate {
    pub natural_scroll: Option<bool>,
    pub accel_profile: Option<DesktopPointerAccelProfile>,
    pub accel_speed: Option<f64>,
    pub left_handed: Option<bool>,
    pub middle_emulation: Option<bool>,
    pub scroll_factor: Option<f64>,
}

/// The pointer's appearance, as the desktop profile may state it.
///
/// Every field is optional because the profile overrides the core config
/// rather than replacing it: a profile that names only a theme keeps the core
/// size, and a profile with no cursor block at all leaves the core config
/// entirely in charge. `shape` is deliberately absent -- it selects a semantic
/// cursor the Engine draws, which is not something a desktop profile has an
/// opinion about.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DesktopCursorCandidate {
    pub theme: Option<String>,
    pub size: Option<u32>,
    pub shake_to_find: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DesktopInputCandidate {
    pub generation: ConfigGeneration,
    pub digest: ConfigDigest,
    pub inherit_sophia: bool,
    pub keyboard: Option<DesktopKeyboardCandidate>,
    pub pointer: Option<DesktopPointerCandidate>,
    pub cursor: Option<DesktopCursorCandidate>,
}

fn schema_error(message: impl Into<String>) -> DesktopProfileError {
    DesktopProfileError::Schema(format!("input candidate: {}", message.into()))
}

fn single_node(encoded: &str) -> Result<KdlNode, DesktopProfileError> {
    let document = KdlDocument::parse_v2(encoded)
        .map_err(|error| schema_error(format!("invalid staged value: {error}")))?;
    if document.nodes().len() != 1 {
        return Err(schema_error("staged value must contain exactly one node"));
    }
    Ok(document.nodes()[0].clone())
}

fn children<'a>(node: &'a KdlNode, setting: &str) -> Result<&'a KdlDocument, DesktopProfileError> {
    if !node.entries().is_empty() || node.ty().is_some() {
        return Err(schema_error(format!("{setting} has an ambiguous shape")));
    }
    node.children()
        .ok_or_else(|| schema_error(format!("{setting} requires children")))
}

fn one_bool(node: &KdlNode, setting: &str) -> Result<bool, DesktopProfileError> {
    if node.entries().len() != 1 || node.children().is_some() || node.ty().is_some() {
        return Err(schema_error(format!("{setting} requires one boolean")));
    }
    node.get(0)
        .and_then(|value| value.as_bool())
        .ok_or_else(|| schema_error(format!("{setting} requires one boolean")))
}

fn one_integer(
    node: &KdlNode,
    setting: &str,
    minimum: i128,
    maximum: i128,
) -> Result<i128, DesktopProfileError> {
    if node.entries().len() != 1 || node.children().is_some() || node.ty().is_some() {
        return Err(schema_error(format!("{setting} requires one integer")));
    }
    node.get(0)
        .and_then(|value| value.as_integer())
        .filter(|value| (minimum..=maximum).contains(value))
        .ok_or_else(|| schema_error(format!("{setting} is outside its supported range")))
}

fn one_number(
    node: &KdlNode,
    setting: &str,
    minimum: f64,
    maximum: f64,
) -> Result<f64, DesktopProfileError> {
    if node.entries().len() != 1 || node.children().is_some() || node.ty().is_some() {
        return Err(schema_error(format!("{setting} requires one number")));
    }
    let number = node.get(0).and_then(|value| {
        value.as_float().or_else(|| {
            value
                .as_integer()
                .and_then(|integer| i64::try_from(integer).ok())
                .map(|integer| integer as f64)
        })
    });
    number
        .filter(|value| value.is_finite() && (minimum..=maximum).contains(value))
        .ok_or_else(|| schema_error(format!("{setting} is outside its supported range")))
}

fn one_string(
    node: &KdlNode,
    setting: &str,
    minimum: usize,
    maximum: usize,
) -> Result<String, DesktopProfileError> {
    if node.entries().len() != 1 || node.children().is_some() || node.ty().is_some() {
        return Err(schema_error(format!("{setting} requires one string")));
    }
    node.get(0)
        .and_then(|value| value.as_string())
        .filter(|value| (minimum..=maximum).contains(&value.len()))
        .map(ToOwned::to_owned)
        .ok_or_else(|| schema_error(format!("{setting} has an invalid length")))
}

fn parse_xkb(node: &KdlNode) -> Result<DesktopXkbCandidate, DesktopProfileError> {
    let mut result = DesktopXkbCandidate::default();
    for child in children(node, "keyboard xkb")?.nodes() {
        let target = match child.name().value() {
            "rules" => &mut result.rules,
            "model" => &mut result.model,
            "layout" => &mut result.layout,
            "variant" => &mut result.variant,
            "options" => &mut result.options,
            _ => return Err(schema_error("unsupported keyboard xkb setting")),
        };
        if target.is_some() {
            return Err(schema_error("duplicate keyboard xkb setting"));
        }
        let minimum = usize::from(child.name().value() == "layout");
        let maximum = if child.name().value() == "options" {
            256
        } else {
            64
        };
        *target = Some(one_string(child, "keyboard xkb setting", minimum, maximum)?);
    }
    Ok(result)
}

fn parse_keyboard(node: &KdlNode) -> Result<DesktopKeyboardCandidate, DesktopProfileError> {
    let mut result = DesktopKeyboardCandidate::default();
    for child in children(node, "keyboard")?.nodes() {
        match child.name().value() {
            "repeat-rate" if result.repeat_rate.is_none() => {
                result.repeat_rate = Some(
                    u32::try_from(one_integer(child, "repeat-rate", 1, 1_000)?)
                        .expect("bounded repeat rate fits u32"),
                );
            }
            "repeat-delay" if result.repeat_delay_msec.is_none() => {
                result.repeat_delay_msec = Some(
                    u64::try_from(one_integer(child, "repeat-delay", 1, 10_000)?)
                        .expect("bounded repeat delay fits u64"),
                );
            }
            "numlock" if result.num_lock.is_none() => {
                result.num_lock = Some(one_bool(child, "numlock")?);
            }
            "capslock" if result.caps_lock.is_none() => {
                result.caps_lock = Some(one_bool(child, "capslock")?);
            }
            "xkb" if result.xkb.is_none() => result.xkb = Some(parse_xkb(child)?),
            "repeat-rate" | "repeat-delay" | "numlock" | "capslock" | "xkb" => {
                return Err(schema_error("duplicate keyboard setting"));
            }
            _ => return Err(schema_error("unsupported keyboard setting")),
        }
    }
    Ok(result)
}

fn parse_pointer(node: &KdlNode) -> Result<DesktopPointerCandidate, DesktopProfileError> {
    let mut result = DesktopPointerCandidate::default();
    for child in children(node, "pointer")?.nodes() {
        match child.name().value() {
            "natural-scroll" if result.natural_scroll.is_none() => {
                result.natural_scroll = Some(one_bool(child, "natural-scroll")?);
            }
            "accel-profile" if result.accel_profile.is_none() => {
                result.accel_profile =
                    Some(match one_string(child, "accel-profile", 1, 16)?.as_str() {
                        "flat" => DesktopPointerAccelProfile::Flat,
                        "adaptive" => DesktopPointerAccelProfile::Adaptive,
                        _ => return Err(schema_error("unsupported pointer acceleration profile")),
                    });
            }
            "accel-speed" if result.accel_speed.is_none() => {
                result.accel_speed = Some(one_number(child, "accel-speed", -1.0, 1.0)?);
            }
            "left-handed" if result.left_handed.is_none() => {
                result.left_handed = Some(one_bool(child, "left-handed")?);
            }
            "middle-emulation" if result.middle_emulation.is_none() => {
                result.middle_emulation = Some(one_bool(child, "middle-emulation")?);
            }
            "scroll-factor" if result.scroll_factor.is_none() => {
                result.scroll_factor = Some(one_number(child, "scroll-factor", 0.01, 10.0)?);
            }
            "natural-scroll" | "accel-profile" | "accel-speed" | "left-handed"
            | "middle-emulation" | "scroll-factor" => {
                return Err(schema_error("duplicate pointer setting"));
            }
            _ => return Err(schema_error("unsupported pointer setting")),
        }
    }
    Ok(result)
}

fn parse_cursor(node: &KdlNode) -> Result<DesktopCursorCandidate, DesktopProfileError> {
    let mut result = DesktopCursorCandidate::default();
    for child in children(node, "cursor")?.nodes() {
        match child.name().value() {
            "theme" if result.theme.is_none() => {
                let theme = one_string(
                    child,
                    "theme",
                    1,
                    crate::SOPHIA_CONFIG_MAX_CURSOR_NAME_BYTES,
                )?;
                // The same alphabet the core config accepts. A theme name
                // becomes a directory under an icon path, so anything outside
                // it is a traversal attempt or a typo, and neither should
                // reach the filesystem.
                if !theme
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
                {
                    return Err(schema_error(
                        "cursor theme contains an unsupported character",
                    ));
                }
                result.theme = Some(theme);
            }
            "size" if result.size.is_none() => {
                let size = one_integer(
                    child,
                    "size",
                    1,
                    i128::from(crate::SOPHIA_CONFIG_MAX_CURSOR_SIZE),
                )?;
                result.size = Some(
                    u32::try_from(size)
                        .map_err(|_| schema_error("cursor size is outside its supported range"))?,
                );
            }
            "shake-to-find" if result.shake_to_find.is_none() => {
                result.shake_to_find = Some(one_bool(child, "shake-to-find")?);
            }
            "theme" | "size" | "shake-to-find" => {
                return Err(schema_error("duplicate cursor setting"));
            }
            _ => return Err(schema_error("unsupported cursor setting")),
        }
    }
    Ok(result)
}

pub fn prepare_desktop_input_candidate(
    candidate: &DesktopAuthorityCandidate,
) -> Result<DesktopInputCandidate, DesktopProfileError> {
    if candidate.authority != DesktopAuthority::Input {
        return Err(schema_error("candidate crossed its authority boundary"));
    }
    let mut prepared = DesktopInputCandidate {
        generation: candidate.generation,
        digest: candidate.digest,
        inherit_sophia: true,
        keyboard: None,
        pointer: None,
        cursor: None,
    };
    let mut inheritance_seen = false;
    for value in &candidate.values {
        let node = single_node(&value.encoded)?;
        match node.name().value() {
            "inherit-sophia" if !inheritance_seen => {
                prepared.inherit_sophia = one_bool(&node, "inherit-sophia")?;
                inheritance_seen = true;
            }
            "keyboard" if prepared.keyboard.is_none() => {
                prepared.keyboard = Some(parse_keyboard(&node)?);
            }
            "pointer" if prepared.pointer.is_none() => {
                prepared.pointer = Some(parse_pointer(&node)?)
            }
            "cursor" if prepared.cursor.is_none() => {
                prepared.cursor = Some(parse_cursor(&node)?);
            }
            "inherit-sophia" | "keyboard" | "pointer" | "cursor" => {
                return Err(schema_error("duplicate input setting"));
            }
            _ => return Err(schema_error("candidate contains a non-input setting")),
        }
    }
    Ok(prepared)
}
