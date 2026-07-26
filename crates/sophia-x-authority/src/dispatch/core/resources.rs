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
                    let outputs = runtime
                        .set_graphics_context_clip_rectangles(context.namespace, gc, rectangles)
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
                XWireRequest::FreeGraphicsContext { gc } => {
                    let outputs = runtime
                        .free_graphics_context(context.namespace, gc)
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
                XWireRequest::OpenFont { font, .. } => {
                    let outputs =
                        match runtime.open_font(context.namespace, font, u64::from(context.sequence)) {
                            Ok(()) => Vec::new(),
                            Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(font.local.raw()).unwrap_or(0),
                            ))],
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
                        Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(font.local.raw()).unwrap_or(0),
                        ))],
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::QueryFont { font } => {
                    let output = match runtime.validate_font_access(context.namespace, font) {
                        Ok(()) => XClientOutput::Reply(XClientReply::QueryFont {
                            sequence: context.sequence,
                            font_ascent: 8,
                            font_descent: 2,
                        }),
                        Err(error) => XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(font.local.raw()).unwrap_or(0),
                        )),
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
                    pixmap,
                    drawable,
                    width,
                    height,
                    ..
                } => {
                    let outputs =
                        if let Err(error) = runtime.validate_drawable_access(context.namespace, drawable) {
                            vec![XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(drawable.local.raw()).unwrap_or(0),
                            ))]
                        } else if let Err(error) =
                            runtime.create_pixmap(
                                context.namespace,
                                pixmap,
                                sophia_protocol::Size {
                                    width: i32::from(width),
                                    height: i32::from(height),
                                },
                                u64::from(context.sequence),
                            )
                        {
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
                    let outputs = match runtime.free_pixmap(context.namespace, pixmap) {
                        Ok(_) => Vec::new(),
                        Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(pixmap.local.raw()).unwrap_or(0),
                        ))],
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
        _ => unreachable!("request family checked before dispatch"),
    })
}
