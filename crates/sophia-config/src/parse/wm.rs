use super::*;

pub(super) fn parse_wm_actions(
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

pub(super) fn parse_wm_bindings(
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
