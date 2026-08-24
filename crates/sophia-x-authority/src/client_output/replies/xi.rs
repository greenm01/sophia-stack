fn encode_x_input_reply(
    byte_order: XByteOrder,
    reply: XClientReply,
) -> Result<Vec<u8>, XClientReply> {
    if !matches!(
        &reply,
            XClientReply::XiQueryVersion { .. }
            | XClientReply::GeQueryVersion { .. }
            | XClientReply::XiGetClientPointer { .. }
            | XClientReply::XiGetExtensionVersion { .. }
            | XClientReply::XiQueryDevice { .. }
            | XClientReply::XiListInputDevices { .. }
            | XClientReply::XiQueryPointer { .. }
            | XClientReply::XiGetFocus { .. }
            | XClientReply::XiGetProperty { .. }
    ) {
        return Err(reply);
    }
    Ok(match reply {
                XClientReply::XiQueryVersion {
                    sequence,
                    major_version,
                    minor_version,
                } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    out[1] = crate::X_INPUT_QUERY_VERSION_MINOR_OPCODE;
                    put_u16(byte_order, &mut out[8..10], major_version);
                    put_u16(byte_order, &mut out[10..12], minor_version);
                    out
                }
                XClientReply::GeQueryVersion {
                    sequence,
                    major_version,
                    minor_version,
                } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    put_u16(byte_order, &mut out[8..10], major_version);
                    put_u16(byte_order, &mut out[10..12], minor_version);
                    out
                }
                XClientReply::XiGetClientPointer {
                    sequence,
                    device_id,
                } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    out[8] = 1;
                    put_u16(byte_order, &mut out[10..12], device_id);
                    out
                }
                XClientReply::XiGetExtensionVersion {
                    sequence,
                    server_major,
                    server_minor,
                } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    out[1] = crate::X_INPUT_GET_EXTENSION_VERSION_MINOR_OPCODE;
                    put_u16(byte_order, &mut out[8..10], server_major);
                    put_u16(byte_order, &mut out[10..12], server_minor);
                    out[12] = 1;
                    out
                }
                // XI1 `ListInputDevices`. The reply is three concatenated sections
                // after the header -- every DeviceInfo, then every class info, then
                // every name -- rather than one self-contained record per device, so
                // the loops below cannot be merged. Layout derived from the X.Org
                // protocol description, not from an X server implementation.
                XClientReply::XiListInputDevices { sequence, devices } => {
                    let mut body = Vec::new();
                    for device in &devices {
                        push_u32(byte_order, &mut body, device.device_type);
                        body.push(device.device_id);
                        body.push(u8::try_from(device.classes.len()).unwrap_or(0));
                        body.push(device.device_use);
                        body.push(0);
                    }
                    for class in devices.iter().flat_map(|device| &device.classes) {
                        match class {
                            XXiLegacyDeviceClass::Key {
                                min_keycode,
                                max_keycode,
                            } => {
                                body.push(crate::X_INPUT_LEGACY_CLASS_KEY);
                                body.push(8);
                                body.push(*min_keycode);
                                body.push(*max_keycode);
                                push_u16(
                                    byte_order,
                                    &mut body,
                                    u16::from(max_keycode.saturating_sub(*min_keycode)) + 1,
                                );
                                body.extend_from_slice(&[0, 0]);
                            }
                            XXiLegacyDeviceClass::Button { button_count } => {
                                body.push(crate::X_INPUT_LEGACY_CLASS_BUTTON);
                                body.push(4);
                                push_u16(byte_order, &mut body, *button_count);
                            }
                        }
                    }
                    for device in &devices {
                        // XI1 names are `Str`: a length byte and the bytes, with no
                        // per-name padding. Only the assembled body pads to 4.
                        body.push(u8::try_from(device.name.len()).unwrap_or(0));
                        body.extend_from_slice(device.name.as_bytes());
                    }
                    body.resize(padded_len(body.len()), 0);
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(
                        byte_order,
                        &mut out,
                        sequence,
                        u32::try_from(body.len() / 4).unwrap_or(0),
                    );
                    out[1] = crate::X_INPUT_LIST_INPUT_DEVICES_MINOR_OPCODE;
                    // The device count is one byte here, unlike XI2's u16 at the same
                    // offset; the remaining header bytes stay zero.
                    out[8] = u8::try_from(devices.len()).unwrap_or(0);
                    out.extend_from_slice(&body);
                    out
                }
                XClientReply::XiQueryDevice { sequence, devices } => {
                    let mut body = Vec::new();
                    for device in &devices {
                        push_u16(byte_order, &mut body, device.device_id);
                        push_u16(byte_order, &mut body, device.device_type);
                        push_u16(byte_order, &mut body, device.attachment);
                        push_u16(
                            byte_order,
                            &mut body,
                            u16::try_from(device.classes.len()).unwrap_or(0),
                        );
                        push_u16(
                            byte_order,
                            &mut body,
                            u16::try_from(device.name.len()).unwrap_or(0),
                        );
                        body.extend_from_slice(&[1, 0]);
                        body.extend_from_slice(device.name.as_bytes());
                        body.resize(padded_len(body.len()), 0);
                        for class in &device.classes {
                            match class {
                                XXiDeviceClass::Key { source_id, keys } => {
                                    push_u16(byte_order, &mut body, 0);
                                    push_u16(
                                        byte_order,
                                        &mut body,
                                        u16::try_from(2 + keys.len()).unwrap_or(u16::MAX),
                                    );
                                    push_u16(byte_order, &mut body, *source_id);
                                    push_u16(
                                        byte_order,
                                        &mut body,
                                        u16::try_from(keys.len()).unwrap_or(0),
                                    );
                                    for key in keys {
                                        push_u32(byte_order, &mut body, *key);
                                    }
                                }
                                XXiDeviceClass::Button {
                                    source_id,
                                    button_count,
                                } => {
                                    push_u16(byte_order, &mut body, 1);
                                    push_u16(byte_order, &mut body, 2 + 1 + *button_count);
                                    push_u16(byte_order, &mut body, *source_id);
                                    push_u16(byte_order, &mut body, *button_count);
                                    push_u32(byte_order, &mut body, 0);
                                    for _ in 0..*button_count {
                                        push_u32(byte_order, &mut body, 0);
                                    }
                                }
                                XXiDeviceClass::Valuator {
                                    source_id,
                                    number,
                                    min,
                                    max,
                                    value,
                                } => {
                                    push_u16(byte_order, &mut body, 2);
                                    push_u16(byte_order, &mut body, 11);
                                    push_u16(byte_order, &mut body, *source_id);
                                    push_u16(byte_order, &mut body, *number);
                                    push_u32(byte_order, &mut body, 0);
                                    push_i64(byte_order, &mut body, *min);
                                    push_i64(byte_order, &mut body, *max);
                                    push_i64(byte_order, &mut body, *value);
                                    push_u32(byte_order, &mut body, 1);
                                    body.extend_from_slice(&[0; 4]);
                                }
                                XXiDeviceClass::Scroll {
                                    source_id,
                                    number,
                                    scroll_type,
                                    flags,
                                    increment,
                                } => {
                                    push_u16(byte_order, &mut body, 3);
                                    push_u16(byte_order, &mut body, 6);
                                    push_u16(byte_order, &mut body, *source_id);
                                    push_u16(byte_order, &mut body, *number);
                                    push_u16(byte_order, &mut body, *scroll_type);
                                    push_u16(byte_order, &mut body, 0);
                                    push_u32(byte_order, &mut body, *flags);
                                    push_i64(byte_order, &mut body, *increment);
                                }
                            }
                        }
                    }
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(
                        byte_order,
                        &mut out,
                        sequence,
                        u32::try_from(body.len() / 4).unwrap_or(0),
                    );
                    out[1] = crate::X_INPUT_QUERY_DEVICE_MINOR_OPCODE;
                    put_u16(
                        byte_order,
                        &mut out[8..10],
                        u16::try_from(devices.len()).unwrap_or(0),
                    );
                    out.extend_from_slice(&body);
                    out
                }
                XClientReply::XiQueryPointer {
                    sequence,
                    root,
                    child,
                    root_x,
                    root_y,
                    win_x,
                    win_y,
                    buttons,
                    modifiers,
                } => {
                    let buttons_len = u16::from(buttons != 0);
                    let mut out = vec![0; 56 + usize::from(buttons_len) * 4];
                    write_reply_header(
                        byte_order,
                        &mut out,
                        sequence,
                        6 + u32::from(buttons_len),
                    );
                    put_resource(byte_order, &mut out[8..12], root);
                    put_resource(byte_order, &mut out[12..16], child);
                    put_u32(byte_order, &mut out[16..20], (i32::from(root_x) << 16) as u32);
                    put_u32(byte_order, &mut out[20..24], (i32::from(root_y) << 16) as u32);
                    put_u32(byte_order, &mut out[24..28], (i32::from(win_x) << 16) as u32);
                    put_u32(byte_order, &mut out[28..32], (i32::from(win_y) << 16) as u32);
                    out[32] = 1;
                    put_u16(byte_order, &mut out[34..36], buttons_len);
                    put_u32(byte_order, &mut out[48..52], u32::from(modifiers));
                    if buttons_len != 0 {
                        put_u32(byte_order, &mut out[56..60], buttons);
                    }
                    out
                }
                XClientReply::XiGetFocus { sequence, focus } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    out[1] = crate::X_INPUT_GET_FOCUS_MINOR_OPCODE;
                    put_resource(byte_order, &mut out[8..12], focus);
                    out
                }
                XClientReply::XiGetProperty { sequence } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    out[1] = crate::X_INPUT_GET_PROPERTY_MINOR_OPCODE;
                    out
                }
        _ => unreachable!("reply family checked before encoding"),
    })
}
