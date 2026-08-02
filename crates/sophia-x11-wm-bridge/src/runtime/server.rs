pub fn run_wm_socket_server(
    path: impl AsRef<Path>,
    wm: LegacyWmLaunchSpec,
) -> Result<(), BridgeRuntimeError> {
    let path = path.as_ref();
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(BridgeRuntimeError::new(format!(
                "failed to remove stale WM socket {}: {error}",
                path.display()
            )));
        }
    }
    let profile = wm.profile;
    let mut runtime: Option<LegacyX11WmBridgeRuntime> = None;
    let listener = UnixListener::bind(path).map_err(|error| {
        BridgeRuntimeError::new(format!(
            "failed to bind WM socket {}: {error}",
            path.display()
        ))
    })?;
    for stream in listener.incoming() {
        let mut stream = stream.map_err(|error| {
            BridgeRuntimeError::new(format!("failed to accept WM socket client: {error}"))
        })?;
        let hello = encode_wm_hello_frame(&profile.hello()).map_err(|error| {
            BridgeRuntimeError::new(format!("failed to encode WM hello: {error:?}"))
        })?;
        stream
            .write_all(&hello)
            .and_then(|()| stream.flush())
            .map_err(|error| {
                BridgeRuntimeError::new(format!("failed to write WM hello: {error}"))
            })?;
        let descriptor = read_wm_session_descriptor(&mut stream)?;

        while let Some(request) = read_wm_request(&mut stream)? {
            if runtime.is_none() {
                let initial_root = match &request.kind {
                    WmRequestKind::ManageSurface(manage) => Some(manage.bounds),
                    WmRequestKind::RelayoutWorkspace(relayout) => Some(relayout.bounds),
                    _ => None,
                };
                if let Some(initial_root) = initial_root {
                    let mut started =
                        LegacyX11WmBridgeRuntime::start_with_root(wm.clone(), initial_root)?;
                    started.configure_session(descriptor.clone())?;
                    runtime = Some(started);
                }
            }
            let response = if let Some(runtime) = runtime.as_mut() {
                runtime.handle_request(&request)?
            } else if profile == LegacyWmProfile::Xmonad {
                translate_xmonad_profile_action(&request, &descriptor)?.unwrap_or(
                    WmResponsePacket {
                        transaction: request.transaction,
                        commands: Vec::new(),
                        timeout_msec: 0,
                    },
                )
            } else {
                WmResponsePacket {
                    transaction: request.transaction,
                    commands: Vec::new(),
                    timeout_msec: 0,
                }
            };
            let frame = encode_wm_response_frame(&response).map_err(|error| {
                BridgeRuntimeError::new(format!("failed to encode WM response: {error:?}"))
            })?;
            stream.write_all(&frame).map_err(|error| {
                BridgeRuntimeError::new(format!("failed to write WM response: {error}"))
            })?;
            stream.flush().map_err(|error| {
                BridgeRuntimeError::new(format!("failed to flush WM response: {error}"))
            })?;
        }
    }
    Ok(())
}

fn read_wm_request(stream: &mut UnixStream) -> Result<Option<WmRequestPacket>, BridgeRuntimeError> {
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    match stream.read_exact(&mut header) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => {
            return Err(BridgeRuntimeError::new(format!(
                "failed to read WM request header: {error}"
            )));
        }
    }
    let payload_len = u32::from_le_bytes(header[16..20].try_into().expect("fixed header")) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(BridgeRuntimeError::new(format!(
            "WM request payload too large: {payload_len}"
        )));
    }
    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    stream
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .map_err(|error| BridgeRuntimeError::new(format!("failed to read WM payload: {error}")))?;
    decode_wm_request_frame(&frame)
        .map(Some)
        .map_err(|error| BridgeRuntimeError::new(format!("failed to decode WM request: {error:?}")))
}

struct PrivateDisplayBinding {
    listener: UnixListener,
    display: u16,
    socket_path: PathBuf,
    lease: File,
}

fn bind_private_display() -> Result<PrivateDisplayBinding, BridgeRuntimeError> {
    fs::create_dir_all("/tmp/.X11-unix").map_err(|error| {
        BridgeRuntimeError::new(format!("failed to create /tmp/.X11-unix: {error}"))
    })?;
    for display in FIRST_PRIVATE_X_DISPLAY..=LAST_PRIVATE_X_DISPLAY {
        let path = PathBuf::from(format!("/tmp/.X11-unix/X{display}"));
        let listener = match UnixListener::bind(&path) {
            Ok(listener) => listener,
            Err(error) if error.kind() == ErrorKind::AddrInUse => continue,
            Err(error) if error.kind() == ErrorKind::PermissionDenied => continue,
            Err(error) => {
                return Err(BridgeRuntimeError::new(format!(
                    "failed to bind private X socket {}: {error}",
                    path.display()
                )));
            }
        };
        let lease_path = PathBuf::from(format!("/tmp/.X11-unix/.sophia-X{display}.lease"));
        let lease = match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .open(&lease_path)
        {
            Ok(lease) => lease,
            Err(error) if error.kind() == ErrorKind::PermissionDenied => {
                drop(listener);
                let _ = fs::remove_file(&path);
                continue;
            }
            Err(error) => {
                drop(listener);
                let _ = fs::remove_file(&path);
                return Err(BridgeRuntimeError::new(format!(
                    "failed to open private X display lease {}: {error}",
                    lease_path.display()
                )));
            }
        };
        match lease.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => {
                drop(listener);
                let _ = fs::remove_file(&path);
                continue;
            }
            Err(fs::TryLockError::Error(error)) => {
                drop(listener);
                let _ = fs::remove_file(&path);
                return Err(BridgeRuntimeError::new(format!(
                    "failed to acquire private X display lease {}: {error}",
                    lease_path.display()
                )));
            }
        }
        if let Err(error) = fs::set_permissions(&path, fs::Permissions::from_mode(0o600)) {
            drop(listener);
            let _ = fs::remove_file(&path);
            return Err(BridgeRuntimeError::new(format!(
                    "failed to secure private X socket {}: {error}",
                    path.display()
                )));
        }
        return Ok(PrivateDisplayBinding {
            listener,
            display,
            socket_path: path,
            lease,
        });
    }
    Err(BridgeRuntimeError::new(format!(
        "no private X display available in bounded range {FIRST_PRIVATE_X_DISPLAY}..={LAST_PRIVATE_X_DISPLAY}"
    )))
}

fn send_engine_update(
    bridge: &X11WmBridgeState,
    update: &BridgeEngineUpdate,
    commands: &SyncSender<ServerCommand>,
) -> Result<LegacyResponseExpectation, BridgeRuntimeError> {
    let mut expected = LegacyResponseExpectation::default();
    let last_configure = update
        .events
        .iter()
        .rposition(|event| matches!(event, SyntheticXEvent::ConfigureNotify { .. }));
    for (index, event) in update.events.iter().enumerate() {
        let command = match *event {
            SyntheticXEvent::RootConfigured { bounds } => ServerCommand::Root(bounds),
            SyntheticXEvent::MapRequest { window } => {
                expected.configured.insert(window);
                expected.map_admissions.insert(window);
                ServerCommand::Map(
                    window,
                    bridge
                        .synthetic_geometry(window)
                        .ok_or_else(|| BridgeRuntimeError::new("synthetic map has no geometry"))?,
                    bridge.synthetic_manage_profile(window).ok_or_else(|| {
                        BridgeRuntimeError::new("synthetic map has no manage profile")
                    })?,
                )
            }
            SyntheticXEvent::ConfigureNotify { window, geometry } => {
                expected.configured.insert(window);
                ServerCommand::Configure {
                    window,
                    geometry,
                    notify_root: Some(index) == last_configure,
                }
            }
            SyntheticXEvent::UnmapNotify { window } => ServerCommand::Unmap(window),
            SyntheticXEvent::DestroyNotify { window } => ServerCommand::Destroy(window),
        };
        commands
            .send(command)
            .map_err(|_| BridgeRuntimeError::new("legacy WM command channel disconnected"))?;
    }
    Ok(expected)
}

#[derive(Clone, Copy)]
struct WindowState {
    geometry: Rect,
    mapped: bool,
    manage_profile: SyntheticManageProfile,
}

struct XServerState {
    sequence: u16,
    root: Rect,
    windows: BTreeMap<u32, WindowState>,
    atoms_by_name: BTreeMap<Vec<u8>, u32>,
    atom_names: BTreeMap<u32, Vec<u8>>,
    next_atom: u32,
    input_focus: u32,
    key_grabs: BTreeSet<(u8, u16)>,
}

impl XServerState {
    fn new(root: Rect) -> Self {
        let atoms_by_name = BTreeMap::from([
            (b"WM_NORMAL_HINTS".to_vec(), 40),
            (b"WM_SIZE_HINTS".to_vec(), 41),
        ]);
        let atom_names = atoms_by_name
            .iter()
            .map(|(name, atom)| (*atom, name.clone()))
            .collect();
        Self {
            sequence: 0,
            root,
            windows: BTreeMap::new(),
            atoms_by_name,
            atom_names,
            next_atom: FIRST_DYNAMIC_ATOM,
            input_focus: SYNTHETIC_ROOT_XID,
            key_grabs: BTreeSet::new(),
        }
    }
}
