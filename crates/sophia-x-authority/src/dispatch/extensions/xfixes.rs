fn dispatch_xfixes_request(
    context: XDispatchContext,
    request: XWireRequest,
    runtime: &mut XAuthorityRuntime,
    atoms: &mut XAtomTable,
) -> XDispatchFamilyResult {
    if !matches!(
        &request,
            XWireRequest::XfixesCreateRegion { .. }
            | XWireRequest::XfixesSetRegion { .. }
            | XWireRequest::XfixesDestroyRegion { .. }
            | XWireRequest::XfixesSelectSelectionInput { .. }
    ) {
        return Unhandled(request);
    }
    Handled(match request {
                XWireRequest::XfixesCreateRegion { region, rectangles } => {
                    let output = runtime
                        .create_xfixes_region(
                            context.namespace,
                            region,
                            rectangles,
                            u64::from(context.sequence),
                        )
                        .err()
                        .map(|error| {
                            XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u16::from(crate::X_XFIXES_CREATE_REGION_MINOR_OPCODE),
                                u32::try_from(region.local.raw()).unwrap_or(0)))
                        });
                    XDispatchResult {
                        response: None,
                        outputs: output.into_iter().collect(),
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::XfixesSetRegion { region, rectangles } => {
                    let output = runtime
                        .set_xfixes_region(context.namespace, region, rectangles)
                        .err()
                        .map(|error| {
                            XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u16::from(crate::X_XFIXES_SET_REGION_MINOR_OPCODE),
                                u32::try_from(region.local.raw()).unwrap_or(0)))
                        });
                    XDispatchResult {
                        response: None,
                        outputs: output.into_iter().collect(),
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::XfixesDestroyRegion { region } => {
                    let output = runtime
                        .destroy_xfixes_region(context.namespace, region)
                        .err()
                        .map(|error| {
                            XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u16::from(crate::X_XFIXES_DESTROY_REGION_MINOR_OPCODE),
                                u32::try_from(region.local.raw()).unwrap_or(0)))
                        });
                    XDispatchResult {
                        response: None,
                        outputs: output.into_iter().collect(),
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::XfixesSelectSelectionInput {
                    window,
                    selection,
                    event_mask,
                } => {
                    let output = if event_mask & !0b111 != 0 {
                        Some(XClientOutput::Error(crate::XClientError {
                            code: XErrorCode::BadValue,
                            sequence: context.sequence,
                            resource_id: event_mask,
                            minor_code: crate::X_XFIXES_SELECT_SELECTION_INPUT_MINOR_OPCODE.into(),
                            major_code: context.major_opcode,
                        }))
                    } else if atoms.name(selection).is_none() {
                        Some(XClientOutput::Error(crate::XClientError {
                            code: XErrorCode::BadAtom,
                            sequence: context.sequence,
                            resource_id: selection,
                            minor_code: crate::X_XFIXES_SELECT_SELECTION_INPUT_MINOR_OPCODE.into(),
                            major_code: context.major_opcode,
                        }))
                    } else if let Err(error) =
                        validate_window_or_root_access(runtime, context.namespace, window)
                    {
                        let error = x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u16::from(crate::X_XFIXES_SELECT_SELECTION_INPUT_MINOR_OPCODE),
                            u32::try_from(window.local.raw()).unwrap_or(0));
                        Some(XClientOutput::Error(error))
                    } else {
                        None
                    };
                    XDispatchResult {
                        response: None,
                        outputs: output.into_iter().collect(),
                        metadata_candidates: Vec::new(),
                    }
                }
        _ => unreachable!("request family checked before dispatch"),
    })
}
