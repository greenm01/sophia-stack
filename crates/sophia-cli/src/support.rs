use sophia_protocol::{
    BufferSource, LayerSnapshot, Rect, Region, ResizeSyncCapability, SurfaceId, Transform,
};

pub(crate) fn arg_value(args: &[String], key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    args.iter()
        .find_map(|arg| arg.strip_prefix(&prefix).map(str::to_owned))
}

#[cfg(feature = "atomic-scanout-live")]
pub(crate) fn parse_usize(value: &str) -> Result<usize, Box<dyn std::error::Error>> {
    value
        .parse::<usize>()
        .map_err(|error| format!("invalid usize value {value:?}: {error}").into())
}

#[cfg(feature = "atomic-scanout-live")]
pub(crate) fn parse_u64(value: &str) -> Result<u64, Box<dyn std::error::Error>> {
    value
        .parse::<u64>()
        .map_err(|error| format!("invalid u64 value {value:?}: {error}").into())
}

pub(crate) fn synthetic_layers() -> Vec<LayerSnapshot> {
    vec![LayerSnapshot {
        surface: SurfaceId::new(1, 1),
        authority_local_id: None,
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
