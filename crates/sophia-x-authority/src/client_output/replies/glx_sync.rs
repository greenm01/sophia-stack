fn encode_glx_sync_reply(
    byte_order: XByteOrder,
    reply: XClientReply,
) -> Result<Vec<u8>, XClientReply> {
    if !matches!(
        &reply,
            XClientReply::GlxQueryVersion { .. }
            | XClientReply::GlxString { .. }
            | XClientReply::GlxVisualConfigs { .. }
            | XClientReply::GlxFbConfigs { .. }
            | XClientReply::GlxIsDirect { .. }
            | XClientReply::GlxDrawableAttributes { .. }
            | XClientReply::SyncInitialize { .. }
            | XClientReply::SyncListSystemCounters { .. }
            | XClientReply::SyncQueryCounter { .. }
    ) {
        return Err(reply);
    }
    Ok(match reply {
                XClientReply::GlxQueryVersion {
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
                XClientReply::GlxString { sequence, value } => {
                    let mut bytes = value.into_bytes();
                    bytes.push(0);
                    let padded = padded_len(bytes.len());
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + padded];
                    write_reply_header(
                        byte_order,
                        &mut out,
                        sequence,
                        u32::try_from(padded / 4).unwrap_or(u32::MAX),
                    );
                    put_u32(
                        byte_order,
                        &mut out[12..16],
                        u32::try_from(bytes.len()).unwrap_or(u32::MAX),
                    );
                    out[32..32 + bytes.len()].copy_from_slice(&bytes);
                    out
                }
                XClientReply::GlxVisualConfigs { sequence, configs } => {
                    let body_len = configs.len() * 18 * 4;
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + body_len];
                    write_reply_header(byte_order, &mut out, sequence, (body_len / 4) as u32);
                    put_u32(byte_order, &mut out[8..12], configs.len() as u32);
                    put_u32(byte_order, &mut out[12..16], 18);
                    for (index, value) in configs.into_iter().flatten().enumerate() {
                        put_u32(byte_order, &mut out[32 + index * 4..36 + index * 4], value);
                    }
                    out
                }
                XClientReply::GlxFbConfigs { sequence, configs } => {
                    let attributes = configs.first().map_or(0, Vec::len);
                    let body_len = configs.len() * attributes * 8;
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + body_len];
                    write_reply_header(byte_order, &mut out, sequence, (body_len / 4) as u32);
                    put_u32(byte_order, &mut out[8..12], configs.len() as u32);
                    put_u32(byte_order, &mut out[12..16], attributes as u32);
                    let mut offset = 32;
                    for (name, value) in configs.into_iter().flatten() {
                        put_u32(byte_order, &mut out[offset..offset + 4], name);
                        put_u32(byte_order, &mut out[offset + 4..offset + 8], value);
                        offset += 8;
                    }
                    out
                }
                XClientReply::GlxIsDirect { sequence, direct } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    out[8] = u8::from(direct);
                    out
                }
                XClientReply::GlxDrawableAttributes {
                    sequence,
                    attributes,
                } => {
                    let body_len = attributes.len() * 8;
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + body_len];
                    write_reply_header(byte_order, &mut out, sequence, (body_len / 4) as u32);
                    put_u32(byte_order, &mut out[8..12], attributes.len() as u32);
                    let mut offset = 32;
                    for (name, value) in attributes {
                        put_u32(byte_order, &mut out[offset..offset + 4], name);
                        put_u32(byte_order, &mut out[offset + 4..offset + 8], value);
                        offset += 8;
                    }
                    out
                }
                XClientReply::SyncInitialize {
                    sequence,
                    major_version,
                    minor_version,
                } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    out[8] = major_version;
                    out[9] = minor_version;
                    out
                }
                XClientReply::SyncListSystemCounters { sequence } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    put_u32(byte_order, &mut out[8..12], 0);
                    out
                }
                XClientReply::SyncQueryCounter { sequence, value } => {
                    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
                    write_reply_header(byte_order, &mut out, sequence, 0);
                    put_u32(byte_order, &mut out[8..12], (value >> 32) as u32);
                    put_u32(byte_order, &mut out[12..16], value as u32);
                    out
                }
        _ => unreachable!("reply family checked before encoding"),
    })
}
