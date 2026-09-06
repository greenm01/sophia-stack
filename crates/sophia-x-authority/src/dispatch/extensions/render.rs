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

/// The protocol error each picture-request refusal maps to. The extension
/// has error codes of its own and a client's fallback logic keys on which
/// one arrives, so the mapping is total and explicit.
fn render_picture_error_code(error: crate::XRenderPictureError) -> XErrorCode {
    match error {
        crate::XRenderPictureError::Drawable => XErrorCode::BadDrawable,
        crate::XRenderPictureError::IdInUse => XErrorCode::BadIdChoice,
        crate::XRenderPictureError::UnknownFormat => XErrorCode::RenderPictFormat,
        crate::XRenderPictureError::DepthMismatch => XErrorCode::BadMatch,
        crate::XRenderPictureError::InvalidValue => XErrorCode::BadValue,
        crate::XRenderPictureError::RefusedAttribute => XErrorCode::BadImplementation,
        crate::XRenderPictureError::UnknownPicture => XErrorCode::RenderPicture,
    }
}

/// How an operator without an implementation is refused: the Disjoint,
/// Conjoint and PDF ranges are defined by the protocol and withheld here,
/// while the gaps between them are values no version ever defined -- those
/// get the extension's own PictOp error.
fn render_operator_refusal(op: u8) -> XErrorCode {
    match op {
        0x10..=0x2b | 0x30..=0x3e => XErrorCode::BadImplementation,
        _ => XErrorCode::RenderPictOp,
    }
}

fn render_error_output(
    context: XDispatchContext,
    code: XErrorCode,
    resource_id: u32,
    minor_opcode: u8,
) -> XClientOutput {
    XClientOutput::Error(crate::XClientError {
        code,
        sequence: context.sequence,
        resource_id,
        minor_code: u16::from(minor_opcode),
        major_code: context.major_opcode,
    })
}

fn dispatch_render_picture_request(
    context: XDispatchContext,
    request: XWireRequest,
    runtime: &mut XAuthorityRuntime,
) -> XDispatchFamilyResult {
    if !matches!(
        &request,
        XWireRequest::RenderCreatePicture { .. }
            | XWireRequest::RenderChangePicture { .. }
            | XWireRequest::RenderSetPictureClipRectangles { .. }
            | XWireRequest::RenderFreePicture { .. }
            | XWireRequest::RenderFillRectangles { .. }
    ) {
        return Unhandled(request);
    }
    let lifecycle_result = |outcome: Result<(), crate::XRenderPictureError>,
                            resource_id: u32,
                            minor_opcode: u8| {
        XDispatchResult {
            response: None,
            outputs: outcome
                .err()
                .map(|error| {
                    render_error_output(
                        context,
                        render_picture_error_code(error),
                        resource_id,
                        minor_opcode,
                    )
                })
                .into_iter()
                .collect(),
            metadata_candidates: Vec::new(),
        }
    };
    Handled(match request {
        XWireRequest::RenderCreatePicture {
            picture,
            drawable,
            format,
            values,
        } => lifecycle_result(
            runtime.render_create_picture(
                context.namespace,
                picture,
                drawable,
                format,
                &values,
                u64::from(context.sequence),
            ),
            u32::try_from(picture.local.raw()).unwrap_or(0),
            crate::X_RENDER_CREATE_PICTURE_MINOR_OPCODE,
        ),
        XWireRequest::RenderChangePicture { picture, values } => lifecycle_result(
            runtime.render_change_picture(context.namespace, picture, &values),
            u32::try_from(picture.local.raw()).unwrap_or(0),
            crate::X_RENDER_CHANGE_PICTURE_MINOR_OPCODE,
        ),
        XWireRequest::RenderSetPictureClipRectangles {
            picture,
            clip_x_origin,
            clip_y_origin,
            rectangles,
        } => lifecycle_result(
            runtime.render_set_picture_clip_rectangles(
                context.namespace,
                picture,
                clip_x_origin,
                clip_y_origin,
                rectangles,
            ),
            u32::try_from(picture.local.raw()).unwrap_or(0),
            crate::X_RENDER_SET_PICTURE_CLIP_RECTANGLES_MINOR_OPCODE,
        ),
        XWireRequest::RenderFreePicture { picture } => lifecycle_result(
            runtime.render_free_picture(context.namespace, picture),
            u32::try_from(picture.local.raw()).unwrap_or(0),
            crate::X_RENDER_FREE_PICTURE_MINOR_OPCODE,
        ),
        XWireRequest::RenderFillRectangles {
            op,
            picture,
            color,
            rectangles,
        } => {
            let transaction = context.transaction;
            if !crate::software::render_operator_is_implemented(op) {
                return Handled(XDispatchResult {
                    response: None,
                    outputs: vec![render_error_output(
                        context,
                        render_operator_refusal(op),
                        0,
                        crate::X_RENDER_FILL_RECTANGLES_MINOR_OPCODE,
                    )],
                    metadata_candidates: Vec::new(),
                });
            }
            match runtime.render_apply_fill_rectangles(
                transaction,
                context.namespace,
                op,
                picture,
                color,
                &rectangles,
            ) {
                Ok(response) => {
                    let outputs =
                        if let XAuthorityResponseOutcome::Rejected(error) = response.outcome {
                            vec![XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u16::from(crate::X_RENDER_FILL_RECTANGLES_MINOR_OPCODE),
                                u32::try_from(picture.local.raw()).unwrap_or(0),
                            ))]
                        } else {
                            Vec::new()
                        };
                    XDispatchResult {
                        response: Some(response),
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                Err(error) => XDispatchResult {
                    response: Some(XAuthorityResponsePacket::rejected(
                        transaction,
                        XAuthorityRuntimeError::InvalidResource,
                    )),
                    outputs: vec![render_error_output(
                        context,
                        render_picture_error_code(error),
                        u32::try_from(picture.local.raw()).unwrap_or(0),
                        crate::X_RENDER_FILL_RECTANGLES_MINOR_OPCODE,
                    )],
                    metadata_candidates: Vec::new(),
                },
            }
        }
        other => return Unhandled(other),
    })
}
