use sophia_protocol::{
    BufferSource, LayerSnapshot, Rect, Region, ResizeSyncCapability, SurfaceId, Transform,
};

pub fn arg_value(args: &[String], key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    args.iter()
        .find_map(|arg| arg.strip_prefix(&prefix).map(str::to_owned))
}

#[cfg(feature = "native-session")]
pub fn parse_usize(value: &str) -> Result<usize, Box<dyn std::error::Error>> {
    value
        .parse::<usize>()
        .map_err(|error| format!("invalid usize value {value:?}: {error}").into())
}

#[cfg(feature = "native-session")]
pub fn parse_u64(value: &str) -> Result<u64, Box<dyn std::error::Error>> {
    value
        .parse::<u64>()
        .map_err(|error| format!("invalid u64 value {value:?}: {error}").into())
}

pub fn synthetic_layers() -> Vec<LayerSnapshot> {
    vec![LayerSnapshot {
        surface: SurfaceId::new(1, 1),
        authority_local_id: None,
        // Synthetic: no projection placed it.
        output: None,
        namespace: None,
        stack_rank: 0,
        geometry: Rect {
            x: 10,
            y: 10,
            width: 320,
            height: 200,
        },
        source: BufferSource::CpuBuffer { handle: 1 },
        source_size: sophia_protocol::Size {
            width: 320,
            height: 200,
        },
        damage: Region::single(Rect {
            x: 10,
            y: 10,
            width: 320,
            height: 200,
        }),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 1,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    }]
}

pub fn resolve_external_probe_binary(
    label: &str,
    binary: &str,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let env_name = format!("SOPHIA_XAUTHORITY_{}", label.to_ascii_uppercase());
    if let Ok(override_path) = std::env::var(&env_name) {
        if override_path.is_empty() {
            return Err(format!("{env_name} is set but empty").into());
        }
        return Ok(std::path::PathBuf::from(override_path));
    }
    if binary.contains('/') {
        let path = std::path::PathBuf::from(binary);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "{label} probe binary {binary:?} was not found; set {env_name} to override"
        )
        .into());
    }
    let Some(path_var) = std::env::var_os("PATH") else {
        return Err(format!("{label} probe binary {binary:?} needs PATH or {env_name}").into());
    };
    for directory in std::env::split_paths(&path_var) {
        let candidate = directory.join(binary);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "{label} probe binary {binary:?} was not found in PATH; set {env_name} to override"
    )
    .into())
}

pub fn x11_keycode_for_ascii(byte: u8) -> Option<u8> {
    [
        (b"qwertyuiop".as_slice(), 24u8),
        (b"asdfghjkl".as_slice(), 38u8),
        (b"zxcvbnm".as_slice(), 52u8),
    ]
    .into_iter()
    .find_map(|(row, base)| {
        row.iter()
            .position(|candidate| *candidate == byte)
            .map(|position| base + position as u8)
    })
}
