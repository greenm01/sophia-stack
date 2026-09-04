use super::*;

pub(super) fn parse_compositor(
    node: &KdlNode,
) -> Result<(ChromePolicy, u32, CursorConfig), ConfigParseError> {
    exact_shape(node, 0, &[], true)?;
    let children = children(node)?;
    validate_root_names(children, &["chrome-fallback", "chrome-limits", "cursor"])?;
    require_singletons(children, &["chrome-fallback", "chrome-limits", "cursor"])?;
    let max = children
        .get("chrome-limits")
        .map(|limits| {
            exact_shape(limits, 0, &["max-width"], false)?;
            integer_property_u32(
                limits,
                "max-width",
                0,
                SOPHIA_CONFIG_COMPILED_MAX_CHROME_WIDTH,
            )
        })
        .transpose()?
        .unwrap_or(SOPHIA_CONFIG_COMPILED_MAX_CHROME_WIDTH);
    let style = children
        .get("chrome-fallback")
        .map(|chrome| parse_chrome_policy(chrome, max))
        .transpose()?
        .unwrap_or_default();
    if style.clearance() > max {
        return schema_error("chrome fallback width exceeds configured maximum");
    }
    let cursor = children
        .get("cursor")
        .map(parse_cursor)
        .transpose()?
        .unwrap_or_default();
    Ok((style, max, cursor))
}

fn parse_cursor(node: &KdlNode) -> Result<CursorConfig, ConfigParseError> {
    exact_shape(node, 0, &["theme", "size", "shape"], false)?;
    let theme = required_string_property(node, "theme", 1, SOPHIA_CONFIG_MAX_CURSOR_NAME_BYTES)?;
    let shape = required_string_property(node, "shape", 1, SOPHIA_CONFIG_MAX_CURSOR_NAME_BYTES)?;
    if !theme.bytes().all(cursor_name_byte) {
        return schema_error("cursor theme contains an unsupported character");
    }
    if sophia_engine_cursor_shape_name(shape).is_none() {
        return schema_error(format!("unsupported semantic cursor shape {shape:?}"));
    }
    Ok(CursorConfig {
        theme: theme.to_owned(),
        size: integer_property_u32(node, "size", 1, SOPHIA_CONFIG_MAX_CURSOR_SIZE)?,
        shape: shape.to_owned(),
    })
}

const fn cursor_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
}

// Keep the config dialect protocol-neutral. These names map to Engine's
// semantic CursorShape at the session boundary; arbitrary renderer names do
// not cross configuration as hidden policy.
fn sophia_engine_cursor_shape_name(name: &str) -> Option<()> {
    matches!(
        name,
        "left_ptr"
            | "default"
            | "text"
            | "xterm"
            | "pointer"
            | "hand2"
            | "move"
            | "fleur"
            | "wait"
            | "watch"
            | "crosshair"
            | "ew-resize"
            | "sb_h_double_arrow"
            | "ns-resize"
            | "sb_v_double_arrow"
            | "nwse-resize"
            | "nesw-resize"
    )
    .then_some(())
}

pub(super) fn parse_chrome_policy(
    node: &KdlNode,
    max_width: u32,
) -> Result<ChromePolicy, ConfigParseError> {
    exact_shape(node, 0, &[], true)?;
    let children = children(node)?;
    validate_root_names(children, &["focus-ring", "frame"])?;
    require_singletons(children, &["focus-ring", "frame"])?;
    let focus_ring = children
        .get("focus-ring")
        .map(|node| parse_focus_ring(node, max_width))
        .transpose()?
        .unwrap_or_default();
    let frame = children
        .get("frame")
        .map(|node| parse_frame(node, max_width))
        .transpose()?
        .unwrap_or_default();
    Ok(ChromePolicy { focus_ring, frame })
}

fn parse_focus_ring(node: &KdlNode, max_width: u32) -> Result<FocusRingStyle, ConfigParseError> {
    exact_shape(node, 0, &["enabled", "width", "color"], false)?;
    let enabled = optional_bool_property(node, "enabled", true)?;
    let width = integer_property_u32(node, "width", 0, max_width)?;
    validate_chrome_width("focus ring", enabled, width)?;
    Ok(FocusRingStyle {
        enabled,
        width,
        color: parse_rgb(required_string_property(node, "color", 7, 7)?)?,
    })
}

fn parse_frame(node: &KdlNode, max_width: u32) -> Result<FrameStyle, ConfigParseError> {
    exact_shape(
        node,
        0,
        &["enabled", "width", "focused-color", "unfocused-color"],
        false,
    )?;
    let enabled = optional_bool_property(node, "enabled", false)?;
    let width = integer_property_u32(node, "width", 0, max_width)?;
    validate_chrome_width("frame", enabled, width)?;
    Ok(FrameStyle {
        enabled,
        width,
        focused_color: parse_rgb(required_string_property(node, "focused-color", 7, 7)?)?,
        unfocused_color: parse_rgb(required_string_property(node, "unfocused-color", 7, 7)?)?,
    })
}

fn validate_chrome_width(role: &str, enabled: bool, width: u32) -> Result<(), ConfigParseError> {
    if enabled && width == 0 {
        return schema_error(format!("enabled {role} must have nonzero width"));
    }
    if !enabled && width != 0 {
        return schema_error(format!("disabled {role} must have zero width"));
    }
    Ok(())
}
