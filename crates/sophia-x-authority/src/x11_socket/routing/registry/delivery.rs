impl XServerFrontendRouteRegistry {
    fn advance_input_control_epoch(&self) -> Result<usize, XServerFrontendRouteError> {
        self.input_authority
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .advance_security_epoch();
        let drained = {
            let mut frozen = self
                .frozen_input
                .lock()
                .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
            std::mem::take(&mut *frozen)
        };
        let count = drained.len();
        for deferred in drained {
            self.send_input_delivery(
                deferred.client,
                deferred.route.delivery,
                XAuthorityInputDeliveryOutcome::EpochRevoked,
            )?;
        }
        Ok(count)
    }

    fn release_route_lease(
        &self,
        release: XAuthorityRouteLeaseRelease,
    ) -> Result<(), XServerFrontendRouteError> {
        let client = self
            .clients
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .iter()
            .find_map(|(client, route)| {
                (route.admission == Some(release.admission)).then_some(*client)
            });
        if let Some(client) = client {
            self.input_authority
                .lock()
                .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                .ungrab_pointer(release.admission.namespace.id, client.raw());
        }
        let Some(sender) = self.route_lease_update_sender.as_ref() else {
            return Ok(());
        };
        let _ = sender.send(XAuthorityRouteLeaseUpdate {
            identity: release.identity,
            target_surface: self
                .surfaces
                .lock()
                .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                .iter()
                .find_map(|(surface, route)| {
                    (route.admission == Some(release.admission)).then_some(*surface)
                })
                .unwrap_or(SurfaceId::INVALID),
            admission: release.admission,
            kind: XAuthorityRouteLeaseUpdateKind::Released,
        });
        Ok(())
    }

    fn route_engine_input(
        &self,
        route: XAuthorityRoutedInput,
        route_control_epoch: u64,
        current_control_epoch: u64,
    ) -> Result<(), XServerFrontendRouteError> {
        // An event stamped with a closed epoch is one the session revoked
        // between routing and delivery, not one that failed to route.
        if route_control_epoch != current_control_epoch {
            if let Ok(surfaces) = self.surfaces.lock()
                && let Some(surface_route) = surfaces.get(&route.request.target_surface)
            {
                self.send_input_delivery(
                    surface_route.client,
                    route.delivery,
                    XAuthorityInputDeliveryOutcome::EpochRevoked,
                )?;
            }
            return Ok(());
        }
        if route.mode == XAuthorityRoutedInputMode::StateOnly {
            if let InputEventKind::Key { keycode, pressed } = route.request.kind {
                let _ = self.xkb_worker.request(XkbWorkerCommand::Key {
                    seat: route.request.seat,
                    keycode,
                    pressed,
                })?;
            }
            return Ok(());
        }
        let surface_route = self
            .surfaces
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .get(&route.request.target_surface)
            .copied()
            .ok_or(XServerFrontendRouteError::UnknownSurface {
                surface: route.request.target_surface,
            })?;
        if self.route_is_frozen(&route, surface_route.namespace)? {
            let mut frozen = self
                .frozen_input
                .lock()
                .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
            if frozen.len() >= self.per_client_input_capacity.get() {
                drop(frozen);
                self.send_input_delivery(
                    surface_route.client,
                    route.delivery,
                    XAuthorityInputDeliveryOutcome::RouteRejected,
                )?;
                return Err(XServerFrontendRouteError::ClientQueueFull {
                    client: surface_route.client,
                });
            }
            frozen.push_back(XDeferredRoutedInput {
                client: surface_route.client,
                control_epoch: route_control_epoch,
                route,
            });
            return Ok(());
        }
        let mut client = surface_route.client;
        // Engine already selected the committed target surface. Preserve its
        // owning window as the start of core propagation; X grabs may replace
        // it below, but event-mask update order must never choose the target.
        let mut target_window = Some(surface_route.window);
        let mut pointers = self
            .pointer_state
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        let pointer = pointers
            .entry(route.request.seat)
            .or_insert_with(crate::XCorePointerMapper::new);
        let time_msec = u32::try_from(route.request.time_msec).unwrap_or(u32::MAX);
        let event = match route.request.kind {
            InputEventKind::Key { .. } => {
                let XKeyboardRouteResolution::Routed(resolved) = resolve_x_keyboard_input(
                    &route,
                    surface_route,
                    &self.input_authority,
                    &self.xkb_worker,
                    pointer.state(),
                    time_msec,
                )?
                else {
                    return self.send_input_delivery(
                        client,
                        route.delivery,
                        XAuthorityInputDeliveryOutcome::RouteRejected,
                    );
                };
                client = resolved.client;
                target_window = resolved.target_window;
                resolved.event
            }
            InputEventKind::PointerMotion => {
                if let Some(grab) = self
                    .input_authority
                    .lock()
                    .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                    .pointer_grab(surface_route.namespace)
                {
                    client = XServerFrontendClientId(grab.owner);
                    target_window = Some(if grab.owner_events && client == surface_route.client {
                        surface_route.window
                    } else {
                        grab.window
                    });
                }
                XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                    kind: XAuthorityPointerEventKind::Motion,
                    surface: route.request.target_surface,
                    root_x: clamp_input_coordinate(route.request.global_position.x),
                    root_y: clamp_input_coordinate(route.request.global_position.y),
                    event_x: clamp_input_coordinate(route.request.local_position.x),
                    event_y: clamp_input_coordinate(route.request.local_position.y),
                    state: self
                        .xkb_worker
                        .request(XkbWorkerCommand::Modifiers {
                            seat: route.request.seat,
                        })?
                        .map_or(0, |(_, state, _)| state)
                        | pointer.state(),
                    time_msec,
                })
            }
            InputEventKind::PointerButton { button, pressed } => {
                if let Some(grab) = self
                    .input_authority
                    .lock()
                    .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                    .pointer_grab(surface_route.namespace)
                {
                    client = XServerFrontendClientId(grab.owner);
                    target_window = Some(if grab.owner_events && client == surface_route.client {
                        surface_route.window
                    } else {
                        grab.window
                    });
                }
                let Some((button, state)) = pointer.map_evdev_button(button, pressed) else {
                    return self.send_input_delivery(
                        client,
                        route.delivery,
                        XAuthorityInputDeliveryOutcome::RouteRejected,
                    );
                };
                if pressed {
                    let grab = self
                        .input_authority
                        .lock()
                        .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                        .activate_button(
                            surface_route.namespace,
                            button,
                            state & 0xff,
                            crate::XActiveInputGrab {
                                owner: surface_route.client.raw(),
                                window: surface_route.window,
                                owner_events: true,
                                pointer_mode: 1,
                                keyboard_mode: 1,
                                event_mask: u16::MAX,
                            },
                        );
                    client = XServerFrontendClientId(grab.owner);
                    target_window = Some(if grab.owner_events && client == surface_route.client {
                        surface_route.window
                    } else {
                        grab.window
                    });
                } else {
                    self.input_authority
                        .lock()
                        .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                        .release_button(surface_route.namespace, button, pointer.state() == 0);
                }
                XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                    kind: XAuthorityPointerEventKind::Button { button, pressed },
                    surface: route.request.target_surface,
                    root_x: clamp_input_coordinate(route.request.global_position.x),
                    root_y: clamp_input_coordinate(route.request.global_position.y),
                    event_x: clamp_input_coordinate(route.request.local_position.x),
                    event_y: clamp_input_coordinate(route.request.local_position.y),
                    state: self
                        .xkb_worker
                        .request(XkbWorkerCommand::Modifiers {
                            seat: route.request.seat,
                        })?
                        .map_or(0, |(_, state, _)| state)
                        | state,
                    time_msec,
                })
            }
            InputEventKind::PointerAxis {
                horizontal_v120,
                vertical_v120,
            } => {
                if let Some(grab) = self
                    .input_authority
                    .lock()
                    .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                    .pointer_grab(surface_route.namespace)
                {
                    client = XServerFrontendClientId(grab.owner);
                    target_window = Some(if grab.owner_events && client == surface_route.client {
                        surface_route.window
                    } else {
                        grab.window
                    });
                }
                let Some(axis) = pointer.map_axis(horizontal_v120, vertical_v120) else {
                    return self.send_input_delivery(
                        client,
                        route.delivery,
                        XAuthorityInputDeliveryOutcome::RouteRejected,
                    );
                };
                let state = self
                    .xkb_worker
                    .request(XkbWorkerCommand::Modifiers {
                        seat: route.request.seat,
                    })?
                    .map_or(0, |(_, state, _)| state)
                    | pointer.state();
                let release_state = state | pointer.axis_release_state(axis.button);
                let pointer_event = |pressed| {
                    XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                        kind: XAuthorityPointerEventKind::Axis {
                            button: axis.button,
                            pressed,
                            horizontal_position_v120: pressed
                                .then_some(axis.horizontal_position_v120)
                                .flatten(),
                            vertical_position_v120: pressed
                                .then_some(axis.vertical_position_v120)
                                .flatten(),
                        },
                        surface: route.request.target_surface,
                        root_x: clamp_input_coordinate(route.request.global_position.x),
                        root_y: clamp_input_coordinate(route.request.global_position.y),
                        event_x: clamp_input_coordinate(route.request.local_position.x),
                        event_y: clamp_input_coordinate(route.request.local_position.y),
                        state: if pressed { state } else { release_state },
                        time_msec,
                    })
                };
                drop(pointers);
                if let Err(error) = self.route_resolved_input(
                    surface_route.namespace,
                    client,
                    surface_route.window,
                    target_window,
                    pointer_event(true),
                    None,
                ) {
                    self.send_input_delivery(
                        client,
                        route.delivery,
                        XAuthorityInputDeliveryOutcome::RouteRejected,
                    )?;
                    return Err(error);
                }
                return self.route_resolved_input(
                    surface_route.namespace,
                    client,
                    surface_route.window,
                    target_window,
                    pointer_event(false),
                    route.delivery,
                );
            }
        };
        drop(pointers);
        let lease_update = route.route_lease.and_then(|identity| {
            let kind = match route.request.kind {
                InputEventKind::PointerButton { pressed: true, .. } => {
                    XAuthorityRouteLeaseUpdateKind::Confirmed
                }
                InputEventKind::PointerButton { pressed: false, .. } => {
                    XAuthorityRouteLeaseUpdateKind::Released
                }
                _ => return None,
            };
            let admission = self.client_senders(client).ok()?.admission?;
            Some((identity, kind, admission))
        });
        let result = self.route_resolved_input(
            surface_route.namespace,
            client,
            surface_route.window,
            target_window,
            event,
            route.delivery,
        );
        if let Some((identity, kind, admission)) = lease_update {
            let reported_kind = if result.is_ok() {
                kind
            } else {
                if kind == XAuthorityRouteLeaseUpdateKind::Confirmed {
                    self.input_authority
                        .lock()
                        .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                        .ungrab_pointer(surface_route.namespace, client.raw());
                }
                XAuthorityRouteLeaseUpdateKind::Rejected
            };
            self.send_route_lease_update(
                identity,
                route.request.target_surface,
                admission,
                reported_kind,
            )?;
        }
        result
    }

    fn route_control(
        &self,
        route: XAuthorityClientControlCommand,
    ) -> Result<(), XServerFrontendRouteError> {
        if let Some(result) = self.route_focus_control(route) {
            return result;
        }
        let sender = self.client_senders(route.client)?.control;
        self.route_to_client(
            route.client,
            sender,
            X11RoutedControl::Authority {
                command: route.command,
                focus: None,
            },
        )
    }

    fn acknowledge_stale_control(
        &self,
        route: XAuthorityClientControlCommand,
    ) -> Result<(), XServerFrontendRouteError> {
        match self
            .acknowledgement_sender
            .try_send(XAuthorityClientControlAck {
                client: route.client,
                acknowledgement: XAuthorityControlAck {
                    kind: route.command.kind(),
                    transaction: route.command.transaction(),
                    surface: route.command.surface(),
                    outcome: XAuthorityControlOutcome::ClientGone,
                },
            }) {
            Ok(()) | Err(TrySendError::Disconnected(_)) => Ok(()),
            Err(TrySendError::Full(_)) => {
                Err(XServerFrontendRouteError::ClientQueueFull {
                    client: route.client,
                })
            }
        }
    }

    fn route_protocol(
        &self,
        client: XServerFrontendClientId,
        event: XClientEvent,
    ) -> Result<(), XServerFrontendRouteError> {
        let sender = match self.client_senders(client) {
            Ok(senders) => senders.protocol,
            Err(XServerFrontendRouteError::UnknownClient { .. }) => return Ok(()),
            Err(error) => return Err(error),
        };
        match self.route_to_client(client, sender, event) {
            Err(
                XServerFrontendRouteError::UnknownClient { .. }
                | XServerFrontendRouteError::ClientQueueDisconnected { .. },
            ) => Ok(()),
            result => result,
        }
    }

    fn client_senders(
        &self,
        client: XServerFrontendClientId,
    ) -> Result<XServerFrontendClientRouteSenders, XServerFrontendRouteError> {
        self.clients
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .get(&client)
            .cloned()
            .ok_or(XServerFrontendRouteError::UnknownClient { client })
    }

    fn route_to_client<T>(
        &self,
        client: XServerFrontendClientId,
        sender: SyncSender<T>,
        value: T,
    ) -> Result<(), XServerFrontendRouteError> {
        match sender.try_send(value) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                Err(XServerFrontendRouteError::ClientQueueFull { client })
            }
            Err(TrySendError::Disconnected(_)) => {
                self.clients
                    .lock()
                    .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                    .remove(&client);
                Err(XServerFrontendRouteError::ClientQueueDisconnected { client })
            }
        }
    }

    fn registered_client_count(&self) -> usize {
        self.clients
            .lock()
            .map(|clients| clients.len())
            .unwrap_or(0)
    }

    fn send_input_delivery(
        &self,
        client: XServerFrontendClientId,
        delivery: Option<XAuthorityInputDeliveryId>,
        outcome: XAuthorityInputDeliveryOutcome,
    ) -> Result<(), XServerFrontendRouteError> {
        let Some(delivery) = delivery else {
            return Ok(());
        };
        let Some(sender) = self.input_delivery_sender.as_ref() else {
            return Ok(());
        };
        match sender.send(XAuthorityClientInputDelivery {
            client,
            delivery,
            outcome,
        }) {
            Ok(()) | Err(_) => Ok(()),
        }
    }

    fn send_route_lease_update(
        &self,
        identity: sophia_protocol::ApplicationRouteLeaseIdentity,
        target_surface: SurfaceId,
        admission: ClientAdmissionContext,
        kind: XAuthorityRouteLeaseUpdateKind,
    ) -> Result<(), XServerFrontendRouteError> {
        let Some(sender) = self.route_lease_update_sender.as_ref() else {
            return Ok(());
        };
        let _ = sender.send(XAuthorityRouteLeaseUpdate {
            identity,
            target_surface,
            admission,
            kind,
        });
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for XServerFrontendClientRouteRegistration {
    fn drop(&mut self) {
        if let Ok(mut clients) = self.clients.lock() {
            clients.remove(&self.client);
        }
        if let Ok(mut surfaces) = self.surfaces.lock() {
            surfaces.retain(|_, route| route.client != self.client);
        }
        if let Ok(mut focused) = self.focused_surface.lock()
            && focused.is_some_and(|route| route.client == self.client)
        {
            *focused = None;
        }
        if let Ok(mut parents) = self.window_parents.lock() {
            parents.retain(|(client, _), _| *client != self.client);
        }
        if let Ok(mut subscriptions) = self.core_event_subscriptions.lock() {
            subscriptions.retain(|(client, _), _| *client != self.client);
        }
        if let Ok(mut subscriptions) = self.randr_subscriptions.lock() {
            subscriptions.remove(&self.client);
        }
        if let Ok(mut subscriptions) = self.present_subscriptions.lock() {
            subscriptions.retain(|(client, _), _| *client != self.client);
        }
        if let Ok(mut pending) = self.pending_presentations.entries.lock() {
            pending.retain(|_, presentation| presentation.client != self.client);
            self.pending_presentations.capacity_changed.notify_all();
        }
        if let Ok(mut frozen) = self.frozen_input.lock() {
            frozen.retain(|route| route.client != self.client);
        }
    }
}
