fn encode_core_early_reply(
    byte_order: XByteOrder,
    reply: XClientReply,
) -> Result<Vec<u8>, XClientReply> {
    if !matches!(
        &reply,
            XClientReply::GrabStatus { .. }
            | XClientReply::InternAtom { .. }
            | XClientReply::GetAtomName { .. }
            | XClientReply::GetGeometry { .. }
            | XClientReply::GetImage { .. }
            | XClientReply::QueryTree { .. }
            | XClientReply::GetWindowAttributes { .. }
            | XClientReply::QueryExtension { .. }
            | XClientReply::ListExtensions { .. }
            | XClientReply::ListFonts { .. }
            | XClientReply::ListFontsWithInfo { .. }
            | XClientReply::QueryBestSize { .. }
    ) {
        return Err(reply);
    }
    Ok(match reply {
                XClientReply::GrabStatus { sequence, status } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    out[1] = status;
                    out
                }
                XClientReply::InternAtom { sequence, atom } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    put_u32(byte_order, &mut out[8..12], atom);
                    out
                }
                XClientReply::GetAtomName { sequence, name } => {
                    let bytes = name.as_bytes();
                    let padded_name_len = padded_len(bytes.len());
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + padded_name_len];
                    write_reply_header(
                        byte_order,
                        &mut out[..X_CLIENT_OUTPUT_RECORD_LEN],
                        sequence,
                        u32::try_from(padded_name_len / 4).unwrap_or(0),
                    );
                    put_u16(
                        byte_order,
                        &mut out[8..10],
                        u16::try_from(bytes.len()).unwrap_or(0),
                    );
                    out[X_CLIENT_OUTPUT_RECORD_LEN..X_CLIENT_OUTPUT_RECORD_LEN + bytes.len()]
                        .copy_from_slice(bytes);
                    out
                }
                XClientReply::GetGeometry {
                    sequence,
                    depth,
                    root,
                    geometry,
                    border_width,
                } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    out[1] = depth;
                    put_resource(byte_order, &mut out[8..12], root);
                    put_i16(
                        byte_order,
                        &mut out[12..14],
                        i16::try_from(geometry.x).unwrap_or(0),
                    );
                    put_i16(
                        byte_order,
                        &mut out[14..16],
                        i16::try_from(geometry.y).unwrap_or(0),
                    );
                    put_u16(
                        byte_order,
                        &mut out[16..18],
                        u16::try_from(geometry.width).unwrap_or(0),
                    );
                    put_u16(
                        byte_order,
                        &mut out[18..20],
                        u16::try_from(geometry.height).unwrap_or(0),
                    );
                    put_u16(byte_order, &mut out[20..22], border_width);
                    out
                }
                XClientReply::GetImage {
                    sequence,
                    depth,
                    visual,
                    data,
                } => {
                    let padded_len = (data.len() + 3) & !3;
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + padded_len];
                    write_reply_header(
                        byte_order,
                        &mut out[..X_CLIENT_OUTPUT_RECORD_LEN],
                        sequence,
                        u32::try_from(padded_len / 4).unwrap_or(u32::MAX),
                    );
                    out[1] = depth;
                    put_u32(byte_order, &mut out[8..12], visual);
                    out[X_CLIENT_OUTPUT_RECORD_LEN..X_CLIENT_OUTPUT_RECORD_LEN + data.len()]
                        .copy_from_slice(&data);
                    out
                }
                XClientReply::QueryTree {
                    sequence,
                    root,
                    parent,
                    children,
                } => {
                    let children_len = children.len().saturating_mul(4);
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + children_len];
                    write_reply_header(
                        byte_order,
                        &mut out[..X_CLIENT_OUTPUT_RECORD_LEN],
                        sequence,
                        u32::try_from(children_len / 4).unwrap_or(0),
                    );
                    put_resource(byte_order, &mut out[8..12], root);
                    put_resource(byte_order, &mut out[12..16], parent);
                    put_u16(
                        byte_order,
                        &mut out[16..18],
                        u16::try_from(children.len()).unwrap_or(0),
                    );
                    let mut offset = X_CLIENT_OUTPUT_RECORD_LEN;
                    for child in children {
                        put_resource(byte_order, &mut out[offset..offset + 4], child);
                        offset += 4;
                    }
                    out
                }
                XClientReply::GetWindowAttributes {
                    sequence,
                    visual,
                    colormap,
                    map_state,
                    override_redirect,
                } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + 12];
                    write_reply_header(
                        byte_order,
                        &mut out[..X_CLIENT_OUTPUT_RECORD_LEN],
                        sequence,
                        3,
                    );
                    out[1] = 0;
                    put_u32(byte_order, &mut out[8..12], visual);
                    put_u16(byte_order, &mut out[12..14], 1);
                    out[14] = 0;
                    out[15] = 1;
                    put_u32(byte_order, &mut out[16..20], 0);
                    put_u32(byte_order, &mut out[20..24], 0);
                    out[24] = 0;
                    out[25] = 1;
                    out[26] = map_state;
                    out[27] = u8::from(override_redirect);
                    put_resource(byte_order, &mut out[28..32], colormap);
                    out
                }
                XClientReply::QueryExtension {
                    sequence,
                    present,
                    major_opcode,
                    first_event,
                    first_error,
                } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    out[8] = u8::from(present);
                    out[9] = major_opcode;
                    out[10] = first_event;
                    out[11] = first_error;
                    out
                }
                XClientReply::ListExtensions { sequence } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    out[1] = 0;
                    out
                }
                XClientReply::ListFonts { sequence, names } => {
                    let names_len = names.iter().map(|name| 1 + name.len()).sum::<usize>();
                    let padded_names_len = padded_len(names_len);
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + padded_names_len];
                    write_reply_header(
                        byte_order,
                        &mut out[..X_CLIENT_OUTPUT_RECORD_LEN],
                        sequence,
                        u32::try_from(padded_names_len / 4).unwrap_or(0),
                    );
                    put_u16(
                        byte_order,
                        &mut out[8..10],
                        u16::try_from(names.len()).unwrap_or(0),
                    );
                    let mut offset = X_CLIENT_OUTPUT_RECORD_LEN;
                    for name in names {
                        let bytes = name.as_bytes();
                        out[offset] = u8::try_from(bytes.len()).unwrap_or(0);
                        offset += 1;
                        out[offset..offset + bytes.len()].copy_from_slice(bytes);
                        offset += bytes.len();
                    }
                    out
                }
                XClientReply::ListFontsWithInfo { sequence, names } => {
                    let mut out = Vec::new();
                    for name in names {
                        out.extend(encode_font_info_reply(
                            byte_order,
                            sequence,
                            8,
                            2,
                            Some(name.as_bytes()),
                        ));
                    }
                    out.extend(encode_font_info_reply(byte_order, sequence, 0, 0, None));
                    out
                }
                XClientReply::QueryBestSize {
                    sequence,
                    width,
                    height,
                } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    put_u16(byte_order, &mut out[8..10], width);
                    put_u16(byte_order, &mut out[10..12], height);
                    out
                }
        _ => unreachable!("reply family checked before encoding"),
    })
}
