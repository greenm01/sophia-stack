fn synthetic_id(state: &XServerState, raw: u32) -> Option<SyntheticXWindowId> {
    state
        .windows
        .contains_key(&raw)
        .then_some(SyntheticXWindowId(raw))
}

fn reply_window_attributes(
    stream: &mut UnixStream,
    state: &XServerState,
    window: u32,
) -> Result<(), BridgeRuntimeError> {
    let mapped = state
        .windows
        .get(&window)
        .is_some_and(|window| window.mapped);
    let mut reply = vec![1, 0];
    push_u16(&mut reply, state.sequence);
    push_u32(&mut reply, 3);
    push_u32(&mut reply, sophia_x_authority::X_SETUP_DEFAULT_VISUAL);
    push_u16(&mut reply, 1);
    reply.extend_from_slice(&[0, 0]);
    push_u32(&mut reply, u32::MAX);
    push_u32(&mut reply, 0);
    reply.extend_from_slice(&[0, 1, if mapped { 2 } else { 0 }, 0]);
    push_u32(&mut reply, sophia_x_authority::X_SETUP_DEFAULT_COLORMAP);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, 0);
    push_u16(&mut reply, 0);
    reply.resize(44, 0);
    write_packet(stream, &reply)
}

fn reply_geometry(
    stream: &mut UnixStream,
    state: &XServerState,
    window: u32,
) -> Result<(), BridgeRuntimeError> {
    let geometry = if window == SYNTHETIC_ROOT_XID {
        state.root
    } else {
        state
            .windows
            .get(&window)
            .map(|window| window.geometry)
            .unwrap_or(state.root)
    };
    let mut reply = vec![1, 24];
    push_u16(&mut reply, state.sequence);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, SYNTHETIC_ROOT_XID);
    push_i16(&mut reply, geometry.x as i16);
    push_i16(&mut reply, geometry.y as i16);
    push_u16(&mut reply, geometry.width as u16);
    push_u16(&mut reply, geometry.height as u16);
    push_u16(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_query_tree(
    stream: &mut UnixStream,
    state: &XServerState,
) -> Result<(), BridgeRuntimeError> {
    let children = state.windows.keys().copied().collect::<Vec<_>>();
    let mut reply = vec![1, 0];
    push_u16(&mut reply, state.sequence);
    push_u32(&mut reply, children.len() as u32);
    push_u32(&mut reply, SYNTHETIC_ROOT_XID);
    push_u32(&mut reply, 0);
    push_u16(&mut reply, children.len() as u16);
    reply.resize(32, 0);
    for child in children {
        push_u32(&mut reply, child);
    }
    write_packet(stream, &reply)
}

fn reply_intern_atom(
    stream: &mut UnixStream,
    state: &mut XServerState,
    only_if_exists: bool,
    body: &[u8],
) -> Result<(), BridgeRuntimeError> {
    let len = usize::from(read_u16(body, 0));
    let name = body
        .get(4..4 + len)
        .ok_or_else(|| BridgeRuntimeError::new("truncated InternAtom name"))?
        .to_vec();
    let atom = if let Some(atom) = state.atoms_by_name.get(&name) {
        *atom
    } else if only_if_exists {
        0
    } else {
        let atom = state.next_atom;
        state.next_atom = state.next_atom.saturating_add(1);
        state.atoms_by_name.insert(name.clone(), atom);
        state.atom_names.insert(atom, name);
        atom
    };
    reply_u32(stream, state.sequence, 0, atom)
}

fn reply_atom_name(
    stream: &mut UnixStream,
    state: &XServerState,
    atom: u32,
) -> Result<(), BridgeRuntimeError> {
    let name = state
        .atom_names
        .get(&atom)
        .map(Vec::as_slice)
        .unwrap_or(b"");
    let padded = (name.len() + 3) & !3;
    let mut reply = vec![1, 0];
    push_u16(&mut reply, state.sequence);
    push_u32(&mut reply, (padded / 4) as u32);
    push_u16(&mut reply, name.len() as u16);
    reply.resize(32, 0);
    reply.extend_from_slice(name);
    reply.resize(32 + padded, 0);
    write_packet(stream, &reply)
}

fn reply_empty_property(stream: &mut UnixStream, sequence: u16) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_list_properties(stream: &mut UnixStream, sequence: u16) -> Result<(), BridgeRuntimeError> {
    reply_simple(stream, sequence, 0)
}

fn reply_query_pointer(
    stream: &mut UnixStream,
    state: &XServerState,
) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 1];
    push_u16(&mut reply, state.sequence);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, SYNTHETIC_ROOT_XID);
    push_u32(&mut reply, 0);
    push_i16(&mut reply, 0);
    push_i16(&mut reply, 0);
    push_i16(&mut reply, 0);
    push_i16(&mut reply, 0);
    push_u16(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_translate_coordinates(
    stream: &mut UnixStream,
    state: &XServerState,
) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 1];
    push_u16(&mut reply, state.sequence);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, 0);
    push_i16(&mut reply, 0);
    push_i16(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_query_keymap(stream: &mut UnixStream, sequence: u16) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 2);
    reply.resize(40, 0);
    write_packet(stream, &reply)
}

fn reply_alloc_color(
    stream: &mut UnixStream,
    sequence: u16,
    body: &[u8],
) -> Result<(), BridgeRuntimeError> {
    let red = read_u16(body, 4);
    let green = read_u16(body, 6);
    let blue = read_u16(body, 8);
    let pixel = (u32::from(red >> 8) << 16) | (u32::from(green >> 8) << 8) | u32::from(blue >> 8);
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    push_u16(&mut reply, red);
    push_u16(&mut reply, green);
    push_u16(&mut reply, blue);
    reply.resize(20, 0);
    push_u32(&mut reply, pixel);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_alloc_named_color(
    stream: &mut UnixStream,
    sequence: u16,
) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_best_size(
    stream: &mut UnixStream,
    state: &XServerState,
    body: &[u8],
) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, state.sequence);
    push_u32(&mut reply, 0);
    push_u16(&mut reply, read_u16(body, 4));
    push_u16(&mut reply, read_u16(body, 6));
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_query_extension(stream: &mut UnixStream, sequence: u16) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_list_extensions(stream: &mut UnixStream, sequence: u16) -> Result<(), BridgeRuntimeError> {
    reply_simple(stream, sequence, 0)
}

fn reply_keyboard_mapping(
    stream: &mut UnixStream,
    sequence: u16,
    first_keycode: u8,
    body: &[u8],
) -> Result<(), BridgeRuntimeError> {
    let count = usize::from(body.first().copied().unwrap_or(0));
    let mut reply = vec![1, 1];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, count as u32);
    reply.resize(32, 0);
    for keycode in first_keycode..first_keycode.saturating_add(count as u8) {
        push_u32(&mut reply, u32::from(keycode));
    }
    write_packet(stream, &reply)
}

fn reply_keyboard_control(
    stream: &mut UnixStream,
    sequence: u16,
) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 5);
    reply.resize(52, 0);
    write_packet(stream, &reply)
}

fn reply_pointer_control(stream: &mut UnixStream, sequence: u16) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    push_u16(&mut reply, 1);
    push_u16(&mut reply, 1);
    push_u16(&mut reply, 4);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_screen_saver(stream: &mut UnixStream, sequence: u16) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_pointer_mapping(stream: &mut UnixStream, sequence: u16) -> Result<(), BridgeRuntimeError> {
    reply_simple(stream, sequence, 0)
}

fn reply_modifier_mapping(
    stream: &mut UnixStream,
    sequence: u16,
) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_simple(
    stream: &mut UnixStream,
    sequence: u16,
    detail: u8,
) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, detail];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_u32(
    stream: &mut UnixStream,
    sequence: u16,
    detail: u8,
    value: u32,
) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, detail];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, value);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn write_configure_notify(
    stream: &mut UnixStream,
    sequence: u16,
    window: u32,
    geometry: Rect,
) -> Result<(), BridgeRuntimeError> {
    let mut event = vec![22, 0];
    push_u16(&mut event, sequence);
    push_u32(&mut event, window);
    push_u32(&mut event, window);
    push_u32(&mut event, 0);
    push_i16(&mut event, geometry.x as i16);
    push_i16(&mut event, geometry.y as i16);
    push_u16(&mut event, geometry.width as u16);
    push_u16(&mut event, geometry.height as u16);
    push_u16(&mut event, 0);
    event.resize(32, 0);
    write_packet(stream, &event)
}

fn write_window_event(
    stream: &mut UnixStream,
    sequence: u16,
    event_type: u8,
    window: u32,
) -> Result<(), BridgeRuntimeError> {
    let mut event = vec![event_type, 0];
    push_u16(&mut event, sequence);
    push_u32(&mut event, window);
    push_u32(&mut event, window);
    event.resize(32, 0);
    write_packet(stream, &event)
}
