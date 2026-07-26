use super::*;

pub(super) fn parse_compositor(node: &KdlNode) -> Result<(ChromePolicy, u32), ConfigParseError> {
    exact_shape(node, 0, &[], true)?;
    let children = children(node)?;
    validate_root_names(children, &["chrome-fallback", "chrome-limits"])?;
    require_singletons(children, &["chrome-fallback", "chrome-limits"])?;
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
    Ok((style, max))
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
