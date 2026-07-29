fn run_x_authority_x11_smoke() -> Result<XAuthorityX11SmokeReport, Box<dyn std::error::Error>> {
    use std::io::Write;

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x-authority-x11-{}-{}.sock",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let server_path = socket_path.clone();
    let server = std::thread::spawn(move || {
        run_x11_core_socket_server_once(&server_path, NamespaceId::from_raw(41))
    });

    wait_for_socket_path(&socket_path)?;
    let mut stream = UnixStream::connect(&socket_path)?;
    stream.write_all(&x11_setup_request(XByteOrder::LittleEndian))?;
    read_x11_setup_success(&mut stream, XByteOrder::LittleEndian)?;

    stream.write_all(&x11_intern_atom_request(
        XByteOrder::LittleEndian,
        false,
        "_NET_WM_NAME",
    ))?;
    let net_wm_name = read_x11_record(&mut stream)?;
    let net_wm_name = read_x11_u32(XByteOrder::LittleEndian, &net_wm_name[8..12]);

    stream.write_all(&x11_intern_atom_request(
        XByteOrder::LittleEndian,
        false,
        "UTF8_STRING",
    ))?;
    let utf8 = read_x11_record(&mut stream)?;
    let utf8 = read_x11_u32(XByteOrder::LittleEndian, &utf8[8..12]);

    stream.write_all(&x11_create_window_request(
        XByteOrder::LittleEndian,
        0x0020_0001,
        20,
        30,
        640,
        480,
    ))?;
    let configure = read_x11_record(&mut stream)?;

    stream.write_all(&x11_resource_request(
        XByteOrder::LittleEndian,
        8,
        0x0020_0001,
    ))?;
    let map = read_x11_record(&mut stream)?;

    stream.write_all(&x11_change_property_request(
        XByteOrder::LittleEndian,
        0x0020_0001,
        net_wm_name,
        utf8,
        b"Sophia Socket",
    ))?;
    let property_notify = read_x11_record(&mut stream)?;

    stream.write_all(&x11_get_property_request(
        XByteOrder::LittleEndian,
        0x0020_0001,
        net_wm_name,
        0,
        0,
        64,
    ))?;
    let property = read_x11_reply(&mut stream, XByteOrder::LittleEndian)?;

    let records = [configure, map, property_notify];
    let configure_notify = records.iter().filter(|record| record[0] == 22).count();
    let map_notify = records.iter().filter(|record| record[0] == 19).count();
    let errors = records.iter().filter(|record| record[0] == 0).count();

    drop(stream);
    let _ = std::fs::remove_file(&socket_path);
    server
        .join()
        .map_err(|_| "X authority X11 socket server thread panicked")??;

    Ok(XAuthorityX11SmokeReport {
        configure_notify,
        map_notify,
        property_bytes: usize::try_from(read_x11_u32(XByteOrder::LittleEndian, &property[16..20]))?,
        errors,
    })
}

fn run_x_authority_x11rb_smoke() -> Result<XAuthorityX11rbSmokeReport, Box<dyn std::error::Error>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{
        AtomEnum, ConnectionExt, CreateWindowAux, PropMode, WindowClass,
    };
    use x11rb::wrapper::ConnectionExt as _;

    let display_number = 600 + (std::process::id() % 1000);
    let display = format!(":{display_number}");
    let socket_path = std::path::PathBuf::from(format!("/tmp/.X11-unix/X{display_number}"));
    std::fs::create_dir_all("/tmp/.X11-unix")?;
    let server_path = socket_path.clone();
    let server = std::thread::spawn(move || {
        run_x11_core_socket_server_once(&server_path, NamespaceId::from_raw(42))
    });

    wait_for_socket_path(&socket_path)?;
    let (connection, screen_index) = x11rb::connect(Some(&display))?;
    let screen = &connection.setup().roots[screen_index];
    let net_wm_name = connection
        .intern_atom(false, b"_NET_WM_NAME")?
        .reply()?
        .atom;
    let utf8 = connection.intern_atom(false, b"UTF8_STRING")?.reply()?.atom;
    let window = connection.generate_id()?;
    connection.create_window(
        screen.root_depth,
        window,
        screen.root,
        20,
        30,
        320,
        200,
        0,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &CreateWindowAux::new(),
    )?;
    let title = b"Sophia x11rb";
    connection.change_property8(PropMode::REPLACE, window, net_wm_name, utf8, title)?;
    let property = connection
        .get_property(false, window, net_wm_name, AtomEnum::ANY, 0, 64)?
        .reply()?;
    connection.map_window(window)?;
    connection.flush()?;

    let mut configure_notify = 0usize;
    let mut map_notify = 0usize;
    let mut errors = 0usize;
    for _ in 0..8 {
        match connection.poll_for_event()? {
            Some(Event::ConfigureNotify(_)) => configure_notify += 1,
            Some(Event::MapNotify(_)) => map_notify += 1,
            Some(Event::Error(_)) => errors += 1,
            Some(_) => {}
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }

    drop(connection);
    let _ = std::fs::remove_file(&socket_path);
    server
        .join()
        .map_err(|_| "X authority X11 socket server thread panicked")??;

    Ok(XAuthorityX11rbSmokeReport {
        display,
        window,
        title_bytes: property.value.len(),
        configure_notify,
        map_notify,
        errors,
    })
}

fn run_x_authority_xdpyinfo_smoke()
-> Result<XAuthorityXdpyinfoSmokeReport, Box<dyn std::error::Error>> {
    let (display, socket_path) = temp_xauthority_display(1600)?;
    let server_path = socket_path.clone();
    let server = std::thread::spawn(move || {
        run_x11_core_socket_server_once(&server_path, NamespaceId::from_raw(43))
    });

    wait_for_socket_path(&socket_path)?;
    let output = std::process::Command::new("xdpyinfo")
        .arg("-display")
        .arg(&display)
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let status = output.status.code().unwrap_or(-1);
    let report = XAuthorityXdpyinfoSmokeReport {
        display: display.clone(),
        status,
        stdout_bytes: output.stdout.len(),
        stderr_bytes: output.stderr.len(),
        mentions_sophia: stdout.contains("Sophia") || stderr.contains("Sophia"),
        mentions_root: stdout.contains("root window id") || stderr.contains("root window id"),
    };

    let _ = std::fs::remove_file(&socket_path);
    server
        .join()
        .map_err(|_| "X authority X11 socket server thread panicked")??;

    if !output.status.success() {
        return Err(format!(
            "xdpyinfo failed for {display}: status={status} stderr={}",
            stderr.trim()
        )
        .into());
    }

    Ok(report)
}

fn run_x_authority_xlib_smoke() -> Result<XAuthorityXlibSmokeReport, Box<dyn std::error::Error>> {
    let (display, socket_path) = temp_xauthority_display(2600)?;
    let server_path = socket_path.clone();
    let server = std::thread::spawn(move || {
        run_x11_core_socket_server_once(&server_path, NamespaceId::from_raw(44))
    });
    wait_for_socket_path(&socket_path)?;
    let output = run_compiled_xlib_probe(&display, "xlib", XLIB_SMOKE_SOURCE)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let status = output.status.code().unwrap_or(-1);
    let title_bytes = xlib_smoke_title_bytes(&stdout).unwrap_or(0);
    let title_match = stdout.contains("title_match=1");

    let _ = std::fs::remove_file(&socket_path);
    server
        .join()
        .map_err(|_| "X authority X11 socket server thread panicked")??;

    if !output.status.success() {
        return Err(format!(
            "Xlib smoke failed for {display}: status={status} stdout={} stderr={}",
            stdout.trim(),
            stderr.trim()
        )
        .into());
    }

    Ok(XAuthorityXlibSmokeReport {
        display,
        status,
        stdout_bytes: output.stdout.len(),
        stderr_bytes: output.stderr.len(),
        title_bytes,
        title_match,
    })
}

fn run_x_authority_xlib_drawing_smoke()
-> Result<XAuthorityXlibDrawingSmokeReport, Box<dyn std::error::Error>> {
    let (display, socket_path) = temp_xauthority_display(3600)?;
    let server_path = socket_path.clone();
    let server = std::thread::spawn(move || -> Result<Vec<SurfaceTransaction>, String> {
        let mut transactions = Vec::new();
        run_x11_core_socket_server_once_observed(
            &server_path,
            NamespaceId::from_raw(45),
            |result| {
                if let Some(response) = &result.response {
                    transactions.extend(response.transactions.iter().cloned());
                }
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(transactions)
    });
    wait_for_socket_path(&socket_path)?;
    let output = run_compiled_xlib_probe(&display, "xlib-drawing", XLIB_DRAWING_SMOKE_SOURCE)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let status = output.status.code().unwrap_or(-1);
    let draw_ops = xlib_smoke_field(&stdout, "draw_ops").unwrap_or(0);

    let _ = std::fs::remove_file(&socket_path);
    let transactions = server
        .join()
        .map_err(|_| "X authority X11 socket server thread panicked")?
        .map_err(|error| format!("X authority X11 socket server failed: {error}"))?;
    let runtime_state = runtime_state_from_observed_transactions(&transactions)?;

    if !output.status.success() {
        return Err(format!(
            "Xlib drawing smoke failed for {display}: status={status} stdout={} stderr={}",
            stdout.trim(),
            stderr.trim()
        )
        .into());
    }

    Ok(XAuthorityXlibDrawingSmokeReport {
        display,
        status,
        stdout_bytes: output.stdout.len(),
        stderr_bytes: output.stderr.len(),
        draw_ops,
        transactions: transactions.len(),
        runtime_committed: runtime_state.authority_transactions_committed,
        runtime_surfaces: runtime_state.authority_surfaces_applied,
    })
}

fn run_x_authority_xlib_put_image_smoke()
-> Result<XAuthorityXlibPutImageSmokeReport, Box<dyn std::error::Error>> {
    let (display, socket_path) = temp_xauthority_display(4600)?;
    let server_path = socket_path.clone();
    let server = std::thread::spawn(move || -> Result<Vec<SurfaceTransaction>, String> {
        let mut transactions = Vec::new();
        run_x11_core_socket_server_once_observed(
            &server_path,
            NamespaceId::from_raw(46),
            |result| {
                if let Some(response) = &result.response {
                    transactions.extend(response.transactions.iter().cloned());
                }
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(transactions)
    });
    wait_for_socket_path(&socket_path)?;
    let output = run_compiled_xlib_probe(&display, "xlib-put-image", XLIB_PUT_IMAGE_SMOKE_SOURCE)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let status = output.status.code().unwrap_or(-1);
    let image_ops = xlib_smoke_field(&stdout, "image_ops").unwrap_or(0);

    let _ = std::fs::remove_file(&socket_path);
    let transactions = server
        .join()
        .map_err(|_| "X authority X11 socket server thread panicked")?
        .map_err(|error| format!("X authority X11 socket server failed: {error}"))?;
    let runtime_state = runtime_state_from_observed_transactions(&transactions)?;

    if !output.status.success() {
        return Err(format!(
            "Xlib PutImage smoke failed for {display}: status={status} stdout={} stderr={}",
            stdout.trim(),
            stderr.trim()
        )
        .into());
    }

    Ok(XAuthorityXlibPutImageSmokeReport {
        display,
        status,
        stdout_bytes: output.stdout.len(),
        stderr_bytes: output.stderr.len(),
        image_ops,
        transactions: transactions.len(),
        runtime_committed: runtime_state.authority_transactions_committed,
        runtime_surfaces: runtime_state.authority_surfaces_applied,
    })
}

fn run_x_authority_external_probe_smoke_spec(
    spec: &ExternalProbeSmokeSpec,
) -> Result<XAuthorityExternalProbeSmokeReport, Box<dyn std::error::Error>> {
    let command = resolve_external_probe_binary(spec.label, spec.binary)?;
    let (display, socket_path) = temp_xauthority_display(spec.display_base)?;
    run_x_authority_external_probe_smoke(ExternalProbeInvocation {
        label: spec.label,
        command: &command,
        display_mode: spec.display_mode,
        command_args: spec.args,
        display,
        socket_path,
        namespace: NamespaceId::from_raw(spec.namespace),
        require_transactions: spec.require_transactions,
        pixel_proof: spec.pixel_proof,
        allow_proof_kill_without_transactions: spec.allow_proof_kill_without_transactions,
        allow_client_failure_without_x_error: spec.allow_client_failure_without_x_error,
        render_device_provider: None,
    })
}

fn run_x_authority_xmobar_smoke()
-> Result<XAuthorityExternalProbeSmokeReport, Box<dyn std::error::Error>> {
    let command = match std::env::var_os("SOPHIA_XMOBAR_BIN") {
        Some(path) => std::path::PathBuf::from(path),
        None => resolve_external_probe_binary("xmobar", "xmobar")?,
    };
    if !command.is_file() {
        return Err(format!("xmobar smoke executable does not exist: {}", command.display()).into());
    }
    let config = std::env::var_os("SOPHIA_XMOBAR_CONFIG")
        .map(std::path::PathBuf::from)
        .unwrap_or(std::env::current_dir()?.join("tools/fixtures/xmobar_sophia.config"));
    if !config.is_file() {
        return Err(format!("xmobar smoke config does not exist: {}", config.display()).into());
    }
    let config = config
        .to_str()
        .ok_or("xmobar smoke config path is not valid UTF-8")?;
    let (display, socket_path) = temp_xauthority_display(7_850)?;
    run_x_authority_external_probe_smoke(ExternalProbeInvocation {
        label: "xmobar",
        command: &command,
        display_mode: ExternalProbeDisplayMode::Environment,
        command_args: &[config],
        display,
        socket_path,
        namespace: NamespaceId::from_raw(62),
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::Nonzero,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
        render_device_provider: None,
    })
}

struct ExternalProbeRenderDeviceProvider {
    device: std::fs::File,
}

impl XServerFrontendRenderDeviceProvider for ExternalProbeRenderDeviceProvider {
    fn open_render_device_fd(
        &self,
    ) -> Result<std::os::fd::OwnedFd, XServerFrontendRenderDeviceError> {
        use std::os::fd::AsRawFd as _;

        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/self/fd/{}", self.device.as_raw_fd()))
            .map(std::os::fd::OwnedFd::from)
            .map_err(|_| XServerFrontendRenderDeviceError::OpenFailed)
    }
}

fn run_x_authority_zenity_render_smoke()
-> Result<XAuthorityExternalProbeSmokeReport, Box<dyn std::error::Error>> {
    let command = resolve_external_probe_binary("zenity_render", "zenity")?;
    let provider = Arc::new(ExternalProbeRenderDeviceProvider {
        device: first_openable_render_node()?,
    });
    let (display, socket_path) = temp_xauthority_display(7760)?;
    run_x_authority_external_probe_smoke(ExternalProbeInvocation {
        label: "zenity_render",
        command: &command,
        display_mode: ExternalProbeDisplayMode::Environment,
        command_args: &[
            "--entry",
            "--title",
            "Sophia zenity render",
            "--text",
            "Sophia GTK render-provider probe",
        ],
        display,
        socket_path,
        namespace: NamespaceId::from_raw(60),
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::Nonzero,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
        render_device_provider: Some(provider),
    })
}

fn run_x_authority_vkcube_smoke()
-> Result<XAuthorityExternalProbeSmokeReport, Box<dyn std::error::Error>> {
    let command = resolve_external_probe_binary("vkcube", "vkcube")?;
    let render_node = first_openable_render_node()?;
    let provider = Arc::new(ExternalProbeRenderDeviceProvider {
        device: render_node,
    });
    let (display, socket_path) = temp_xauthority_display(6680)?;
    run_x_authority_external_probe_smoke(ExternalProbeInvocation {
        label: "vkcube",
        command: &command,
        display_mode: ExternalProbeDisplayMode::Environment,
        command_args: &["--wsi", "xcb", "--c", "2", "--suppress_popups"],
        display,
        socket_path,
        namespace: NamespaceId::from_raw(58),
        require_transactions: false,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: true,
        allow_client_failure_without_x_error: false,
        render_device_provider: Some(provider),
    })
}

fn run_x_authority_glxgears_smoke()
-> Result<XAuthorityExternalProbeSmokeReport, Box<dyn std::error::Error>> {
    let command = resolve_external_probe_binary("glxgears", "glxgears")?;
    let provider = Arc::new(ExternalProbeRenderDeviceProvider {
        device: first_openable_render_node()?,
    });
    let (display, socket_path) = temp_xauthority_display(6685)?;
    run_x_authority_external_probe_smoke(ExternalProbeInvocation {
        label: "glxgears",
        command: &command,
        display_mode: ExternalProbeDisplayMode::Environment,
        command_args: &[
            "-info",
            "-swapinterval",
            "1",
            "-geometry",
            "500x500",
        ],
        display,
        socket_path,
        namespace: NamespaceId::from_raw(59),
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
        render_device_provider: Some(provider),
    })
}

fn run_x_authority_kitty_smoke()
-> Result<XAuthorityExternalProbeSmokeReport, Box<dyn std::error::Error>> {
    let command = resolve_external_probe_binary("kitty", "kitty")?;
    let provider = Arc::new(ExternalProbeRenderDeviceProvider {
        device: first_openable_render_node()?,
    });
    let (display, socket_path) = temp_xauthority_display(6690)?;
    run_x_authority_external_probe_smoke(ExternalProbeInvocation {
        label: "kitty",
        command: &command,
        display_mode: ExternalProbeDisplayMode::Environment,
        command_args: &[
            "--config",
            "NONE",
            "--override",
            "close_on_child_death=yes",
            "--title",
            "Sophia Kitty GLX proof",
            "sh",
            "-c",
            "printf 'Sophia Kitty proof\\n'; sleep 5",
        ],
        display,
        socket_path,
        namespace: NamespaceId::from_raw(62),
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
        render_device_provider: Some(provider),
    })
}
