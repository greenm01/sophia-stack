fn dispatch_core_input_discovery_request(
    context: XDispatchContext,
    request: XWireRequest,
    runtime: &mut XAuthorityRuntime,
    _atoms: &mut XAtomTable,
    _properties: &mut XPropertyTable,
) -> XDispatchFamilyResult {
    if !matches!(
        &request,
            XWireRequest::GetInputFocus
            | XWireRequest::SetInputFocus { .. }
            | XWireRequest::GetModifierMapping
            | XWireRequest::GetPointerMapping
            | XWireRequest::GetKeyboardMapping { .. }
            | XWireRequest::GetKeyboardControl
            | XWireRequest::Bell
            | XWireRequest::TranslateCoordinates { .. }
            | XWireRequest::QueryPointer { .. }
            | XWireRequest::QueryExtension { .. }
            | XWireRequest::ListExtensions
            | XWireRequest::QueryBestSize { .. }
            | XWireRequest::QueryColors { .. }
            | XWireRequest::CreateColormap { .. }
            | XWireRequest::FreeColormap { .. }
            | XWireRequest::AllocNamedColor { .. }
            | XWireRequest::AllocColor { .. }
    ) {
        return Unhandled(request);
    }
    Handled(match request {
                XWireRequest::GetInputFocus => {
                    let (focus, revert_to) = runtime.input_focus(context.namespace);
                    XDispatchResult {
                        response: None,
                        outputs: vec![XClientOutput::Reply(XClientReply::GetInputFocus {
                            sequence: context.sequence,
                            focus,
                            revert_to,
                        })],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::SetInputFocus {
                    focus, revert_to, ..
                } => {
                    let (previous, _) = runtime.input_focus(context.namespace);
                    let outputs = match runtime.set_input_focus(context.namespace, focus, revert_to) {
                        Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(focus.local.raw()).unwrap_or(0),
                        ))],
                        Ok(()) if previous == focus => Vec::new(),
                        Ok(()) => {
                            let mut outputs = Vec::with_capacity(2);
                            if previous.local.raw() != 0 {
                                outputs.push(XClientOutput::Event(XClientEvent::Focus {
                                    sequence: context.sequence,
                                    focused: false,
                                    detail: 3,
                                    event: previous,
                                    mode: 0,
                                }));
                            }
                            if focus.local.raw() != 0 {
                                outputs.push(XClientOutput::Event(XClientEvent::Focus {
                                    sequence: context.sequence,
                                    focused: true,
                                    detail: 3,
                                    event: focus,
                                    mode: 0,
                                }));
                            }
                            outputs
                        }
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::GetModifierMapping => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::GetModifierMapping {
                        sequence: context.sequence,
                        keycodes_per_modifier: 2,
                        keycodes: vec![50, 62, 66, 0, 37, 105, 64, 108, 77, 0, 0, 0, 133, 134, 0, 0],
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::GetPointerMapping => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::GetPointerMapping {
                        sequence: context.sequence,
                        mapping: vec![1, 2, 3, 4, 5, 6, 7],
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::GetKeyboardMapping {
                    first_keycode,
                    count,
                } => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::GetKeyboardMapping {
                        sequence: context.sequence,
                        keysyms_per_keycode: 2,
                        keysyms: runtime.xkb_keymap().core_mapping(first_keycode, count),
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::GetKeyboardControl => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::GetKeyboardControl {
                        sequence: context.sequence,
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::Bell => XDispatchResult {
                    response: None,
                    outputs: Vec::new(),
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::TranslateCoordinates {
                    source,
                    destination,
                    src_x,
                    src_y,
                } => {
                    let output =
                        if let Err(error) = runtime.validate_drawable_access(context.namespace, source) {
                            XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(source.local.raw()).unwrap_or(0),
                            ))
                        } else if let Err(error) =
                            runtime.validate_drawable_access(context.namespace, destination)
                        {
                            XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(destination.local.raw()).unwrap_or(0),
                            ))
                        } else {
                            XClientOutput::Reply(XClientReply::TranslateCoordinates {
                                sequence: context.sequence,
                                same_screen: true,
                                child: None,
                                dst_x: src_x,
                                dst_y: src_y,
                            })
                        };
                    XDispatchResult {
                        response: None,
                        outputs: vec![output],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::QueryPointer { window } => {
                    let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT)
                        || runtime
                            .validate_window_access(context.namespace, window)
                            .is_ok()
                    {
                        XClientOutput::Reply(XClientReply::QueryPointer {
                            sequence: context.sequence,
                            root: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
                            child: XResourceId::NONE,
                            root_x: 0,
                            root_y: 0,
                            win_x: 0,
                            win_y: 0,
                            mask: 0,
                        })
                    } else {
                        XClientOutput::Error(crate::XClientError {
                            code: XErrorCode::BadWindow,
                            sequence: context.sequence,
                            resource_id: u32::try_from(window.local.raw()).unwrap_or(0),
                            minor_code: 0,
                            major_code: context.major_opcode,
                        })
                    };
                    XDispatchResult {
                        response: None,
                        outputs: vec![output],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::QueryExtension { name } => {
                    let extension = extension_query_result(&name);
                    XDispatchResult {
                        response: None,
                        outputs: vec![XClientOutput::Reply(XClientReply::QueryExtension {
                            sequence: context.sequence,
                            present: extension.present,
                            major_opcode: extension.major_opcode,
                            first_event: extension.first_event,
                            first_error: extension.first_error,
                        })],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::ListExtensions => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::ListExtensions {
                        sequence: context.sequence,
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::QueryBestSize { width, height, .. } => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::QueryBestSize {
                        sequence: context.sequence,
                        width,
                        height,
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::QueryColors { pixels, .. } => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::QueryColors {
                        sequence: context.sequence,
                        pixels,
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::CreateColormap { window, .. } => {
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
                XWireRequest::FreeColormap { .. } => XDispatchResult {
                    response: None,
                    outputs: Vec::new(),
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::AllocNamedColor { name, .. } => {
                    let black = name.eq_ignore_ascii_case("black");
                    let intensity = if black { 0 } else { u16::MAX };
                    XDispatchResult {
                        response: None,
                        outputs: vec![XClientOutput::Reply(XClientReply::AllocNamedColor {
                            sequence: context.sequence,
                            pixel: if black { 0 } else { 1 },
                            red: intensity,
                            green: intensity,
                            blue: intensity,
                        })],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::AllocColor {
                    red, green, blue, ..
                } => {
                    let pixel = true_color_pixel_from_rgb16(red, green, blue);
                    XDispatchResult {
                        response: None,
                        outputs: vec![XClientOutput::Reply(XClientReply::AllocColor {
                            sequence: context.sequence,
                            pixel,
                            red,
                            green,
                            blue,
                        })],
                        metadata_candidates: Vec::new(),
                    }
                }
        _ => unreachable!("request family checked before dispatch"),
    })
}
