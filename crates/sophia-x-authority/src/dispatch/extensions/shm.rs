fn dispatch_shm_request(
    context: XDispatchContext,
    request: XWireRequest,
    runtime: &mut XAuthorityRuntime,
    _atoms: &mut XAtomTable,
) -> XDispatchFamilyResult {
    if !matches!(
        &request,
            XWireRequest::BigRequestsEnable
            | XWireRequest::ShmAttach { .. }
            | XWireRequest::ShmDetach { .. }
            | XWireRequest::ShmCreatePixmap { .. }
            | XWireRequest::ShmPutImage { .. }
            | XWireRequest::ShmGetImage { .. }
    ) {
        return Unhandled(request);
    }
    Handled(match request {
                XWireRequest::BigRequestsEnable => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::BigRequestsEnable {
                        sequence: context.sequence,
                        maximum_request_length: u32::from(crate::X_SETUP_DEFAULT_MAX_REQUEST_UNITS),
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::ShmAttach {
                    segment,
                    shmid,
                    read_only,
                } => {
                    let outputs = match runtime.attach_shm_segment(
                        context.namespace,
                        segment,
                        shmid,
                        read_only,
                        u64::from(context.sequence),
                    ) {
                        Ok(()) => Vec::new(),
                        Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(segment.local.raw()).unwrap_or(0),
                        ))],
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::ShmDetach { segment } => {
                    let outputs = match runtime.detach_shm_segment(context.namespace, segment) {
                        Ok(()) => Vec::new(),
                        Err(
                            XAuthorityRuntimeError::InvalidResource
                            | XAuthorityRuntimeError::UnknownResource,
                        ) => Vec::new(),
                        Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(segment.local.raw()).unwrap_or(0),
                        ))],
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::ShmCreatePixmap {
                    pixmap,
                    drawable,
                    width,
                    height,
                    depth,
                    segment,
                    offset,
                } => {
                    let valid_shape = width != 0
                        && height != 0
                        && matches!(depth, 24 | 32)
                        && usize::from(width)
                            .checked_mul(usize::from(height))
                            .and_then(|pixels| pixels.checked_mul(4))
                            .and_then(|bytes| usize::try_from(offset).ok()?.checked_add(bytes))
                            .is_some_and(|end| end <= 64 * 1024 * 1024);
                    let result = runtime
                        .validate_drawable_access(context.namespace, drawable)
                        .and_then(|()| runtime.validate_shm_segment_access(context.namespace, segment))
                        .and_then(|()| {
                            valid_shape
                                .then_some(())
                                .ok_or(crate::XAuthorityRuntimeError::InvalidResource)
                        })
                        .and_then(|()| {
                            runtime.create_shm_pixmap(
                                context.namespace,
                                pixmap,
                                sophia_protocol::Size {
                                    width: i32::from(width),
                                    height: i32::from(height),
                                },
                                u64::from(context.sequence),
                                segment,
                                offset,
                            )
                        });
                    let outputs = result
                        .err()
                        .map(|error| {
                            XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(pixmap.local.raw()).unwrap_or(0),
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
                XWireRequest::ShmPutImage {
                    drawable,
                    segment,
                    total_width,
                    total_height,
                    src_x,
                    src_y,
                    src_width,
                    src_height,
                    dst_x,
                    dst_y,
                    depth,
                    format,
                    offset,
                    send_event,
                    ..
                } => {
                    let transaction = TransactionId::from_raw(u64::from(context.sequence));
                    if runtime
                        .validate_shm_segment_access(context.namespace, segment)
                        .is_err()
                    {
                        return Handled(XDispatchResult {
                            response: Some(XAuthorityResponsePacket::accepted(transaction)),
                            outputs: vec![XClientOutput::Error(crate::XClientError {
                                code: XErrorCode::BadAccess,
                                sequence: context.sequence,
                                resource_id: u32::try_from(segment.local.raw()).unwrap_or(0),
                                minor_code: 3,
                                major_code: context.major_opcode,
                            })],
                            metadata_candidates: Vec::new(),
                        });
                    }
                    let damage = Region::single(Rect {
                        x: i32::from(dst_x),
                        y: i32::from(dst_y),
                        width: i32::from(src_width),
                        height: i32::from(src_height),
                    });
                    let image = runtime
                        .shm_segment_shmid(context.namespace, segment)
                        .ok()
                        .and_then(|shmid| {
                            copy_shm_image_region(XShmImageCopy {
                                shmid,
                                offset,
                                total_width,
                                total_height,
                                src_x,
                                src_y,
                                src_width,
                                src_height,
                                depth,
                                format,
                            })
                        });
                    let response = runtime.apply_put_image(
                        transaction,
                        context.namespace,
                        drawable,
                        damage,
                        image.as_deref(),
                    );
                    let outputs = if let XAuthorityResponseOutcome::Rejected(error) = response.outcome {
                        vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(drawable.local.raw()).unwrap_or(0),
                        ))]
                    } else if send_event {
                        vec![XClientOutput::Event(XClientEvent::ShmCompletion {
                            sequence: context.sequence,
                            drawable,
                            segment,
                            offset,
                        })]
                    } else {
                        Vec::new()
                    };
                    XDispatchResult {
                        response: Some(response),
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::ShmGetImage {
                    drawable,
                    x,
                    y,
                    width,
                    height,
                    format,
                    segment,
                    offset,
                    ..
                } => {
                    let image_len = usize::from(width)
                        .checked_mul(usize::from(height))
                        .and_then(|pixels| pixels.checked_mul(4))
                        .filter(|len| *len <= crate::X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES);
                    let result = runtime
                        .validate_drawable_access(context.namespace, drawable)
                        .and_then(|()| {
                            runtime.validate_shm_segment_access(context.namespace, segment)
                        })
                        .and_then(|()| {
                            (format == 2)
                                .then_some(())
                                .ok_or(XAuthorityRuntimeError::InvalidResource)
                        })
                        .and_then(|()| {
                            image_len.ok_or(XAuthorityRuntimeError::InvalidResource)?;
                            let shmid =
                                runtime.shm_segment_shmid(context.namespace, segment)?;
                            let image = runtime.drawable_image_region(
                                context.namespace,
                                drawable,
                                Rect {
                                    x: i32::from(x),
                                    y: i32::from(y),
                                    width: i32::from(width),
                                    height: i32::from(height),
                                },
                            )?;
                            sophia_sysv_shm::write_bytes(
                                shmid,
                                usize::try_from(offset)
                                    .map_err(|_| XAuthorityRuntimeError::InvalidResource)?,
                                &image,
                            )
                            .map_err(|_| XAuthorityRuntimeError::InvalidResource)
                        });
                    let outputs = match result {
                        Ok(()) => {
                            let (depth, visual, _) = runtime.window_visual(drawable);
                            vec![XClientOutput::Reply(XClientReply::ShmGetImage {
                                sequence: context.sequence,
                                depth,
                                visual,
                                size: u32::try_from(image_len.unwrap_or(0)).unwrap_or(u32::MAX),
                            })]
                        }
                        Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(drawable.local.raw()).unwrap_or(0),
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
