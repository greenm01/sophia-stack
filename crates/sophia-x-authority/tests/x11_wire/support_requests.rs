fn present_dispatch_result(transaction: TransactionId) -> XDispatchResult {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let window = XResourceId::new(0x220530, 1);
    runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(1),
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window,
            surface: SurfaceId::new(40, 1),
            geometry: Rect {
                x: 0,
                y: 0,
                width: 300,
                height: 200,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    });

    dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            u16::try_from(transaction.raw()).unwrap_or(u16::MAX),
            XByteOrder::LittleEndian,
            X_SOPHIA_PRESENT_MAJOR_OPCODE,
        ),
        XWireRequest::Authority(XAuthorityRequestPacket {
            transaction,
            namespace,
            kind: XAuthorityRequestKind::PresentPixmap {
                window,
                pixmap: 0x900,
                damage: Region::single(Rect {
                    x: 4,
                    y: 5,
                    width: 64,
                    height: 48,
                }),
                previous_committed_generation: 1,
                timeout_msec: 250,
            },
        }),
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
}

fn context(namespace: NamespaceId, transaction: u64, byte_order: XByteOrder) -> XWireClientContext {
    XWireClientContext {
        byte_order,
        namespace,
        transaction: TransactionId::from_raw(transaction),
        resource_id_range: None,
    }
}

fn dispatch_context(
    namespace: NamespaceId,
    sequence: u16,
    byte_order: XByteOrder,
    major_opcode: u8,
) -> XDispatchContext {
    XDispatchContext {
        byte_order,
        namespace,
        sequence,
        major_opcode,
        client_id: 1,
    }
}

fn setup_request(
    byte_order: XByteOrder,
    major: u16,
    minor: u16,
    auth_name: &[u8],
    auth_data: &[u8],
) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(byte_order.marker());
    out.push(0);
    push_u16(&mut out, byte_order, major);
    push_u16(&mut out, byte_order, minor);
    push_u16(&mut out, byte_order, auth_name.len() as u16);
    push_u16(&mut out, byte_order, auth_data.len() as u16);
    push_u16(&mut out, byte_order, 0);
    out.extend_from_slice(auth_name);
    pad_to_four(&mut out);
    out.extend_from_slice(auth_data);
    pad_to_four(&mut out);
    out
}

fn create_window_request(
    byte_order: XByteOrder,
    window: u32,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
) -> Vec<u8> {
    let mut out = vec![1, 24];
    push_u16(&mut out, byte_order, 8);
    push_u32(&mut out, byte_order, window);
    push_u32(&mut out, byte_order, 0x20);
    push_i16(&mut out, byte_order, x);
    push_i16(&mut out, byte_order, y);
    push_u16(&mut out, byte_order, width);
    push_u16(&mut out, byte_order, height);
    push_u16(&mut out, byte_order, 0);
    push_u16(&mut out, byte_order, 1);
    push_u32(&mut out, byte_order, 0);
    push_u32(&mut out, byte_order, 0);
    out
}

#[allow(clippy::too_many_arguments)]
fn create_window_background_request(
    byte_order: XByteOrder,
    window: u32,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
    background_pixel: u32,
) -> Vec<u8> {
    let mut out = create_window_request(byte_order, window, x, y, width, height);
    out[2..4].copy_from_slice(&match byte_order {
        XByteOrder::LittleEndian => 9u16.to_le_bytes(),
        XByteOrder::BigEndian => 9u16.to_be_bytes(),
    });
    out[28..32].copy_from_slice(&match byte_order {
        XByteOrder::LittleEndian => 2u32.to_le_bytes(),
        XByteOrder::BigEndian => 2u32.to_be_bytes(),
    });
    push_u32(&mut out, byte_order, background_pixel);
    out
}

fn create_window_override_redirect_request(
    byte_order: XByteOrder,
    window: u32,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
) -> Vec<u8> {
    let mut out = create_window_request(byte_order, window, x, y, width, height);
    out[2..4].copy_from_slice(&match byte_order {
        XByteOrder::LittleEndian => 9u16.to_le_bytes(),
        XByteOrder::BigEndian => 9u16.to_be_bytes(),
    });
    out[28..32].copy_from_slice(&match byte_order {
        XByteOrder::LittleEndian => (1u32 << 9).to_le_bytes(),
        XByteOrder::BigEndian => (1u32 << 9).to_be_bytes(),
    });
    push_u32(&mut out, byte_order, 1);
    out
}

fn resource_request(byte_order: XByteOrder, opcode: u8, id: u32) -> Vec<u8> {
    let mut out = vec![opcode, 0];
    push_u16(&mut out, byte_order, 2);
    push_u32(&mut out, byte_order, id);
    out
}

fn translate_coordinates_request(
    byte_order: XByteOrder,
    source: u32,
    destination: u32,
    src_x: i16,
    src_y: i16,
) -> Vec<u8> {
    let mut out = vec![40, 0];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, source);
    push_u32(&mut out, byte_order, destination);
    push_i16(&mut out, byte_order, src_x);
    push_i16(&mut out, byte_order, src_y);
    out
}

fn intern_atom_request(byte_order: XByteOrder, only_if_exists: bool, name: &str) -> Vec<u8> {
    let mut out = vec![16, u8::from(only_if_exists)];
    let len_units = (8 + padded_len_for_test(name.len())) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u16(&mut out, byte_order, name.len() as u16);
    push_u16(&mut out, byte_order, 0);
    out.extend_from_slice(name.as_bytes());
    pad_to_four(&mut out);
    out
}

fn get_atom_name_request(byte_order: XByteOrder, atom: u32) -> Vec<u8> {
    let mut out = vec![17, 0];
    push_u16(&mut out, byte_order, 2);
    push_u32(&mut out, byte_order, atom);
    out
}

fn change_window_attributes_request(byte_order: XByteOrder, window: u32) -> Vec<u8> {
    let mut out = vec![2, 0];
    push_u16(&mut out, byte_order, 3);
    push_u32(&mut out, byte_order, window);
    push_u32(&mut out, byte_order, 0);
    out
}

fn change_window_override_redirect_request(
    byte_order: XByteOrder,
    window: u32,
    override_redirect: bool,
) -> Vec<u8> {
    let mut out = vec![2, 0];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, window);
    push_u32(&mut out, byte_order, 1 << 9);
    push_u32(&mut out, byte_order, u32::from(override_redirect));
    out
}

fn set_selection_owner_request(
    byte_order: XByteOrder,
    owner: u32,
    selection: u32,
    timestamp: u32,
) -> Vec<u8> {
    let mut out = vec![22, 0];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, owner);
    push_u32(&mut out, byte_order, selection);
    push_u32(&mut out, byte_order, timestamp);
    out
}

fn convert_selection_request(
    byte_order: XByteOrder,
    requestor: u32,
    selection: u32,
    target: u32,
    property: u32,
    timestamp: u32,
) -> Vec<u8> {
    let mut out = vec![24, 0];
    push_u16(&mut out, byte_order, 6);
    push_u32(&mut out, byte_order, requestor);
    push_u32(&mut out, byte_order, selection);
    push_u32(&mut out, byte_order, target);
    push_u32(&mut out, byte_order, property);
    push_u32(&mut out, byte_order, timestamp);
    out
}

fn send_selection_notify_request(
    byte_order: XByteOrder,
    requestor: u32,
    timestamp: u32,
    selection: u32,
    target: u32,
    property: u32,
) -> Vec<u8> {
    let mut out = vec![25, 0];
    push_u16(&mut out, byte_order, 11);
    push_u32(&mut out, byte_order, requestor);
    push_u32(&mut out, byte_order, 0);
    out.push(31);
    out.push(0);
    push_u16(&mut out, byte_order, 0);
    push_u32(&mut out, byte_order, timestamp);
    push_u32(&mut out, byte_order, requestor);
    push_u32(&mut out, byte_order, selection);
    push_u32(&mut out, byte_order, target);
    push_u32(&mut out, byte_order, property);
    out.extend_from_slice(&[0; 8]);
    out
}

fn grab_button_request(
    byte_order: XByteOrder,
    window: u32,
    event_mask: u16,
    button: u8,
    modifiers: u16,
) -> Vec<u8> {
    let mut out = vec![28, 1];
    push_u16(&mut out, byte_order, 6);
    push_u32(&mut out, byte_order, window);
    push_u16(&mut out, byte_order, event_mask);
    out.push(1);
    out.push(1);
    push_u32(&mut out, byte_order, 0);
    push_u32(&mut out, byte_order, 0);
    out.push(button);
    out.push(0);
    push_u16(&mut out, byte_order, modifiers);
    out
}

fn ungrab_button_request(
    byte_order: XByteOrder,
    window: u32,
    button: u8,
    modifiers: u16,
) -> Vec<u8> {
    let mut out = vec![29, button];
    push_u16(&mut out, byte_order, 3);
    push_u32(&mut out, byte_order, window);
    push_u16(&mut out, byte_order, modifiers);
    push_u16(&mut out, byte_order, 0);
    out
}

fn change_property_request(
    byte_order: XByteOrder,
    mode: XPropertyMode,
    window: u32,
    property: u32,
    property_type: u32,
    format: u8,
    bytes: &[u8],
) -> Vec<u8> {
    let mode = match mode {
        XPropertyMode::Replace => 0,
        XPropertyMode::Prepend => 1,
        XPropertyMode::Append => 2,
    };
    let mut out = vec![18, mode];
    let len_units = (24 + padded_len_for_test(bytes.len())) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, window);
    push_u32(&mut out, byte_order, property);
    push_u32(&mut out, byte_order, property_type);
    out.push(format);
    out.extend_from_slice(&[0, 0, 0]);
    push_u32(&mut out, byte_order, bytes.len() as u32);
    out.extend_from_slice(bytes);
    pad_to_four(&mut out);
    out
}

fn get_property_request(
    byte_order: XByteOrder,
    delete: bool,
    window: u32,
    property: u32,
    property_type: u32,
    long_offset: u32,
    long_length: u32,
) -> Vec<u8> {
    let mut out = vec![20, u8::from(delete)];
    push_u16(&mut out, byte_order, 6);
    push_u32(&mut out, byte_order, window);
    push_u32(&mut out, byte_order, property);
    push_u32(&mut out, byte_order, property_type);
    push_u32(&mut out, byte_order, long_offset);
    push_u32(&mut out, byte_order, long_length);
    out
}

fn create_gc_request(byte_order: XByteOrder, gc: u32, drawable: u32) -> Vec<u8> {
    let mut out = vec![55, 0];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, gc);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, 0);
    out
}

#[allow(clippy::too_many_arguments)]
fn create_gc_values_request(
    byte_order: XByteOrder,
    gc: u32,
    drawable: u32,
    function: u32,
    plane_mask: u32,
    foreground: u32,
    background: u32,
    line_width: u32,
    font: u32,
) -> Vec<u8> {
    let value_mask = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 14);
    let mut out = vec![55, 0];
    push_u16(&mut out, byte_order, 10);
    push_u32(&mut out, byte_order, gc);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, value_mask);
    for value in [
        function, plane_mask, foreground, background, line_width, font,
    ] {
        push_u32(&mut out, byte_order, value);
    }
    out
}

fn change_gc_request(byte_order: XByteOrder, gc: u32, mask: u32, values: &[u32]) -> Vec<u8> {
    let mut out = vec![56, 0];
    push_u16(
        &mut out,
        byte_order,
        u16::try_from(3 + values.len()).unwrap(),
    );
    push_u32(&mut out, byte_order, gc);
    push_u32(&mut out, byte_order, mask);
    for value in values {
        push_u32(&mut out, byte_order, *value);
    }
    out
}

fn set_clip_rectangles_request(
    byte_order: XByteOrder,
    gc: u32,
    rectangles: &[(i16, i16, u16, u16)],
) -> Vec<u8> {
    let mut out = vec![59, 0];
    push_u16(&mut out, byte_order, 3 + (rectangles.len() as u16 * 2));
    push_u32(&mut out, byte_order, gc);
    push_i16(&mut out, byte_order, 0);
    push_i16(&mut out, byte_order, 0);
    for &(x, y, width, height) in rectangles {
        push_i16(&mut out, byte_order, x);
        push_i16(&mut out, byte_order, y);
        push_u16(&mut out, byte_order, width);
        push_u16(&mut out, byte_order, height);
    }
    out
}

fn change_window_event_mask_request(
    byte_order: XByteOrder,
    window: u32,
    event_mask: u32,
) -> Vec<u8> {
    let mut out = vec![2, 0];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, window);
    push_u32(&mut out, byte_order, 1 << 11);
    push_u32(&mut out, byte_order, event_mask);
    out
}

fn create_pixmap_request(
    byte_order: XByteOrder,
    depth: u8,
    pixmap: u32,
    drawable: u32,
    width: u16,
    height: u16,
) -> Vec<u8> {
    let mut out = vec![53, depth];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, pixmap);
    push_u32(&mut out, byte_order, drawable);
    push_u16(&mut out, byte_order, width);
    push_u16(&mut out, byte_order, height);
    out
}

fn clear_area_request(
    byte_order: XByteOrder,
    exposures: bool,
    window: u32,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
) -> Vec<u8> {
    let mut out = vec![61, u8::from(exposures)];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, window);
    push_i16(&mut out, byte_order, x);
    push_i16(&mut out, byte_order, y);
    push_u16(&mut out, byte_order, width);
    push_u16(&mut out, byte_order, height);
    out
}

fn open_font_request(byte_order: XByteOrder, font: u32, name: &str) -> Vec<u8> {
    let mut out = vec![45, 0];
    let len_units = (12 + padded_len_for_test(name.len())) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, font);
    push_u16(&mut out, byte_order, name.len() as u16);
    push_u16(&mut out, byte_order, 0);
    out.extend_from_slice(name.as_bytes());
    pad_to_four(&mut out);
    out
}

fn list_fonts_request(byte_order: XByteOrder, max_names: u16, pattern: &str) -> Vec<u8> {
    let mut out = vec![49, 0];
    let len_units = (8 + padded_len_for_test(pattern.len())) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u16(&mut out, byte_order, max_names);
    push_u16(&mut out, byte_order, pattern.len() as u16);
    out.extend_from_slice(pattern.as_bytes());
    pad_to_four(&mut out);
    out
}

fn list_fonts_with_info_request(byte_order: XByteOrder, max_names: u16, pattern: &str) -> Vec<u8> {
    let mut out = vec![50, 0];
    let len_units = (8 + padded_len_for_test(pattern.len())) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u16(&mut out, byte_order, max_names);
    push_u16(&mut out, byte_order, pattern.len() as u16);
    out.extend_from_slice(pattern.as_bytes());
    pad_to_four(&mut out);
    out
}

fn create_glyph_cursor_request(
    byte_order: XByteOrder,
    cursor: u32,
    source_font: u32,
    mask_font: u32,
) -> Vec<u8> {
    let mut out = vec![94, 0];
    push_u16(&mut out, byte_order, 8);
    push_u32(&mut out, byte_order, cursor);
    push_u32(&mut out, byte_order, source_font);
    push_u32(&mut out, byte_order, mask_font);
    push_u16(&mut out, byte_order, 1);
    push_u16(&mut out, byte_order, 2);
    push_u16(&mut out, byte_order, u16::MAX);
    push_u16(&mut out, byte_order, u16::MAX);
    push_u16(&mut out, byte_order, u16::MAX);
    push_u16(&mut out, byte_order, 0);
    push_u16(&mut out, byte_order, 0);
    push_u16(&mut out, byte_order, 0);
    out
}

fn recolor_cursor_request(byte_order: XByteOrder, cursor: u32) -> Vec<u8> {
    let mut out = vec![96, 0];
    push_u16(&mut out, byte_order, 5);
    push_u32(&mut out, byte_order, cursor);
    for value in [u16::MAX, u16::MAX, u16::MAX, 0, 0, 0] {
        push_u16(&mut out, byte_order, value);
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn copy_area_request(
    byte_order: XByteOrder,
    source: u32,
    destination: u32,
    gc: u32,
    src_x: i16,
    src_y: i16,
    dst_x: i16,
    dst_y: i16,
    width: u16,
    height: u16,
) -> Vec<u8> {
    let mut out = vec![62, 0];
    push_u16(&mut out, byte_order, 7);
    push_u32(&mut out, byte_order, source);
    push_u32(&mut out, byte_order, destination);
    push_u32(&mut out, byte_order, gc);
    push_i16(&mut out, byte_order, src_x);
    push_i16(&mut out, byte_order, src_y);
    push_i16(&mut out, byte_order, dst_x);
    push_i16(&mut out, byte_order, dst_y);
    push_u16(&mut out, byte_order, width);
    push_u16(&mut out, byte_order, height);
    out
}

fn poly_fill_rectangle_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    rectangles: &[(i16, i16, u16, u16)],
) -> Vec<u8> {
    let mut out = vec![70, 0];
    let len_units = 3 + rectangles.len() * 2;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    for (x, y, width, height) in rectangles {
        push_i16(&mut out, byte_order, *x);
        push_i16(&mut out, byte_order, *y);
        push_u16(&mut out, byte_order, *width);
        push_u16(&mut out, byte_order, *height);
    }
    out
}

fn poly_text8_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    x: i16,
    y: i16,
    text: &[u8],
) -> Vec<u8> {
    let mut out = vec![74, 0];
    let len_units = padded_len_for_test(18 + text.len()) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    push_i16(&mut out, byte_order, x);
    push_i16(&mut out, byte_order, y);
    out.push(u8::try_from(text.len()).unwrap());
    out.push(0);
    out.extend_from_slice(text);
    pad_to_four(&mut out);
    out
}

fn image_text8_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    x: i16,
    y: i16,
    text: &[u8],
) -> Vec<u8> {
    let mut out = vec![76, u8::try_from(text.len()).unwrap()];
    let len_units = (16 + padded_len_for_test(text.len())) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    push_i16(&mut out, byte_order, x);
    push_i16(&mut out, byte_order, y);
    out.extend_from_slice(text);
    pad_to_four(&mut out);
    out
}

fn poly_text8_compact_item_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    x: i16,
    y: i16,
    text: &[u8],
) -> Vec<u8> {
    let mut out = vec![74, 0];
    let len_units = padded_len_for_test(18 + text.len()) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    push_i16(&mut out, byte_order, x);
    push_i16(&mut out, byte_order, y);
    out.push(u8::try_from(text.len() + 1).unwrap());
    out.push(0);
    out.extend_from_slice(text);
    pad_to_four(&mut out);
    out
}

fn poly_segment_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    segments: &[(i16, i16, i16, i16)],
) -> Vec<u8> {
    let mut out = vec![66, 0];
    let len_units = 3 + segments.len() * 2;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    for (x1, y1, x2, y2) in segments {
        push_i16(&mut out, byte_order, *x1);
        push_i16(&mut out, byte_order, *y1);
        push_i16(&mut out, byte_order, *x2);
        push_i16(&mut out, byte_order, *y2);
    }
    out
}

fn poly_line_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    points: &[(i16, i16)],
) -> Vec<u8> {
    let mut out = vec![65, 0];
    let len_units = 3 + points.len();
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    for (x, y) in points {
        push_i16(&mut out, byte_order, *x);
        push_i16(&mut out, byte_order, *y);
    }
    out
}

fn poly_fill_arc_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    arcs: &[(i16, i16, u16, u16, i16, i16)],
) -> Vec<u8> {
    let mut out = vec![71, 0];
    let len_units = 3 + arcs.len() * 3;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    for (x, y, width, height, angle1, angle2) in arcs {
        push_i16(&mut out, byte_order, *x);
        push_i16(&mut out, byte_order, *y);
        push_u16(&mut out, byte_order, *width);
        push_u16(&mut out, byte_order, *height);
        push_i16(&mut out, byte_order, *angle1);
        push_i16(&mut out, byte_order, *angle2);
    }
    out
}

fn fill_poly_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    points: &[(i16, i16)],
) -> Vec<u8> {
    let mut out = vec![69, 0];
    let len_units = 4 + points.len();
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    out.extend_from_slice(&[0, 0, 0, 0]);
    for (x, y) in points {
        push_i16(&mut out, byte_order, *x);
        push_i16(&mut out, byte_order, *y);
    }
    out
}

struct PutImageGeometry {
    width: u16,
    height: u16,
    dst_x: i16,
    dst_y: i16,
}

fn put_image_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    geometry: PutImageGeometry,
    data: &[u8],
) -> Vec<u8> {
    let mut out = vec![72, 2];
    let len_units = (24 + padded_len_for_test(data.len())) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    push_u16(&mut out, byte_order, geometry.width);
    push_u16(&mut out, byte_order, geometry.height);
    push_i16(&mut out, byte_order, geometry.dst_x);
    push_i16(&mut out, byte_order, geometry.dst_y);
    out.push(0);
    out.push(24);
    push_u16(&mut out, byte_order, 0);
    out.extend_from_slice(data);
    pad_to_four(&mut out);
    out
}
