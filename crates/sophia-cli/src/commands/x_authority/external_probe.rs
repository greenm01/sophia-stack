fn print_external_probe_smoke_report(
    command_name: &str,
    report: &XAuthorityExternalProbeSmokeReport,
) {
    println!(
        "{} display={} outcome={} status={} stdout_bytes={} stderr_bytes={} requests={} opcode_count={} opcodes={} transactions={} runtime_committed={} runtime_surfaces={} cpu_buffers={} cpu_buffer_bytes={} nonzero_pixel_bytes={} ascii_marker_match={} first_error={}",
        command_name,
        report.display,
        report.outcome,
        report.status,
        report.stdout_bytes,
        report.stderr_bytes,
        report.requests,
        report.opcode_count,
        report.opcodes,
        report.transactions,
        report.runtime_committed,
        report.runtime_surfaces,
        report.cpu_buffers,
        report.cpu_buffer_bytes,
        report.nonzero_pixel_bytes,
        report.ascii_marker_match,
        report.first_error.as_deref().unwrap_or("none")
    );
}

struct ExternalProbeInvocation<'a> {
    label: &'a str,
    command: &'a std::path::Path,
    display_mode: ExternalProbeDisplayMode,
    command_args: &'a [&'a str],
    display: String,
    socket_path: std::path::PathBuf,
    namespace: NamespaceId,
    require_transactions: bool,
    pixel_proof: ExternalProbePixelProof,
    allow_proof_kill_without_transactions: bool,
    allow_client_failure_without_x_error: bool,
    render_device_provider: Option<Arc<dyn XServerFrontendRenderDeviceProvider>>,
}

fn run_x_authority_external_probe_smoke(
    invocation: ExternalProbeInvocation<'_>,
) -> Result<XAuthorityExternalProbeSmokeReport, Box<dyn std::error::Error>> {
    let ExternalProbeInvocation {
        label,
        command,
        display_mode,
        command_args,
        display,
        socket_path,
        namespace,
        require_transactions,
        pixel_proof,
        allow_proof_kill_without_transactions,
        allow_client_failure_without_x_error,
        render_device_provider,
    } = invocation;
    let server_path = socket_path.clone();
    let proof_timeout = if label == "kitty" {
        Duration::from_secs(20)
    } else {
        Duration::from_secs(8)
    };
    // One X request can produce an opcode, detail, transaction, and buffer
    // update. Keep the diagnostic channel large enough that a replacement
    // update cannot be dropped while a later patch is retained.
    let (sender, receiver) = sync_channel(4_096);
    let mut server_config = XServerFrontendConfig::new(&server_path, namespace)?;
    if label == "kitty" {
        server_config =
            server_config.with_output_topology(two_output_external_probe_topology())?;
    }
    if let Some(provider) = render_device_provider {
        server_config = server_config.with_render_device_provider(provider);
    }
    let server = std::thread::spawn(move || {
        run_x11_core_socket_server_once_config_traced_with_idle_timeout(
            server_config,
            proof_timeout,
            |trace| {
                let _ = sender.try_send(ExternalProbeObservation::Opcode(trace.major_opcode));
                if trace.request_stage != sophia_x_authority::X11ObservedRequestStage::Other {
                    let _ = sender.try_send(ExternalProbeObservation::Detail(
                        trace.request_stage.evidence_name().to_owned(),
                    ));
                }
                if trace.failure.is_some() {
                    let _ = sender.try_send(ExternalProbeObservation::Error(format!(
                        "parse_error:major={}:minor={}",
                        trace.major_opcode, trace.minor_opcode
                    )));
                }
                for output in &trace.result.outputs {
                    if let XClientOutput::Error(error) = output {
                        if error.code == sophia_x_authority::XErrorCode::BadWindow
                            && error.resource_id == 0
                            && error.minor_code == 0
                            && matches!(error.major_code, 3 | 14)
                        {
                            continue;
                        }
                        let _ = sender.try_send(ExternalProbeObservation::Error(format!(
                            "{:?}:major={}:resource={:#x}",
                            error.code, error.major_code, error.resource_id
                        )));
                    }
                }
                if let Some(response) = &trace.result.response
                    && !response.transactions.is_empty() {
                        let _ = sender.try_send(ExternalProbeObservation::Transactions(
                            response.transactions.clone(),
                        ));
                    }
                if let Some(buffer) = trace.cpu_buffer_update {
                    let _ =
                        sender.try_send(ExternalProbeObservation::CpuBufferUpdate(buffer.clone()));
                }
                Ok(())
            },
        )
    });
    wait_for_socket_path(&socket_path)?;

    let mut command = std::process::Command::new(command);
    let firefox_profile = (label == "firefox").then(|| {
        std::env::temp_dir().join(format!(
            "sophia-firefox-profile-{}-{}",
            std::process::id(),
            namespace.raw()
        ))
    });
    if let Some(profile) = firefox_profile.as_ref() {
        std::fs::create_dir(profile)?;
        command.arg("--profile").arg(profile);
    }
    match display_mode {
        ExternalProbeDisplayMode::Argument(display_arg) => {
            command.arg(display_arg).arg(&display);
        }
        ExternalProbeDisplayMode::Environment => {
            command
                .env("DISPLAY", &display)
                .env("GDK_BACKEND", "x11")
                .env("GTK_USE_PORTAL", "0")
                .env("MOZ_ENABLE_WAYLAND", "0")
                .env_remove("WAYLAND_DISPLAY");
            if label == "kitty" && std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
                // A missing address invokes dbus-launch, which opens the
                // smoke's single X connection before Kitty. A deliberately
                // unavailable address makes desktop integration fail quickly
                // while leaving the graphics proof deterministic.
                command.env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/dev/null");
            }
        }
    }
    command
        .args(command_args)
        .process_group(0)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn()?;

    let deadline = std::time::Instant::now() + proof_timeout;
    let mut transactions = Vec::new();
    let mut cpu_buffers = std::collections::BTreeMap::new();
    let mut cpu_buffer_updates = 0usize;
    let mut first_error = None;
    let mut opcodes = std::collections::BTreeSet::new();
    let mut details = std::collections::BTreeSet::new();
    let mut requests = 0usize;
    let minimum_transactions = if label == "xmobar" { 6 } else { 1 };

    while std::time::Instant::now() < deadline {
        while let Ok(observation) = receiver.try_recv() {
            match observation {
                ExternalProbeObservation::Opcode(opcode) => {
                    requests = requests.saturating_add(1);
                    opcodes.insert(opcode);
                }
                ExternalProbeObservation::Transactions(batch) => transactions.extend(batch),
                ExternalProbeObservation::CpuBufferUpdate(update) => {
                    cpu_buffer_updates = cpu_buffer_updates.saturating_add(1);
                    update.apply_to(&mut cpu_buffers)?;
                }
                ExternalProbeObservation::Detail(detail) => {
                    details.insert(detail);
                }
                ExternalProbeObservation::Error(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        let pixel_proof_ready = match pixel_proof {
            ExternalProbePixelProof::None => true,
            ExternalProbePixelProof::Nonzero if label == "xmobar" => {
                latest_transaction_cpu_buffer(&transactions, &cpu_buffers)
                    .is_some_and(|buffer| buffer.bytes.iter().any(|byte| *byte != 0))
            }
            ExternalProbePixelProof::Nonzero => cpu_buffers
                .values()
                .any(|buffer| buffer.bytes.iter().any(|byte| *byte != 0)),
            ExternalProbePixelProof::Ascii(marker) => {
                cpu_buffers_contain_fixed_text(&cpu_buffers, marker)
            }
        };
        if transactions.len() >= minimum_transactions && pixel_proof_ready {
            break;
        }
        if child.try_wait()?.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let mut proof_window_killed = false;
    let output = if child.try_wait()?.is_none() {
        if let Some(group) = rustix::process::Pid::from_raw(child.id() as i32) {
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::TERM);
            std::thread::sleep(Duration::from_millis(25));
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
        }
        proof_window_killed = true;
        child.wait_with_output()?
    } else {
        child.wait_with_output()?
    };
    let status = output.status.code().unwrap_or(-1);

    let _ = std::fs::remove_file(&socket_path);
    if let Some(profile) = firefox_profile.as_ref() {
        let _ = std::fs::remove_dir_all(profile);
    }
    if !allow_proof_kill_without_transactions || !proof_window_killed {
        server
            .join()
            .map_err(|_| format!("X authority {label} socket server thread panicked"))?
            .map_err(|error| format!("X authority {label} socket server failed: {error}"))?;
    }

    while let Ok(observation) = receiver.try_recv() {
        match observation {
            ExternalProbeObservation::Opcode(opcode) => {
                requests = requests.saturating_add(1);
                opcodes.insert(opcode);
            }
            ExternalProbeObservation::Transactions(batch) => transactions.extend(batch),
            ExternalProbeObservation::CpuBufferUpdate(update) => {
                cpu_buffer_updates = cpu_buffer_updates.saturating_add(1);
                update.apply_to(&mut cpu_buffers)?;
            }
            ExternalProbeObservation::Detail(detail) => {
                details.insert(detail);
            }
            ExternalProbeObservation::Error(error) => {
                first_error.get_or_insert(error);
            }
        }
    }

    let opcode_count = opcodes.len();
    let opcodes = opcodes
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let details = details.into_iter().collect::<Vec<_>>().join(",");

    if label == "kitty" {
        for required in [
            "GLX:QueryServerString",
            "GLX:GetFBConfigs",
            "GLX:CreateContext",
            "GLX:CreateWindow",
            "DRI3:PixmapFromBuffers",
            "PRESENT:Pixmap",
        ] {
            if !details.contains(required) {
                return Err(format!(
                    "kitty trace omitted required direct-GLX stage {required} for {display}: details={details}"
                )
                .into());
            }
        }
    }

    if let Some(error) = &first_error {
        return Err(format!(
            "{label} produced an X protocol error for {display}: status={status} requests={requests} opcode_count={opcode_count} opcodes={opcodes} details={details} first_error={error} stderr={}",
            String::from_utf8_lossy(&output.stderr).trim(),
        )
        .into());
    }

    if require_transactions && transactions.is_empty() {
        return Err(format!(
            "{label} did not produce an authority transaction for {display}: status={status} requests={requests} opcode_count={opcode_count} opcodes={opcodes} details={details} stderr={} first_error={}",
            String::from_utf8_lossy(&output.stderr).trim(),
            first_error.as_deref().unwrap_or("none")
        )
        .into());
    }

    if !require_transactions
        && !output.status.success()
        && !(allow_proof_kill_without_transactions && proof_window_killed)
        && !(allow_client_failure_without_x_error && requests > 0)
    {
        return Err(format!(
            "{label} probe failed for {display}: status={status} requests={requests} opcode_count={opcode_count} opcodes={opcodes} details={details} stderr={} first_error={}",
            String::from_utf8_lossy(&output.stderr).trim(),
            first_error.as_deref().unwrap_or("none")
        )
        .into());
    }

    let runtime_state = if transactions.is_empty() {
        None
    } else {
        Some(runtime_state_from_observed_transactions(&transactions)?)
    };
    let runtime_committed = runtime_state
        .as_ref()
        .map(|state| state.authority_transactions_committed)
        .unwrap_or(0);
    let runtime_surfaces = runtime_state
        .as_ref()
        .map(|state| state.authority_surfaces_applied)
        .unwrap_or(0);
    let cpu_buffer_bytes = cpu_buffers.values().map(|buffer| buffer.bytes.len()).sum();
    let nonzero_pixel_bytes = cpu_buffers
        .values()
        .flat_map(|buffer| buffer.bytes.iter())
        .filter(|byte| **byte != 0)
        .count();
    let ascii_marker_match = cpu_buffers_contain_fixed_text(&cpu_buffers, b"Sophia");
    let pixel_proof_passed = match pixel_proof {
        ExternalProbePixelProof::None => true,
        ExternalProbePixelProof::Nonzero if label == "xmobar" => {
            latest_transaction_cpu_buffer(&transactions, &cpu_buffers)
                .is_some_and(|buffer| buffer.bytes.iter().any(|byte| *byte != 0))
        }
        ExternalProbePixelProof::Nonzero => nonzero_pixel_bytes != 0,
        ExternalProbePixelProof::Ascii(marker) => {
            cpu_buffers_contain_fixed_text(&cpu_buffers, marker)
        }
    };
    if !pixel_proof_passed {
        return Err(format!(
            "{label} did not satisfy its pixel proof for {display}: requests={requests} opcodes={opcodes} details={details}"
        )
        .into());
    }
    if require_transactions
        && (runtime_committed < u64::try_from(minimum_transactions).unwrap_or(u64::MAX)
            || runtime_surfaces == 0)
    {
        return Err(format!(
            "{label} transactions did not commit through runtime for {display}: transactions={} minimum={} committed={} surfaces={}",
            transactions.len(),
            minimum_transactions,
            runtime_committed,
            runtime_surfaces
        )
        .into());
    }

    let outcome = if proof_window_killed {
        "proof_window_killed"
    } else if output.status.success() {
        "client_exited_success"
    } else {
        "client_exited_failure"
    };

    Ok(XAuthorityExternalProbeSmokeReport {
        display,
        outcome: outcome.to_owned(),
        status,
        stdout_bytes: output.stdout.len(),
        stderr_bytes: output.stderr.len(),
        requests,
        opcode_count,
        opcodes,
        transactions: transactions.len(),
        runtime_committed,
        runtime_surfaces,
        cpu_buffers: cpu_buffer_updates,
        cpu_buffer_bytes,
        nonzero_pixel_bytes,
        ascii_marker_match,
        first_error,
        observed_transactions: transactions,
        observed_cpu_buffers: cpu_buffers.into_values().collect(),
    })
}

fn latest_transaction_cpu_buffer<'a>(
    transactions: &[SurfaceTransaction],
    buffers: &'a std::collections::BTreeMap<u64, XAuthorityCpuBufferSnapshot>,
) -> Option<&'a XAuthorityCpuBufferSnapshot> {
    let handle = transactions
        .last()
        .and_then(|transaction| match transaction.target_buffer {
            BufferSource::CpuBuffer { handle } => Some(handle),
            _ => None,
        })?;
    buffers.get(&handle)
}

fn two_output_external_probe_topology() -> sophia_protocol::OutputTopologySnapshot {
    sophia_protocol::OutputTopologySnapshot {
        generation: 1,
        primary: sophia_protocol::OutputId::from_raw(1),
        outputs: vec![
            sophia_protocol::OutputTopologyEntry {
                output: sophia_protocol::OutputId::from_raw(1),
                logical: Rect {
                    x: 0,
                    y: 0,
                    width: 1280,
                    height: 720,
                },
                pixel_size: Size {
                    width: 1280,
                    height: 720,
                },
                scale: 1,
                refresh_millihz: 60_000,
            },
            sophia_protocol::OutputTopologyEntry {
                output: sophia_protocol::OutputId::from_raw(2),
                logical: Rect {
                    x: 1280,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                pixel_size: Size {
                    width: 1920,
                    height: 1080,
                },
                scale: 1,
                refresh_millihz: 60_000,
            },
        ],
    }
}

fn cpu_buffers_contain_fixed_text(
    buffers: &std::collections::BTreeMap<u64, XAuthorityCpuBufferSnapshot>,
    text: &[u8],
) -> bool {
    buffers.values().any(|buffer| {
        let Ok(width) = usize::try_from(buffer.size.width) else {
            return false;
        };
        let Ok(height) = usize::try_from(buffer.size.height) else {
            return false;
        };
        let Some(text_width) = text.len().checked_mul(8) else {
            return false;
        };
        if width < text_width || height < 12 {
            return false;
        }
        (0..=height - 12).any(|top| {
            (0..=width - text_width).any(|left| fixed_text_matches_at(buffer, left, top, text))
        })
    })
}

fn fixed_text_matches_at(
    buffer: &XAuthorityCpuBufferSnapshot,
    left: usize,
    top: usize,
    text: &[u8],
) -> bool {
    let Some(background) = xrgb_pixel(buffer, left, top) else {
        return false;
    };
    let first_rows = x_fixed_glyph_rows(text[0]);
    let Some((first_row, first_column)) = first_rows.iter().enumerate().find_map(|(row, bits)| {
        (0..5)
            .find(|column| bits & (1 << (4 - column)) != 0)
            .map(|column| (row, column))
    }) else {
        return false;
    };
    let Some(foreground) = xrgb_pixel(
        buffer,
        left.saturating_add(first_column + 1),
        top.saturating_add(first_row + 2),
    ) else {
        return false;
    };
    if foreground == background {
        return false;
    }
    for (index, byte) in text.iter().copied().enumerate() {
        let rows = x_fixed_glyph_rows(byte);
        let cell_left = left.saturating_add(index.saturating_mul(8));
        for (row, bits) in rows.into_iter().enumerate() {
            for column in 0..5 {
                let expected = if bits & (1 << (4 - column)) != 0 {
                    foreground
                } else {
                    background
                };
                if xrgb_pixel(
                    buffer,
                    cell_left.saturating_add(column + 1),
                    top.saturating_add(row + 2),
                ) != Some(expected)
                {
                    return false;
                }
            }
        }
    }
    true
}

fn xrgb_pixel(buffer: &XAuthorityCpuBufferSnapshot, x: usize, y: usize) -> Option<u32> {
    let stride = usize::try_from(buffer.stride).ok()?;
    let offset = y.checked_mul(stride)?.checked_add(x.checked_mul(4)?)?;
    Some(u32::from_le_bytes(
        buffer
            .bytes
            .get(offset..offset.checked_add(4)?)?
            .try_into()
            .ok()?,
    ))
}

#[derive(Clone, Debug)]
enum ExternalProbeObservation {
    Opcode(u8),
    Transactions(Vec<SurfaceTransaction>),
    CpuBufferUpdate(XAuthorityCpuBufferUpdate),
    Detail(String),
    Error(String),
}
