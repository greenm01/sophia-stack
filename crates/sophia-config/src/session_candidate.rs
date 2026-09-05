use kdl::{KdlDocument, KdlNode};

use crate::{
    ConfigDigest, ConfigGeneration, DesktopAuthority, DesktopAuthorityCandidate,
    DesktopProfileError,
};

pub const DESKTOP_SESSION_MAX_APPLICATION_NAME_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DesktopControlAccess {
    #[default]
    Disabled,
    HostAdmin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopSessionCandidate {
    pub generation: ConfigGeneration,
    pub digest: ConfigDigest,
    pub terminal: Option<String>,
    pub browser: Option<String>,
    pub startup: Option<String>,
    pub logout_enabled: Option<bool>,
    pub control: DesktopControlAccess,
}

fn schema_error(message: impl Into<String>) -> DesktopProfileError {
    DesktopProfileError::Schema(format!("session candidate: {}", message.into()))
}

fn single_node(encoded: &str) -> Result<KdlNode, DesktopProfileError> {
    let document = KdlDocument::parse_v2(encoded)
        .map_err(|error| schema_error(format!("invalid staged value: {error}")))?;
    if document.nodes().len() != 1 {
        return Err(schema_error("staged value must contain exactly one node"));
    }
    Ok(document.nodes()[0].clone())
}

fn application_name(node: &KdlNode, setting: &str) -> Result<String, DesktopProfileError> {
    if node.entries().len() != 1 || node.children().is_some() || node.ty().is_some() {
        return Err(schema_error(format!(
            "{setting} requires one application identity"
        )));
    }
    let name = node
        .get(0)
        .and_then(|value| value.as_string())
        .filter(|name| !name.is_empty() && name.len() <= DESKTOP_SESSION_MAX_APPLICATION_NAME_BYTES)
        .ok_or_else(|| schema_error(format!("{setting} application identity is invalid")))?;
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(schema_error(format!(
            "{setting} application identity contains unsupported characters"
        )));
    }
    Ok(name.to_owned())
}

fn logout_enabled(node: &KdlNode) -> Result<bool, DesktopProfileError> {
    if node.entries().len() != 1 || node.children().is_some() || node.ty().is_some() {
        return Err(schema_error("logout requires one boolean"));
    }
    node.get(0)
        .and_then(|value| value.as_bool())
        .ok_or_else(|| schema_error("logout requires one boolean"))
}

pub fn prepare_desktop_session_candidate(
    candidate: &DesktopAuthorityCandidate,
) -> Result<DesktopSessionCandidate, DesktopProfileError> {
    if candidate.authority != DesktopAuthority::Session {
        return Err(schema_error("candidate crossed its authority boundary"));
    }
    let mut prepared = DesktopSessionCandidate {
        generation: candidate.generation,
        digest: candidate.digest,
        terminal: None,
        browser: None,
        startup: None,
        logout_enabled: None,
        control: DesktopControlAccess::Disabled,
    };
    for value in &candidate.values {
        let node = single_node(&value.encoded)?;
        match node.name().value() {
            "terminal" => prepared.terminal = Some(application_name(&node, "terminal")?),
            "browser" => prepared.browser = Some(application_name(&node, "browser")?),
            "startup" => prepared.startup = Some(application_name(&node, "startup")?),
            "logout" => prepared.logout_enabled = Some(logout_enabled(&node)?),
            "control" => {
                if node.entries().len() != 1 || node.children().is_some() || node.ty().is_some() {
                    return Err(schema_error("control requires one access mode"));
                }
                prepared.control = match node.get(0).and_then(|value| value.as_string()) {
                    Some("disabled") => DesktopControlAccess::Disabled,
                    Some("host-admin") => DesktopControlAccess::HostAdmin,
                    _ => return Err(schema_error("control must be disabled or host-admin")),
                };
            }
            _ => return Err(schema_error("candidate contains a non-session setting")),
        }
    }
    Ok(prepared)
}
