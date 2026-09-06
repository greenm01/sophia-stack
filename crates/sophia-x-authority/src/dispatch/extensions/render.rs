/// The protocol version each RENDER minor entered at, as a minor-version
/// number, or `None` for a value no version of the protocol ever defined.
///
/// This is what splits the two refusal tiers: a minor gated at or below the
/// advertised version answers `BadImplementation` -- the request exists here
/// and is not offered -- while one gated above it answers `BadRequest`,
/// because a genuine server of the advertised version had no dispatch entry
/// for it at all.
fn render_minor_version_gate(minor_opcode: u8) -> Option<u32> {
    match minor_opcode {
        // Minor 16 was reserved for a Transform request that never entered
        // the protocol.
        16 => None,
        crate::X_RENDER_QUERY_PICT_INDEX_VALUES_MINOR_OPCODE => Some(7),
        crate::X_RENDER_CREATE_CURSOR_MINOR_OPCODE => Some(5),
        crate::X_RENDER_SET_PICTURE_TRANSFORM_MINOR_OPCODE
        | crate::X_RENDER_QUERY_FILTERS_MINOR_OPCODE
        | crate::X_RENDER_SET_PICTURE_FILTER_MINOR_OPCODE => Some(6),
        crate::X_RENDER_CREATE_ANIM_CURSOR_MINOR_OPCODE => Some(8),
        crate::X_RENDER_ADD_TRAPS_MINOR_OPCODE => Some(9),
        crate::X_RENDER_CREATE_SOLID_FILL_MINOR_OPCODE
        | crate::X_RENDER_CREATE_LINEAR_GRADIENT_MINOR_OPCODE
        | crate::X_RENDER_CREATE_RADIAL_GRADIENT_MINOR_OPCODE
        | crate::X_RENDER_CREATE_CONICAL_GRADIENT_MINOR_OPCODE => Some(10),
        minor if minor <= crate::X_RENDER_LAST_MINOR_OPCODE => Some(0),
        _ => None,
    }
}

fn dispatch_render_request(
    context: XDispatchContext,
    request: XWireRequest,
    _runtime: &mut XAuthorityRuntime,
) -> XDispatchFamilyResult {
    if !matches!(
        &request,
        XWireRequest::RenderQueryVersion { .. }
            | XWireRequest::RenderQueryPictFormats
            | XWireRequest::RenderUnimplemented { .. }
    ) {
        return Unhandled(request);
    }
    Handled(match request {
        // The answer is the lower of the two versions, and the server's side
        // of that comparison is the constant that moves only when the
        // requests behind the next version answer.
        XWireRequest::RenderQueryVersion { major, minor } => {
            let (major_version, minor_version) = if major == crate::X_RENDER_MAJOR_VERSION {
                (major, minor.min(crate::X_RENDER_MINOR_VERSION))
            } else {
                (crate::X_RENDER_MAJOR_VERSION, crate::X_RENDER_MINOR_VERSION)
            };
            XDispatchResult {
                response: None,
                outputs: vec![XClientOutput::Reply(XClientReply::RenderQueryVersion {
                    sequence: context.sequence,
                    major_version,
                    minor_version,
                })],
                metadata_candidates: Vec::new(),
            }
        }
        // The reply is a constant: the four formats are the four pixel
        // layouts the server can represent, so the encoder owns the table and
        // dispatch carries only the sequence.
        XWireRequest::RenderQueryPictFormats => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::RenderQueryPictFormats {
                sequence: context.sequence,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::RenderUnimplemented { minor_opcode } => {
            let code = match render_minor_version_gate(minor_opcode) {
                Some(gate) if gate <= crate::X_RENDER_MINOR_VERSION => {
                    XErrorCode::BadImplementation
                }
                _ => XErrorCode::BadRequest,
            };
            XDispatchResult {
                response: None,
                outputs: vec![XClientOutput::Error(crate::XClientError {
                    code,
                    sequence: context.sequence,
                    resource_id: 0,
                    minor_code: u16::from(minor_opcode),
                    major_code: context.major_opcode,
                })],
                metadata_candidates: Vec::new(),
            }
        }
        other => return Unhandled(other),
    })
}
