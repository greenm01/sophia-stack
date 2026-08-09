use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use kdl::{KdlDocument, KdlNode};
use sha2::{Digest, Sha256};

use crate::{ConfigDigest, ConfigGeneration, ConfigIoError, ConfigParseError, read_config_file};

pub const DESKTOP_PROFILE_MAX_DEPTH: usize = 10;
pub const DESKTOP_PROFILE_MAX_FILES: usize = 64;
pub const DESKTOP_PROFILE_MAX_BYTES: usize = 1024 * 1024;

pub const COMPILED_DESKTOP_PROFILE: &str = r#"schema 1
policy {
  layout "scroller"
  view-count 9
  outer-gap 0
  inner-gap 0
}
shell { enabled #false; }
shortcut { profile "compiled"; }
session { terminal "terminal"; browser "browser"; }
input { inherit-sophia #true; }
output { inherit-sophia #true; }
broker { enabled #false; }
"#;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DesktopAuthority {
    Policy,
    Shell,
    Shortcut,
    Session,
    Input,
    Output,
    Broker,
}

impl DesktopAuthority {
    pub const ALL: [Self; 7] = [
        Self::Policy,
        Self::Shell,
        Self::Shortcut,
        Self::Session,
        Self::Input,
        Self::Output,
        Self::Broker,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Policy => "policy",
            Self::Shell => "shell",
            Self::Shortcut => "shortcut",
            Self::Session => "session",
            Self::Input => "input",
            Self::Output => "output",
            Self::Broker => "broker",
        }
    }

    fn parse(name: &str) -> Result<Self, DesktopProfileError> {
        Self::ALL
            .into_iter()
            .find(|authority| authority.name() == name)
            .ok_or_else(|| DesktopProfileError::Schema(format!("unsupported section {name:?}")))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopValueProvenance {
    pub path: PathBuf,
    pub ordinal: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopProfileValue {
    pub key: String,
    pub encoded: String,
    pub provenance: DesktopValueProvenance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopAuthorityCandidate {
    pub authority: DesktopAuthority,
    pub generation: ConfigGeneration,
    pub digest: ConfigDigest,
    pub values: Vec<DesktopProfileValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopProfileGeneration {
    pub generation: ConfigGeneration,
    pub digest: ConfigDigest,
    pub sources: Vec<PathBuf>,
    pub candidates: BTreeMap<DesktopAuthority, DesktopAuthorityCandidate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DesktopProfileError {
    Io(ConfigIoError),
    Parse(ConfigParseError),
    Schema(String),
    Limit(String),
    Stage(String),
}

impl fmt::Display for DesktopProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Parse(error) => error.fmt(formatter),
            Self::Schema(error) => write!(formatter, "desktop profile schema error: {error}"),
            Self::Limit(error) => write!(formatter, "desktop profile limit exceeded: {error}"),
            Self::Stage(error) => write!(formatter, "desktop profile staging failed: {error}"),
        }
    }
}

#[derive(Debug)]
pub struct DesktopProfileFragments {
    pub generation: ConfigGeneration,
    pub digest: ConfigDigest,
    paths: BTreeMap<DesktopAuthority, PathBuf>,
}

impl DesktopProfileFragments {
    pub fn path(&self, authority: DesktopAuthority) -> &Path {
        self.paths
            .get(&authority)
            .expect("every desktop authority has a staged fragment")
    }
}

impl Drop for DesktopProfileFragments {
    fn drop(&mut self) {
        for path in self.paths.values() {
            let _ = fs::remove_file(path);
        }
    }
}

impl std::error::Error for DesktopProfileError {}

impl From<ConfigIoError> for DesktopProfileError {
    fn from(error: ConfigIoError) -> Self {
        Self::Io(error)
    }
}

impl From<ConfigParseError> for DesktopProfileError {
    fn from(error: ConfigParseError) -> Self {
        Self::Parse(error)
    }
}

pub fn discover_desktop_profile_source(
    explicit: Option<&Path>,
    xdg_config_home: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return Some(path.to_path_buf());
    }
    if let Some(root) = xdg_config_home {
        let path = root.join("hagia/config.kdl");
        if path.is_file() {
            return Some(path);
        }
    }
    let system = PathBuf::from("/etc/hagia/config.kdl");
    system.is_file().then_some(system)
}

pub fn load_desktop_profile(
    source: Option<&Path>,
    generation: ConfigGeneration,
) -> Result<DesktopProfileGeneration, DesktopProfileError> {
    if generation.raw() == 0 {
        return Err(DesktopProfileError::Schema(
            "generation must be nonzero".to_owned(),
        ));
    }
    let mut expansion = Expansion::default();
    let nodes = if let Some(source) = source {
        expansion.expand(source, 0)?
    } else {
        expansion.sources.push(PathBuf::from("<compiled>"));
        expansion.digest_input.extend_from_slice(b"<compiled>\0");
        expansion
            .digest_input
            .extend_from_slice(COMPILED_DESKTOP_PROFILE.as_bytes());
        parse_nodes(COMPILED_DESKTOP_PROFILE, Path::new("<compiled>"))?
    };
    let digest = ConfigDigest::new(Sha256::digest(&expansion.digest_input).into());
    let candidates = partition(&nodes, generation, digest)?;
    Ok(DesktopProfileGeneration {
        generation,
        digest,
        sources: expansion.sources,
        candidates,
    })
}

pub fn stage_desktop_profile(
    profile: &DesktopProfileGeneration,
    directory: &Path,
) -> Result<DesktopProfileFragments, DesktopProfileError> {
    use std::io::Write as _;
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};

    if !directory.is_absolute() {
        return Err(DesktopProfileError::Stage(
            "staging directory must be absolute".to_owned(),
        ));
    }
    let metadata = fs::symlink_metadata(directory)
        .map_err(|error| DesktopProfileError::Stage(error.to_string()))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(DesktopProfileError::Stage(
            "staging directory must be a private directory owned by the effective user".to_owned(),
        ));
    }

    let mut paths = BTreeMap::new();
    for authority in DesktopAuthority::ALL {
        let candidate = profile.candidates.get(&authority).ok_or_else(|| {
            DesktopProfileError::Stage(format!("missing {} candidate", authority.name()))
        })?;
        if candidate.authority != authority
            || candidate.generation != profile.generation
            || candidate.digest != profile.digest
        {
            return Err(DesktopProfileError::Stage(format!(
                "inconsistent {} candidate identity",
                authority.name()
            )));
        }
        let path = directory.join(format!("{}.profile.kdl", authority.name()));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|error| DesktopProfileError::Stage(error.to_string()))?;
        let result = (|| -> Result<(), DesktopProfileError> {
            writeln!(file, "schema 1")
                .and_then(|_| writeln!(file, "profile-generation {}", profile.generation.raw()))
                .and_then(|_| writeln!(file, "profile-digest \"{}\"", profile.digest))
                .and_then(|_| writeln!(file, "{} {{", authority.name()))
                .map_err(|error| DesktopProfileError::Stage(error.to_string()))?;
            for value in &candidate.values {
                writeln!(file, "  {}", value.encoded)
                    .map_err(|error| DesktopProfileError::Stage(error.to_string()))?;
            }
            writeln!(file, "}}").map_err(|error| DesktopProfileError::Stage(error.to_string()))?;
            file.sync_all()
                .map_err(|error| DesktopProfileError::Stage(error.to_string()))?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                .map_err(|error| DesktopProfileError::Stage(error.to_string()))?;
            Ok(())
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(&path);
            for staged in paths.values() {
                let _ = fs::remove_file(staged);
            }
            return Err(error);
        }
        paths.insert(authority, path);
    }
    if let Err(error) = fs::File::open(directory).and_then(|directory| directory.sync_all()) {
        for staged in paths.values() {
            let _ = fs::remove_file(staged);
        }
        return Err(DesktopProfileError::Stage(error.to_string()));
    }
    Ok(DesktopProfileFragments {
        generation: profile.generation,
        digest: profile.digest,
        paths,
    })
}

#[derive(Default)]
struct Expansion {
    files: usize,
    bytes: usize,
    stack: Vec<PathBuf>,
    seen: BTreeSet<PathBuf>,
    sources: Vec<PathBuf>,
    digest_input: Vec<u8>,
}

#[derive(Clone)]
struct ExpandedNode {
    node: KdlNode,
    provenance: DesktopValueProvenance,
}

impl Expansion {
    fn expand(
        &mut self,
        path: &Path,
        depth: usize,
    ) -> Result<Vec<ExpandedNode>, DesktopProfileError> {
        if depth > DESKTOP_PROFILE_MAX_DEPTH {
            return Err(DesktopProfileError::Limit(
                "include depth exceeds 10".to_owned(),
            ));
        }
        if !path.is_absolute() {
            return Err(ConfigIoError::InvalidPath.into());
        }
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| ConfigIoError::Metadata(error.to_string()))?;
        if metadata.file_type().is_symlink() {
            return Err(ConfigIoError::NotRegularFile.into());
        }
        let canonical =
            fs::canonicalize(path).map_err(|error| ConfigIoError::Metadata(error.to_string()))?;
        if self.stack.contains(&canonical) {
            return Err(DesktopProfileError::Schema(format!(
                "include cycle reaches {}",
                canonical.display()
            )));
        }
        if !self.seen.insert(canonical.clone()) {
            return Err(DesktopProfileError::Schema(format!(
                "source included more than once: {}",
                canonical.display()
            )));
        }
        self.files += 1;
        if self.files > DESKTOP_PROFILE_MAX_FILES {
            return Err(DesktopProfileError::Limit("more than 64 files".to_owned()));
        }
        let bytes = read_config_file(&canonical)?;
        if bytes.is_empty() {
            return Err(DesktopProfileError::Schema("source is empty".to_owned()));
        }
        self.bytes = self.bytes.saturating_add(bytes.len());
        if self.bytes > DESKTOP_PROFILE_MAX_BYTES {
            return Err(DesktopProfileError::Limit(
                "aggregate exceeds one MiB".to_owned(),
            ));
        }
        let source = std::str::from_utf8(&bytes).map_err(|_| ConfigParseError::NotUtf8)?;
        let document = KdlDocument::parse_v2(source)
            .map_err(|error| ConfigParseError::Syntax(error.to_string()))?;
        self.stack.push(canonical.clone());
        self.sources.push(canonical.clone());
        self.digest_input
            .extend_from_slice(canonical.as_os_str().as_encoded_bytes());
        self.digest_input.push(0);
        self.digest_input.extend_from_slice(&bytes);
        self.digest_input.push(0);
        let mut result = Vec::new();
        for (index, node) in document.nodes().iter().enumerate() {
            if node.name().value() == "include" {
                let include = exact_string_argument(node, "include")?;
                let resolved = if Path::new(include).is_absolute() {
                    PathBuf::from(include)
                } else {
                    canonical
                        .parent()
                        .ok_or(ConfigIoError::InvalidPath)?
                        .join(include)
                };
                result.extend(self.expand(&resolved, depth + 1)?);
            } else {
                result.push(ExpandedNode {
                    node: node.clone(),
                    provenance: DesktopValueProvenance {
                        path: canonical.clone(),
                        ordinal: index + 1,
                    },
                });
            }
        }
        self.stack.pop();
        Ok(result)
    }
}

fn parse_nodes(source: &str, path: &Path) -> Result<Vec<ExpandedNode>, DesktopProfileError> {
    let document = KdlDocument::parse_v2(source)
        .map_err(|error| ConfigParseError::Syntax(error.to_string()))?;
    Ok(document
        .nodes()
        .iter()
        .enumerate()
        .map(|(index, node)| ExpandedNode {
            node: node.clone(),
            provenance: DesktopValueProvenance {
                path: path.to_path_buf(),
                ordinal: index + 1,
            },
        })
        .collect())
}

fn partition(
    nodes: &[ExpandedNode],
    generation: ConfigGeneration,
    digest: ConfigDigest,
) -> Result<BTreeMap<DesktopAuthority, DesktopAuthorityCandidate>, DesktopProfileError> {
    let mut candidates = DesktopAuthority::ALL
        .into_iter()
        .map(|authority| {
            (
                authority,
                DesktopAuthorityCandidate {
                    authority,
                    generation,
                    digest,
                    values: Vec::new(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut schema_seen = false;
    let mut keys = BTreeSet::new();
    for expanded in nodes {
        let node = &expanded.node;
        if node.name().value() == "schema" {
            if schema_seen || integer_argument(node) != Some(1) || node.children().is_some() {
                return Err(DesktopProfileError::Schema(
                    "requires exactly one schema 1 declaration".to_owned(),
                ));
            }
            schema_seen = true;
            continue;
        }
        if !node.entries().is_empty() || node.ty().is_some() {
            return Err(DesktopProfileError::Schema(format!(
                "ambiguous authority section {:?}",
                node.name().value()
            )));
        }
        let authority = DesktopAuthority::parse(node.name().value())?;
        let children = node.children().ok_or_else(|| {
            DesktopProfileError::Schema(format!(
                "authority section {:?} requires children",
                node.name().value()
            ))
        })?;
        for child in children.nodes() {
            validate_setting(authority, child)?;
            let key = setting_key(authority, child)?;
            if !keys.insert(key.clone()) {
                return Err(DesktopProfileError::Schema(format!(
                    "duplicate setting {key:?}"
                )));
            }
            candidates
                .get_mut(&authority)
                .expect("all authority candidates exist")
                .values
                .push(DesktopProfileValue {
                    key,
                    encoded: child.to_string().trim().to_owned(),
                    provenance: expanded.provenance.clone(),
                });
        }
    }
    if !schema_seen {
        return Err(DesktopProfileError::Schema(
            "missing schema declaration".to_owned(),
        ));
    }
    Ok(candidates)
}

fn validate_setting(
    authority: DesktopAuthority,
    node: &KdlNode,
) -> Result<(), DesktopProfileError> {
    if node.ty().is_some() || node.entries().len() > 32 {
        return Err(DesktopProfileError::Schema(
            "typed or structurally excessive setting".to_owned(),
        ));
    }
    let name = node.name().value();
    if [
        "emergency-chord",
        "policy-timeout",
        "max-surfaces",
        "max-outputs",
        "renderer",
        "scanout",
        "namespace-profile",
    ]
    .contains(&name)
    {
        return Err(DesktopProfileError::Schema(
            "reserved control override".to_owned(),
        ));
    }
    let supported = match authority {
        DesktopAuthority::Policy => [
            "layout",
            "view-count",
            "outer-gap",
            "inner-gap",
            "viewport-offset",
        ]
        .contains(&name),
        DesktopAuthority::Shell => ["enabled", "panel"].contains(&name),
        DesktopAuthority::Shortcut => ["profile", "bind", "pointer-bind"].contains(&name),
        DesktopAuthority::Session => ["terminal", "browser", "logout", "startup"].contains(&name),
        DesktopAuthority::Input => ["inherit-sophia", "keyboard", "pointer"].contains(&name),
        DesktopAuthority::Output => ["inherit-sophia", "named"].contains(&name),
        DesktopAuthority::Broker => ["enabled", "capability"].contains(&name),
    };
    if !supported {
        return Err(DesktopProfileError::Schema(format!(
            "unsupported {} setting {name:?}",
            authority.name()
        )));
    }
    if authority == DesktopAuthority::Policy
        && name == "layout"
        && exact_string_argument(node, "policy layout")? != "scroller"
    {
        return Err(DesktopProfileError::Schema(
            "unsupported policy layout".to_owned(),
        ));
    }
    if authority == DesktopAuthority::Policy && name == "view-count" {
        let value = exact_integer_argument(node, "policy view-count")?;
        if !(1..=9).contains(&value) {
            return Err(DesktopProfileError::Schema(
                "policy view-count must be in 1..=9".to_owned(),
            ));
        }
    }
    if authority == DesktopAuthority::Policy
        && ["outer-gap", "inner-gap", "viewport-offset"].contains(&name)
    {
        let value = exact_integer_argument(node, "policy geometry")?;
        if !(0..=i128::from(i32::MAX)).contains(&value) {
            return Err(DesktopProfileError::Schema(
                "policy geometry must be a nonnegative 32-bit integer".to_owned(),
            ));
        }
    }
    if matches!(
        authority,
        DesktopAuthority::Shell | DesktopAuthority::Broker
    ) && name == "enabled"
        && node.get(0).and_then(|value| value.as_bool()) != Some(false)
    {
        return Err(DesktopProfileError::Schema(
            "unavailable capability cannot be enabled".to_owned(),
        ));
    }
    Ok(())
}

fn setting_key(authority: DesktopAuthority, node: &KdlNode) -> Result<String, DesktopProfileError> {
    let mut key = format!("{}.{}", authority.name(), node.name().value());
    if ["bind", "pointer-bind", "application", "device", "named"].contains(&node.name().value()) {
        key.push('.');
        key.push_str(exact_first_string(node)?);
    }
    Ok(key)
}

fn exact_string_argument<'a>(
    node: &'a KdlNode,
    context: &str,
) -> Result<&'a str, DesktopProfileError> {
    if node.entries().len() != 1 || node.children().is_some() || node.ty().is_some() {
        return Err(DesktopProfileError::Schema(format!(
            "{context} requires one string"
        )));
    }
    exact_first_string(node)
}

fn exact_first_string(node: &KdlNode) -> Result<&str, DesktopProfileError> {
    node.get(0)
        .and_then(|value| value.as_string())
        .ok_or_else(|| DesktopProfileError::Schema("string identity required".to_owned()))
}

fn exact_integer_argument(node: &KdlNode, context: &str) -> Result<i128, DesktopProfileError> {
    if node.entries().len() != 1 || node.children().is_some() || node.ty().is_some() {
        return Err(DesktopProfileError::Schema(format!(
            "{context} requires one integer"
        )));
    }
    node.get(0)
        .and_then(|value| value.as_integer())
        .ok_or_else(|| DesktopProfileError::Schema(format!("{context} requires one integer")))
}

fn integer_argument(node: &KdlNode) -> Option<i128> {
    (node.entries().len() == 1 && node.ty().is_none())
        .then(|| node.get(0))
        .flatten()
        .and_then(|value| value.as_integer())
}
