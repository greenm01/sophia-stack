fn dispatch_present_request(
    context: XDispatchContext,
    request: XWireRequest,
    runtime: &mut XAuthorityRuntime,
) -> Result<XDispatchResult, XWireRequest> {
    if !matches!(
        &request,
            XWireRequest::PresentQueryVersion { .. }
            | XWireRequest::PresentQueryCapabilities { .. }
            | XWireRequest::PresentSelectInput { .. }
            | XWireRequest::PresentPixmap { .. }
    ) {
        return Err(request);
    }
    Ok(match request {
                XWireRequest::PresentQueryVersion { .. } => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::PresentQueryVersion {
                        sequence: context.sequence,
                        major_version: 1,
                        minor_version: 2,
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::PresentQueryCapabilities { target } => {
                    let outputs = if target.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT)
                        || runtime
                            .validate_window_access(context.namespace, target)
                            .is_ok()
                    {
                        vec![XClientOutput::Reply(
                            XClientReply::PresentQueryCapabilities {
                                sequence: context.sequence,
                                capabilities: 1 << 1,
                            },
                        )]
                    } else {
                        vec![XClientOutput::Error(crate::XClientError {
                            code: XErrorCode::BadWindow,
                            sequence: context.sequence,
                            resource_id: u32::try_from(target.local.raw()).unwrap_or(0),
                            minor_code: u16::from(crate::X_PRESENT_QUERY_CAPABILITIES_MINOR_OPCODE),
                            major_code: context.major_opcode,
                        })]
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::PresentSelectInput {
                    window, event_mask, ..
                } => {
                    let outputs = if event_mask & !0x0f != 0 {
                        vec![XClientOutput::Error(crate::XClientError {
                            code: XErrorCode::BadValue,
                            sequence: context.sequence,
                            resource_id: event_mask,
                            minor_code: u16::from(crate::X_PRESENT_SELECT_INPUT_MINOR_OPCODE),
                            major_code: context.major_opcode,
                        })]
                    } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
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
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::PresentPixmap {
                    transaction,
                    window,
                    pixmap,
                    valid_region,
                    update_region,
                    target_crtc,
                    wait_fence,
                    idle_fence,
                    options,
                    divisor,
                    remainder,
                    ..
                } => {
                    let invalid_value = target_crtc != 0
                        || options & !0x0f != 0
                        || (divisor == 0 && remainder != 0)
                        || (divisor != 0 && remainder >= divisor);
                    let validation = if invalid_value {
                        Err(XAuthorityRuntimeError::InvalidResource)
                    } else {
                        let valid_region = XResourceId::new(u64::from(valid_region), 1);
                        let update_region = XResourceId::new(u64::from(update_region), 1);
                        runtime
                            .validate_window_access(context.namespace, window)
                            .and_then(|()| {
                                valid_region
                                    .is_valid()
                                    .then_some(valid_region)
                                    .map_or(Ok(()), |region| {
                                        runtime.validate_xfixes_region_access(context.namespace, region)
                                    })
                            })
                            .and_then(|()| {
                                update_region
                                    .is_valid()
                                    .then_some(update_region)
                                    .map_or(Ok(()), |region| {
                                        runtime.validate_xfixes_region_access(context.namespace, region)
                                    })
                            })
                            .and_then(|()| runtime.validate_pixmap_access(context.namespace, pixmap))
                            .and_then(|()| {
                                wait_fence.map_or(Ok(()), |fence| {
                                    runtime.validate_dri3_fence_access(context.namespace, fence)
                                })
                            })
                            .and_then(|()| {
                                idle_fence.map_or(Ok(()), |fence| {
                                    runtime.validate_dri3_fence_access(context.namespace, fence)
                                })
                            })
                    };
                    if let Err(error) = validation {
                        if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some() {
                            tracing::warn!(
                                "sophia_present_validation schema=1 sequence={} status=rejected invalid_field={} has_valid_region={} has_update_region={} has_wait_fence={} has_idle_fence={}",
                                context.sequence,
                                invalid_value,
                                valid_region != 0,
                                update_region != 0,
                                wait_fence.is_some(),
                                idle_fence.is_some(),
                            );
                        }
                        return Ok(XDispatchResult {
                            response: None,
                            outputs: vec![XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(pixmap.local.raw()).unwrap_or(0),
                            ))],
                            metadata_candidates: Vec::new(),
                        });
                    }
                    let response =
                        runtime.present_standard_pixmap(transaction, context.namespace, window, pixmap);
                    let outputs = match response.outcome {
                        XAuthorityResponseOutcome::Accepted => Vec::new(),
                        XAuthorityResponseOutcome::Rejected(error) => {
                            vec![XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(pixmap.local.raw()).unwrap_or(0),
                            ))]
                        }
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
