#[cfg(unix)]
fn clamp_input_coordinate(value: f64) -> i16 {
    if !value.is_finite() {
        return 0;
    }
    value
        .floor()
        .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
}

#[cfg(unix)]
fn encode_xi_device_event(
    byte_order: XByteOrder,
    sequence: u16,
    event_type: u16,
    event: XAuthorityInputEvent,
    event_window: XResourceId,
) -> Vec<u8> {
    let (device, time, detail, root_x, root_y, event_x, event_y, state) = match event {
        XAuthorityInputEvent::Key(key) => (
            3,
            key.time_msec,
            u32::from(key.keycode),
            0,
            0,
            0,
            0,
            key.state,
        ),
        XAuthorityInputEvent::Pointer(pointer) => (
            2,
            pointer.time_msec,
            match pointer.kind {
                XAuthorityPointerEventKind::Button { button, .. } => u32::from(button),
                XAuthorityPointerEventKind::Axis { .. } => 0,
                XAuthorityPointerEventKind::Motion => 0,
            },
            pointer.root_x,
            pointer.root_y,
            pointer.event_x,
            pointer.event_y,
            pointer.state,
        ),
    };
    let mut out = vec![0; 80];
    out[0] = 35;
    out[1] = crate::X_INPUT_MAJOR_OPCODE;
    write_xi_u16(byte_order, &mut out[2..4], sequence);
    write_xi_u16(byte_order, &mut out[8..10], event_type);
    write_xi_u16(byte_order, &mut out[10..12], device);
    write_xi_u32(byte_order, &mut out[12..16], time);
    write_xi_u32(byte_order, &mut out[16..20], detail);
    write_xi_u32(byte_order, &mut out[20..24], X_SETUP_DEFAULT_ROOT);
    write_xi_u32(
        byte_order,
        &mut out[24..28],
        u32::try_from(event_window.local.raw()).unwrap_or(0),
    );
    write_xi_u32(
        byte_order,
        &mut out[32..36],
        (i32::from(root_x) << 16) as u32,
    );
    write_xi_u32(
        byte_order,
        &mut out[36..40],
        (i32::from(root_y) << 16) as u32,
    );
    write_xi_u32(
        byte_order,
        &mut out[40..44],
        (i32::from(event_x) << 16) as u32,
    );
    write_xi_u32(
        byte_order,
        &mut out[44..48],
        (i32::from(event_y) << 16) as u32,
    );
    write_xi_u16(byte_order, &mut out[52..54], device);
    write_xi_u32(byte_order, &mut out[72..76], u32::from(state & 0xff));
    if let XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
        kind:
            XAuthorityPointerEventKind::Axis {
                horizontal_position_v120,
                vertical_position_v120,
                ..
            },
        ..
    }) = event
        && (horizontal_position_v120.is_some() || vertical_position_v120.is_some())
    {
        write_xi_u16(byte_order, &mut out[50..52], 1);
        let mut mask = 0u8;
        if horizontal_position_v120.is_some() {
            mask |= 1;
        }
        if vertical_position_v120.is_some() {
            mask |= 1 << 1;
        }
        out.extend_from_slice(&[mask, 0, 0, 0]);
        for position in [horizontal_position_v120, vertical_position_v120]
            .into_iter()
            .flatten()
        {
            let fixed = i64::from(position) << 32;
            match byte_order {
                XByteOrder::LittleEndian => out.extend_from_slice(&fixed.to_le_bytes()),
                XByteOrder::BigEndian => out.extend_from_slice(&fixed.to_be_bytes()),
            }
        }
    }
    let length = u32::try_from((out.len() - 32) / 4).unwrap_or(u32::MAX);
    write_xi_u32(byte_order, &mut out[4..8], length);
    out
}

#[cfg(unix)]
fn encode_xi_crossing_event(
    byte_order: XByteOrder,
    sequence: u16,
    event_type: u16,
    event: XAuthorityInputEvent,
    event_window: XResourceId,
) -> Vec<u8> {
    let (device, time, root_x, root_y, event_x, event_y, state) = match event {
        XAuthorityInputEvent::Key(key) => (3, key.time_msec, 0, 0, 0, 0, key.state),
        XAuthorityInputEvent::Pointer(pointer) => (
            2,
            pointer.time_msec,
            pointer.root_x,
            pointer.root_y,
            pointer.event_x,
            pointer.event_y,
            pointer.state,
        ),
    };
    let mut out = vec![0; 72];
    out[0] = 35;
    out[1] = crate::X_INPUT_MAJOR_OPCODE;
    write_xi_u16(byte_order, &mut out[2..4], sequence);
    write_xi_u32(byte_order, &mut out[4..8], 10);
    write_xi_u16(byte_order, &mut out[8..10], event_type);
    write_xi_u16(byte_order, &mut out[10..12], device);
    write_xi_u32(byte_order, &mut out[12..16], time);
    write_xi_u16(byte_order, &mut out[16..18], device);
    out[18] = 0;
    out[19] = 3;
    write_xi_u32(byte_order, &mut out[20..24], X_SETUP_DEFAULT_ROOT);
    write_xi_u32(
        byte_order,
        &mut out[24..28],
        u32::try_from(event_window.local.raw()).unwrap_or(0),
    );
    write_xi_u32(
        byte_order,
        &mut out[32..36],
        (i32::from(root_x) << 16) as u32,
    );
    write_xi_u32(
        byte_order,
        &mut out[36..40],
        (i32::from(root_y) << 16) as u32,
    );
    write_xi_u32(
        byte_order,
        &mut out[40..44],
        (i32::from(event_x) << 16) as u32,
    );
    write_xi_u32(
        byte_order,
        &mut out[44..48],
        (i32::from(event_y) << 16) as u32,
    );
    out[48] = 1;
    out[49] = 1;
    write_xi_u32(byte_order, &mut out[64..68], u32::from(state & 0xff));
    out
}

#[cfg(unix)]
fn write_xi_u16(byte_order: XByteOrder, out: &mut [u8], value: u16) {
    match byte_order {
        XByteOrder::LittleEndian => out.copy_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.copy_from_slice(&value.to_be_bytes()),
    }
}

#[cfg(unix)]
fn write_xi_u32(byte_order: XByteOrder, out: &mut [u8], value: u32) {
    match byte_order {
        XByteOrder::LittleEndian => out.copy_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.copy_from_slice(&value.to_be_bytes()),
    }
}

#[cfg(unix)]
enum X11InputEventReceiver {
    Plain(Receiver<XAuthorityInputEvent>),
    Routed {
        receiver: Receiver<XAuthorityClientInputEvent>,
        deliveries: Option<SyncSender<XAuthorityClientInputDelivery>>,
    },
}

#[cfg(unix)]
type X11ReceivedInputEvent = (
    XAuthorityInputEvent,
    Option<XResourceId>,
    Option<u16>,
    Option<XResourceId>,
    u16,
    Option<XAuthorityInputDeliveryId>,
);

#[cfg(unix)]
impl X11InputEventReceiver {
    fn recv_timeout(
        &self,
        client: XServerFrontendClientId,
    ) -> Result<X11ReceivedInputEvent, RecvTimeoutError> {
        match self {
            Self::Plain(receiver) => receiver
                .recv_timeout(Duration::from_millis(10))
                .map(|event| (event, None, None, None, 0, None)),
            Self::Routed { receiver, .. } => {
                match receiver.recv_timeout(Duration::from_millis(10)) {
                    Ok(route) if route.client == client => Ok((
                        route.event,
                        route.target_window,
                        route.xi_event_type,
                        route.xi_event_window,
                        route.xi_transition_mask,
                        route.delivery,
                    )),
                    // Drop one misaddressed route, then let the writer loop
                    // observe its stop flag before it receives again.
                    Ok(_) => Err(RecvTimeoutError::Timeout),
                    Err(error) => Err(error),
                }
            }
        }
    }

    fn send_delivery(
        &self,
        client: XServerFrontendClientId,
        delivery: Option<XAuthorityInputDeliveryId>,
        outcome: XAuthorityInputDeliveryOutcome,
    ) -> Result<(), X11SetupSocketError> {
        let Some(delivery) = delivery else {
            return Ok(());
        };
        let Self::Routed {
            deliveries: Some(sender),
            ..
        } = self
        else {
            return Ok(());
        };
        match sender.try_send(XAuthorityClientInputDelivery {
            client,
            delivery,
            outcome,
        }) {
            Ok(()) | Err(TrySendError::Disconnected(_)) => Ok(()),
            Err(TrySendError::Full(_)) => Err(X11SetupSocketError::new(
                "X11 input delivery acknowledgement channel is full",
            )),
        }
    }
}

#[cfg(unix)]
enum X11ControlChannels {
    Routed {
        receiver: Receiver<XAuthorityClientControlCommand>,
        acknowledgements: SyncSender<XAuthorityClientControlAck>,
    },
    ClientBound {
        receiver: Receiver<XAuthorityControlCommand>,
        acknowledgements: SyncSender<XAuthorityClientControlAck>,
    },
}

#[cfg(unix)]
impl X11ControlChannels {
    fn recv_timeout(
        &self,
        client: XServerFrontendClientId,
    ) -> Result<XAuthorityControlCommand, RecvTimeoutError> {
        match self {
            Self::Routed { receiver, .. } => {
                match receiver.recv_timeout(Duration::from_millis(10)) {
                    Ok(route) if route.client == client => Ok(route.command),
                    // Drop one misaddressed route, then let the writer
                    // loop observe its stop flag before it receives again.
                    Ok(_) => Err(RecvTimeoutError::Timeout),
                    Err(error) => Err(error),
                }
            }
            Self::ClientBound { receiver, .. } => receiver.recv_timeout(Duration::from_millis(10)),
        }
    }

    fn send_ack(
        &self,
        client: XServerFrontendClientId,
        acknowledgement: XAuthorityControlAck,
    ) -> Result<(), X11SetupSocketError> {
        match self {
            Self::Routed {
                acknowledgements, ..
            }
            | Self::ClientBound {
                acknowledgements, ..
            } => match acknowledgements.try_send(XAuthorityClientControlAck {
                client,
                acknowledgement,
            }) {
                Ok(()) | Err(TrySendError::Disconnected(_)) => Ok(()),
                Err(TrySendError::Full(_)) => Err(X11SetupSocketError::new(
                    "X11 control acknowledgement channel is full",
                )),
            },
        }
    }
}

#[cfg(unix)]
impl From<XAuthorityKeyEvent> for XAuthorityInputEvent {
    fn from(event: XAuthorityKeyEvent) -> Self {
        Self::Key(event)
    }
}

#[cfg(unix)]
impl XServerFrontendRouteRegistry {
    fn route_resolved_input(
        &self,
        namespace: NamespaceId,
        client: XServerFrontendClientId,
        surface_window: XResourceId,
        target_window: Option<XResourceId>,
        event: XAuthorityInputEvent,
        delivery: Option<XAuthorityInputDeliveryId>,
    ) -> Result<(), XServerFrontendRouteError> {
        let (xi_device, selected_type) = match event {
            XAuthorityInputEvent::Key(key) => (3, Some(if key.pressed { 2 } else { 3 })),
            XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                kind: XAuthorityPointerEventKind::Button { pressed, .. },
                ..
            }) => (2, Some(if pressed { 4 } else { 5 })),
            XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                kind:
                    XAuthorityPointerEventKind::Axis {
                        horizontal_position_v120,
                        vertical_position_v120,
                        ..
                    },
                ..
            }) => (
                2,
                (horizontal_position_v120.is_some() || vertical_position_v120.is_some())
                    .then_some(6),
            ),
            XAuthorityInputEvent::Pointer(_) => (2, Some(6)),
        };
        let event_window = target_window.unwrap_or(surface_window);
        let event_ancestry = self.window_ancestry(client, event_window)?;
        let xi_event_window = if let Some(selected_type) = selected_type {
            let authority = self
                .input_authority
                .lock()
                .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
            event_ancestry
                .iter()
                .find(|window| {
                    authority.xi_event_selected(
                        namespace,
                        client.raw(),
                        **window,
                        xi_device,
                        selected_type,
                    )
                })
                .copied()
        } else {
            None
        };
        let xi_event_type = xi_event_window.and(selected_type);
        let transition_types: &[u16] = if xi_device == 3 { &[9, 10] } else { &[7, 8] };
        let authority = self
            .input_authority
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        let xi_transition_mask = transition_types.iter().fold(0u16, |mask, event_type| {
            if event_ancestry.iter().any(|window| {
                authority.xi_event_selected(
                    namespace,
                    client.raw(),
                    *window,
                    xi_device,
                    *event_type,
                )
            }) {
                mask | (1 << event_type)
            } else {
                mask
            }
        });
        self.route_input(XAuthorityClientInputEvent {
            client,
            event,
            target_window,
            xi_event_type,
            xi_event_window,
            xi_transition_mask,
            delivery,
        })
    }

    fn route_is_frozen(
        &self,
        route: &XAuthorityRoutedInput,
        namespace: NamespaceId,
    ) -> Result<bool, XServerFrontendRouteError> {
        let authority = self
            .input_authority
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        Ok(match route.request.kind {
            InputEventKind::Key { .. } => authority.keyboard_frozen(namespace),
            InputEventKind::PointerMotion
            | InputEventKind::PointerButton { .. }
            | InputEventKind::PointerAxis { .. } => authority.pointer_frozen(namespace),
        })
    }

    fn drain_thawed_input(&self) -> Result<usize, XServerFrontendRouteError> {
        let queued = self
            .frozen_input
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .len();
        let mut routed = 0usize;
        for _ in 0..queued {
            let deferred = self
                .frozen_input
                .lock()
                .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                .pop_front();
            let Some(deferred) = deferred else { break };
            let route = deferred.route;
            let surface_route = self
                .surfaces
                .lock()
                .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                .get(&route.request.target_surface)
                .copied();
            let Some(surface_route) = surface_route else {
                self.send_input_delivery(
                    deferred.client,
                    route.delivery,
                    XAuthorityInputDeliveryOutcome::RouteRejected,
                )?;
                continue;
            };
            if self.route_is_frozen(&route, surface_route.namespace)? {
                self.frozen_input
                    .lock()
                    .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                    .push_back(XDeferredRoutedInput {
                        client: deferred.client,
                        route,
                    });
            } else {
                self.route_engine_input(route)?;
                routed = routed.saturating_add(1);
            }
        }
        Ok(routed)
    }
}
