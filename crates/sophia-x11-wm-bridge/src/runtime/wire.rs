use super::*;

pub(super) fn write_packet(
    stream: &mut UnixStream,
    bytes: &[u8],
) -> Result<(), BridgeRuntimeError> {
    stream.write_all(bytes).map_err(|error| {
        BridgeRuntimeError::new(format!("failed to write legacy WM packet: {error}"))
    })?;
    stream.flush().map_err(|error| {
        BridgeRuntimeError::new(format!("failed to flush legacy WM packet: {error}"))
    })
}

pub(super) fn write_x11_error(
    stream: &mut UnixStream,
    sequence: u16,
    code: u8,
    value: u32,
    major_opcode: u8,
) -> Result<(), BridgeRuntimeError> {
    let mut packet = vec![0, code];
    push_u16(&mut packet, sequence);
    push_u32(&mut packet, value);
    push_u16(&mut packet, 0);
    packet.push(major_opcode);
    packet.resize(32, 0);
    write_packet(stream, &packet)
}

pub(super) fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .unwrap_or(0)
}

pub(super) fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .unwrap_or(0)
}

pub(super) fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn push_i16(bytes: &mut Vec<u8>, value: i16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn read_wm_session_descriptor(
    stream: &mut UnixStream,
) -> Result<WmSessionDescriptor, BridgeRuntimeError> {
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    stream.read_exact(&mut header).map_err(|error| {
        BridgeRuntimeError::new(format!(
            "failed to read WM session descriptor header: {error}"
        ))
    })?;
    let payload_len = u32::from_le_bytes(header[16..20].try_into().expect("fixed header")) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(BridgeRuntimeError::new(format!(
            "WM session descriptor payload too large: {payload_len}"
        )));
    }
    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    stream
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .map_err(|error| {
            BridgeRuntimeError::new(format!(
                "failed to read WM session descriptor payload: {error}"
            ))
        })?;
    decode_wm_session_descriptor_frame(&frame).map_err(|error| {
        BridgeRuntimeError::new(format!("failed to decode WM session descriptor: {error:?}"))
    })
}
