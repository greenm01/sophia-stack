fn dispatch_core_window_request(
    context: XDispatchContext,
    request: XWireRequest,
    runtime: &mut XAuthorityRuntime,
    atoms: &mut XAtomTable,
    properties: &mut XPropertyTable,
) -> XDispatchFamilyResult {
    if !matches!(
        &request,
            XWireRequest::CreateWindow { .. }
            | XWireRequest::Authority(..)
            | XWireRequest::ChangeWindowAttributes { .. }
            | XWireRequest::GetWindowAttributes { .. }
            | XWireRequest::DestroyWindow { .. }
            | XWireRequest::ReparentWindow { .. }
            | XWireRequest::MapSubwindows { .. }
            | XWireRequest::UnmapWindow { .. }
            | XWireRequest::ConfigureWindow { .. }
            | XWireRequest::GetGeometry { .. }
            | XWireRequest::GetImage { .. }
            | XWireRequest::QueryTree { .. }
    ) {
        return Unhandled(request);
    }
    Handled(match request {
                XWireRequest::CreateWindow {
                    packet,
                    parent,
                    background_pixel,
                    override_redirect,
                    depth,
                    visual,
                    colormap,
                    ..
                } => {
                    let kind = packet.kind.clone();
                    let namespace = packet.namespace;
                    let transaction = packet.transaction;
                    let parent_access = if parent.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                        Ok(())
                    } else {
                        runtime.validate_window_access(namespace, parent)
                    };
                    let mut response = match parent_access {
                        Ok(()) => runtime.apply(packet),
                        Err(error) => XAuthorityResponsePacket::rejected(transaction, error),
                    };
                    if response.outcome == XAuthorityResponseOutcome::Accepted
                        && let XAuthorityRequestKind::CreateWindow { window, .. } = &kind
                    {
                        if let Err(error) = runtime.set_window_parent(namespace, *window, parent) {
                            let _ = runtime.destroy_window(namespace, *window);
                            response = XAuthorityResponsePacket::rejected(transaction, error);
                        }
                    }
                    if response.outcome == XAuthorityResponseOutcome::Accepted
                        && let XAuthorityRequestKind::CreateWindow { window, .. } = &kind
                    {
                        if let Ok(surface) = runtime.set_window_override_redirect(
                            namespace,
                            *window,
                            override_redirect,
                        ) {
                            response.surfaces.clear();
                            response.surfaces.push(surface);
                        }
                        let _ = runtime.set_window_background_pixel(
                            namespace,
                            *window,
                            background_pixel.unwrap_or(0),
                        );
                        let resolved_visual = if visual == 0 {
                            X_SETUP_DEFAULT_VISUAL
                        } else {
                            visual
                        };
                        let resolved_depth = if depth == 0 {
                            if resolved_visual == X_SETUP_ARGB_VISUAL {
                                32
                            } else {
                                24
                            }
                        } else {
                            depth
                        };
                        runtime.set_window_visual(
                            *window,
                            resolved_depth,
                            resolved_visual,
                            colormap.unwrap_or(XResourceId::new(u64::from(X_SETUP_DEFAULT_COLORMAP), 1)),
                        );
                    }
                    let outputs = outputs_from_authority_response(context, &kind, &response);
                    XDispatchResult {
                        response: Some(response),
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::Authority(mut packet) => {
                    if let XAuthorityRequestKind::RequestSelection {
                        target,
                        target_name,
                        ..
                    } = &mut packet.kind
                        && let Some(name) = atoms.name(*target)
                    {
                        *target_name = name.to_owned();
                    }
                    let kind = packet.kind.clone();
                    let response = runtime.apply(packet);
                    if let XAuthorityRequestKind::RequestSelection { transfer, .. } = &kind {
                        runtime.set_pending_clipboard_byte_order(*transfer, context.byte_order);
                    }
                    let outputs = outputs_from_authority_response(context, &kind, &response);
                    XDispatchResult {
                        response: Some(response),
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::ChangeWindowAttributes {
                    window,
                    override_redirect,
                    ..
                } => {
                    let transaction = TransactionId::from_raw(u64::from(context.sequence));
                    let mut response = XAuthorityResponsePacket::accepted(transaction);
                    let outputs = if let Err(error) =
                        runtime.validate_drawable_access(context.namespace, window)
                    {
                        response = XAuthorityResponsePacket::rejected(transaction, error);
                        vec![XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(window.local.raw()).unwrap_or(0),
                            ))]
                    } else if let Some(override_redirect) = override_redirect {
                        match runtime.set_window_override_redirect(
                            context.namespace,
                            window,
                            override_redirect,
                        ) {
                            Ok(surface) => {
                                response.surfaces.push(surface);
                                Vec::new()
                            }
                            Err(error) => {
                                response = XAuthorityResponsePacket::rejected(transaction, error);
                                vec![XClientOutput::Error(x_error_from_runtime(
                                    error,
                                    context.sequence,
                                    context.major_opcode,
                                    u32::try_from(window.local.raw()).unwrap_or(0),
                                ))]
                            }
                        }
                    } else {
                        Vec::new()
                    };
                    XDispatchResult {
                        response: override_redirect.map(|_| response),
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::GetWindowAttributes { window } => {
                    let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                        XClientOutput::Reply(XClientReply::GetWindowAttributes {
                            sequence: context.sequence,
                            visual: X_SETUP_DEFAULT_VISUAL,
                            colormap: XResourceId::new(u64::from(X_SETUP_DEFAULT_COLORMAP), 1),
                            map_state: 2,
                            override_redirect: false,
                        })
                    } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
                        XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(window.local.raw()).unwrap_or(0),
                        ))
                    } else {
                        let (_, visual, colormap) = runtime.window_visual(window);
                        let override_redirect = runtime
                            .window_override_redirect(context.namespace, window)
                            .unwrap_or(false);
                        let map_state = runtime
                            .window_map_state(context.namespace, window)
                            .map_or(0, x11_map_state);
                        XClientOutput::Reply(XClientReply::GetWindowAttributes {
                            sequence: context.sequence,
                            visual,
                            colormap,
                            map_state,
                            override_redirect,
                        })
                    };
                    XDispatchResult {
                        response: None,
                        outputs: vec![output],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::DestroyWindow { window } => {
                    let transaction = TransactionId::from_raw(u64::from(context.sequence));
                    let mut response = XAuthorityResponsePacket::accepted(transaction);
                    let outputs = match runtime.destroy_window(context.namespace, window) {
                        Ok(surface) => {
                            properties.remove_window(context.namespace, window);
                            response.removed_surfaces.push(surface);
                            Vec::new()
                        }
                        Err(error) => {
                            response = XAuthorityResponsePacket::rejected(transaction, error);
                            vec![XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(window.local.raw()).unwrap_or(0),
                            ))]
                        }
                    };
                    XDispatchResult {
                        response: Some(response),
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::ReparentWindow {
                    window,
                    parent,
                    x,
                    y,
                } => {
                    let result = runtime
                        .set_window_parent(context.namespace, window, parent)
                        .and_then(|()| {
                            runtime.configure_window_geometry(
                                context.namespace,
                                window,
                                XWindowGeometryUpdate {
                                    x: Some(x),
                                    y: Some(y),
                                    generation: u64::from(context.sequence),
                                    ..XWindowGeometryUpdate::default()
                                },
                            )
                        });
                    let outputs = result
                        .err()
                        .map(|error| {
                            XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(window.local.raw()).unwrap_or(0),
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
                XWireRequest::MapSubwindows { window } => {
                    let transaction = TransactionId::from_raw(u64::from(context.sequence));
                    let mut response = XAuthorityResponsePacket::accepted(transaction);
                    let outputs = match runtime.map_direct_subwindows(
                        context.namespace,
                        window,
                        u64::from(context.sequence),
                    ) {
                        Ok(surfaces) => {
                            response.surfaces = surfaces;
                            response
                                .surfaces
                                .iter()
                                .filter(|surface| surface.mapped)
                                .flat_map(|surface| {
                                    let window = XResourceId {
                                        local: surface.local_id,
                                    };
                                    let override_redirect =
                                        surface.presentation
                                            == sophia_protocol::SurfacePresentationRole::ClientPositioned;
                                    vec![
                                        XClientOutput::Event(XClientEvent::MapNotify {
                                            sequence: context.sequence,
                                            event: window,
                                            window,
                                            override_redirect,
                                        }),
                                        XClientOutput::Event(XClientEvent::VisibilityNotify {
                                            sequence: context.sequence,
                                            window,
                                            state: 0,
                                        }),
                                        XClientOutput::Event(XClientEvent::Expose {
                                            sequence: context.sequence,
                                            window,
                                            x: 0,
                                            y: 0,
                                            width: clamp_u16(surface.geometry.width),
                                            height: clamp_u16(surface.geometry.height),
                                            count: 0,
                                        }),
                                    ]
                                })
                                .collect()
                        }
                        Err(error) => {
                            response = XAuthorityResponsePacket::rejected(transaction, error);
                            vec![XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(window.local.raw()).unwrap_or(0),
                            ))]
                        }
                    };
                    XDispatchResult {
                        response: Some(response),
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::UnmapWindow { window } => {
                    let outputs = if let Err(error) =
                        runtime.unmap_window(context.namespace, window)
                    {
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
                XWireRequest::ConfigureWindow {
                    window,
                    x,
                    y,
                    width,
                    height,
                    ..
                } => {
                    let outputs = if let Err(error) = runtime.configure_window_geometry(
                        context.namespace,
                        window,
                        XWindowGeometryUpdate {
                            x,
                            y,
                            width,
                            height,
                            generation: u64::from(context.sequence),
                        },
                    ) {
                        vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(window.local.raw()).unwrap_or(0),
                        ))]
                    } else {
                        match runtime.window_geometry(context.namespace, window) {
                            Ok(geometry) => {
                                let override_redirect = runtime
                                    .window_override_redirect(context.namespace, window)
                                    .unwrap_or(false);
                                vec![XClientOutput::Event(XClientEvent::ConfigureNotify {
                                sequence: context.sequence,
                                event: window,
                                window,
                                above_sibling: None,
                                x: clamp_i16(geometry.x),
                                y: clamp_i16(geometry.y),
                                width: clamp_u16(geometry.width),
                                height: clamp_u16(geometry.height),
                                border_width: 0,
                                override_redirect,
                            })]
                            }
                            Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(window.local.raw()).unwrap_or(0),
                            ))],
                        }
                    };
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::GetGeometry { drawable } => {
                    let output = if drawable.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                        XClientOutput::Reply(XClientReply::GetGeometry {
                            sequence: context.sequence,
                            depth: 24,
                            root: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
                            geometry: Rect {
                                x: 0,
                                y: 0,
                                width: runtime
                                    .output_topology()
                                    .root_size()
                                    .expect("validated output topology")
                                    .width,
                                height: runtime
                                    .output_topology()
                                    .root_size()
                                    .expect("validated output topology")
                                    .height,
                            },
                            border_width: 0,
                        })
                    } else {
                        match runtime.window_geometry(context.namespace, drawable) {
                            Ok(geometry) => XClientOutput::Reply(XClientReply::GetGeometry {
                                sequence: context.sequence,
                                depth: runtime.window_visual(drawable).0,
                                root: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
                                geometry,
                                border_width: 0,
                            }),
                            Err(error) => XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(drawable.local.raw()).unwrap_or(0),
                            )),
                        }
                    };
                    XDispatchResult {
                        response: None,
                        outputs: vec![output],
                        metadata_candidates: Vec::new(),
                    }
                }
                XWireRequest::GetImage {
                    drawable,
                    width,
                    height,
                    ..
                } => {
                    let outputs = match runtime.validate_drawable_access(context.namespace, drawable) {
                        Ok(()) => vec![XClientOutput::Reply(XClientReply::GetImage {
                            sequence: context.sequence,
                            depth: 24,
                            visual: crate::X_SETUP_DEFAULT_VISUAL,
                            data: vec![0; usize::from(width) * usize::from(height) * 4],
                        })],
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
                XWireRequest::QueryTree { window } => {
                    let output =
                        match runtime.window_parent_and_children(context.namespace, window) {
                            Ok((parent, children)) => {
                                XClientOutput::Reply(XClientReply::QueryTree {
                                    sequence: context.sequence,
                                    root: XResourceId::new(
                                        u64::from(X_SETUP_DEFAULT_ROOT),
                                        1,
                                    ),
                                    parent,
                                    children,
                                })
                            }
                            Err(error) => XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(window.local.raw()).unwrap_or(0),
                        )),
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

fn x11_map_state(state: crate::XMapState) -> u8 {
    match state {
        crate::XMapState::Unmapped | crate::XMapState::PolicyPending => 0,
        crate::XMapState::Mapped => 2,
    }
}
