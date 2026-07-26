use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::SOPHIA_CONFIG_MAX_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigDomain {
    Core,
    Wm,
}

impl ConfigDomain {
    pub const fn filename(self) -> &'static str {
        match self {
            Self::Core => "config.kdl",
            Self::Wm => "wm.kdl",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigSourceClass {
    Explicit,
    User,
    System,
    CompiledDefault,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigSource {
    pub class: ConfigSourceClass,
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigIoError {
    InvalidPath,
    InvalidSource,
    Metadata(String),
    NotRegularFile,
    UnsafeOwner,
    UnsafeMode,
    TooLarge { bytes: u64 },
    Read(String),
}

impl fmt::Display for ConfigIoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath => formatter.write_str("configuration path must be absolute"),
            Self::InvalidSource => formatter.write_str("configuration source is inconsistent"),
            Self::Metadata(error) => write!(formatter, "configuration metadata failed: {error}"),
            Self::NotRegularFile => formatter.write_str("configuration is not a regular file"),
            Self::UnsafeOwner => formatter.write_str("configuration has an unsafe owner"),
            Self::UnsafeMode => {
                formatter.write_str("configuration is writable by group or other users")
            }
            Self::TooLarge { bytes } => write!(
                formatter,
                "configuration is {bytes} bytes; maximum is {SOPHIA_CONFIG_MAX_BYTES}"
            ),
            Self::Read(error) => write!(formatter, "configuration read failed: {error}"),
        }
    }
}

pub fn default_user_config_root() -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from) {
        if root.is_absolute() {
            return Some(root);
        }
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|root| root.is_absolute())
        .map(|root| root.join(".config"))
}

pub fn discover_default_config_source(
    domain: ConfigDomain,
    explicit: Option<&Path>,
) -> ConfigSource {
    let user_root = default_user_config_root();
    discover_config_source(domain, explicit, user_root.as_deref())
}

impl std::error::Error for ConfigIoError {}

pub fn discover_config_source(
    domain: ConfigDomain,
    explicit: Option<&Path>,
    xdg_config_home: Option<&Path>,
) -> ConfigSource {
    if let Some(path) = explicit {
        return ConfigSource {
            class: ConfigSourceClass::Explicit,
            path: Some(path.to_path_buf()),
        };
    }
    if let Some(root) = xdg_config_home {
        let path = root.join("sophia").join(domain.filename());
        if path.is_file() {
            return ConfigSource {
                class: ConfigSourceClass::User,
                path: Some(path),
            };
        }
    }
    let system = Path::new("/etc/sophia").join(domain.filename());
    if system.is_file() {
        return ConfigSource {
            class: ConfigSourceClass::System,
            path: Some(system),
        };
    }
    ConfigSource {
        class: ConfigSourceClass::CompiledDefault,
        path: None,
    }
}

pub fn read_config_file(path: &Path) -> Result<Vec<u8>, ConfigIoError> {
    if !path.is_absolute() {
        return Err(ConfigIoError::InvalidPath);
    }
    let mut file =
        fs::File::open(path).map_err(|error| ConfigIoError::Metadata(error.to_string()))?;
    let metadata = file
        .metadata()
        .map_err(|error| ConfigIoError::Metadata(error.to_string()))?;
    if !metadata.is_file() {
        return Err(ConfigIoError::NotRegularFile);
    }
    if metadata.len() > SOPHIA_CONFIG_MAX_BYTES as u64 {
        return Err(ConfigIoError::TooLarge {
            bytes: metadata.len(),
        });
    }
    validate_unix_metadata(&metadata)?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len())
            .unwrap_or(SOPHIA_CONFIG_MAX_BYTES)
            .min(SOPHIA_CONFIG_MAX_BYTES),
    );
    file.by_ref()
        .take((SOPHIA_CONFIG_MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| ConfigIoError::Read(error.to_string()))?;
    if bytes.len() > SOPHIA_CONFIG_MAX_BYTES {
        return Err(ConfigIoError::TooLarge {
            bytes: bytes.len() as u64,
        });
    }
    Ok(bytes)
}

#[cfg(unix)]
fn validate_unix_metadata(metadata: &fs::Metadata) -> Result<(), ConfigIoError> {
    use std::os::unix::fs::MetadataExt;

    let effective_uid = rustix::process::geteuid().as_raw();
    if metadata.uid() != effective_uid && metadata.uid() != 0 {
        return Err(ConfigIoError::UnsafeOwner);
    }
    if metadata.mode() & 0o022 != 0 {
        return Err(ConfigIoError::UnsafeMode);
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_unix_metadata(_metadata: &fs::Metadata) -> Result<(), ConfigIoError> {
    Ok(())
}
