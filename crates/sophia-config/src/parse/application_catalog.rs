use super::*;

pub(super) fn parse(node: &KdlNode) -> Result<crate::ApplicationCatalogConfig, ConfigParseError> {
    exact_shape(node, 1, &["launch-policy"], true)?;
    let name = string_argument(node, 0, 1, 32)?.to_owned();
    validate_identifier(&name, "catalog name")?;
    if node.get("launch-policy").and_then(KdlValue::as_string) != Some("trusted-host") {
        return schema_error(
            "application catalog requires explicit launch-policy=\"trusted-host\"; other policies are not implemented",
        );
    }
    let mut catalog = crate::ApplicationCatalogConfig {
        name,
        sources: Vec::new(),
        applications: Vec::new(),
        terminal: None,
        terminal_arguments: Vec::new(),
    };
    let contents = children(node)?;
    validate_root_names(contents, &["source", "application", "terminal"])?;
    for child in contents.nodes() {
        match child.name().value() {
            "source" => {
                exact_shape(child, 1, &[], false)?;
                let path = PathBuf::from(string_argument(child, 0, 1, 4096)?);
                if !path.is_absolute()
                    || path
                        .components()
                        .any(|c| matches!(c, std::path::Component::ParentDir))
                    || catalog.sources.len() >= 16
                    || catalog.sources.contains(&path)
                {
                    return schema_error(
                        "catalog source must be a unique absolute directory without parent traversal; maximum 16",
                    );
                }
                catalog.sources.push(path);
            }
            "application" => {
                exact_shape(child, 1, &[], false)?;
                let id = string_argument(child, 0, 1, 64)?.to_owned();
                validate_identifier(&id, "catalog application")?;
                if catalog.applications.len() >= 256 || catalog.applications.contains(&id) {
                    return schema_error("duplicate or excessive catalog application");
                }
                catalog.applications.push(id);
            }
            "terminal" => {
                exact_shape(child, 1, &[], true)?;
                if catalog.terminal.is_some() {
                    return schema_error("duplicate catalog terminal");
                }
                let id = string_argument(child, 0, 1, 64)?.to_owned();
                validate_identifier(&id, "catalog terminal")?;
                catalog.terminal = Some(id);
                catalog.terminal_arguments = child
                    .children()
                    .map(parse_arguments)
                    .transpose()?
                    .unwrap_or_default();
            }
            _ => unreachable!(),
        }
    }
    Ok(catalog)
}
