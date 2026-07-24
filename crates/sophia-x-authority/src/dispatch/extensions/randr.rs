fn dispatch_randr_request(
    context: XDispatchContext,
    request: XWireRequest,
    runtime: &mut XAuthorityRuntime,
    atoms: &mut XAtomTable,
) -> XDispatchFamilyResult {
    if !matches!(
        &request,
            XWireRequest::RandrQueryVersion { .. }
            | XWireRequest::RandrSelectInput { .. }
            | XWireRequest::RandrGetScreenSizeRange { .. }
            | XWireRequest::RandrGetScreenResources { .. }
            | XWireRequest::RandrGetOutputInfo { .. }
            | XWireRequest::RandrGetOutputProperty { .. }
            | XWireRequest::RandrGetCrtcInfo { .. }
            | XWireRequest::RandrGetCrtcGammaSize { .. }
            | XWireRequest::RandrGetOutputPrimary { .. }
            | XWireRequest::RandrGetProviders { .. }
            | XWireRequest::RandrGetMonitors { .. }
    ) {
        return Unhandled(request);
    }
    Handled(match request {
                XWireRequest::RandrQueryVersion { .. } => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::RandrQueryVersion {
                        sequence: context.sequence,
                        major_version: 1,
                        minor_version: 5,
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::RandrSelectInput { window, .. } => {
                    let outputs = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                        Vec::new()
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
                XWireRequest::RandrGetScreenSizeRange { window } => {
                    let root_size = runtime
                        .output_topology()
                        .root_size()
                        .expect("validated output topology");
                    let root_width = u16::try_from(root_size.width).expect("validated output width");
                    let root_height = u16::try_from(root_size.height).expect("validated output height");
                    let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                        XClientOutput::Reply(XClientReply::RandrGetScreenSizeRange {
                            sequence: context.sequence,
                            min_width: root_width,
                            min_height: root_height,
                            max_width: root_width,
                            max_height: root_height,
                        })
                    } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
                        XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(window.local.raw()).unwrap_or(0),
                        ))
                    } else {
                        XClientOutput::Reply(XClientReply::RandrGetScreenSizeRange {
                            sequence: context.sequence,
                            min_width: root_width,
                            min_height: root_height,
                            max_width: root_width,
                            max_height: root_height,
                        })
                    };
                    XDispatchResult {
                        response: None,
                        outputs: vec![output],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::RandrGetScreenResources { window, .. } => {
                    let resources = randr_resources(runtime.output_topology());
                    let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                        XClientOutput::Reply(XClientReply::RandrGetScreenResources {
                            sequence: context.sequence,
                            timestamp: resources.timestamp,
                            crtcs: resources.crtcs.clone(),
                            outputs: resources.outputs.clone(),
                            modes: resources.modes.clone(),
                        })
                    } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
                        XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(window.local.raw()).unwrap_or(0),
                        ))
                    } else {
                        XClientOutput::Reply(XClientReply::RandrGetScreenResources {
                            sequence: context.sequence,
                            timestamp: resources.timestamp,
                            crtcs: resources.crtcs,
                            outputs: resources.outputs,
                            modes: resources.modes,
                        })
                    };
                    XDispatchResult {
                        response: None,
                        outputs: vec![output],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::RandrGetOutputInfo { output, .. } => {
                    let resources = randr_resources(runtime.output_topology());
                    let client_output = resources
                        .outputs
                        .iter()
                        .position(|candidate| *candidate == output)
                        .map(|index| {
                            let entry = &runtime.output_topology().outputs[index];
                            let mode = resources.modes[index].id;
                            XClientOutput::Reply(XClientReply::RandrGetOutputInfo {
                                sequence: context.sequence,
                                timestamp: resources.timestamp,
                                crtc: resources.crtcs[index],
                                mm_width: logical_pixels_to_millimeters(entry.logical.width),
                                mm_height: logical_pixels_to_millimeters(entry.logical.height),
                                crtcs: vec![resources.crtcs[index]],
                                modes: vec![mode],
                                name: format!("SOPHIA-{}", entry.output.raw()).into_bytes(),
                            })
                        })
                        .unwrap_or_else(|| {
                            XClientOutput::Error(crate::XClientError {
                                code: XErrorCode::BadValue,
                                sequence: context.sequence,
                                resource_id: output,
                                minor_code: crate::X_RANDR_GET_OUTPUT_INFO_MINOR_OPCODE.into(),
                                major_code: context.major_opcode,
                            })
                        });
                    XDispatchResult {
                        response: None,
                        outputs: vec![client_output],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::RandrGetOutputProperty {
                    output,
                    property,
                    property_type: _,
                    long_offset: _,
                    long_length: _,
                    delete: _,
                    pending: _,
                } => {
                    let resources = randr_resources(runtime.output_topology());
                    let client_output =
                        if resources.outputs.contains(&output) && atoms.name(property).is_some() {
                            XClientOutput::Reply(XClientReply::RandrGetOutputProperty {
                                sequence: context.sequence,
                                property_type: 0,
                                bytes_after: 0,
                                format: 0,
                                data: Vec::new(),
                            })
                        } else {
                            XClientOutput::Error(crate::XClientError {
                                code: XErrorCode::BadValue,
                                sequence: context.sequence,
                                resource_id: output,
                                minor_code: crate::X_RANDR_GET_OUTPUT_PROPERTY_MINOR_OPCODE.into(),
                                major_code: context.major_opcode,
                            })
                        };
                    XDispatchResult {
                        response: None,
                        outputs: vec![client_output],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::RandrGetCrtcInfo { crtc, .. } => {
                    let resources = randr_resources(runtime.output_topology());
                    let client_output = resources
                        .crtcs
                        .iter()
                        .position(|candidate| *candidate == crtc)
                        .map(|index| {
                            let entry = &runtime.output_topology().outputs[index];
                            XClientOutput::Reply(XClientReply::RandrGetCrtcInfo {
                                sequence: context.sequence,
                                timestamp: resources.timestamp,
                                x: i16::try_from(entry.logical.x).unwrap_or(i16::MAX),
                                y: i16::try_from(entry.logical.y).unwrap_or(i16::MAX),
                                width: u16::try_from(entry.logical.width).expect("validated output width"),
                                height: u16::try_from(entry.logical.height)
                                    .expect("validated output height"),
                                mode: resources.modes[index].id,
                                outputs: vec![resources.outputs[index]],
                            })
                        })
                        .unwrap_or_else(|| {
                            XClientOutput::Error(crate::XClientError {
                                code: XErrorCode::BadValue,
                                sequence: context.sequence,
                                resource_id: crtc,
                                minor_code: crate::X_RANDR_GET_CRTC_INFO_MINOR_OPCODE.into(),
                                major_code: context.major_opcode,
                            })
                        });
                    XDispatchResult {
                        response: None,
                        outputs: vec![client_output],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::RandrGetCrtcGammaSize { crtc } => {
                    let resources = randr_resources(runtime.output_topology());
                    let client_output = if resources.crtcs.contains(&crtc) {
                        XClientOutput::Reply(XClientReply::RandrGetCrtcGammaSize {
                            sequence: context.sequence,
                            size: 0,
                        })
                    } else {
                        XClientOutput::Error(crate::XClientError {
                            code: XErrorCode::BadValue,
                            sequence: context.sequence,
                            resource_id: crtc,
                            minor_code: crate::X_RANDR_GET_CRTC_GAMMA_SIZE_MINOR_OPCODE.into(),
                            major_code: context.major_opcode,
                        })
                    };
                    XDispatchResult {
                        response: None,
                        outputs: vec![client_output],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::RandrGetOutputPrimary { window } => {
                    let resources = randr_resources(runtime.output_topology());
                    let primary = runtime
                        .output_topology()
                        .outputs
                        .iter()
                        .position(|entry| entry.output == runtime.output_topology().primary)
                        .map(|index| resources.outputs[index])
                        .expect("validated primary output");
                    let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                        XClientOutput::Reply(XClientReply::RandrGetOutputPrimary {
                            sequence: context.sequence,
                            output: primary,
                        })
                    } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
                        XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(window.local.raw()).unwrap_or(0),
                        ))
                    } else {
                        XClientOutput::Reply(XClientReply::RandrGetOutputPrimary {
                            sequence: context.sequence,
                            output: primary,
                        })
                    };
                    XDispatchResult {
                        response: None,
                        outputs: vec![output],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::RandrGetProviders { window } => {
                    let timestamp = u32::try_from(runtime.output_topology().generation)
                        .unwrap_or(u32::MAX)
                        .max(1);
                    let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                        XClientOutput::Reply(XClientReply::RandrGetProviders {
                            sequence: context.sequence,
                            timestamp,
                        })
                    } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
                        XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(window.local.raw()).unwrap_or(0),
                        ))
                    } else {
                        XClientOutput::Reply(XClientReply::RandrGetProviders {
                            sequence: context.sequence,
                            timestamp,
                        })
                    };
                    XDispatchResult {
                        response: None,
                        outputs: vec![output],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::RandrGetMonitors { window, .. } => {
                    let timestamp = u32::try_from(runtime.output_topology().generation)
                        .unwrap_or(u32::MAX)
                        .max(1);
                    let monitors = randr_monitors(runtime.output_topology(), atoms);
                    let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                        XClientOutput::Reply(XClientReply::RandrGetMonitors {
                            sequence: context.sequence,
                            timestamp,
                            monitors: monitors.clone(),
                        })
                    } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
                        XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(window.local.raw()).unwrap_or(0),
                        ))
                    } else {
                        XClientOutput::Reply(XClientReply::RandrGetMonitors {
                            sequence: context.sequence,
                            timestamp,
                            monitors,
                        })
                    };
                    XDispatchResult {
                        response: None,
                        outputs: vec![output],
                        metadata_candidates: Vec::new(),
                    }
                }
        _ => unreachable!("request family checked before dispatch"),
    })
}
