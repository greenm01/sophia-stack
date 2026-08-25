/// The extent Sophia will record for a requested pbuffer, if it will record one.
///
/// `GLX_LARGEST_PBUFFER` asks for the largest available rather than an exact
/// size, so an oversized request clamps instead of failing. Without it, the
/// bound is a refusal. Both read the same maxima the catalog advertises.
fn admitted_pbuffer_size(width: u32, height: u32, largest: bool) -> Option<sophia_protocol::Size> {
    let (width, height) = if largest {
        (
            width.min(crate::X_GLX_MAX_PBUFFER_WIDTH),
            height.min(crate::X_GLX_MAX_PBUFFER_HEIGHT),
        )
    } else {
        (width, height)
    };
    if width > crate::X_GLX_MAX_PBUFFER_WIDTH
        || height > crate::X_GLX_MAX_PBUFFER_HEIGHT
        || width.saturating_mul(height) > crate::X_GLX_MAX_PBUFFER_PIXELS
    {
        return None;
    }
    Some(sophia_protocol::Size {
        width: i32::try_from(width).ok()?,
        height: i32::try_from(height).ok()?,
    })
}

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
            | XWireRequest::GlxCreatePbuffer { .. }
            | XWireRequest::GlxDestroyPbuffer { .. }
            | XWireRequest::GlxQueryContext { .. }
            | XWireRequest::GlxChangeDrawableAttributes { .. }
            | XWireRequest::GlxMakeContextCurrent { .. }
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
                XWireRequest::GlxCreatePbuffer {
                    screen,
                    fbconfig,
                    pbuffer,
                    width,
                    height,
                    largest,
                } => {
                    let outputs = if screen != 0 || crate::x_glx_fb_config(fbconfig).is_none() {
                        vec![glx_bad_value(
                            &context,
                            fbconfig,
                            crate::X_GLX_CREATE_PBUFFER_MINOR_OPCODE,
                        )]
                    } else if let Some(size) = admitted_pbuffer_size(width, height, largest) {
                        runtime
                            .create_glx_pbuffer(context.namespace, pbuffer, fbconfig, size)
                            .err()
                            .map(|error| {
                                XClientOutput::Error(x_error_from_runtime(
                                    error,
                                    context.sequence,
                                    context.major_opcode,
                                    pbuffer.local.raw() as u32,
                                ))
                            })
                            .into_iter()
                            .collect()
                    } else {
                        // Sophia stores no pixels, so the maximum is a refusal
                        // threshold rather than an allocation that failed. A
                        // client asking for the largest available gets a clamp
                        // instead, which is what the attribute means.
                        vec![XClientOutput::Error(crate::XClientError {
                            code: XErrorCode::BadAlloc,
                            sequence: context.sequence,
                            resource_id: pbuffer.local.raw() as u32,
                            minor_code: crate::X_GLX_CREATE_PBUFFER_MINOR_OPCODE.into(),
                            major_code: context.major_opcode,
                        })]
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                // GLX 1.3's context query. Direct Mesa knows its own context
                // attributes, but a client that asks is entitled to an answer from
                // a server claiming this version. The reply shares the drawable
                // attributes shape: a pair count, twenty bytes of pad, then pairs.
                XWireRequest::GlxQueryContext { context: glx_context } => {
                    let outputs = match runtime.glx_context(context.namespace, glx_context) {
                        Ok((fbconfig, _)) => {
                            vec![XClientOutput::Reply(XClientReply::GlxDrawableAttributes {
                                sequence: context.sequence,
                                attributes: vec![
                                    (0x8013, fbconfig),
                                    (0x8011, 0x8014),
                                    (0x800C, 0),
                                ],
                            })]
                        }
                        Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            glx_context.local.raw() as u32,
                        ))],
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                // Sets the drawable event mask, which selects the clobber events a
                // pbuffer can report. Sophia never sends them, so the drawable is
                // validated and the request records nothing -- refusing it would be
                // the worse answer, since the client is entitled to ask.
                XWireRequest::GlxChangeDrawableAttributes { drawable } => {
                    let outputs = runtime
                        .drawable_facts(context.namespace, drawable)
                        .err()
                        .map(|error| {
                            XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                drawable.local.raw() as u32,
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
                XWireRequest::GlxDestroyPbuffer { pbuffer } => {
                    let outputs = runtime
                        .destroy_glx_pbuffer(context.namespace, pbuffer)
                        .err()
                        .map(|error| {
                            XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                pbuffer.local.raw() as u32,
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
                // GLX 1.3's replacement for MakeCurrent, which Sophia already
                // answers for the clients that send the older one. Both drawables
                // are validated so a context cannot be bound to a surface the
                // client does not own.
                XWireRequest::GlxMakeContextCurrent {
                    drawable,
                    read_drawable,
                    context: glx_context,
                } => {
                    let bound = glx_context.map_or(Ok(()), |glx_context| {
                        runtime
                            .glx_context(context.namespace, glx_context)
                            .map(|_| ())
                    });
                    let outputs = match bound
                        .and_then(|()| runtime.drawable_facts(context.namespace, drawable))
                        .and_then(|_| runtime.drawable_facts(context.namespace, read_drawable))
                    {
                        Ok(_) => vec![XClientOutput::Reply(XClientReply::GlxMakeCurrent {
                            sequence: context.sequence,
                            context_tag: u32::from(glx_context.is_some()),
                        })],
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
                    // A window alias reports its backing window's live geometry;
                    // an offscreen surface reports the extent it was created
                    // with, because no window is tracking it.
                    let resolved = runtime
                        .glx_pbuffer(context.namespace, drawable)
                        .map(|(size, config)| {
                            (
                                Rect {
                                    x: 0,
                                    y: 0,
                                    width: size.width,
                                    height: size.height,
                                },
                                config,
                            )
                        })
                        .or_else(|_| {
                            runtime.glx_drawable(context.namespace, drawable).and_then(
                                |(window, config)| {
                                    runtime
                                        .window_geometry(context.namespace, window)
                                        .map(|geometry| (geometry, config))
                                },
                            )
                        });
                    let outputs = match resolved {
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
