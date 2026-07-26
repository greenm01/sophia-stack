use std::fmt;

use crate::{
    COMPILED_CORE_CONFIG, COMPILED_WM_CONFIG, ConfigGeneration, ConfigIoError, ConfigParseError,
    ConfigSource, ConfigSourceClass, CoreConfigDelta, CoreConfigSnapshot, ReloadDisposition,
    WmConfigDelta, WmConfigSnapshot, parse_core_config, parse_wm_config, read_config_file,
};

#[derive(Debug)]
pub enum ConfigLoadError {
    Io(ConfigIoError),
    Parse(ConfigParseError),
}

impl fmt::Display for ConfigLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Parse(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ConfigLoadError {}

impl From<ConfigIoError> for ConfigLoadError {
    fn from(error: ConfigIoError) -> Self {
        Self::Io(error)
    }
}

impl From<ConfigParseError> for ConfigLoadError {
    fn from(error: ConfigParseError) -> Self {
        Self::Parse(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreReloadReport {
    pub disposition: ReloadDisposition,
    pub generation: ConfigGeneration,
    pub delta: CoreConfigDelta,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WmReloadReport {
    pub disposition: ReloadDisposition,
    pub generation: ConfigGeneration,
    pub delta: WmConfigDelta,
}

#[derive(Clone, Debug)]
pub struct CoreConfigState {
    active: CoreConfigSnapshot,
    pending_restart: Option<CoreConfigSnapshot>,
}

impl CoreConfigState {
    pub fn load(source: &ConfigSource) -> Result<Self, ConfigLoadError> {
        Ok(Self {
            active: load_core_snapshot(source, ConfigGeneration::INITIAL)?,
            pending_restart: None,
        })
    }

    pub fn from_snapshot(active: CoreConfigSnapshot) -> Self {
        Self {
            active,
            pending_restart: None,
        }
    }

    pub const fn active(&self) -> &CoreConfigSnapshot {
        &self.active
    }

    pub const fn pending_restart(&self) -> Option<&CoreConfigSnapshot> {
        self.pending_restart.as_ref()
    }

    pub fn reload(&mut self, bytes: &[u8]) -> Result<CoreReloadReport, ConfigParseError> {
        let generation = next_generation(self.active.generation)?;
        let candidate = parse_core_config(bytes, generation)?;
        let delta = CoreConfigDelta::between(&self.active, &candidate);
        if candidate.digest == self.active.digest {
            self.pending_restart = None;
            return Ok(CoreReloadReport {
                disposition: ReloadDisposition::Deferred,
                generation: self.active.generation,
                delta,
            });
        }
        if delta.restart_required {
            self.pending_restart = Some(candidate);
            return Ok(CoreReloadReport {
                disposition: ReloadDisposition::PendingRestart,
                generation,
                delta,
            });
        }
        self.active = candidate;
        self.pending_restart = None;
        Ok(CoreReloadReport {
            disposition: ReloadDisposition::Applied,
            generation,
            delta,
        })
    }
}

#[derive(Clone, Debug)]
pub struct WmConfigState {
    active: WmConfigSnapshot,
}

impl WmConfigState {
    pub fn load(source: &ConfigSource) -> Result<Self, ConfigLoadError> {
        Ok(Self {
            active: load_wm_snapshot(source, ConfigGeneration::INITIAL)?,
        })
    }

    pub fn from_snapshot(active: WmConfigSnapshot) -> Self {
        Self { active }
    }

    pub const fn active(&self) -> &WmConfigSnapshot {
        &self.active
    }

    pub fn reload(&mut self, bytes: &[u8]) -> Result<WmReloadReport, ConfigParseError> {
        let generation = next_generation(self.active.generation)?;
        let candidate = parse_wm_config(bytes, generation)?;
        let delta = WmConfigDelta::between(&self.active, &candidate);
        if candidate.digest == self.active.digest {
            return Ok(WmReloadReport {
                disposition: ReloadDisposition::Deferred,
                generation: self.active.generation,
                delta,
            });
        }
        self.active = candidate;
        Ok(WmReloadReport {
            disposition: ReloadDisposition::Applied,
            generation,
            delta,
        })
    }
}

pub fn load_core_snapshot(
    source: &ConfigSource,
    generation: ConfigGeneration,
) -> Result<CoreConfigSnapshot, ConfigLoadError> {
    let bytes = source_bytes(source, COMPILED_CORE_CONFIG)?;
    Ok(parse_core_config(&bytes, generation)?)
}

pub fn load_wm_snapshot(
    source: &ConfigSource,
    generation: ConfigGeneration,
) -> Result<WmConfigSnapshot, ConfigLoadError> {
    let bytes = source_bytes(source, COMPILED_WM_CONFIG)?;
    Ok(parse_wm_config(&bytes, generation)?)
}

fn source_bytes(source: &ConfigSource, compiled: &str) -> Result<Vec<u8>, ConfigIoError> {
    match (&source.class, &source.path) {
        (ConfigSourceClass::CompiledDefault, None) => Ok(compiled.as_bytes().to_vec()),
        (ConfigSourceClass::CompiledDefault, Some(_)) | (_, None) => {
            Err(ConfigIoError::InvalidSource)
        }
        (_, Some(path)) => read_config_file(path),
    }
}

fn next_generation(active: ConfigGeneration) -> Result<ConfigGeneration, ConfigParseError> {
    active.next().ok_or_else(|| {
        ConfigParseError::Schema("configuration generation counter exhausted".to_owned())
    })
}
