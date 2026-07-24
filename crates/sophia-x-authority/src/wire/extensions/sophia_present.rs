fn decode_sophia_present(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    if bytes[1] != X_SOPHIA_PRESENT_PIXMAP_MINOR_OPCODE {
        return Err(XWireParseError::UnknownOpcode(bytes[0]));
    }
    require_exact_len(
        X_SOPHIA_PRESENT_MAJOR_OPCODE,
        X_SOPHIA_PRESENT_PIXMAP_REQ_LEN,
        bytes.len(),
    )?;
    let window = XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1);
    let damage = Region::single(Rect {
        x: i32::from(context.byte_order.i16(&bytes[12..14])),
        y: i32::from(context.byte_order.i16(&bytes[14..16])),
        width: i32::from(context.byte_order.u16(&bytes[16..18])),
        height: i32::from(context.byte_order.u16(&bytes[18..20])),
    });
    Ok(XWireRequest::Authority(XAuthorityRequestPacket {
        transaction: context.transaction,
        namespace: context.namespace,
        kind: XAuthorityRequestKind::PresentPixmap {
            window,
            pixmap: context.byte_order.u32(&bytes[8..12]),
            damage,
            previous_committed_generation: context.byte_order.u64(&bytes[20..28]),
            timeout_msec: context.byte_order.u32(&bytes[28..32]),
        },
    }))
}
