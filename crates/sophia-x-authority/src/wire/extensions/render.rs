fn decode_render(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    match bytes[1] {
        X_RENDER_QUERY_VERSION_MINOR_OPCODE => {
            require_exact_len(
                X_RENDER_MAJOR_OPCODE,
                X_RENDER_QUERY_VERSION_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::RenderQueryVersion {
                major: context.byte_order.u32(&bytes[4..8]),
                minor: context.byte_order.u32(&bytes[8..12]),
            })
        }
        X_RENDER_QUERY_PICT_FORMATS_MINOR_OPCODE => {
            require_exact_len(
                X_RENDER_MAJOR_OPCODE,
                X_RENDER_QUERY_PICT_FORMATS_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::RenderQueryPictFormats)
        }
        // Decoded so the refusal can name the request. RENDER has thirty-six
        // minors and this server implements a subset; a parse rejection would
        // tell a client only that the extension exists, not which request it
        // was denied.
        minor_opcode => Ok(XWireRequest::RenderUnimplemented { minor_opcode }),
    }
}
