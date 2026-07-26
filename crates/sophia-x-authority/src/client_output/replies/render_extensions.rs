fn encode_render_extension_reply(
    byte_order: XByteOrder,
    reply: XClientReply,
) -> Result<Vec<u8>, XClientReply> {
    if !matches!(
        &reply,
            XClientReply::ShmQueryVersion { .. }
            | XClientReply::ShmGetImage { .. }
            | XClientReply::Dri3QueryVersion { .. }
            | XClientReply::Dri3Open { .. }
            | XClientReply::Dri3GetSupportedModifiers { .. }
            | XClientReply::XfixesQueryVersion { .. }
            | XClientReply::PresentQueryVersion { .. }
            | XClientReply::PresentQueryCapabilities { .. }
    ) {
        return Err(reply);
    }
    Ok(match reply {
                XClientReply::ShmQueryVersion {
                    sequence,
                    major_version,
                    minor_version,
                    shared_pixmaps,
                    pixmap_format,
                } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    out[1] = u8::from(shared_pixmaps);
                    put_u16(byte_order, &mut out[8..10], major_version);
                    put_u16(byte_order, &mut out[10..12], minor_version);
                    out[16] = pixmap_format;
                    out
                }
                XClientReply::ShmGetImage {
                    sequence,
                    depth,
                    visual,
                    size,
                } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    out[1] = depth;
                    put_u32(byte_order, &mut out[8..12], visual);
                    put_u32(byte_order, &mut out[12..16], size);
                    out
                }
                XClientReply::Dri3QueryVersion {
                    sequence,
                    major_version,
                    minor_version,
                }
                | XClientReply::XfixesQueryVersion {
                    sequence,
                    major_version,
                    minor_version,
                }
                | XClientReply::PresentQueryVersion {
                    sequence,
                    major_version,
                    minor_version,
                } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    put_u32(byte_order, &mut out[8..12], major_version);
                    put_u32(byte_order, &mut out[12..16], minor_version);
                    out
                }
                XClientReply::Dri3Open { sequence } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    out[1] = 1;
                    out
                }
                XClientReply::Dri3GetSupportedModifiers {
                    sequence,
                    window_modifiers,
                    screen_modifiers,
                } => {
                    let modifier_count = window_modifiers
                        .len()
                        .saturating_add(screen_modifiers.len());
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + modifier_count.saturating_mul(8)];
                    write_reply_header(
                        byte_order,
                        &mut out,
                        sequence,
                        u32::try_from(modifier_count.saturating_mul(2)).unwrap_or(u32::MAX),
                    );
                    put_u32(
                        byte_order,
                        &mut out[8..12],
                        u32::try_from(window_modifiers.len()).unwrap_or(u32::MAX),
                    );
                    put_u32(
                        byte_order,
                        &mut out[12..16],
                        u32::try_from(screen_modifiers.len()).unwrap_or(u32::MAX),
                    );
                    let mut offset = X_CLIENT_OUTPUT_RECORD_LEN;
                    for modifier in window_modifiers.into_iter().chain(screen_modifiers) {
                        put_u64(byte_order, &mut out[offset..offset + 8], modifier);
                        offset += 8;
                    }
                    out
                }
                XClientReply::PresentQueryCapabilities {
                    sequence,
                    capabilities,
                } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    put_u32(byte_order, &mut out[8..12], capabilities);
                    out
                }
        _ => unreachable!("reply family checked before encoding"),
    })
}
