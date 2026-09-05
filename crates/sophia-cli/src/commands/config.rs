use std::path::{Path, PathBuf};

use sophia_config::{
    ConfigDomain, ConfigGeneration, ConfigSource, ConfigSourceClass,
    discover_default_config_source, load_core_snapshot, load_desktop_profile, load_wm_snapshot,
};

pub(crate) fn try_run(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
    if args.first().map(String::as_str) != Some("config") {
        return Ok(false);
    }
    let operation = args.get(1).map(String::as_str).unwrap_or("check");
    if let Some(path) = desktop_profile_path(args)? {
        validate_desktop_profile_options(args)?;
        if operation != "check" {
            return Err("desktop profiles currently support only config check".into());
        }
        check_desktop_profile(&path)?;
        return Ok(true);
    }
    let wm = args.iter().skip(2).any(|argument| argument == "--wm");
    let domain = if wm {
        ConfigDomain::Wm
    } else {
        ConfigDomain::Core
    };
    let explicit = explicit_path(args, wm)?;
    validate_options(args, wm)?;
    let source = discover_default_config_source(domain, explicit.as_deref());
    match operation {
        "check" => check(domain, &source)?,
        "print-effective" => print_effective(domain, &source)?,
        other => {
            return Err(format!(
                "unknown config operation {other:?}; expected check or print-effective"
            )
            .into());
        }
    }
    Ok(true)
}

fn desktop_profile_path(args: &[String]) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    let values = args
        .iter()
        .skip(2)
        .filter_map(|argument| argument.strip_prefix("--desktop-profile="))
        .collect::<Vec<_>>();
    if values.len() > 1 {
        return Err("duplicate --desktop-profile".into());
    }
    let path = values.first().map(PathBuf::from);
    if path
        .as_ref()
        .is_some_and(|path| !path.is_absolute() || path.as_os_str().is_empty())
    {
        return Err("--desktop-profile requires an absolute path".into());
    }
    Ok(path)
}

fn validate_desktop_profile_options(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    for argument in args.iter().skip(2) {
        if argument == "-v" || argument == "--verbose" || argument.starts_with("--desktop-profile=")
        {
            continue;
        }
        return Err(format!("desktop profile check rejects option {argument:?}").into());
    }
    Ok(())
}

fn check_desktop_profile(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let profile = load_desktop_profile(Some(path), ConfigGeneration::INITIAL)?;
    println!(
        "valid domain=desktop-profile schema=1 generation={} digest={} sources={} policy_validation=delegated",
        profile.generation.raw(),
        profile.digest,
        profile.sources.len()
    );
    Ok(())
}

fn validate_options(args: &[String], wm: bool) -> Result<(), Box<dyn std::error::Error>> {
    for argument in args.iter().skip(2) {
        let valid_path = if wm {
            argument.starts_with("--wm-config=")
        } else {
            argument.starts_with("--config=")
        };
        if argument == "--wm" || argument == "-v" || argument == "--verbose" || valid_path {
            continue;
        }
        return Err(format!("unknown config option {argument:?}").into());
    }
    Ok(())
}

fn explicit_path(args: &[String], wm: bool) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    let option = if wm { "--wm-config" } else { "--config" };
    let incompatible = if wm { "--config=" } else { "--wm-config=" };
    if args
        .iter()
        .skip(2)
        .any(|argument| argument.starts_with(incompatible))
    {
        return Err(format!("{incompatible} does not select this configuration domain").into());
    }
    let values = args
        .iter()
        .skip(2)
        .filter_map(|argument| argument.strip_prefix(&format!("{option}=")))
        .collect::<Vec<_>>();
    if values.len() > 1 {
        return Err(format!("duplicate {option}").into());
    }
    let path = values.first().map(PathBuf::from);
    if path
        .as_ref()
        .is_some_and(|path| !path.is_absolute() || path.as_os_str().is_empty())
    {
        return Err(format!("{option} requires an absolute path").into());
    }
    Ok(path)
}

fn check(domain: ConfigDomain, source: &ConfigSource) -> Result<(), Box<dyn std::error::Error>> {
    let (schema, digest) = match domain {
        ConfigDomain::Core => {
            let snapshot = load_core_snapshot(source, ConfigGeneration::INITIAL)?;
            (snapshot.schema, snapshot.digest)
        }
        ConfigDomain::Wm => {
            let snapshot = load_wm_snapshot(source, ConfigGeneration::INITIAL)?;
            (snapshot.schema, snapshot.digest)
        }
    };
    println!(
        "valid domain={} schema={} digest={} source={}",
        domain_name(domain),
        schema,
        digest,
        source_name(source)
    );
    Ok(())
}

fn print_effective(
    domain: ConfigDomain,
    source: &ConfigSource,
) -> Result<(), Box<dyn std::error::Error>> {
    match domain {
        ConfigDomain::Core => {
            let snapshot = load_core_snapshot(source, ConfigGeneration::INITIAL)?;
            println!("{snapshot:#?}");
        }
        ConfigDomain::Wm => {
            let snapshot = load_wm_snapshot(source, ConfigGeneration::INITIAL)?;
            println!("{snapshot:#?}");
        }
    }
    Ok(())
}

fn source_name(source: &ConfigSource) -> String {
    match (&source.class, &source.path) {
        (ConfigSourceClass::CompiledDefault, _) => "compiled-default".to_owned(),
        (_, Some(path)) => path.display().to_string(),
        (_, None) => "invalid-source".to_owned(),
    }
}

const fn domain_name(domain: ConfigDomain) -> &'static str {
    match domain {
        ConfigDomain::Core => "core",
        ConfigDomain::Wm => "wm",
    }
}
