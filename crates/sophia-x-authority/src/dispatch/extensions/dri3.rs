fn dispatch_dri3_request(
    context: XDispatchContext,
    request: XWireRequest,
    runtime: &mut XAuthorityRuntime,
) -> XDispatchFamilyResult {
    if !matches!(
        &request,
            XWireRequest::Dri3Open { .. }
            | XWireRequest::Dri3PixmapFromBuffer { .. }
            | XWireRequest::Dri3PixmapFromBuffers { .. }
            | XWireRequest::Dri3FenceFromFd { .. }
            | XWireRequest::Dri3GetSupportedModifiers { .. }
    ) {
        return Unhandled(request);
    }
    Handled(match request {
                XWireRequest::Dri3Open { drawable, provider } => {
                    let outputs = if provider != 0 {
                        vec![XClientOutput::Error(crate::XClientError {
                            code: XErrorCode::BadValue,
                            sequence: context.sequence,
                            resource_id: provider,
                            minor_code: u16::from(crate::X_DRI3_OPEN_MINOR_OPCODE),
                            major_code: context.major_opcode,
                        })]
                    } else if let Err(error) = runtime.validate_dri3_drawable_access(context.namespace, drawable)
                    {
                        let mut error = x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(drawable.local.raw()).unwrap_or(0),
                        );
                        error.minor_code = u16::from(crate::X_DRI3_OPEN_MINOR_OPCODE);
                        vec![XClientOutput::Error(error)]
                    } else {
                        vec![XClientOutput::Reply(XClientReply::Dri3Open {
                            sequence: context.sequence,
                        })]
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::Dri3PixmapFromBuffer {
                    pixmap,
                    drawable,
                    size_bytes,
                    width,
                    height,
                    stride,
                    depth,
                    bits_per_pixel,
                } => {
                    let outputs =
                        if let Err(error) = runtime.validate_dri3_drawable_access(context.namespace, drawable) {
                            vec![XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(drawable.local.raw()).unwrap_or(0),
                            ))]
                        } else if let Err(error) = runtime.create_dri3_pixmap(
                            context.namespace,
                            pixmap,
                            u64::from(context.sequence),
                            size_bytes,
                            width,
                            height,
                            stride,
                            depth,
                            bits_per_pixel,
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
                XWireRequest::Dri3PixmapFromBuffers {
                    pixmap,
                    window,
                    num_buffers,
                    width,
                    height,
                    strides,
                    offsets,
                    depth,
                    bits_per_pixel,
                    modifier,
                } => {
                    let outputs =
                        if let Err(error) = runtime.validate_dri3_drawable_access(context.namespace, window) {
                            vec![XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(window.local.raw()).unwrap_or(0),
                            ))]
                        } else if let Err(error) = runtime.create_dri3_pixmap_from_buffers(
                            context.namespace,
                            pixmap,
                            u64::from(context.sequence),
                            num_buffers,
                            width,
                            height,
                            strides,
                            offsets,
                            depth,
                            bits_per_pixel,
                            modifier,
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
                XWireRequest::Dri3FenceFromFd {
                    drawable, fence, ..
                } => {
                    let outputs =
                        if let Err(error) = runtime.validate_dri3_drawable_access(context.namespace, drawable) {
                            vec![XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(drawable.local.raw()).unwrap_or(0),
                            ))]
                        } else if let Err(error) =
                            runtime.create_dri3_fence(context.namespace, fence, u64::from(context.sequence))
                        {
                            vec![XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(fence.local.raw()).unwrap_or(0),
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
                XWireRequest::Dri3GetSupportedModifiers {
                    window,
                    depth,
                    bits_per_pixel,
                } => {
                    let outputs = if !matches!((depth, bits_per_pixel), (24 | 32, 32)) {
                        vec![XClientOutput::Error(crate::XClientError {
                            code: XErrorCode::BadValue,
                            sequence: context.sequence,
                            resource_id: u32::from(depth),
                            minor_code: u16::from(crate::X_DRI3_GET_SUPPORTED_MODIFIERS_MINOR_OPCODE),
                            major_code: context.major_opcode,
                        })]
                    } else if window.local.raw() != u64::from(crate::X_SETUP_DEFAULT_ROOT) {
                        match runtime.validate_dri3_drawable_access(context.namespace, window) {
                            Ok(()) => vec![XClientOutput::Reply(
                                XClientReply::Dri3GetSupportedModifiers {
                                    sequence: context.sequence,
                                    window_modifiers: Vec::new(),
                                    screen_modifiers: vec![0, DRM_FORMAT_MOD_INVALID],
                                },
                            )],
                            Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(window.local.raw()).unwrap_or(0),
                            ))],
                        }
                    } else {
                        vec![XClientOutput::Reply(
                            XClientReply::Dri3GetSupportedModifiers {
                                sequence: context.sequence,
                                window_modifiers: Vec::new(),
                                screen_modifiers: vec![0, DRM_FORMAT_MOD_INVALID],
                            },
                        )]
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
