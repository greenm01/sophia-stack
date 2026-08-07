fn serve_legacy_wm(
    stream: &mut UnixStream,
    commands: Receiver<ServerCommand>,
    legacy: SyncSender<LegacyWmRequest>,
    initial_root: Rect,
) -> Result<(), BridgeRuntimeError> {
    let setup = serve_x11_setup_socket_client_with_root_size(
        stream,
        sophia_protocol::Size {
            width: initial_root.width,
            height: initial_root.height,
        },
    )
    .map_err(|error| BridgeRuntimeError::new(format!("X11 setup failed: {error}")))?;
    if setup.byte_order != XByteOrder::LittleEndian {
        return Err(BridgeRuntimeError::new(
            "private legacy WM server currently requires little-endian X11",
        ));
    }
    stream.set_read_timeout(Some(IO_POLL)).map_err(|error| {
        BridgeRuntimeError::new(format!("failed to configure X11 socket timeout: {error}"))
    })?;
    let mut state = XServerState::new(initial_root);
    let mut pending_focus_queries = Vec::new();
    let mut last_socket_activity = Instant::now();
    loop {
        loop {
            match commands.try_recv() {
                Ok(ServerCommand::QueryFocus(reply)) => {
                    pending_focus_queries.push(reply);
                    last_socket_activity = Instant::now();
                }
                Ok(ServerCommand::ValidateKeyGrab { chord, reply }) => {
                    let _ = reply.send(has_matching_key_grab(&state, chord));
                    last_socket_activity = Instant::now();
                }
                Ok(command) => apply_server_command(stream, &mut state, command)?,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        let mut header = [0_u8; 4];
        match stream.read_exact(&mut header) {
            Ok(()) => {}
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                if last_socket_activity.elapsed() >= QUIET_PERIOD {
                    for reply in pending_focus_queries.drain(..) {
                        let _ = reply.send(mapped_synthetic_id(&state, state.input_focus));
                    }
                }
                continue;
            }
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => {
                return Err(BridgeRuntimeError::new(format!(
                    "failed to read legacy WM request header: {error}"
                )));
            }
        }
        let units = usize::from(u16::from_le_bytes([header[2], header[3]]));
        if units == 0 || units > 65_535 {
            return Err(BridgeRuntimeError::new(format!(
                "invalid legacy WM request length {units}"
            )));
        }
        let mut body = vec![0_u8; units * 4 - 4];
        stream.read_exact(&mut body).map_err(|error| {
            BridgeRuntimeError::new(format!("failed to read legacy WM request body: {error}"))
        })?;
        last_socket_activity = Instant::now();
        state.sequence = state.sequence.wrapping_add(1);
        if std::env::var_os("SOPHIA_X11_WM_TRACE").is_some() {
            tracing::trace!(
                "sophia_x11_wm_bridge schema=1 sequence={} opcode={}",
                state.sequence,
                header[0]
            );
        }
        dispatch_request(stream, &mut state, &legacy, header[0], header[1], &body)?;
    }
}

fn apply_server_command(
    stream: &mut UnixStream,
    state: &mut XServerState,
    command: ServerCommand,
) -> Result<(), BridgeRuntimeError> {
    match command {
        ServerCommand::Root(bounds) => {
            state.root = bounds;
            write_configure_notify(stream, state.sequence, SYNTHETIC_ROOT_XID, bounds)?;
        }
        ServerCommand::Map(window, geometry, manage_profile) => {
            if let Some(entry) = state.windows.get_mut(&window.raw()) {
                // Unmap preserves an X11 window and its stack position. A
                // later map updates the retained policy object in place.
                entry.geometry = geometry;
                entry.mapped = false;
                entry.manage_profile = manage_profile;
            } else {
                state.stacking.push(window.raw());
                state.windows.insert(
                    window.raw(),
                    WindowState {
                        geometry,
                        mapped: false,
                        manage_profile,
                    },
                );
            }
            let mut event = vec![20, 0];
            push_u16(&mut event, state.sequence);
            push_u32(&mut event, SYNTHETIC_ROOT_XID);
            push_u32(&mut event, window.raw());
            event.resize(32, 0);
            write_packet(stream, &event)?;
        }
        ServerCommand::Configure {
            window,
            geometry,
            notify_root,
        } => {
            if let Some(entry) = state.windows.get_mut(&window.raw()) {
                entry.geometry = geometry;
            }
            // A root ConfigureNotify is the bounded, metadata-free signal that
            // makes compatible WMs re-run their current layout for an existing
            // set. One notification covers the complete ordered geometry batch.
            if notify_root {
                write_configure_notify(stream, state.sequence, SYNTHETIC_ROOT_XID, state.root)?;
            }
        }
        ServerCommand::ManageProfile { window, profile } => {
            let entry = state.windows.get_mut(&window.raw()).ok_or_else(|| {
                BridgeRuntimeError::new("synthetic property update targeted an unknown window")
            })?;
            entry.manage_profile = profile;
            write_property_notify(
                stream,
                state.sequence,
                window.raw(),
                profile.constraints.is_none(),
            )?;
        }
        ServerCommand::Unmap(window) => {
            let entry = state.windows.get_mut(&window.raw()).ok_or_else(|| {
                BridgeRuntimeError::new("synthetic unmap targeted an unknown window")
            })?;
            // X11 unmapping changes viewability; it does not destroy the
            // child or remove it from QueryTree stacking order.
            entry.mapped = false;
            if state.input_focus == window.raw() {
                state.input_focus = SYNTHETIC_ROOT_XID;
            }
            write_window_event(stream, state.sequence, 18, window.raw())?;
        }
        ServerCommand::Destroy(window) => {
            if state.input_focus == window.raw() {
                state.input_focus = SYNTHETIC_ROOT_XID;
            }
            state.windows.remove(&window.raw());
            state.stacking.retain(|candidate| *candidate != window.raw());
            write_window_event(stream, state.sequence, 17, window.raw())?;
        }
        ServerCommand::Key { chord, pressed } => {
            if !has_matching_key_grab(state, chord) {
                return Err(BridgeRuntimeError::new(format!(
                    "profile key chord was not registered by the legacy WM: keycode={} modifiers=0x{:x}",
                    chord.keycode, chord.modifiers
                )));
            }
            let mut event = vec![if pressed { 2 } else { 3 }, chord.keycode];
            push_u16(&mut event, state.sequence);
            push_u32(&mut event, 0);
            push_u32(&mut event, SYNTHETIC_ROOT_XID);
            push_u32(&mut event, SYNTHETIC_ROOT_XID);
            push_u32(&mut event, 0);
            push_i16(&mut event, 0);
            push_i16(&mut event, 0);
            push_i16(&mut event, 0);
            push_i16(&mut event, 0);
            push_u16(&mut event, chord.modifiers);
            event.push(1);
            event.push(0);
            write_packet(stream, &event)?;
        }
        ServerCommand::Button {
            window,
            button,
            modifiers,
            root_x,
            root_y,
            pressed,
        } => {
            let geometry = state
                .windows
                .get(&window.raw())
                .map(|entry| entry.geometry)
                .ok_or_else(|| BridgeRuntimeError::new("focus click targeted an unknown window"))?;
            let root_x = root_x.unwrap_or_else(|| {
                i16::try_from(geometry.x.saturating_add(1))
                    .unwrap_or(if geometry.x < 0 { i16::MIN } else { i16::MAX })
            });
            let root_y = root_y.unwrap_or_else(|| {
                i16::try_from(geometry.y.saturating_add(1))
                    .unwrap_or(if geometry.y < 0 { i16::MIN } else { i16::MAX })
            });
            state.pointer_x = root_x;
            state.pointer_y = root_y;
            state.pointer_mask = if pressed { modifiers | (1 << 8) } else { modifiers };
            let mut event = vec![if pressed { 4 } else { 5 }, button];
            push_u16(&mut event, state.sequence);
            push_u32(&mut event, 0);
            push_u32(&mut event, SYNTHETIC_ROOT_XID);
            push_u32(&mut event, window.raw());
            push_u32(&mut event, 0);
            push_i16(&mut event, root_x);
            push_i16(&mut event, root_y);
            push_i16(&mut event, 1);
            push_i16(&mut event, 1);
            push_u16(&mut event, modifiers);
            event.push(1);
            event.push(0);
            write_packet(stream, &event)?;
        }
        ServerCommand::PointerGesture {
            window,
            button,
            modifiers,
            start_x,
            start_y,
            delta_x,
            delta_y,
        } => {
            state.pointer_x = start_x;
            state.pointer_y = start_y;
            state.pointer_mask = modifiers | (1 << (7 + button));
            state.pending_pointer_gesture = Some(PendingPointerGesture {
                window,
                button,
                modifiers,
                delta_x,
                delta_y,
            });
            write_synthetic_pointer_event(
                stream,
                state,
                window,
                button,
                modifiers,
                true,
                start_x,
                start_y,
            )?;
        }
        ServerCommand::Wake => {
            write_configure_notify(stream, state.sequence, SYNTHETIC_ROOT_XID, state.root)?;
        }
        ServerCommand::QueryFocus(_) => unreachable!("focus queries are socket-order barriers"),
        ServerCommand::ValidateKeyGrab { .. } => {
            unreachable!("key-grab validation is a socket-order barrier")
        }
    }
    Ok(())
}

fn write_property_notify(
    stream: &mut UnixStream,
    sequence: u16,
    window: u32,
    deleted: bool,
) -> Result<(), BridgeRuntimeError> {
    const WM_NORMAL_HINTS: u32 = 40;
    const WM_TRANSIENT_FOR: u32 = 42;
    for atom in [WM_NORMAL_HINTS, WM_TRANSIENT_FOR] {
        let mut event = vec![28, 0];
        push_u16(&mut event, sequence);
        push_u32(&mut event, window);
        push_u32(&mut event, atom);
        push_u32(&mut event, 0);
        event.push(u8::from(deleted));
        event.resize(32, 0);
        write_packet(stream, &event)?;
    }
    Ok(())
}

fn has_matching_key_grab(state: &XServerState, chord: SyntheticKeyChord) -> bool {
    state.key_grabs.iter().any(|(keycode, modifiers)| {
        (*keycode == X11_ANY_KEY || *keycode == chord.keycode)
            && (*modifiers == X11_ANY_MODIFIER || *modifiers == chord.modifiers)
    })
}

fn dispatch_request(
    stream: &mut UnixStream,
    state: &mut XServerState,
    legacy: &SyncSender<LegacyWmRequest>,
    opcode: u8,
    detail: u8,
    body: &[u8],
) -> Result<(), BridgeRuntimeError> {
    match opcode {
        2 => {}
        3 => reply_window_attributes(stream, state, read_u32(body, 0))?,
        8 => {
            let window = read_u32(body, 0);
            if let Some(entry) = state.windows.get_mut(&window) {
                entry.mapped = true;
            }
        }
        10 => {
            let window = read_u32(body, 0);
            if let Some(entry) = state.windows.get_mut(&window) {
                entry.mapped = false;
            }
        }
        12 => configure_window(state, legacy, body)?,
        14 => reply_geometry(stream, state, read_u32(body, 0))?,
        15 => reply_query_tree(stream, state)?,
        16 => reply_intern_atom(stream, state, detail != 0, body)?,
        17 => reply_atom_name(stream, state, read_u32(body, 0))?,
        18 | 19 => {}
        20 => reply_window_property(stream, state, body)?,
        21 => reply_list_properties(stream, state, read_u32(body, 0))?,
        22 => {}
        23 => reply_u32(stream, state.sequence, 0, 0)?,
        25 | 28 | 29 | 30 | 32 | 35 | 36 | 37 | 39 => {}
        33 => {
            if read_u32(body, 0) != SYNTHETIC_ROOT_XID {
                return Err(BridgeRuntimeError::new(
                    "private GrabKey targeted a non-root window",
                ));
            }
            let grab = (body[6], read_u16(body, 4));
            if !state.key_grabs.contains(&grab) && state.key_grabs.len() >= MAX_PRIVATE_KEY_GRABS {
                return Err(BridgeRuntimeError::new("private key grab limit reached"));
            }
            state.key_grabs.insert(grab);
        }
        34 => {
            let modifiers = read_u16(body, 4);
            if detail == 0 {
                state
                    .key_grabs
                    .retain(|(_, registered)| *registered != modifiers);
            } else {
                state.key_grabs.remove(&(detail, modifiers));
            }
        }
        26 => {
            reply_simple(stream, state.sequence, 0)?;
            complete_pending_pointer_gesture(stream, state)?;
        }
        31 => reply_simple(stream, state.sequence, 0)?,
        38 => reply_query_pointer(stream, state)?,
        40 => reply_translate_coordinates(stream, state)?,
        41 => apply_warp_pointer(state, body),
        42 => {
            let window = read_u32(body, 0);
            if matches!(window, 0 | 1 | SYNTHETIC_ROOT_XID) {
                state.input_focus = window;
            } else if let Some(window) = mapped_synthetic_id(state, window) {
                state.input_focus = window.raw();
                legacy
                    .send(LegacyWmRequest::FocusWindow { window })
                    .map_err(|_| BridgeRuntimeError::new("legacy request channel disconnected"))?;
            } else {
                // SetInputFocus requires a viewable target. Report BadMatch
                // without changing focus, as a real X server does.
                write_x11_error(stream, state.sequence, 8, window, 42)?;
            }
        }
        43 => reply_u32(stream, state.sequence, 0, state.input_focus)?,
        44 => reply_query_keymap(stream, state.sequence)?,
        53..=83 => {}
        84 => reply_alloc_color(stream, state.sequence, body)?,
        85 => reply_alloc_named_color(stream, state.sequence)?,
        91 => reply_best_size(stream, state, body)?,
        98 => reply_query_extension(stream, state.sequence)?,
        99 => reply_list_extensions(stream, state.sequence)?,
        101 => reply_keyboard_mapping(stream, state.sequence, body)?,
        103 => reply_keyboard_control(stream, state.sequence)?,
        106 => reply_pointer_control(stream, state.sequence)?,
        108 => reply_screen_saver(stream, state.sequence)?,
        117 => reply_pointer_mapping(stream, state.sequence)?,
        119 => reply_modifier_mapping(stream, state.sequence)?,
        127 => {}
        other => {
            return Err(BridgeRuntimeError::new(format!(
                "unsupported legacy WM core request opcode {other}"
            )));
        }
    }
    Ok(())
}

fn apply_warp_pointer(state: &mut XServerState, body: &[u8]) {
    let destination = read_u32(body, 4);
    let destination_x = read_u16(body, 16) as i16;
    let destination_y = read_u16(body, 18) as i16;
    if let Some(window) = state.windows.get(&destination) {
        state.pointer_x = i16::try_from(window.geometry.x)
            .unwrap_or_default()
            .saturating_add(destination_x);
        state.pointer_y = i16::try_from(window.geometry.y)
            .unwrap_or_default()
            .saturating_add(destination_y);
    }
}

fn complete_pending_pointer_gesture(
    stream: &mut UnixStream,
    state: &mut XServerState,
) -> Result<(), BridgeRuntimeError> {
    let Some(gesture) = state.pending_pointer_gesture.take() else {
        return Ok(());
    };
    let end_x = state.pointer_x.saturating_add(gesture.delta_x);
    let end_y = state.pointer_y.saturating_add(gesture.delta_y);
    state.pointer_x = end_x;
    state.pointer_y = end_y;
    state.pointer_mask = gesture.modifiers | (1 << (7 + gesture.button));
    let mut motion = vec![6, 0];
    push_u16(&mut motion, state.sequence);
    push_u32(&mut motion, 0);
    push_u32(&mut motion, SYNTHETIC_ROOT_XID);
    push_u32(&mut motion, gesture.window.raw());
    push_u32(&mut motion, 0);
    push_i16(&mut motion, end_x);
    push_i16(&mut motion, end_y);
    push_i16(&mut motion, 0);
    push_i16(&mut motion, 0);
    push_u16(&mut motion, state.pointer_mask);
    motion.push(1);
    motion.push(0);
    write_packet(stream, &motion)?;
    write_synthetic_pointer_event(
        stream,
        state,
        gesture.window,
        gesture.button,
        gesture.modifiers,
        false,
        end_x,
        end_y,
    )
}

#[allow(clippy::too_many_arguments)]
fn write_synthetic_pointer_event(
    stream: &mut UnixStream,
    state: &XServerState,
    window: SyntheticXWindowId,
    button: u8,
    modifiers: u16,
    pressed: bool,
    root_x: i16,
    root_y: i16,
) -> Result<(), BridgeRuntimeError> {
    let geometry = state
        .windows
        .get(&window.raw())
        .map(|entry| entry.geometry)
        .ok_or_else(|| BridgeRuntimeError::new("pointer gesture targeted an unknown window"))?;
    let mut event = vec![if pressed { 4 } else { 5 }, button];
    push_u16(&mut event, state.sequence);
    push_u32(&mut event, 0);
    push_u32(&mut event, SYNTHETIC_ROOT_XID);
    push_u32(&mut event, window.raw());
    push_u32(&mut event, 0);
    push_i16(&mut event, root_x);
    push_i16(&mut event, root_y);
    push_i16(
        &mut event,
        root_x.saturating_sub(i16::try_from(geometry.x).unwrap_or_default()),
    );
    push_i16(
        &mut event,
        root_y.saturating_sub(i16::try_from(geometry.y).unwrap_or_default()),
    );
    push_u16(&mut event, modifiers);
    event.push(1);
    event.push(0);
    write_packet(stream, &event)
}

fn configure_window(
    state: &mut XServerState,
    legacy: &SyncSender<LegacyWmRequest>,
    body: &[u8],
) -> Result<(), BridgeRuntimeError> {
    let raw = read_u32(body, 0);
    let mask = read_u16(body, 4);
    let Some(window) = synthetic_id(state, raw) else {
        return Ok(());
    };
    let mut geometry = state
        .windows
        .get(&raw)
        .expect("known synthetic window")
        .geometry;
    let mut sibling = None;
    let mut stack_mode = None;
    let mut cursor = 8;
    for bit in 0..7 {
        if mask & (1 << bit) == 0 {
            continue;
        }
        let value = read_u32(body, cursor);
        cursor += 4;
        match bit {
            0 => geometry.x = value as i32,
            1 => geometry.y = value as i32,
            2 => geometry.width = value as i32,
            3 => geometry.height = value as i32,
            5 => sibling = Some(value),
            6 => stack_mode = Some(value as u8),
            _ => {}
        }
    }
    state
        .windows
        .get_mut(&raw)
        .expect("known synthetic window")
        .geometry = geometry;
    if stack_mode.is_some() || sibling.is_some() {
        state.stacking.retain(|candidate| *candidate != raw);
        let sibling_index = sibling.and_then(|sibling| {
            state
                .stacking
                .iter()
                .position(|candidate| *candidate == sibling)
        });
        let index = match (stack_mode, sibling_index) {
            (Some(1 | 3), Some(index)) => index,
            (Some(1 | 3), None) => 0,
            (Some(0 | 2 | 4), Some(index)) => index.saturating_add(1),
            _ => state.stacking.len(),
        };
        state.stacking.insert(index.min(state.stacking.len()), raw);
    }
    let z_index = state
        .stacking
        .iter()
        .position(|candidate| *candidate == raw)
        .and_then(|index| i32::try_from(index).ok())
        .unwrap_or_default();
    legacy
        .send(LegacyWmRequest::ConfigureWindow {
            window,
            geometry,
            z_index,
        })
        .map_err(|_| BridgeRuntimeError::new("legacy request channel disconnected"))
}
