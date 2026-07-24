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
                                } => {
                                    push_u16(byte_order, &mut body, 2);
                                    push_u16(byte_order, &mut body, 11);
                                    push_u16(byte_order, &mut body, *source_id);
                                    push_u16(byte_order, &mut body, *number);
                                    push_u32(byte_order, &mut body, 0);
                                    push_i64(byte_order, &mut body, *min);
                                    push_i64(byte_order, &mut body, *max);
                                    push_i64(byte_order, &mut body, 0);
                                    push_u32(byte_order, &mut body, 1);
                                    body.extend_from_slice(&[0; 4]);
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
                } => {
                    let mut out = vec![0; 56];
                    write_reply_header(byte_order, &mut out, sequence, 6);
                    put_resource(byte_order, &mut out[8..12], root);
                    put_resource(byte_order, &mut out[12..16], child);
                    out[32] = 1;
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
