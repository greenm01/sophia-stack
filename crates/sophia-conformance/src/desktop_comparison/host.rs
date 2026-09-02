use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostIdentity {
    pub kernel: String,
    pub mesa: String,
    pub gpu: String,
}

pub fn detect() -> Result<HostIdentity, String> {
    let kernel = token_file("/proc/sys/kernel/osrelease", "kernel")?;
    let mesa_version = command_output("pkg-config", &["--modversion", "gbm"], "Mesa/GBM")?;
    let mesa = format!("gbm-{mesa_version}");
    require_token("Mesa", &mesa)?;
    let gpu = gpu_identity()?;
    Ok(HostIdentity { kernel, mesa, gpu })
}

fn gpu_identity() -> Result<String, String> {
    let entries = fs::read_dir("/sys/class/drm")
        .map_err(|error| format!("could not enumerate DRM devices: {error}"))?;
    let mut cards = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.strip_prefix("card").is_some_and(|suffix| {
                        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
                    })
                })
        })
        .collect::<Vec<_>>();
    cards.sort();
    let mut identity = Vec::new();
    for card in cards {
        let device = card.join("device");
        if !device.is_dir() {
            continue;
        }
        let card_name = card
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("DRM card name is not UTF-8")?;
        identity.extend_from_slice(card_name.as_bytes());
        for property in ["vendor", "device", "revision"] {
            let path = device.join(property);
            let value = fs::read(&path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?;
            identity.extend_from_slice(&value);
        }
        let driver = fs::read_link(device.join("driver"))
            .map_err(|error| format!("could not resolve DRM driver for {card_name}: {error}"))?;
        let driver_name = driver
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("DRM driver name is not UTF-8")?;
        identity.extend_from_slice(driver_name.as_bytes());
    }
    if identity.is_empty() {
        return Err("desktop comparison requires at least one DRM card".to_owned());
    }
    Ok(format!("sha256-{:x}", Sha256::digest(identity)))
}

fn token_file(path: impl Into<PathBuf>, name: &str) -> Result<String, String> {
    let path = path.into();
    let value = fs::read_to_string(&path)
        .map_err(|error| format!("could not read {name} identity {}: {error}", path.display()))?
        .trim()
        .to_owned();
    require_token(name, &value)?;
    Ok(value)
}

fn command_output(program: &str, arguments: &[&str], name: &str) -> Result<String, String> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("could not run {program} for {name} identity: {error}"))?;
    if !output.status.success() {
        return Err(format!("{program} could not resolve {name} identity"));
    }
    let value = String::from_utf8(output.stdout)
        .map_err(|_| format!("{name} identity is not UTF-8"))?
        .trim()
        .to_owned();
    require_token(name, &value)?;
    Ok(value)
}

fn require_token(name: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.chars().any(char::is_whitespace) || value.contains('=') {
        Err(format!("{name} identity is not a key-value-safe token"))
    } else {
        Ok(())
    }
}
