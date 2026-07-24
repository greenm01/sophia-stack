fn encode_font_info_reply(
    byte_order: XByteOrder,
    sequence: u16,
    font_ascent: i16,
    font_descent: i16,
    name: Option<&[u8]>,
) -> Vec<u8> {
    let name = name.unwrap_or_default();
    let padded_name_len = padded_len(name.len());
    let mut out = vec![0; 60 + padded_name_len];
    write_reply_header(
        byte_order,
        &mut out[..X_CLIENT_OUTPUT_RECORD_LEN],
        sequence,
        u32::try_from(7 + (padded_name_len / 4)).unwrap_or(7),
    );
    out[1] = u8::try_from(name.len()).unwrap_or(0);
    // min_bounds charinfo
    put_i16(byte_order, &mut out[8..10], 0);
    put_i16(byte_order, &mut out[10..12], 8);
    put_i16(byte_order, &mut out[12..14], 8);
    put_i16(byte_order, &mut out[14..16], 8);
    put_i16(byte_order, &mut out[16..18], 2);
    put_u16(byte_order, &mut out[18..20], 0);
    // max_bounds charinfo
    put_i16(byte_order, &mut out[24..26], 0);
    put_i16(byte_order, &mut out[26..28], 8);
    put_i16(byte_order, &mut out[28..30], 8);
    put_i16(byte_order, &mut out[30..32], 8);
    put_i16(byte_order, &mut out[32..34], 2);
    put_u16(byte_order, &mut out[34..36], 0);
    put_u16(byte_order, &mut out[40..42], 0);
    put_u16(byte_order, &mut out[42..44], 255);
    put_u16(byte_order, &mut out[44..46], 0);
    put_u16(byte_order, &mut out[46..48], 0);
    out[48] = 0;
    out[49] = 0;
    out[50] = 0;
    out[51] = 1;
    put_i16(byte_order, &mut out[52..54], font_ascent);
    put_i16(byte_order, &mut out[54..56], font_descent);
    put_u32(byte_order, &mut out[56..60], 0);
    out[60..60 + name.len()].copy_from_slice(name);
    out
}

fn write_event_header(
    byte_order: XByteOrder,
    out: &mut [u8],
    event_type: u8,
    detail: u8,
    sequence: u16,
) {
    out[0] = event_type;
    out[1] = detail;
    put_u16(byte_order, &mut out[2..4], sequence);
}

fn write_reply_header(byte_order: XByteOrder, out: &mut [u8], sequence: u16, length_units: u32) {
    out[0] = 1;
    put_u16(byte_order, &mut out[2..4], sequence);
    put_u32(byte_order, &mut out[4..8], length_units);
}

fn put_resource(byte_order: XByteOrder, out: &mut [u8], resource: XResourceId) {
    put_u32(byte_order, out, raw_xid(resource));
}

fn raw_xid(resource: XResourceId) -> u32 {
    u32::try_from(resource.local.raw()).unwrap_or(0)
}

fn put_u16(byte_order: XByteOrder, out: &mut [u8], value: u16) {
    match byte_order {
        XByteOrder::LittleEndian => out.copy_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.copy_from_slice(&value.to_be_bytes()),
    }
}

fn put_i16(byte_order: XByteOrder, out: &mut [u8], value: i16) {
    match byte_order {
        XByteOrder::LittleEndian => out.copy_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.copy_from_slice(&value.to_be_bytes()),
    }
}

fn push_u32(byte_order: XByteOrder, out: &mut Vec<u8>, value: u32) {
    let mut bytes = [0; 4];
    put_u32(byte_order, &mut bytes, value);
    out.extend_from_slice(&bytes);
}

fn push_u16(byte_order: XByteOrder, out: &mut Vec<u8>, value: u16) {
    let mut bytes = [0; 2];
    put_u16(byte_order, &mut bytes, value);
    out.extend_from_slice(&bytes);
}

fn push_i64(byte_order: XByteOrder, out: &mut Vec<u8>, value: i64) {
    match byte_order {
        XByteOrder::LittleEndian => out.extend_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.extend_from_slice(&value.to_be_bytes()),
    }
}

fn put_u32(byte_order: XByteOrder, out: &mut [u8], value: u32) {
    match byte_order {
        XByteOrder::LittleEndian => out.copy_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.copy_from_slice(&value.to_be_bytes()),
    }
}

fn put_u64(byte_order: XByteOrder, out: &mut [u8], value: u64) {
    let bytes = match byte_order {
        XByteOrder::LittleEndian => value.to_le_bytes(),
        XByteOrder::BigEndian => value.to_be_bytes(),
    };
    out.copy_from_slice(&bytes);
}
