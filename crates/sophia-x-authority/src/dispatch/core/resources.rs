fn dispatch_core_resource_request(
    context: XDispatchContext,
    request: XWireRequest,
    runtime: &mut XAuthorityRuntime,
    _atoms: &mut XAtomTable,
    _properties: &mut XPropertyTable,
) -> XDispatchFamilyResult {
    if !matches!(
        &request,
        XWireRequest::CreateGraphicsContext { .. }
            | XWireRequest::ChangeGraphicsContext { .. }
            | XWireRequest::SetClipRectangles { .. }
            | XWireRequest::FreeGraphicsContext { .. }
            | XWireRequest::ClearArea { .. }
            | XWireRequest::OpenFont { .. }
            | XWireRequest::CloseFont { .. }
            | XWireRequest::QueryFont { .. }
            | XWireRequest::CreateCursor { .. }
            | XWireRequest::CreateGlyphCursor { .. }
            | XWireRequest::FreeCursor { .. }
            | XWireRequest::RecolorCursor { .. }
            | XWireRequest::ListFonts { .. }
            | XWireRequest::ListFontsWithInfo { .. }
            | XWireRequest::CreatePixmap { .. }
            | XWireRequest::FreePixmap { .. }
    ) {
        return Unhandled(request);
    }
    Handled(match request {
        XWireRequest::CreateGraphicsContext {
            gc,
            drawable,
            values,
        } => {
            if runtime.resource_id_in_use(gc) {
                return Handled(core_resource_bad_id_choice(context, gc));
            }
            if let Err(error) = runtime.validate_drawable_access(context.namespace, drawable) {
                return Handled(core_resource_validation_error(
                    context,
                    error,
                    XErrorCode::BadDrawable,
                    drawable,
                ));
            }
            if let Some(font) = values.font
                && let Err(error) = runtime.validate_font_access(context.namespace, font)
            {
                return Handled(core_resource_validation_error(
                    context,
                    error,
                    XErrorCode::BadFont,
                    font,
                ));
            }
            let outputs = runtime
                .create_graphics_context(context.namespace, gc, drawable, values)
                .err()
                .map(|error| {
                    XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(gc.local.raw()).unwrap_or(0),
                    ))
                })
                .into_iter()
                .collect();
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::ChangeGraphicsContext {
            gc,
            value_mask,
            values,
        } => {
            if let Err(error) = runtime.graphics_context_values(context.namespace, gc) {
                return Handled(core_resource_validation_error(
                    context,
                    error,
                    XErrorCode::BadGraphicsContext,
                    gc,
                ));
            }
            if value_mask & (1 << 14) != 0 {
                let font = values.font.unwrap_or(XResourceId::new(0, 1));
                if let Err(error) = runtime.validate_font_access(context.namespace, font) {
                    return Handled(core_resource_validation_error(
                        context,
                        error,
                        XErrorCode::BadFont,
                        font,
                    ));
                }
            }
            let outputs = runtime
                .change_graphics_context(context.namespace, gc, value_mask, values)
                .err()
                .map(|error| {
                    XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(gc.local.raw()).unwrap_or(0),
                    ))
                })
                .into_iter()
                .collect();
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::SetClipRectangles { gc, rectangles } => {
            if let Err(error) = runtime.set_graphics_context_clip_rectangles(
                context.namespace,
                gc,
                rectangles,
            ) {
                return Handled(core_resource_validation_error(
                    context,
                    error,
                    XErrorCode::BadGraphicsContext,
                    gc,
                ));
            }
            XDispatchResult {
                response: None,
                outputs: Vec::new(),
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::FreeGraphicsContext { gc } => {
            if let Err(error) = runtime.free_graphics_context(context.namespace, gc) {
                return Handled(core_resource_validation_error(
                    context,
                    error,
                    XErrorCode::BadGraphicsContext,
                    gc,
                ));
            }
            XDispatchResult {
                response: None,
                outputs: Vec::new(),
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::ClearArea {
            window,
            x,
            y,
            width,
            height,
            ..
        } => {
            let transaction = TransactionId::from_raw(u64::from(context.sequence));
            let geometry = runtime.window_geometry(context.namespace, window).ok();
            let clear_width = if width == 0 {
                geometry
                    .map(|geometry| geometry.width.saturating_sub(i32::from(x)).max(0))
                    .unwrap_or(0)
            } else {
                i32::from(width)
            };
            let clear_height = if height == 0 {
                geometry
                    .map(|geometry| geometry.height.saturating_sub(i32::from(y)).max(0))
                    .unwrap_or(0)
            } else {
                i32::from(height)
            };
            let response = match runtime.window_background_pixel(context.namespace, window) {
                Ok(pixel) => runtime.apply_clear_with_pixel(
                    transaction,
                    context.namespace,
                    window,
                    Region::single(Rect {
                        x: i32::from(x),
                        y: i32::from(y),
                        width: clear_width,
                        height: clear_height,
                    }),
                    pixel,
                ),
                Err(error) => XAuthorityResponsePacket::rejected(transaction, error),
            };
            let outputs = if let XAuthorityResponseOutcome::Rejected(error) = response.outcome {
                vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(window.local.raw()).unwrap_or(0),
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
        XWireRequest::OpenFont { font, name } => {
            if runtime.resource_id_in_use(font) {
                return Handled(core_resource_bad_id_choice(context, font));
            }
            let outputs = match XFontFace::from_name(&name) {
                Some(face) => match runtime.open_font_face(
                    context.namespace,
                    font,
                    face,
                    u64::from(context.sequence),
                ) {
                    Ok(()) => Vec::new(),
                    Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(font.local.raw()).unwrap_or(0),
                    ))],
                },
                None => {
                    tracing::debug!(
                        font_name = %name,
                        font = font.local.raw(),
                        "core OpenFont rejected an unsupported font name"
                    );
                    vec![XClientOutput::Error(crate::XClientError {
                        code: XErrorCode::BadName,
                        sequence: context.sequence,
                        resource_id: u32::try_from(font.local.raw()).unwrap_or(0),
                        minor_code: 0,
                        major_code: context.major_opcode,
                    })]
                }
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::CloseFont { font } => {
            let outputs = match runtime.close_font(context.namespace, font) {
                Ok(()) => Vec::new(),
                Err(error) => {
                    core_resource_validation_error(context, error, XErrorCode::BadFont, font)
                        .outputs
                }
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::QueryFont { font } => {
            let output = match runtime.fontable_face(context.namespace, font) {
                Ok(_) => XClientOutput::Reply(XClientReply::QueryFont {
                    sequence: context.sequence,
                    font_ascent: i16::try_from(X_FIXED_6X13_ASCENT).unwrap_or(i16::MAX),
                    font_descent: i16::try_from(X_FIXED_6X13_DESCENT).unwrap_or(i16::MAX),
                }),
                Err(error) => {
                    core_resource_validation_error(context, error, XErrorCode::BadFont, font)
                        .outputs
                        .into_iter()
                        .next()
                        .expect("resource error has one output")
                }
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::CreateCursor {
            cursor,
            source,
            mask,
        } => {
            if runtime.resource_id_in_use(cursor) {
                return Handled(core_resource_bad_id_choice(context, cursor));
            }
            let result = runtime
                .validate_drawable_access(context.namespace, source)
                .and_then(|()| {
                    mask.map_or(Ok(()), |mask| {
                        runtime.validate_drawable_access(context.namespace, mask)
                    })
                })
                .and_then(|()| {
                    runtime.create_cursor(context.namespace, cursor, u64::from(context.sequence))
                });
            let outputs = result
                .err()
                .map(|error| {
                    XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(cursor.local.raw()).unwrap_or(0),
                    ))
                })
                .into_iter()
                .collect();
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::CreateGlyphCursor {
            cursor,
            source_font,
            mask_font,
        } => {
            if runtime.resource_id_in_use(cursor) {
                return Handled(core_resource_bad_id_choice(context, cursor));
            }
            let outputs = if let Err(error) =
                runtime.validate_font_access(context.namespace, source_font)
            {
                vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(source_font.local.raw()).unwrap_or(0),
                ))]
            } else if let Some(mask_font) = mask_font {
                if let Err(error) = runtime.validate_font_access(context.namespace, mask_font) {
                    vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(mask_font.local.raw()).unwrap_or(0),
                    ))]
                } else {
                    match runtime.create_cursor(
                        context.namespace,
                        cursor,
                        u64::from(context.sequence),
                    ) {
                        Ok(()) => Vec::new(),
                        Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(cursor.local.raw()).unwrap_or(0),
                        ))],
                    }
                }
            } else {
                match runtime.create_cursor(context.namespace, cursor, u64::from(context.sequence))
                {
                    Ok(()) => Vec::new(),
                    Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(cursor.local.raw()).unwrap_or(0),
                    ))],
                }
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::FreeCursor { cursor } => {
            let outputs = match runtime.free_cursor(context.namespace, cursor) {
                Ok(()) => Vec::new(),
                Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(cursor.local.raw()).unwrap_or(0),
                ))],
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::RecolorCursor { cursor } => {
            let outputs = match runtime.validate_cursor_access(context.namespace, cursor) {
                Ok(()) => Vec::new(),
                Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(cursor.local.raw()).unwrap_or(0),
                ))],
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::ListFonts { max_names, .. } => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::ListFonts {
                sequence: context.sequence,
                names: if max_names == 0 {
                    Vec::new()
                } else {
                    vec!["fixed".to_owned()]
                },
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::ListFontsWithInfo { max_names, .. } => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::ListFontsWithInfo {
                sequence: context.sequence,
                names: if max_names == 0 {
                    Vec::new()
                } else {
                    vec!["fixed".to_owned()]
                },
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::CreatePixmap {
            depth,
            pixmap,
            drawable,
            width,
            height,
        } => {
            if runtime.resource_id_in_use(pixmap) {
                return Handled(core_resource_bad_id_choice(context, pixmap));
            }
            let outputs = if let Err(error) =
                runtime.validate_drawable_access(context.namespace, drawable)
            {
                return Handled(core_resource_validation_error(
                    context,
                    error,
                    XErrorCode::BadDrawable,
                    drawable,
                ));
            } else if width == 0 || height == 0 {
                vec![XClientOutput::Error(crate::XClientError {
                    code: XErrorCode::BadValue,
                    sequence: context.sequence,
                    resource_id: 0,
                    minor_code: 0,
                    major_code: context.major_opcode,
                })]
            } else if crate::x11_pixmap_format(depth).is_none() {
                    vec![XClientOutput::Error(crate::XClientError {
                        code: XErrorCode::BadValue,
                        sequence: context.sequence,
                        resource_id: u32::from(depth),
                        minor_code: 0,
                        major_code: context.major_opcode,
                    })]
                } else if let Err(error) = runtime.create_pixmap(
                    context.namespace,
                    pixmap,
                    sophia_protocol::Size {
                        width: i32::from(width),
                        height: i32::from(height),
                    },
                    depth,
                    u64::from(context.sequence),
                ) {
                    vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(pixmap.local.raw()).unwrap_or(0),
                    ))]
                } else {
                    Vec::new()
                };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::FreePixmap { pixmap } => {
            if let Err(error) = runtime.free_pixmap(context.namespace, pixmap) {
                return Handled(core_resource_validation_error(
                    context,
                    error,
                    XErrorCode::BadPixmap,
                    pixmap,
                ));
            }
            XDispatchResult {
                response: None,
                outputs: Vec::new(),
                metadata_candidates: Vec::new(),
            }
        }
        _ => unreachable!("request family checked before dispatch"),
    })
}

fn core_resource_bad_id_choice(
    context: XDispatchContext,
    resource: XResourceId,
) -> XDispatchResult {
    XDispatchResult {
        response: None,
        outputs: vec![XClientOutput::Error(crate::XClientError {
            code: XErrorCode::BadIdChoice,
            sequence: context.sequence,
            resource_id: u32::try_from(resource.local.raw()).unwrap_or(0),
            minor_code: 0,
            major_code: context.major_opcode,
        })],
        metadata_candidates: Vec::new(),
    }
}

fn core_resource_validation_error(
    context: XDispatchContext,
    runtime_error: XAuthorityRuntimeError,
    missing_resource_code: XErrorCode,
    resource: XResourceId,
) -> XDispatchResult {
    let resource_id = u32::try_from(resource.local.raw()).unwrap_or(0);
    let code = match runtime_error {
        XAuthorityRuntimeError::InvalidResource
        | XAuthorityRuntimeError::UnknownResource
        | XAuthorityRuntimeError::WrongResourceKind
        | XAuthorityRuntimeError::InvalidSurface => missing_resource_code,
        _ => {
            x_error_from_runtime(
                runtime_error,
                context.sequence,
                context.major_opcode,
                resource_id,
            )
            .code
        }
    };
    XDispatchResult {
        response: None,
        outputs: vec![XClientOutput::Error(crate::XClientError {
            code,
            sequence: context.sequence,
            resource_id,
            minor_code: 0,
            major_code: context.major_opcode,
        })],
        metadata_candidates: Vec::new(),
    }
}
