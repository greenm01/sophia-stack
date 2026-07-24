fn dispatch_core_drawing_request(
    context: XDispatchContext,
    request: XWireRequest,
    runtime: &mut XAuthorityRuntime,
    _atoms: &mut XAtomTable,
    _properties: &mut XPropertyTable,
) -> XDispatchFamilyResult {
    if !matches!(
        &request,
            XWireRequest::PolyFillRectangle { .. }
            | XWireRequest::CopyArea { .. }
            | XWireRequest::PolyLine { .. }
            | XWireRequest::PolySegment { .. }
            | XWireRequest::PolyFillArc { .. }
            | XWireRequest::PolyText8 { .. }
            | XWireRequest::ImageText8 { .. }
            | XWireRequest::FillPoly { .. }
            | XWireRequest::PutImage { .. }
    ) {
        return Unhandled(request);
    }
    Handled(match request {
                XWireRequest::PolyFillRectangle {
                    drawable,
                    gc,
                    rectangles,
                } => {
                    let transaction = TransactionId::from_raw(u64::from(context.sequence));
                    if runtime
                        .validate_pixmap_access(context.namespace, drawable)
                        .is_ok()
                    {
                        return Handled(XDispatchResult {
                            response: Some(XAuthorityResponsePacket::accepted(transaction)),
                            outputs: Vec::new(),
                            metadata_candidates: Vec::new(),
                        });
                    }
                    let mut damage = Region::empty();
                    for rectangle in rectangles {
                        damage.push(rectangle);
                    }
                    let response = match runtime.graphics_context_values(context.namespace, gc) {
                        Ok(values) => runtime.apply_core_draw_with_gc(
                            transaction,
                            context.namespace,
                            drawable,
                            damage,
                            &values,
                        ),
                        Err(error) => XAuthorityResponsePacket::rejected(transaction, error),
                    };
                    let outputs = if let XAuthorityResponseOutcome::Rejected(error) = response.outcome {
                        vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(drawable.local.raw()).unwrap_or(0),
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
                XWireRequest::CopyArea {
                    source,
                    destination,
                    gc,
                    src_x,
                    src_y,
                    dst_x,
                    dst_y,
                    width,
                    height,
                } => {
                    let transaction = TransactionId::from_raw(u64::from(context.sequence));
                    let response = match runtime.graphics_context_values(context.namespace, gc) {
                        Ok(values) => runtime.apply_copy_area_with_gc(
                            transaction,
                            context.namespace,
                            source,
                            destination,
                            src_x,
                            src_y,
                            dst_x,
                            dst_y,
                            width,
                            height,
                            &values,
                        ),
                        Err(error) => XAuthorityResponsePacket::rejected(transaction, error),
                    };
                    let outputs = if let XAuthorityResponseOutcome::Rejected(error) = response.outcome {
                        vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(destination.local.raw()).unwrap_or(0),
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
                XWireRequest::PolyLine {
                    drawable,
                    gc,
                    points,
                } => {
                    let transaction = TransactionId::from_raw(u64::from(context.sequence));
                    if points.len() < 2
                        || runtime
                            .validate_pixmap_access(context.namespace, drawable)
                            .is_ok()
                    {
                        return Handled(XDispatchResult {
                            response: Some(XAuthorityResponsePacket::accepted(transaction)),
                            outputs: Vec::new(),
                            metadata_candidates: Vec::new(),
                        });
                    }
                    let response = match runtime.graphics_context_values(context.namespace, gc) {
                        Ok(values) => runtime.apply_line_draw(
                            transaction,
                            context.namespace,
                            drawable,
                            &points,
                            &values,
                        ),
                        Err(error) => XAuthorityResponsePacket::rejected(transaction, error),
                    };
                    let outputs = if let XAuthorityResponseOutcome::Rejected(error) = response.outcome {
                        vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(drawable.local.raw()).unwrap_or(0),
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
                XWireRequest::PolySegment {
                    drawable, damage, ..
                } => {
                    let transaction = TransactionId::from_raw(u64::from(context.sequence));
                    if runtime
                        .validate_pixmap_access(context.namespace, drawable)
                        .is_ok()
                    {
                        return Handled(XDispatchResult {
                            response: Some(XAuthorityResponsePacket::accepted(transaction)),
                            outputs: Vec::new(),
                            metadata_candidates: Vec::new(),
                        });
                    }
                    let mut region = Region::empty();
                    for rect in damage {
                        region.push(rect);
                    }
                    let response =
                        runtime.apply_core_draw(transaction, context.namespace, drawable, region);
                    let outputs = if let XAuthorityResponseOutcome::Rejected(error) = response.outcome {
                        vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(drawable.local.raw()).unwrap_or(0),
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
                XWireRequest::PolyFillArc {
                    drawable, damage, ..
                } => {
                    let transaction = TransactionId::from_raw(u64::from(context.sequence));
                    if runtime
                        .validate_pixmap_access(context.namespace, drawable)
                        .is_ok()
                    {
                        return Handled(XDispatchResult {
                            response: Some(XAuthorityResponsePacket::accepted(transaction)),
                            outputs: Vec::new(),
                            metadata_candidates: Vec::new(),
                        });
                    }
                    let mut region = Region::empty();
                    for rect in damage {
                        region.push(rect);
                    }
                    let response =
                        runtime.apply_core_draw(transaction, context.namespace, drawable, region);
                    let outputs = if let XAuthorityResponseOutcome::Rejected(error) = response.outcome {
                        vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(drawable.local.raw()).unwrap_or(0),
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
                XWireRequest::PolyText8 {
                    drawable,
                    gc,
                    x,
                    y,
                    text,
                } => dispatch_text_draw(
                    context,
                    runtime,
                    drawable,
                    gc,
                    XTextDraw {
                        x,
                        baseline: y,
                        text: &text,
                        opaque: false,
                    },
                ),
                XWireRequest::ImageText8 {
                    drawable,
                    gc,
                    x,
                    y,
                    text,
                } => dispatch_text_draw(
                    context,
                    runtime,
                    drawable,
                    gc,
                    XTextDraw {
                        x,
                        baseline: y,
                        text: &text,
                        opaque: true,
                    },
                ),
                XWireRequest::FillPoly {
                    drawable, damage, ..
                } => {
                    let transaction = TransactionId::from_raw(u64::from(context.sequence));
                    if damage.is_none()
                        || runtime
                            .validate_pixmap_access(context.namespace, drawable)
                            .is_ok()
                    {
                        return Handled(XDispatchResult {
                            response: Some(XAuthorityResponsePacket::accepted(transaction)),
                            outputs: Vec::new(),
                            metadata_candidates: Vec::new(),
                        });
                    }
                    let response = runtime.apply_core_draw(
                        transaction,
                        context.namespace,
                        drawable,
                        Region::single(damage.unwrap()),
                    );
                    let outputs = if let XAuthorityResponseOutcome::Rejected(error) = response.outcome {
                        vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(drawable.local.raw()).unwrap_or(0),
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
                XWireRequest::PutImage {
                    drawable,
                    width,
                    height,
                    dst_x,
                    dst_y,
                    data,
                    ..
                } => {
                    let transaction = TransactionId::from_raw(u64::from(context.sequence));
                    if runtime
                        .validate_pixmap_access(context.namespace, drawable)
                        .is_ok()
                    {
                        return Handled(XDispatchResult {
                            response: Some(XAuthorityResponsePacket::accepted(transaction)),
                            outputs: Vec::new(),
                            metadata_candidates: Vec::new(),
                        });
                    }
                    let damage = Region::single(Rect {
                        x: i32::from(dst_x),
                        y: i32::from(dst_y),
                        width: i32::from(width),
                        height: i32::from(height),
                    });
                    let response = runtime.apply_put_image(
                        transaction,
                        context.namespace,
                        drawable,
                        damage,
                        Some(&data),
                    );
                    let outputs = if let XAuthorityResponseOutcome::Rejected(error) = response.outcome {
                        vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(drawable.local.raw()).unwrap_or(0),
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
        _ => unreachable!("request family checked before dispatch"),
    })
}
