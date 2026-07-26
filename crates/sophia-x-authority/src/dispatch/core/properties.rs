fn dispatch_core_property_request(
    context: XDispatchContext,
    request: XWireRequest,
    runtime: &mut XAuthorityRuntime,
    atoms: &mut XAtomTable,
    properties: &mut XPropertyTable,
) -> XDispatchFamilyResult {
    if !matches!(
        &request,
            XWireRequest::InternAtom { .. }
            | XWireRequest::GetAtomName { .. }
            | XWireRequest::ChangeProperty(..)
            | XWireRequest::DeleteProperty { .. }
            | XWireRequest::GetProperty(..)
            | XWireRequest::ListProperties { .. }
            | XWireRequest::GetSelectionOwner { .. }
            | XWireRequest::SendSelectionNotify { .. }
    ) {
        return Unhandled(request);
    }
    Handled(match request {
                XWireRequest::InternAtom {
                    only_if_exists,
                    name,
                } => {
                    let output = match atoms.intern(&name, only_if_exists) {
                        Ok(atom) => XClientOutput::Reply(XClientReply::InternAtom {
                            sequence: context.sequence,
                            atom: atom.unwrap_or(0),
                        }),
                        Err(_) => XClientOutput::Error(crate::XClientError {
                            code: crate::XErrorCode::BadValue,
                            sequence: context.sequence,
                            resource_id: 0,
                            minor_code: 0,
                            major_code: context.major_opcode,
                        }),
                    };
                    XDispatchResult {
                        response: None,
                        outputs: vec![output],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::GetAtomName { atom } => {
                    let output = match atoms.name(atom) {
                        Some(name) => XClientOutput::Reply(XClientReply::GetAtomName {
                            sequence: context.sequence,
                            name: name.to_owned(),
                        }),
                        None => XClientOutput::Error(crate::XClientError {
                            code: crate::XErrorCode::BadAtom,
                            sequence: context.sequence,
                            resource_id: atom,
                            minor_code: 0,
                            major_code: context.major_opcode,
                        }),
                    };
                    XDispatchResult {
                        response: None,
                        outputs: vec![output],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::ChangeProperty(change) => {
                    let window_access = if change.window.local.raw() == u64::from(crate::X_SETUP_DEFAULT_ROOT) { Ok(()) } else { runtime.validate_window_access(context.namespace, change.window) };
                    let (output, metadata_candidates) = match window_access {
                        Err(error) => (
                            XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(change.window.local.raw()).unwrap_or(0),
                            )),
                            Vec::new(),
                        ),
                        Ok(()) => match properties.apply_change(context.namespace, change.clone()) {
                            Ok(record) => {
                                let candidate = metadata_property_candidate(&record, atoms);
                                (
                                    XClientOutput::Event(XClientEvent::PropertyNotify {
                                        sequence: context.sequence,
                                        window: record.window,
                                        atom: record.property,
                                        time: 0,
                                        new_value: true,
                                    }),
                                    candidate.into_iter().collect(),
                                )
                            }
                            Err(_) => (
                                XClientOutput::Error(crate::XClientError {
                                    code: crate::XErrorCode::BadValue,
                                    sequence: context.sequence,
                                    resource_id: u32::try_from(change.window.local.raw()).unwrap_or(0),
                                    minor_code: 0,
                                    major_code: context.major_opcode,
                                }),
                                Vec::new(),
                            ),
                        },
                    };
                    XDispatchResult {
                        response: None,
                        outputs: vec![output],
                        metadata_candidates,
                    }
                }
                XWireRequest::DeleteProperty { window, property } => {
                    let access = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                        Ok(())
                    } else {
                        runtime.validate_window_access(context.namespace, window)
                    };
                    let outputs = match access {
                        Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(window.local.raw()).unwrap_or(0),
                        ))],
                        Ok(()) => properties
                            .remove(context.namespace, window, property)
                            .map(|_| {
                                XClientOutput::Event(XClientEvent::PropertyNotify {
                                    sequence: context.sequence,
                                    window,
                                    atom: property,
                                    time: 0,
                                    new_value: false,
                                })
                            })
                            .into_iter()
                            .collect(),
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::GetProperty(read) => {
                    let window = read.window;
                    let property = read.property;
                    let outputs = if property == crate::X_PROPERTY_ANY_TYPE
                        || atoms.name(read.property).is_none()
                        || atom_type_is_unknown(atoms, read.property_type)
                    {
                        vec![XClientOutput::Error(crate::XClientError {
                            code: crate::XErrorCode::BadAtom,
                            sequence: context.sequence,
                            resource_id: property,
                            minor_code: 0,
                            major_code: context.major_opcode,
                        })]
                    } else if window.local.raw() == u64::from(crate::X_SETUP_DEFAULT_ROOT) {
                        x_client_outputs_from_property_read(
                            &context,
                            window,
                            property,
                            properties.read_property(context.namespace, read),
                        )
                    } else if let Err(error) =
                        runtime.validate_window_access(context.namespace, window)
                    {
                        vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(window.local.raw()).unwrap_or(0),
                        ))]
                    } else {
                        x_client_outputs_from_property_read(
                            &context,
                            window,
                            property,
                            properties.read_property(context.namespace, read),
                        )
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::ListProperties { window } => {
                    let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                        XClientOutput::Reply(XClientReply::ListProperties {
                            sequence: context.sequence,
                            atoms: properties.properties_for_window(context.namespace, window),
                        })
                    } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
                        XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(window.local.raw()).unwrap_or(0),
                        ))
                    } else {
                        XClientOutput::Reply(XClientReply::ListProperties {
                            sequence: context.sequence,
                            atoms: properties.properties_for_window(context.namespace, window),
                        })
                    };
                    XDispatchResult {
                        response: None,
                        outputs: vec![output],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::GetSelectionOwner { selection } => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::GetSelectionOwner {
                        sequence: context.sequence,
                        owner: runtime.selection_owner(context.namespace, selection),
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::SendSelectionNotify {
                    destination,
                    event_mask,
                    mut event,
                } => {
                    let requestor = match &event {
                        XClientEvent::SelectionNotify { requestor, .. } => Some(*requestor),
                        XClientEvent::ClientMessage { .. } => None,
                        _ => unreachable!("wire decoder admits only sendable events"),
                    };
                    let validation = if destination.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                        Ok(())
                    } else {
                        runtime.validate_window_access(context.namespace, destination)
                    }
                    .and_then(|()| {
                        requestor
                            .map(|requestor| runtime.validate_window_access(context.namespace, requestor))
                            .unwrap_or(Ok(()))
                    });
                    let outputs = match validation {
                        Ok(())
                            if requestor
                                .is_none_or(|requestor| event_mask == 0 && destination == requestor) =>
                        {
                            match &mut event {
                                XClientEvent::SelectionNotify { sequence, .. }
                                | XClientEvent::ClientMessage { sequence, .. } => {
                                    *sequence = context.sequence;
                                }
                                _ => unreachable!("wire decoder admits only sendable events"),
                            }
                            vec![XClientOutput::Event(event)]
                        }
                        Ok(()) => vec![XClientOutput::Error(crate::XClientError {
                            code: XErrorCode::BadValue,
                            sequence: context.sequence,
                            resource_id: u32::try_from(destination.local.raw()).unwrap_or(0),
                            minor_code: 0,
                            major_code: context.major_opcode,
                        })],
                        Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(destination.local.raw()).unwrap_or(0),
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
