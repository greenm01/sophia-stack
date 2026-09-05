/// Sophia's virtual master pointer, the only pointer any XI path accepts.
const X_VIRTUAL_MASTER_POINTER_ID: u16 = 2;
/// Sophia's virtual master keyboard.
const X_VIRTUAL_MASTER_KEYBOARD_ID: u16 = 3;
/// The core keycode range, matching the setup contract in `keyboard.rs`.
const X_VIRTUAL_MASTER_MIN_KEYCODE: u8 = 8;
const X_VIRTUAL_MASTER_MAX_KEYCODE: u8 = u8::MAX;

/// The virtual master devices Sophia presents to X clients.
///
/// The frontend has no device inventory. The seat enumerates real udev devices, but
/// none of that crosses this boundary, and every XI path -- grabs, ungrabs, client
/// pointer, and event routing -- is written against this fixed pair. Both the XI2
/// `QueryDevice` reply and the XI1 `ListInputDevices` reply are projected from this
/// one table rather than each carrying its own copy, because two device tables that
/// must agree is a skew waiting to happen.
const X_VIRTUAL_MASTER_DEVICES: [XVirtualMasterDevice; 2] = [
    XVirtualMasterDevice {
        device_id: X_VIRTUAL_MASTER_POINTER_ID,
        name: "Sophia master pointer",
        kind: XVirtualMasterKind::Pointer {
            // Derived from the one owner rather than restated, so the master
            // pointer and `GetPointerMapping` cannot disagree about how many
            // buttons exist.
            // Widening a button number; `u16::from` is not const-callable yet.
            button_count: crate::pointer::X_POINTER_BUTTON_COUNT as u16,
        },
    },
    XVirtualMasterDevice {
        device_id: X_VIRTUAL_MASTER_KEYBOARD_ID,
        name: "Sophia master keyboard",
        kind: XVirtualMasterKind::Keyboard {
            // The keycode range the core setup advertises.
            min_keycode: X_VIRTUAL_MASTER_MIN_KEYCODE,
            max_keycode: X_VIRTUAL_MASTER_MAX_KEYCODE,
        },
    },
];

#[derive(Clone, Copy)]
struct XVirtualMasterDevice {
    device_id: u16,
    name: &'static str,
    kind: XVirtualMasterKind,
}

#[derive(Clone, Copy)]
enum XVirtualMasterKind {
    Pointer { button_count: u16 },
    Keyboard { min_keycode: u8, max_keycode: u8 },
}

impl XVirtualMasterDevice {
    /// The XI2 record, carrying the classes only XI2 can express.
    fn xi2_device_info(self) -> XXiDeviceInfo {
        let source_id = self.device_id;
        let (device_type, attachment, classes) = match self.kind {
            XVirtualMasterKind::Pointer { button_count } => (
                1,
                X_VIRTUAL_MASTER_KEYBOARD_ID,
                vec![
                    XXiDeviceClass::Button {
                        source_id,
                        button_count,
                    },
                    XXiDeviceClass::Valuator {
                        source_id,
                        number: 0,
                        min: 0,
                        max: i64::from(u16::MAX) << 32,
                        value: 0,
                    },
                    XXiDeviceClass::Valuator {
                        source_id,
                        number: 1,
                        min: 0,
                        max: i64::from(u16::MAX) << 32,
                        value: 0,
                    },
                    XXiDeviceClass::Valuator {
                        source_id,
                        number: crate::X_POINTER_HORIZONTAL_SCROLL_VALUATOR,
                        min: 0,
                        max: 0,
                        value: 0,
                    },
                    XXiDeviceClass::Valuator {
                        source_id,
                        number: crate::X_POINTER_VERTICAL_SCROLL_VALUATOR,
                        min: 0,
                        max: 0,
                        value: 0,
                    },
                    XXiDeviceClass::Scroll {
                        source_id,
                        number: crate::X_POINTER_HORIZONTAL_SCROLL_VALUATOR,
                        scroll_type: 2,
                        flags: 1 << 1,
                        increment: i64::from(120) << 32,
                    },
                    XXiDeviceClass::Scroll {
                        source_id,
                        number: crate::X_POINTER_VERTICAL_SCROLL_VALUATOR,
                        scroll_type: 1,
                        flags: 1 << 1,
                        increment: i64::from(120) << 32,
                    },
                ],
            ),
            XVirtualMasterKind::Keyboard {
                min_keycode,
                max_keycode,
            } => (
                2,
                X_VIRTUAL_MASTER_POINTER_ID,
                vec![XXiDeviceClass::Key {
                    source_id,
                    keys: (u32::from(min_keycode)..=u32::from(max_keycode)).collect(),
                }],
            ),
        };
        XXiDeviceInfo {
            device_id: self.device_id,
            device_type,
            attachment,
            name: self.name.to_owned(),
            classes,
        }
    }

    /// The XI1 record. A lossy projection by design: XI1 has no scroll class, and a
    /// virtual pointer has no resolution worth stating, so this carries the identity
    /// and the button or key range and nothing invented beyond them. XI2 still
    /// reports the valuators; the asymmetry is deliberate, not an omission.
    ///
    /// `device_use` is load-bearing. `IsXPointer` and `IsXKeyboard` tell an XI1
    /// client these are the core devices and not its to open, which is what keeps it
    /// away from `OpenDevice` and the other XI1 requests Sophia does not implement.
    /// Reporting them as extension devices would answer this request and move the
    /// failure one opcode along.
    fn xi1_device_info(self, device_type: u32) -> XXiLegacyDeviceInfo {
        let (device_use, classes) = match self.kind {
            XVirtualMasterKind::Pointer { button_count } => (
                crate::X_INPUT_LEGACY_USE_POINTER,
                vec![XXiLegacyDeviceClass::Button { button_count }],
            ),
            XVirtualMasterKind::Keyboard {
                min_keycode,
                max_keycode,
            } => (
                crate::X_INPUT_LEGACY_USE_KEYBOARD,
                vec![XXiLegacyDeviceClass::Key {
                    min_keycode,
                    max_keycode,
                }],
            ),
        };
        XXiLegacyDeviceInfo {
            device_id: u8::try_from(self.device_id).unwrap_or(0),
            device_type,
            device_use,
            name: self.name.to_owned(),
            classes,
        }
    }

    /// The atom name XI1 reports as this device's type.
    const fn legacy_type_name(self) -> &'static str {
        match self.kind {
            XVirtualMasterKind::Pointer { .. } => "MOUSE",
            XVirtualMasterKind::Keyboard { .. } => "KEYBOARD",
        }
    }
}

fn dispatch_x_input_request(
    context: XDispatchContext,
    request: XWireRequest,
    runtime: &mut XAuthorityRuntime,
    atoms: &mut XAtomTable,
) -> XDispatchFamilyResult {
    if !matches!(
        &request,
            XWireRequest::XiGetExtensionVersion
            | XWireRequest::XiListInputDevices
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
                        || validate_window_or_root_access(runtime, context.namespace, window).is_err()
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
                    let result = validate_window_or_root_access(
                        runtime,
                        context.namespace,
                        window,
                    )
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
                                u16::from(crate::X_INPUT_CHANGE_CURSOR_MINOR_OPCODE),
                                resource_id))
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
                    // Device 0 is AllDevices and 1 is AllMasterDevices; both name the
                    // whole pair. Anything outside the pair reports no devices rather
                    // than an error, which is what a client probing an absent device
                    // expects.
                    let devices = X_VIRTUAL_MASTER_DEVICES
                        .iter()
                        .filter(|device| {
                            matches!(device_id, 0 | 1) || device_id == device.device_id
                        })
                        .map(|device| device.xi2_device_info())
                        .collect();
                    XDispatchResult {
                        response: None,
                        outputs: vec![XClientOutput::Reply(XClientReply::XiQueryDevice {
                            sequence: context.sequence,
                            devices,
                        })],
                        metadata_candidates: Vec::new(),
                    }
                }
                // XI1's device enumeration. Sophia advertises XInputExtension and
                // answers the XI1 version handshake, so refusing the enumeration that
                // follows it left an advertised extension half-implemented; a real
                // session failed on the resulting BadRequest storm.
                XWireRequest::XiListInputDevices => {
                    let devices = X_VIRTUAL_MASTER_DEVICES
                        .iter()
                        .map(|device| {
                            let device_type = atoms
                                .intern(device.legacy_type_name(), false)
                                .ok()
                                .flatten()
                                .unwrap_or(X_ATOM_NONE);
                            device.xi1_device_info(device_type)
                        })
                        .collect();
                    XDispatchResult {
                        response: None,
                        outputs: vec![XClientOutput::Reply(XClientReply::XiListInputDevices {
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
                                u16::from(crate::X_INPUT_SELECT_EVENTS_MINOR_OPCODE),
                                u32::try_from(window.local.raw()).unwrap_or(0)))
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
