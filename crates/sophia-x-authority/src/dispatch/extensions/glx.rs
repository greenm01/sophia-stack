fn dispatch_glx_request(
    context: XDispatchContext,
    request: XWireRequest,
    runtime: &mut XAuthorityRuntime,
    _atoms: &mut XAtomTable,
) -> XDispatchFamilyResult {
    if !matches!(
        &request,
            XWireRequest::GlxQueryVersion { .. }
            | XWireRequest::GlxGetVisualConfigs { .. }
            | XWireRequest::GlxGetFbConfigs { .. }
            | XWireRequest::GlxClientInfo
            | XWireRequest::GlxCreateContext { .. }
            | XWireRequest::GlxDestroyContext { .. }
            | XWireRequest::GlxMakeCurrent { .. }
            | XWireRequest::GlxIsDirect { .. }
            | XWireRequest::GlxCreateWindow { .. }
            | XWireRequest::GlxDeleteWindow { .. }
            | XWireRequest::GlxGetDrawableAttributes { .. }
            | XWireRequest::GlxQueryExtensionsString
            | XWireRequest::GlxQueryServerString { .. }
    ) {
        return Unhandled(request);
    }
    Handled(match request {
                XWireRequest::GlxQueryVersion { .. } => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::GlxQueryVersion {
                        sequence: context.sequence,
                        major_version: 1,
                        minor_version: 4,
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::GlxGetVisualConfigs { screen } => {
                    let outputs = if screen == 0 {
                        vec![XClientOutput::Reply(XClientReply::GlxVisualConfigs {
                            sequence: context.sequence,
                            configs: glx_visual_configs(),
                        })]
                    } else {
                        vec![glx_bad_value(
                            &context,
                            screen,
                            crate::X_GLX_GET_VISUAL_CONFIGS_MINOR_OPCODE,
                        )]
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::GlxGetFbConfigs { screen } => {
                    let outputs = if screen == 0 {
                        vec![XClientOutput::Reply(XClientReply::GlxFbConfigs {
                            sequence: context.sequence,
                            configs: glx_fb_configs(),
                        })]
                    } else {
                        vec![glx_bad_value(
                            &context,
                            screen,
                            crate::X_GLX_GET_FB_CONFIGS_MINOR_OPCODE,
                        )]
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::GlxClientInfo => XDispatchResult {
                    response: None,
                    outputs: Vec::new(),
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::GlxCreateContext {
                    context: id,
                    config,
                    screen,
                    share,
                    direct,
                } => {
                    let fbconfig = match config {
                        XGlxContextConfig::Visual(X_SETUP_DEFAULT_VISUAL) => Some(1),
                        XGlxContextConfig::Visual(X_SETUP_ARGB_VISUAL) => Some(2),
                        XGlxContextConfig::FbConfig(fbconfig @ 1..=3) => Some(fbconfig),
                        XGlxContextConfig::Visual(_) | XGlxContextConfig::FbConfig(_) => None,
                    };
                    let valid = screen == 0
                        && fbconfig.is_some()
                        && share.is_none_or(|share| {
                            runtime.glx_context(context.namespace, share).is_ok()
                        });
                    let outputs = if valid {
                        runtime
                            .create_glx_context(
                                context.namespace,
                                id,
                                fbconfig.expect("validated GLX config"),
                                direct,
                            )
                            .err()
                            .map(|error| {
                                XClientOutput::Error(x_error_from_runtime(
                                    error,
                                    context.sequence,
                                    context.major_opcode,
                                    id.local.raw() as u32,
                                ))
                            })
                            .into_iter()
                            .collect()
                    } else {
                        vec![glx_bad_value(
                            &context,
                            match config {
                                XGlxContextConfig::Visual(visual) => visual,
                                XGlxContextConfig::FbConfig(fbconfig) => fbconfig,
                            },
                            crate::X_GLX_CREATE_CONTEXT_ATTRIBS_ARB_MINOR_OPCODE,
                        )]
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::GlxDestroyContext { context: id } => {
                    let outputs = runtime
                        .destroy_glx_context(context.namespace, id)
                        .err()
                        .map(|error| {
                            XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                id.local.raw() as u32,
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
                XWireRequest::GlxMakeCurrent {
                    drawable,
                    context: context_id,
                    old_context_tag,
                } => {
                    let valid_old_tag = matches!(old_context_tag, 0 | 1);
                    let valid = match (drawable, context_id) {
                        (None, None) => valid_old_tag,
                        (Some(drawable), Some(context_id)) => {
                            let context_record =
                                runtime.glx_context(context.namespace, context_id);
                            let drawable = runtime.glx_drawable(context.namespace, drawable);
                            valid_old_tag
                                && matches!(
                                    (context_record, drawable),
                                    (Ok((1, true)), Ok((_, 1)))
                                        | (Ok((2 | 3, true)), Ok((_, 2 | 3)))
                                )
                        }
                        (None, Some(_)) | (Some(_), None) => false,
                    };
                    let outputs = if valid {
                        vec![XClientOutput::Reply(XClientReply::GlxMakeCurrent {
                            sequence: context.sequence,
                            context_tag: u32::from(context_id.is_some()),
                        })]
                    } else {
                        vec![glx_bad_value(
                            &context,
                            context_id
                                .or(drawable)
                                .map_or(old_context_tag, |resource| resource.local.raw() as u32),
                            crate::X_GLX_MAKE_CURRENT_MINOR_OPCODE,
                        )]
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::GlxIsDirect { context: id } => {
                    let outputs = match runtime.glx_context(context.namespace, id) {
                        Ok((_, direct)) => vec![XClientOutput::Reply(XClientReply::GlxIsDirect {
                            sequence: context.sequence,
                            direct,
                        })],
                        Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            id.local.raw() as u32,
                        ))],
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::GlxCreateWindow {
                    screen,
                    fbconfig,
                    window,
                    glx_window,
                } => {
                    let visual = runtime.window_visual(window).1;
                    let compatible = matches!(
                        (fbconfig, visual),
                        (1, X_SETUP_DEFAULT_VISUAL) | (2 | 3, X_SETUP_ARGB_VISUAL)
                    );
                    let outputs = if screen == 0 && compatible {
                        runtime
                            .create_glx_window(context.namespace, glx_window, window, fbconfig)
                            .err()
                            .map(|error| {
                                XClientOutput::Error(x_error_from_runtime(
                                    error,
                                    context.sequence,
                                    context.major_opcode,
                                    glx_window.local.raw() as u32,
                                ))
                            })
                            .into_iter()
                            .collect()
                    } else {
                        vec![glx_bad_value(
                            &context,
                            fbconfig,
                            crate::X_GLX_CREATE_WINDOW_MINOR_OPCODE,
                        )]
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::GlxDeleteWindow { glx_window } => {
                    let outputs = runtime
                        .destroy_glx_window(context.namespace, glx_window)
                        .err()
                        .map(|error| {
                            XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                glx_window.local.raw() as u32,
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
                XWireRequest::GlxGetDrawableAttributes { drawable } => {
                    let outputs = match runtime.glx_drawable(context.namespace, drawable).and_then(
                        |(window, config)| {
                            runtime
                                .window_geometry(context.namespace, window)
                                .map(|geometry| (geometry, config))
                        },
                    ) {
                        Ok((geometry, config)) => {
                            vec![XClientOutput::Reply(XClientReply::GlxDrawableAttributes {
                                sequence: context.sequence,
                                attributes: vec![
                                    (0x801D, geometry.width as u32),
                                    (0x801E, geometry.height as u32),
                                    (0x8013, config),
                                    (0x800C, 0),
                                ],
                            })]
                        }
                        Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            drawable.local.raw() as u32,
                        ))],
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::GlxQueryExtensionsString => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::GlxString {
                        sequence: context.sequence,
                        value: GLX_EXTENSIONS.to_owned(),
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::GlxQueryServerString { name } => {
                    let value = match name {
                        1 => "Sophia",
                        2 => "1.4",
                        3 => GLX_EXTENSIONS,
                        0x20f6 => "mesa",
                        _ => "",
                    };
                    XDispatchResult {
                        response: None,
                        outputs: vec![XClientOutput::Reply(XClientReply::GlxString {
                            sequence: context.sequence,
                            value: value.to_owned(),
                        })],
                        metadata_candidates: Vec::new(),
                    }
                }
        _ => unreachable!("request family checked before dispatch"),
    })
}
