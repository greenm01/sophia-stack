use super::*;

pub const DESKTOP_POLICY_LAYOUTS: &[&str] = &[
    "scroller",
    "tile",
    "grid",
    "monocle",
    "vertical-scroller",
    "center-tile",
    "right-tile",
    "vertical-grid",
    "deck",
    "spiral",
    "tgmix",
    "frame-tree",
    "notion",
    "i3",
];

pub(super) fn validate_setting(node: &KdlNode) -> Result<(), DesktopProfileError> {
    let name = node.name().value();
    if name == "layout" {
        validate_policy_layout(exact_string_argument(node, "policy layout")?)?;
    }
    if name == "layout-cycle" {
        if node.entries().is_empty()
            || node.entries().len() > DESKTOP_POLICY_LAYOUTS.len()
            || node.children().is_some()
        {
            return Err(DesktopProfileError::Schema(
                "policy layout-cycle exceeds the native layout bound".to_owned(),
            ));
        }
        let mut layouts = std::collections::BTreeSet::new();
        for entry in node.entries() {
            let layout = entry
                .name()
                .is_none()
                .then(|| entry.value().as_string())
                .flatten()
                .ok_or_else(|| {
                    DesktopProfileError::Schema(
                        "policy layout-cycle requires layout names".to_owned(),
                    )
                })?;
            validate_policy_layout(layout)?;
            if !layouts.insert(if layout == "split-tree" { "i3" } else { layout }) {
                return Err(DesktopProfileError::Schema(
                    "policy layout-cycle contains duplicates".to_owned(),
                ));
            }
        }
    }
    if ["master-count", "master-ratio", "gap-step"].contains(&name) {
        let value = exact_integer_argument(node, "native policy setting")?;
        let bounds = match name {
            "master-count" => 1..=9,
            "master-ratio" => 10..=90,
            _ => 1..=512,
        };
        if !bounds.contains(&value) {
            return Err(DesktopProfileError::Schema(format!(
                "policy {name} exceeds its native bound"
            )));
        }
    }
    if name == "view-count" {
        let value = exact_integer_argument(node, "policy view-count")?;
        if !(1..=9).contains(&value) {
            return Err(DesktopProfileError::Schema(
                "policy view-count must be in 1..=9".to_owned(),
            ));
        }
    }
    if ["outer-gap", "inner-gap", "viewport-offset"].contains(&name) {
        let value = exact_integer_argument(node, "policy geometry")?;
        if !(0..=i128::from(i32::MAX)).contains(&value) {
            return Err(DesktopProfileError::Schema(
                "policy geometry must be a nonnegative 32-bit integer".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_policy_layout(layout: &str) -> Result<(), DesktopProfileError> {
    if layout == "split-tree" || DESKTOP_POLICY_LAYOUTS.contains(&layout) {
        Ok(())
    } else {
        Err(DesktopProfileError::Schema(
            "unsupported policy layout".to_owned(),
        ))
    }
}
