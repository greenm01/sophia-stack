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
            SyntheticXEvent::PropertyNotify { window } => ServerCommand::ManageProfile {
                window,
                profile: bridge.synthetic_manage_profile(window).ok_or_else(|| {
                    BridgeRuntimeError::new("synthetic property update has no manage profile")
                })?,
            },
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
    /// Root children in X11 bottom-to-top order.
    stacking: Vec<u32>,
    atoms_by_name: BTreeMap<Vec<u8>, u32>,
    atom_names: BTreeMap<u32, Vec<u8>>,
    next_atom: u32,
    input_focus: u32,
    pointer_x: i16,
    pointer_y: i16,
    pointer_mask: u16,
    pending_pointer_gesture: Option<PendingPointerGesture>,
    key_grabs: BTreeSet<(u8, u16)>,
}

#[derive(Clone, Copy)]
struct PendingPointerGesture {
    window: SyntheticXWindowId,
    button: u8,
    modifiers: u16,
    delta_x: i16,
    delta_y: i16,
}

impl XServerState {
    fn new(root: Rect) -> Self {
        let atoms_by_name = BTreeMap::from([
            (b"WM_NORMAL_HINTS".to_vec(), 40),
            (b"WM_SIZE_HINTS".to_vec(), 41),
            (b"WM_TRANSIENT_FOR".to_vec(), 42),
            (b"WINDOW".to_vec(), 43),
            (b"_NET_WM_WINDOW_TYPE".to_vec(), 44),
            (b"ATOM".to_vec(), 45),
            (b"_NET_WM_WINDOW_TYPE_NORMAL".to_vec(), 46),
            (b"_NET_WM_WINDOW_TYPE_DIALOG".to_vec(), 47),
            (b"_NET_WM_WINDOW_TYPE_UTILITY".to_vec(), 48),
            (b"_NET_WM_WINDOW_TYPE_POPUP_MENU".to_vec(), 49),
        ]);
        let atom_names = atoms_by_name
            .iter()
            .map(|(name, atom)| (*atom, name.clone()))
            .collect();
        Self {
            sequence: 0,
            root,
            windows: BTreeMap::new(),
            stacking: Vec::new(),
            atoms_by_name,
            atom_names,
            next_atom: FIRST_DYNAMIC_ATOM,
            input_focus: SYNTHETIC_ROOT_XID,
            pointer_x: 0,
            pointer_y: 0,
            pointer_mask: 0,
            pending_pointer_gesture: None,
            key_grabs: BTreeSet::new(),
        }
    }
}
