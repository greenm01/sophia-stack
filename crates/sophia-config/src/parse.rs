use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;

use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use sha2::{Digest, Sha256};

use crate::{
    ApplicationConfig, ChromeStyle, ConfigDigest, ConfigGeneration, CoreConfigSnapshot,
    ExternalWmConfig, InputConfig, InputSourceConfig, OutputConfig, RepeatConfig, Rgb8,
    SOPHIA_CONFIG_COMPILED_MAX_BORDER_THICKNESS, SOPHIA_CONFIG_MAX_APPLICATIONS,
    SOPHIA_CONFIG_MAX_ARGUMENT_BYTES, SOPHIA_CONFIG_MAX_ARGUMENTS, SOPHIA_CONFIG_MAX_OUTPUTS,
    SOPHIA_CONFIG_MAX_WM_ACTIONS, SOPHIA_CONFIG_MAX_WM_BINDINGS, SOPHIA_CONFIG_MAX_WORKSPACES,
    SOPHIA_CONFIG_SCHEMA_VERSION, SessionConfig, WmActionBehavior, WmActionConfig, WmBindingConfig,
    WmConfigSnapshot, WmLayoutKind, XkbConfig,
};

const MODIFIER_SHIFT: u32 = 1 << 0;
const MODIFIER_CONTROL: u32 = 1 << 1;
const MODIFIER_ALT: u32 = 1 << 2;
const MODIFIER_SUPER: u32 = 1 << 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigParseError {
    NotUtf8,
    Syntax(String),
    Schema(String),
}

impl fmt::Display for ConfigParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotUtf8 => formatter.write_str("configuration is not valid UTF-8"),
            Self::Syntax(error) => write!(formatter, "KDL 2 syntax error: {error}"),
            Self::Schema(error) => write!(formatter, "configuration schema error: {error}"),
        }
    }
}

impl std::error::Error for ConfigParseError {}

pub fn parse_core_config(
    bytes: &[u8],
    generation: ConfigGeneration,
) -> Result<CoreConfigSnapshot, ConfigParseError> {
    let source = std::str::from_utf8(bytes).map_err(|_| ConfigParseError::NotUtf8)?;
    let document = KdlDocument::parse_v2(source)
        .map_err(|error| ConfigParseError::Syntax(error.to_string()))?;
    validate_root_names(
        &document,
        &[
            "schema",
            "session",
            "input",
            "outputs",
            "compositor",
            "namespace",
            "external-wm",
            "diagnostics",
        ],
    )?;
    require_singletons(
        &document,
        &[
            "schema",
            "session",
            "input",
            "outputs",
            "compositor",
            "namespace",
            "external-wm",
            "diagnostics",
        ],
    )?;
    let schema = parse_schema(&document)?;
    let session = document
        .get("session")
        .map(parse_session)
        .transpose()?
        .unwrap_or_default();
    let input = document
        .get("input")
        .map(parse_input)
        .transpose()?
        .unwrap_or_default();
    let outputs = document
        .get("outputs")
        .map(parse_outputs)
        .transpose()?
        .unwrap_or_default();
    let (fallback_chrome, max_border_thickness) = document
        .get("compositor")
        .map(parse_compositor)
        .transpose()?
        .unwrap_or((
            ChromeStyle::default(),
            SOPHIA_CONFIG_COMPILED_MAX_BORDER_THICKNESS,
        ));
    let namespace_profile = document
        .get("namespace")
        .map(parse_namespace)
        .transpose()?
        .unwrap_or_else(|| "classic-shared".to_owned());
    let external_wm = document
        .get("external-wm")
        .map(parse_external_wm)
        .transpose()?;
    let verbose_diagnostics = document
        .get("diagnostics")
        .map(parse_diagnostics)
        .transpose()?
        .unwrap_or(false);
    Ok(CoreConfigSnapshot {
        schema,
        generation,
        digest: digest(bytes),
        session,
        input,
        outputs,
        fallback_chrome,
        max_border_thickness,
        namespace_profile,
        external_wm,
        verbose_diagnostics,
    })
}

pub fn parse_wm_config(
    bytes: &[u8],
    generation: ConfigGeneration,
) -> Result<WmConfigSnapshot, ConfigParseError> {
    let source = std::str::from_utf8(bytes).map_err(|_| ConfigParseError::NotUtf8)?;
    let document = KdlDocument::parse_v2(source)
        .map_err(|error| ConfigParseError::Syntax(error.to_string()))?;
    validate_root_names(
        &document,
        &[
            "schema",
            "policy",
            "workspace",
            "layout",
            "action",
            "binding",
            "chrome",
        ],
    )?;
    require_singletons(&document, &["schema", "policy", "layout", "chrome"])?;
    let schema = parse_schema(&document)?;
    let timeout_msec = document
        .get("policy")
        .map(|node| {
            exact_shape(node, 0, &["timeout-ms"], false)?;
            integer_property_u32(node, "timeout-ms", 50, 5_000)
        })
        .transpose()?
        .unwrap_or(300);
    let mut workspaces = Vec::new();
    let mut workspace_ids = BTreeSet::new();
    for node in document
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "workspace")
    {
        exact_shape(node, 1, &[], false)?;
        let workspace = integer_argument_u64(node, 0, 1, u64::MAX)?;
        if !workspace_ids.insert(workspace) {
            return schema_error(format!("duplicate workspace ID {workspace}"));
        }
        if workspaces.len() >= SOPHIA_CONFIG_MAX_WORKSPACES {
            return schema_error("too many workspaces");
        }
        workspaces.push(workspace);
    }
    if workspaces.is_empty() {
        workspaces.extend(1..=9);
        workspace_ids.extend(1..=9);
    }
    let layout = document
        .get("layout")
        .map(|node| {
            exact_shape(node, 1, &[], false)?;
            match string_argument(node, 0, 1, 32)? {
                "columns" => Ok(WmLayoutKind::Columns),
                other => schema_error(format!("unsupported native WM layout {other:?}")),
            }
        })
        .transpose()?
        .unwrap_or(WmLayoutKind::Columns);
    let actions = parse_wm_actions(&document, &workspace_ids)?;
    let bindings = parse_wm_bindings(&document, &actions)?;
    let chrome = document
        .get("chrome")
        .map(|node| parse_chrome_style(node, SOPHIA_CONFIG_COMPILED_MAX_BORDER_THICKNESS))
        .transpose()?
        .unwrap_or_default();
    Ok(WmConfigSnapshot {
        schema,
        generation,
        digest: digest(bytes),
        timeout_msec,
        workspaces,
        layout,
        actions,
        bindings,
        chrome,
    })
}

fn parse_schema(document: &KdlDocument) -> Result<u32, ConfigParseError> {
    let node = document
        .get("schema")
        .ok_or_else(|| ConfigParseError::Schema("missing schema node".to_owned()))?;
    exact_shape(node, 1, &[], false)?;
    let schema = integer_argument_u32(node, 0, 1, u32::MAX)?;
    if schema != SOPHIA_CONFIG_SCHEMA_VERSION {
        return schema_error(format!(
            "unsupported schema {schema}; expected {SOPHIA_CONFIG_SCHEMA_VERSION}"
        ));
    }
    Ok(schema)
}

fn parse_session(node: &KdlNode) -> Result<SessionConfig, ConfigParseError> {
    exact_shape(node, 0, &[], true)?;
    let children = children(node)?;
    validate_root_names(children, &["application", "startup"])?;
    let mut applications = Vec::new();
    let mut application_ids = BTreeSet::new();
    let mut application_names = BTreeSet::new();
    let mut startup = Vec::new();
    let mut startup_ids = BTreeSet::new();
    for child in children.nodes() {
        match child.name().value() {
            "application" => {
                exact_shape(child, 1, &["id", "executable"], true)?;
                if applications.len() >= SOPHIA_CONFIG_MAX_APPLICATIONS {
                    return schema_error("too many session applications");
                }
                let name = string_argument(child, 0, 1, 32)?.to_owned();
                validate_identifier(&name, "application name")?;
                let id = integer_property_u64(child, "id", 1, u64::MAX)?;
                let executable = absolute_path_property(child, "executable")?;
                if !application_ids.insert(id) {
                    return schema_error(format!("duplicate application ID {id}"));
                }
                if !application_names.insert(name.clone()) {
                    return schema_error(format!("duplicate application name {name:?}"));
                }
                let arguments = child
                    .children()
                    .map(parse_arguments)
                    .transpose()?
                    .unwrap_or_default();
                applications.push(ApplicationConfig {
                    id,
                    name,
                    executable,
                    arguments,
                });
            }
            "startup" => {
                exact_shape(child, 1, &[], false)?;
                let id = integer_argument_u64(child, 0, 1, u64::MAX)?;
                if !startup_ids.insert(id) {
                    return schema_error(format!("duplicate startup application ID {id}"));
                }
                startup.push(id);
            }
            _ => unreachable!(),
        }
    }
    for id in &startup {
        if !application_ids.contains(id) {
            return schema_error(format!("startup references unknown application ID {id}"));
        }
    }
    Ok(SessionConfig {
        applications,
        startup,
    })
}

fn parse_arguments(document: &KdlDocument) -> Result<Vec<String>, ConfigParseError> {
    validate_root_names(document, &["arg"])?;
    let mut arguments = Vec::new();
    for node in document.nodes() {
        exact_shape(node, 1, &[], false)?;
        if arguments.len() >= SOPHIA_CONFIG_MAX_ARGUMENTS {
            return schema_error("too many executable arguments");
        }
        arguments.push(string_argument(node, 0, 0, SOPHIA_CONFIG_MAX_ARGUMENT_BYTES)?.to_owned());
    }
    Ok(arguments)
}

fn parse_input(node: &KdlNode) -> Result<InputConfig, ConfigParseError> {
    exact_shape(node, 0, &[], true)?;
    let children = children(node)?;
    validate_root_names(children, &["seat", "device", "keyboard", "repeat"])?;
    require_singletons(children, &["seat", "keyboard", "repeat"])?;
    let seat = children
        .get("seat")
        .map(|seat| {
            exact_shape(seat, 1, &[], false)?;
            let value = string_argument(seat, 0, 1, 64)?.to_owned();
            validate_identifier(&value, "seat")?;
            Ok(value)
        })
        .transpose()?;
    let mut devices = Vec::new();
    for device in children
        .nodes()
        .iter()
        .filter(|child| child.name().value() == "device")
    {
        exact_shape(device, 1, &[], false)?;
        let path = PathBuf::from(string_argument(device, 0, 1, 4_096)?);
        if !path.is_absolute() {
            return schema_error("input device path must be absolute");
        }
        if devices.len() >= 16 {
            return schema_error("too many input devices");
        }
        devices.push(path);
    }
    if seat.is_some() && !devices.is_empty() {
        return schema_error("input seat and device nodes are mutually exclusive");
    }
    let source = match (seat, devices.is_empty()) {
        (Some(seat), true) => InputSourceConfig::Seat(seat),
        (None, false) => InputSourceConfig::Devices(devices),
        (None, true) => InputSourceConfig::Seat("seat0".to_owned()),
        (Some(_), false) => unreachable!(),
    };
    let xkb = children
        .get("keyboard")
        .map(parse_xkb)
        .transpose()?
        .unwrap_or_default();
    let repeat = children
        .get("repeat")
        .map(parse_repeat)
        .transpose()?
        .unwrap_or_default();
    Ok(InputConfig {
        source,
        xkb,
        repeat,
    })
}

fn parse_xkb(node: &KdlNode) -> Result<XkbConfig, ConfigParseError> {
    exact_shape(
        node,
        0,
        &["rules", "model", "layout", "variant", "options"],
        false,
    )?;
    let defaults = XkbConfig::default();
    Ok(XkbConfig {
        rules: optional_string_property(node, "rules", &defaults.rules, 0, 64)?,
        model: optional_string_property(node, "model", &defaults.model, 0, 64)?,
        layout: optional_string_property(node, "layout", &defaults.layout, 1, 64)?,
        variant: optional_string_property(node, "variant", &defaults.variant, 0, 64)?,
        options: optional_string_property(node, "options", &defaults.options, 0, 256)?,
    })
}

fn parse_repeat(node: &KdlNode) -> Result<RepeatConfig, ConfigParseError> {
    exact_shape(node, 0, &["delay-ms", "interval-ms"], false)?;
    Ok(RepeatConfig {
        delay_msec: integer_property_u64(node, "delay-ms", 1, 10_000)?,
        interval_msec: integer_property_u64(node, "interval-ms", 1, 1_000)?,
    })
}

fn parse_outputs(node: &KdlNode) -> Result<Vec<OutputConfig>, ConfigParseError> {
    exact_shape(node, 0, &[], true)?;
    let children = children(node)?;
    validate_root_names(children, &["output"])?;
    let mut outputs = Vec::new();
    let mut identities = BTreeSet::new();
    let mut primary_count = 0usize;
    for output in children.nodes() {
        exact_shape(output, 1, &["x", "y", "mode", "scale", "primary"], false)?;
        if outputs.len() >= SOPHIA_CONFIG_MAX_OUTPUTS {
            return schema_error("too many output policies");
        }
        let identity = string_argument(output, 0, 1, 256)?.to_owned();
        if !identities.insert(identity.clone()) {
            return schema_error(format!("duplicate output identity {identity:?}"));
        }
        let primary = optional_bool_property(output, "primary", false)?;
        primary_count += usize::from(primary);
        outputs.push(OutputConfig {
            identity,
            x: integer_property_i32(output, "x", i32::MIN, i32::MAX)?,
            y: integer_property_i32(output, "y", i32::MIN, i32::MAX)?,
            mode: required_string_property(output, "mode", 1, 128)?.to_owned(),
            scale: integer_property_u32(output, "scale", 1, 8)?,
            primary,
        });
    }
    if primary_count > 1 {
        return schema_error("at most one output may be primary");
    }
    Ok(outputs)
}

fn parse_compositor(node: &KdlNode) -> Result<(ChromeStyle, u32), ConfigParseError> {
    exact_shape(node, 0, &[], true)?;
    let children = children(node)?;
    validate_root_names(children, &["chrome-fallback", "chrome-limits"])?;
    require_singletons(children, &["chrome-fallback", "chrome-limits"])?;
    let max = children
        .get("chrome-limits")
        .map(|limits| {
            exact_shape(limits, 0, &["max-thickness"], false)?;
            integer_property_u32(
                limits,
                "max-thickness",
                0,
                SOPHIA_CONFIG_COMPILED_MAX_BORDER_THICKNESS,
            )
        })
        .transpose()?
        .unwrap_or(SOPHIA_CONFIG_COMPILED_MAX_BORDER_THICKNESS);
    let style = children
        .get("chrome-fallback")
        .map(|chrome| parse_chrome_style(chrome, max))
        .transpose()?
        .unwrap_or_default();
    if style.thickness > max {
        return schema_error("chrome fallback thickness exceeds configured maximum");
    }
    Ok((style, max))
}

fn parse_chrome_style(node: &KdlNode, max_thickness: u32) -> Result<ChromeStyle, ConfigParseError> {
    exact_shape(node, 0, &["enabled", "thickness", "color"], false)?;
    let enabled = optional_bool_property(node, "enabled", true)?;
    let thickness = integer_property_u32(node, "thickness", 0, max_thickness)?;
    if enabled && thickness == 0 {
        return schema_error("enabled chrome must have nonzero thickness");
    }
    let color = parse_rgb(required_string_property(node, "color", 7, 7)?)?;
    Ok(ChromeStyle {
        enabled,
        thickness,
        color,
    })
}

fn parse_namespace(node: &KdlNode) -> Result<String, ConfigParseError> {
    exact_shape(node, 0, &["profile"], false)?;
    let profile = required_string_property(node, "profile", 1, 64)?;
    match profile {
        "classic" | "classic-shared" | "confined" => Ok(profile.to_owned()),
        other => schema_error(format!("unsupported namespace profile {other:?}")),
    }
}

fn parse_external_wm(node: &KdlNode) -> Result<ExternalWmConfig, ConfigParseError> {
    exact_shape(node, 0, &["executable"], true)?;
    Ok(ExternalWmConfig {
        executable: absolute_path_property(node, "executable")?,
        arguments: node
            .children()
            .map(parse_arguments)
            .transpose()?
            .unwrap_or_default(),
    })
}

fn parse_diagnostics(node: &KdlNode) -> Result<bool, ConfigParseError> {
    exact_shape(node, 0, &["verbose"], false)?;
    optional_bool_property(node, "verbose", false)
}

fn parse_wm_actions(
    document: &KdlDocument,
    workspaces: &BTreeSet<u64>,
) -> Result<Vec<WmActionConfig>, ConfigParseError> {
    let mut actions = Vec::new();
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for node in document
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "action")
    {
        exact_shape(
            node,
            1,
            &["id", "behavior", "workspace", "application"],
            false,
        )?;
        if actions.len() >= SOPHIA_CONFIG_MAX_WM_ACTIONS {
            return schema_error("too many WM actions");
        }
        let name = string_argument(node, 0, 1, 32)?.to_owned();
        validate_identifier(&name, "WM action name")?;
        let id = integer_property_u64(node, "id", 1, u64::MAX)?;
        if !ids.insert(id) {
            return schema_error(format!("duplicate WM action ID {id}"));
        }
        if !names.insert(name.clone()) {
            return schema_error(format!("duplicate WM action name {name:?}"));
        }
        let behavior_name = required_string_property(node, "behavior", 1, 64)?;
        let workspace = optional_integer_property_u64(node, "workspace", 1, u64::MAX)?;
        let application = optional_integer_property_u64(node, "application", 1, u64::MAX)?;
        let behavior = match behavior_name {
            "focus-next" if workspace.is_none() && application.is_none() => {
                WmActionBehavior::FocusNext
            }
            "focus-previous" if workspace.is_none() && application.is_none() => {
                WmActionBehavior::FocusPrevious
            }
            "next-layout" if workspace.is_none() && application.is_none() => {
                WmActionBehavior::NextLayout
            }
            "activate-workspace" if application.is_none() => {
                let workspace = workspace.ok_or_else(|| {
                    ConfigParseError::Schema(
                        "activate-workspace action requires workspace".to_owned(),
                    )
                })?;
                if !workspaces.is_empty() && !workspaces.contains(&workspace) {
                    return schema_error(format!(
                        "action references unknown workspace {workspace}"
                    ));
                }
                WmActionBehavior::ActivateWorkspace { workspace }
            }
            "launch-application" if workspace.is_none() => WmActionBehavior::LaunchApplication {
                application: application.ok_or_else(|| {
                    ConfigParseError::Schema(
                        "launch-application action requires application".to_owned(),
                    )
                })?,
            },
            "close-focused" if workspace.is_none() && application.is_none() => {
                WmActionBehavior::CloseFocused
            }
            "logout" if workspace.is_none() && application.is_none() => WmActionBehavior::Logout,
            _ => {
                return schema_error(format!(
                    "invalid properties for WM action behavior {behavior_name:?}"
                ));
            }
        };
        actions.push(WmActionConfig { id, name, behavior });
    }
    Ok(actions)
}

fn parse_wm_bindings(
    document: &KdlDocument,
    actions: &[WmActionConfig],
) -> Result<Vec<WmBindingConfig>, ConfigParseError> {
    let action_ids = actions
        .iter()
        .map(|action| action.id)
        .collect::<BTreeSet<_>>();
    let mut bindings = Vec::new();
    let mut chords = BTreeSet::new();
    let mut bound_actions = BTreeSet::new();
    for node in document
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "binding")
    {
        exact_shape(node, 0, &["action", "keycode", "modifiers"], false)?;
        if bindings.len() >= SOPHIA_CONFIG_MAX_WM_BINDINGS {
            return schema_error("too many WM bindings");
        }
        let action = integer_property_u64(node, "action", 1, u64::MAX)?;
        let keycode = integer_property_u32(node, "keycode", 1, 0x2ff)?;
        let modifiers = parse_modifiers(required_string_property(node, "modifiers", 0, 64)?)?;
        if !action_ids.contains(&action) {
            return schema_error(format!("binding references unknown action ID {action}"));
        }
        if !bound_actions.insert(action) {
            return schema_error(format!("duplicate binding for action ID {action}"));
        }
        if !chords.insert((keycode, modifiers)) {
            return schema_error("duplicate WM key chord");
        }
        if keycode == 14
            && modifiers & (MODIFIER_CONTROL | MODIFIER_ALT) == MODIFIER_CONTROL | MODIFIER_ALT
        {
            return schema_error("reserved emergency chord cannot be bound by a WM");
        }
        bindings.push(WmBindingConfig {
            action,
            keycode,
            modifiers,
        });
    }
    Ok(bindings)
}

fn parse_modifiers(value: &str) -> Result<u32, ConfigParseError> {
    if value.is_empty() {
        return Ok(0);
    }
    let mut modifiers = 0;
    for modifier in value.split('+') {
        let bit = match modifier {
            "shift" => MODIFIER_SHIFT,
            "control" => MODIFIER_CONTROL,
            "alt" => MODIFIER_ALT,
            "super" => MODIFIER_SUPER,
            other => return schema_error(format!("unsupported WM modifier {other:?}")),
        };
        if modifiers & bit != 0 {
            return schema_error(format!("duplicate WM modifier {modifier:?}"));
        }
        modifiers |= bit;
    }
    Ok(modifiers)
}

fn validate_root_names(document: &KdlDocument, allowed: &[&str]) -> Result<(), ConfigParseError> {
    for node in document.nodes() {
        if node.ty().is_some() {
            return schema_error(format!(
                "typed node {:?} is not supported",
                node.name().value()
            ));
        }
        if !allowed.contains(&node.name().value()) {
            return schema_error(format!("unknown node {:?}", node.name().value()));
        }
    }
    Ok(())
}

fn require_singletons(document: &KdlDocument, names: &[&str]) -> Result<(), ConfigParseError> {
    let mut counts = BTreeMap::new();
    for node in document.nodes() {
        *counts.entry(node.name().value()).or_insert(0usize) += 1;
    }
    for name in names {
        if counts.get(name).copied().unwrap_or(0) > 1 {
            return schema_error(format!("duplicate singleton node {name:?}"));
        }
    }
    Ok(())
}

fn exact_shape(
    node: &KdlNode,
    argument_count: usize,
    properties: &[&str],
    children_allowed: bool,
) -> Result<(), ConfigParseError> {
    let mut actual_arguments = 0usize;
    let mut actual_properties = BTreeSet::new();
    for entry in node.entries() {
        if entry.ty().is_some() {
            return schema_error(format!(
                "typed entry on {:?} is not supported",
                node.name().value()
            ));
        }
        match entry.name() {
            Some(name) => {
                if !properties.contains(&name.value()) {
                    return schema_error(format!(
                        "unknown property {:?} on node {:?}",
                        name.value(),
                        node.name().value()
                    ));
                }
                if !actual_properties.insert(name.value()) {
                    return schema_error(format!(
                        "duplicate property {:?} on node {:?}",
                        name.value(),
                        node.name().value()
                    ));
                }
            }
            None => actual_arguments += 1,
        }
    }
    if actual_arguments != argument_count {
        return schema_error(format!(
            "node {:?} expects {argument_count} arguments, observed {actual_arguments}",
            node.name().value()
        ));
    }
    if !children_allowed && node.children().is_some() {
        return schema_error(format!(
            "node {:?} does not accept children",
            node.name().value()
        ));
    }
    Ok(())
}

fn children(node: &KdlNode) -> Result<&KdlDocument, ConfigParseError> {
    node.children().ok_or_else(|| {
        ConfigParseError::Schema(format!("node {:?} requires children", node.name().value()))
    })
}

fn entry_argument<'a>(node: &'a KdlNode, index: usize) -> Result<&'a KdlEntry, ConfigParseError> {
    node.entry(index).ok_or_else(|| {
        ConfigParseError::Schema(format!(
            "missing argument {index} on node {:?}",
            node.name().value()
        ))
    })
}

fn property<'a>(node: &'a KdlNode, name: &str) -> Result<&'a KdlValue, ConfigParseError> {
    node.entry(name).map(KdlEntry::value).ok_or_else(|| {
        ConfigParseError::Schema(format!(
            "missing property {name:?} on node {:?}",
            node.name().value()
        ))
    })
}

fn string_argument<'a>(
    node: &'a KdlNode,
    index: usize,
    minimum: usize,
    maximum: usize,
) -> Result<&'a str, ConfigParseError> {
    bounded_string(
        entry_argument(node, index)?.value(),
        minimum,
        maximum,
        "argument",
    )
}

fn integer_argument_u32(
    node: &KdlNode,
    index: usize,
    minimum: u32,
    maximum: u32,
) -> Result<u32, ConfigParseError> {
    bounded_u32(
        entry_argument(node, index)?.value(),
        minimum,
        maximum,
        "argument",
    )
}

fn integer_argument_u64(
    node: &KdlNode,
    index: usize,
    minimum: u64,
    maximum: u64,
) -> Result<u64, ConfigParseError> {
    bounded_u64(
        entry_argument(node, index)?.value(),
        minimum,
        maximum,
        "argument",
    )
}

fn integer_property_u32(
    node: &KdlNode,
    name: &str,
    minimum: u32,
    maximum: u32,
) -> Result<u32, ConfigParseError> {
    bounded_u32(property(node, name)?, minimum, maximum, name)
}

fn integer_property_i32(
    node: &KdlNode,
    name: &str,
    minimum: i32,
    maximum: i32,
) -> Result<i32, ConfigParseError> {
    let value = property(node, name)?
        .as_integer()
        .ok_or_else(|| ConfigParseError::Schema(format!("{name} must be an integer")))?;
    let value = i32::try_from(value)
        .map_err(|_| ConfigParseError::Schema(format!("{name} is outside i32 range")))?;
    if value < minimum || value > maximum {
        return schema_error(format!("{name} must be in {minimum}..={maximum}"));
    }
    Ok(value)
}

fn integer_property_u64(
    node: &KdlNode,
    name: &str,
    minimum: u64,
    maximum: u64,
) -> Result<u64, ConfigParseError> {
    bounded_u64(property(node, name)?, minimum, maximum, name)
}

fn optional_integer_property_u64(
    node: &KdlNode,
    name: &str,
    minimum: u64,
    maximum: u64,
) -> Result<Option<u64>, ConfigParseError> {
    node.entry(name)
        .map(|entry| bounded_u64(entry.value(), minimum, maximum, name))
        .transpose()
}

fn required_string_property<'a>(
    node: &'a KdlNode,
    name: &str,
    minimum: usize,
    maximum: usize,
) -> Result<&'a str, ConfigParseError> {
    bounded_string(property(node, name)?, minimum, maximum, name)
}

fn optional_string_property(
    node: &KdlNode,
    name: &str,
    default: &str,
    minimum: usize,
    maximum: usize,
) -> Result<String, ConfigParseError> {
    node.entry(name)
        .map(|entry| bounded_string(entry.value(), minimum, maximum, name))
        .transpose()
        .map(|value| value.unwrap_or(default).to_owned())
}

fn optional_bool_property(
    node: &KdlNode,
    name: &str,
    default: bool,
) -> Result<bool, ConfigParseError> {
    node.entry(name)
        .map(|entry| {
            entry
                .value()
                .as_bool()
                .ok_or_else(|| ConfigParseError::Schema(format!("{name} must be a boolean")))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn absolute_path_property(node: &KdlNode, name: &str) -> Result<PathBuf, ConfigParseError> {
    let path = PathBuf::from(required_string_property(node, name, 1, 4_096)?);
    if !path.is_absolute() {
        return schema_error(format!("{name} must be an absolute path"));
    }
    Ok(path)
}

fn bounded_string<'a>(
    value: &'a KdlValue,
    minimum: usize,
    maximum: usize,
    field: &str,
) -> Result<&'a str, ConfigParseError> {
    let value = value
        .as_string()
        .ok_or_else(|| ConfigParseError::Schema(format!("{field} must be a string")))?;
    if value.len() < minimum || value.len() > maximum {
        return schema_error(format!("{field} must contain {minimum}..={maximum} bytes"));
    }
    Ok(value)
}

fn bounded_u32(
    value: &KdlValue,
    minimum: u32,
    maximum: u32,
    field: &str,
) -> Result<u32, ConfigParseError> {
    let value = value
        .as_integer()
        .ok_or_else(|| ConfigParseError::Schema(format!("{field} must be an integer")))?;
    let value = u32::try_from(value)
        .map_err(|_| ConfigParseError::Schema(format!("{field} is outside u32 range")))?;
    if value < minimum || value > maximum {
        return schema_error(format!("{field} must be in {minimum}..={maximum}"));
    }
    Ok(value)
}

fn bounded_u64(
    value: &KdlValue,
    minimum: u64,
    maximum: u64,
    field: &str,
) -> Result<u64, ConfigParseError> {
    let value = value
        .as_integer()
        .ok_or_else(|| ConfigParseError::Schema(format!("{field} must be an integer")))?;
    let value = u64::try_from(value)
        .map_err(|_| ConfigParseError::Schema(format!("{field} is outside u64 range")))?;
    if value < minimum || value > maximum {
        return schema_error(format!("{field} must be in {minimum}..={maximum}"));
    }
    Ok(value)
}

fn parse_rgb(value: &str) -> Result<Rgb8, ConfigParseError> {
    let Some(hex) = value.strip_prefix('#') else {
        return schema_error("chrome color must use #RRGGBB");
    };
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return schema_error("chrome color must use #RRGGBB");
    }
    Ok(Rgb8 {
        red: u8::from_str_radix(&hex[0..2], 16)
            .map_err(|_| ConfigParseError::Schema("invalid red channel".to_owned()))?,
        green: u8::from_str_radix(&hex[2..4], 16)
            .map_err(|_| ConfigParseError::Schema("invalid green channel".to_owned()))?,
        blue: u8::from_str_radix(&hex[4..6], 16)
            .map_err(|_| ConfigParseError::Schema("invalid blue channel".to_owned()))?,
    })
}

fn validate_identifier(value: &str, field: &str) -> Result<(), ConfigParseError> {
    if value.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
    }) {
        Ok(())
    } else {
        schema_error(format!(
            "{field} accepts lowercase ASCII letters, digits, '-' and '_'"
        ))
    }
}

fn digest(bytes: &[u8]) -> ConfigDigest {
    ConfigDigest::new(Sha256::digest(bytes).into())
}

fn schema_error<T>(message: impl Into<String>) -> Result<T, ConfigParseError> {
    Err(ConfigParseError::Schema(message.into()))
}
