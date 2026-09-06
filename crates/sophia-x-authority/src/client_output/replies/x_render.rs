/// One direct picture format for the `QueryPictFormats` table: identifier,
/// depth, and the shift and 16-bit-wide mask of each channel in client pixel
/// order.
struct XRenderDirectFormat {
    id: u32,
    depth: u8,
    red_shift: u16,
    red_mask: u16,
    green_shift: u16,
    green_mask: u16,
    blue_shift: u16,
    blue_mask: u16,
    alpha_shift: u16,
    alpha_mask: u16,
}

/// The four formats Sophia offers, one per representable pixel layout. The
/// channel positions must agree with `x_true_color_visual`: a client binds a
/// picture format to a visual and expects the bytes it drew through core
/// requests to mean the same thing through RENDER.
const X_RENDER_DIRECT_FORMATS: [XRenderDirectFormat; 4] = [
    XRenderDirectFormat {
        id: X_RENDER_FORMAT_ARGB32,
        depth: 32,
        red_shift: 16,
        red_mask: 0xff,
        green_shift: 8,
        green_mask: 0xff,
        blue_shift: 0,
        blue_mask: 0xff,
        alpha_shift: 24,
        alpha_mask: 0xff,
    },
    XRenderDirectFormat {
        id: X_RENDER_FORMAT_RGB24,
        depth: 24,
        red_shift: 16,
        red_mask: 0xff,
        green_shift: 8,
        green_mask: 0xff,
        blue_shift: 0,
        blue_mask: 0xff,
        alpha_shift: 0,
        alpha_mask: 0,
    },
    XRenderDirectFormat {
        id: X_RENDER_FORMAT_A8,
        depth: 8,
        red_shift: 0,
        red_mask: 0,
        green_shift: 0,
        green_mask: 0,
        blue_shift: 0,
        blue_mask: 0,
        alpha_shift: 0,
        alpha_mask: 0xff,
    },
    XRenderDirectFormat {
        id: X_RENDER_FORMAT_A1,
        depth: 1,
        red_shift: 0,
        red_mask: 0,
        green_shift: 0,
        green_mask: 0,
        blue_shift: 0,
        blue_mask: 0,
        alpha_shift: 0,
        alpha_mask: 0x1,
    },
];

fn encode_x_render_reply(
    byte_order: XByteOrder,
    reply: XClientReply,
) -> Result<Vec<u8>, XClientReply> {
    if !matches!(
        &reply,
        XClientReply::RenderQueryVersion { .. } | XClientReply::RenderQueryPictFormats { .. }
    ) {
        return Err(reply);
    }
    Ok(match reply {
        XClientReply::RenderQueryVersion {
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
        XClientReply::RenderQueryPictFormats { sequence } => {
            // Four 28-byte PICTFORMINFOs, then one PICTSCREEN holding the two
            // depths that carry a visual, each with its one PICTVISUAL. The
            // subpixel list is empty: subpixel geometry entered at 0.6, above
            // what is advertised, and nothing here has measured it anyway.
            let formats_len = X_RENDER_DIRECT_FORMATS.len() * 28;
            let screen_len = 8 + 2 * (8 + 8);
            let payload_len = formats_len + screen_len;
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + payload_len];
            write_reply_header(
                byte_order,
                &mut out,
                sequence,
                u32::try_from(payload_len / 4).unwrap_or(0),
            );
            put_u32(
                byte_order,
                &mut out[8..12],
                u32::try_from(X_RENDER_DIRECT_FORMATS.len()).unwrap_or(0),
            );
            put_u32(byte_order, &mut out[12..16], 1);
            put_u32(byte_order, &mut out[16..20], 2);
            put_u32(byte_order, &mut out[20..24], 2);
            put_u32(byte_order, &mut out[24..28], 0);
            let mut offset = X_CLIENT_OUTPUT_RECORD_LEN;
            for format in &X_RENDER_DIRECT_FORMATS {
                put_u32(byte_order, &mut out[offset..offset + 4], format.id);
                out[offset + 4] = X_RENDER_PICT_TYPE_DIRECT;
                out[offset + 5] = format.depth;
                put_u16(byte_order, &mut out[offset + 8..offset + 10], format.red_shift);
                put_u16(byte_order, &mut out[offset + 10..offset + 12], format.red_mask);
                put_u16(
                    byte_order,
                    &mut out[offset + 12..offset + 14],
                    format.green_shift,
                );
                put_u16(
                    byte_order,
                    &mut out[offset + 14..offset + 16],
                    format.green_mask,
                );
                put_u16(
                    byte_order,
                    &mut out[offset + 16..offset + 18],
                    format.blue_shift,
                );
                put_u16(
                    byte_order,
                    &mut out[offset + 18..offset + 20],
                    format.blue_mask,
                );
                put_u16(
                    byte_order,
                    &mut out[offset + 20..offset + 22],
                    format.alpha_shift,
                );
                put_u16(
                    byte_order,
                    &mut out[offset + 22..offset + 24],
                    format.alpha_mask,
                );
                // The colormap field stays zero: direct formats have none.
                offset += 28;
            }
            // PICTSCREEN: two depths, and the fallback a client should use
            // when it has no better information -- the default visual's
            // opaque format.
            put_u32(byte_order, &mut out[offset..offset + 4], 2);
            put_u32(
                byte_order,
                &mut out[offset + 4..offset + 8],
                X_RENDER_FORMAT_RGB24,
            );
            offset += 8;
            for (depth, visual, format) in [
                (24u8, X_SETUP_DEFAULT_VISUAL, X_RENDER_FORMAT_RGB24),
                (32u8, X_SETUP_ARGB_VISUAL, X_RENDER_FORMAT_ARGB32),
            ] {
                out[offset] = depth;
                put_u16(byte_order, &mut out[offset + 2..offset + 4], 1);
                offset += 8;
                put_u32(byte_order, &mut out[offset..offset + 4], visual);
                put_u32(byte_order, &mut out[offset + 4..offset + 8], format);
                offset += 8;
            }
            debug_assert_eq!(offset, out.len());
            out
        }
        other => return Err(other),
    })
}
