fn drm_sysfs_fixture(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("sophia-backend-live-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn ready_drm_sysfs_fixture(name: &str) -> PathBuf {
    let root = drm_sysfs_fixture(name);
    let connector = root.join("card0-HDMI-A-1");
    fs::create_dir_all(&connector).unwrap();
    write_fixture_file(&connector, "status", "connected\n");
    write_fixture_file(&connector, "modes", "1280x720\n");
    root
}

fn write_fixture_file(root: &Path, name: &str, contents: &str) {
    fs::write(root.join(name), contents).unwrap();
}

fn motion_event(serial: u64, x: f64, y: f64) -> InputEventPacket {
    InputEventPacket {
        serial,
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        time_msec: serial * 10,
        kind: InputEventKind::PointerMotion,
        global_position: Some(Point { x, y }),
        target_surface: None,
        local_position: None,
    }
}

fn test_layer(raw_surface: u32) -> LayerSnapshot {
    LayerSnapshot {
        surface: SurfaceId::new(raw_surface, 1),
        authority_local_id: None,
        namespace: Some(NamespaceId::from_raw(7)),
        stack_rank: 0,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 160,
            height: 90,
        },
        source: BufferSource::CpuBuffer { handle: 900 },
        source_size: Size {
            width: 160,
            height: 90,
        },
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: 160,
            height: 90,
        }),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 1,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    }
}
