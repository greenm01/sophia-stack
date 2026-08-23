fn dispatch_x_input_request(
    context: XDispatchContext,
    request: XWireRequest,
    runtime: &mut XAuthorityRuntime,
    _atoms: &mut XAtomTable,
) -> XDispatchFamilyResult {
    if !matches!(
        &request,
            XWireRequest::XiGetExtensionVersion
            | XWireRequest::XiQueryPointer { .. }
            | XWireRequest::XiGrabDevice { .. }
            | XWireRequest::XiUngrabDevice { .. }
            | XWireRequest::XiGetClientPointer
            | XWireRequest::XiDeviceBell
            | XWireRequest::XiChangeCursor { .. }
            | XWireRequest::GeQueryVersion { .. }
            | XWireRequest::XiQueryVersion { .. }
            | XWireRequest::XiQueryDevice { .. }
            | XWireRequest::XiSelectEvents { .. }
            | XWireRequest::XiGetFocus { .. }
            | XWireRequest::XiGetProperty
    ) {
        return Unhandled(request);
    }
    Handled(match request {
                XWireRequest::XiGetExtensionVersion => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::XiGetExtensionVersion {
                        sequence: context.sequence,
                        server_major: 2,
                        server_minor: 0,
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::XiQueryPointer { window, .. } => {
                    let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT)
                        || runtime
                            .validate_window_access(context.namespace, window)
                            .is_ok()
                    {
                        XClientOutput::Reply(XClientReply::XiQueryPointer {
                            sequence: context.sequence,
                            root: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
                            child: XResourceId::NONE,
                            root_x: 0,
                            root_y: 0,
                            win_x: 0,
                            win_y: 0,
                            buttons: 0,
                            modifiers: 0,
                        })
                    } else {
                        XClientOutput::Error(crate::XClientError {
                            code: XErrorCode::BadWindow,
                            sequence: context.sequence,
                            resource_id: u32::try_from(window.local.raw()).unwrap_or(0),
                            minor_code: crate::X_INPUT_QUERY_POINTER_MINOR_OPCODE.into(),
                            major_code: context.major_opcode,
                        })
                    };
                    XDispatchResult {
                        response: None,
                        outputs: vec![output],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::XiUngrabDevice { device_id, .. } => {
                    match device_id {
                        2 => runtime
                            .input_authority_mut()
                            .ungrab_pointer(context.namespace, context.client_id),
                        3 => runtime
                            .input_authority_mut()
                            .ungrab_keyboard(context.namespace, context.client_id),
                        _ => {}
                    }
                    XDispatchResult {
                        response: None,
                        outputs: Vec::new(),
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::XiGrabDevice {
                    window,
                    cursor,
                    device_id,
                    pointer_mode,
                    keyboard_mode,
                    owner_events,
                    event_mask,
                    ..
                } => {
                    let mut xi_event_mask = [0; 8];
                    for (target, source) in xi_event_mask.iter_mut().zip(&event_mask) {
                        *target = *source;
                    }
                    let status = if device_id != 2
                        || validate_grab_window(runtime, context.namespace, window).is_err()
                        || cursor.is_some_and(|cursor| {
                            runtime
                                .validate_cursor_access(context.namespace, cursor)
                                .is_err()
                        })
                    {
                        3
                    } else {
                        runtime
                            .input_authority_mut()
                            .grab_pointer(
                                context.namespace,
                                crate::XActiveInputGrab {
                                    owner: context.client_id,
                                    window,
                                    owner_events,
                                    pointer_mode,
                                    keyboard_mode,
                                    event_mask: 0,
                                    xi_event_mask,
                                    xi_event_mask_words: event_mask.len() as u8,
                                    route_lease: None,
                                },
                            )
                            .map_or(1, |_| 0)
                    };
                    XDispatchResult {
                        response: None,
                        outputs: vec![XClientOutput::Reply(XClientReply::GrabStatus {
                            sequence: context.sequence,
                            status,
                        })],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::XiGetClientPointer => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::XiGetClientPointer {
                        sequence: context.sequence,
                        device_id: 2,
                    })],
                    metadata_candidates: Vec::new(),
                },
                // DeviceBell has no server-side state in Sophia. Accepting the bounded
                // legacy XInput request matches an X server with its bell disabled.
                XWireRequest::XiDeviceBell => XDispatchResult {
                    response: None,
                    outputs: Vec::new(),
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::XiChangeCursor { window, cursor } => {
                    let result = runtime
                        .validate_window_access(context.namespace, window)
                        .and_then(|()| {
                            cursor.map_or(Ok(()), |cursor| {
                                runtime.validate_cursor_access(context.namespace, cursor)
                            })
                        });
                    let resource_id = cursor.map_or_else(
                        || u32::try_from(window.local.raw()).unwrap_or(0),
                        |cursor| u32::try_from(cursor.local.raw()).unwrap_or(0),
                    );
                    let outputs = result
                        .err()
                        .map(|error| {
                            XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                resource_id,
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
                XWireRequest::GeQueryVersion { .. } => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::GeQueryVersion {
                        sequence: context.sequence,
                        major_version: 1,
                        minor_version: 0,
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::XiQueryVersion { .. } => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::XiQueryVersion {
                        sequence: context.sequence,
                        major_version: 2,
                        minor_version: 1,
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::XiQueryDevice { device_id } => {
                    let pointer = XXiDeviceInfo {
                        device_id: 2,
                        device_type: 1,
                        attachment: 3,
                        name: "Sophia master pointer".to_owned(),
                        classes: vec![
                            XXiDeviceClass::Button {
                                source_id: 2,
                                button_count: 7,
                            },
                            XXiDeviceClass::Valuator {
                                source_id: 2,
                                number: 0,
                                min: 0,
                                max: i64::from(u16::MAX) << 32,
                                value: 0,
                            },
                            XXiDeviceClass::Valuator {
                                source_id: 2,
                                number: 1,
                                min: 0,
                                max: i64::from(u16::MAX) << 32,
                                value: 0,
                            },
                            XXiDeviceClass::Valuator {
                                source_id: 2,
                                number: crate::X_POINTER_HORIZONTAL_SCROLL_VALUATOR,
                                min: 0,
                                max: 0,
                                value: 0,
                            },
                            XXiDeviceClass::Valuator {
                                source_id: 2,
                                number: crate::X_POINTER_VERTICAL_SCROLL_VALUATOR,
                                min: 0,
                                max: 0,
                                value: 0,
                            },
                            XXiDeviceClass::Scroll {
                                source_id: 2,
                                number: crate::X_POINTER_HORIZONTAL_SCROLL_VALUATOR,
                                scroll_type: 2,
                                flags: 1 << 1,
                                increment: i64::from(120) << 32,
                            },
                            XXiDeviceClass::Scroll {
                                source_id: 2,
                                number: crate::X_POINTER_VERTICAL_SCROLL_VALUATOR,
                                scroll_type: 1,
                                flags: 1 << 1,
                                increment: i64::from(120) << 32,
                            },
                        ],
                    };
                    let keyboard = XXiDeviceInfo {
                        device_id: 3,
                        device_type: 2,
                        attachment: 2,
                        name: "Sophia master keyboard".to_owned(),
                        classes: vec![XXiDeviceClass::Key {
                            source_id: 3,
                            keys: (8..=255).collect(),
                        }],
                    };
                    let devices = match device_id {
                        0 => vec![pointer, keyboard],
                        1 => vec![pointer, keyboard],
                        2 => vec![pointer],
                        3 => vec![keyboard],
                        _ => Vec::new(),
                    };
                    XDispatchResult {
                        response: None,
                        outputs: vec![XClientOutput::Reply(XClientReply::XiQueryDevice {
                            sequence: context.sequence,
                            devices,
                        })],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::XiSelectEvents { window, masks } => {
                    let outputs = (window.local.raw() != u64::from(X_SETUP_DEFAULT_ROOT))
                        .then(|| {
                            runtime
                                .validate_window_access(context.namespace, window)
                                .err()
                        })
                        .flatten()
                        .map(|error| {
                            XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(window.local.raw()).unwrap_or(0),
                            ))
                        })
                        .into_iter()
                        .collect::<Vec<_>>();
                    if outputs.is_empty() {
                        runtime.input_authority_mut().select_xi_events(
                            context.namespace,
                            context.client_id,
                            window,
                            &masks,
                        );
                    }
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::XiGetFocus { .. } => {
                    let (focus, _) = runtime.input_focus(context.namespace);
                    XDispatchResult {
                        response: None,
                        outputs: vec![XClientOutput::Reply(XClientReply::XiGetFocus {
                            sequence: context.sequence,
                            focus,
                        })],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::XiGetProperty => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::XiGetProperty {
                        sequence: context.sequence,
                    })],
                    metadata_candidates: Vec::new(),
                },
        _ => unreachable!("request family checked before dispatch"),
    })
}
