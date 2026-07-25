#[cfg(unix)]
#[derive(Clone)]
struct XServerFrontendRouteRegistry {
    clients: Arc<Mutex<BTreeMap<XServerFrontendClientId, XServerFrontendClientRouteSenders>>>,
    surfaces: Arc<Mutex<BTreeMap<SurfaceId, XServerFrontendSurfaceRoute>>>,
    randr_subscriptions: Arc<Mutex<BTreeMap<XServerFrontendClientId, (XResourceId, u16)>>>,
    present_subscriptions:
        Arc<Mutex<BTreeMap<(XServerFrontendClientId, XResourceId), XPresentSubscription>>>,
    pending_presentations: Arc<XPendingPresentRegistry>,
    pointer_state: Arc<Mutex<BTreeMap<SeatId, crate::XCorePointerMapper>>>,
    input_authority: Arc<Mutex<crate::XInputAuthorityState>>,
    frozen_input: Arc<Mutex<VecDeque<XDeferredRoutedInput>>>,
    xkb_config: crate::XkbRmlvoConfig,
    xkb_worker: XkbKeyboardWorker,
    acknowledgement_sender: SyncSender<XAuthorityClientControlAck>,
    input_delivery_sender: Option<SyncSender<XAuthorityClientInputDelivery>>,
    per_client_queue_capacity: NonZeroUsize,
    source_payload_sender: SyncSender<crate::ClipboardSourcePayload>,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
struct XServerFrontendSurfaceRoute {
    client: XServerFrontendClientId,
    namespace: NamespaceId,
    window: XResourceId,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
struct XPresentSubscription {
    event_id: XResourceId,
    window: XResourceId,
    mask: u32,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
struct XPendingPresent {
    client: XServerFrontendClientId,
    window: XResourceId,
    pixmap: XResourceId,
    serial: u32,
    idle_fence: Option<XResourceId>,
    completed: bool,
}

#[cfg(unix)]
#[derive(Default)]
struct XPendingPresentRegistry {
    entries: Mutex<BTreeMap<TransactionId, XPendingPresent>>,
    capacity_changed: Condvar,
}

#[cfg(unix)]
#[derive(Clone, Debug)]
struct XDeferredRoutedInput {
    client: XServerFrontendClientId,
    route: XAuthorityRoutedInput,
}

#[cfg(unix)]
#[derive(Clone)]
struct XServerFrontendClientRouteSenders {
    input: SyncSender<XAuthorityClientInputEvent>,
    control: SyncSender<XAuthorityControlCommand>,
    protocol: SyncSender<XClientEvent>,
}

#[cfg(unix)]
struct XServerFrontendClientRouteChannels {
    input: Receiver<XAuthorityClientInputEvent>,
    control: Receiver<XAuthorityControlCommand>,
    protocol: Receiver<XClientEvent>,
}

#[cfg(unix)]
struct XServerFrontendClientRouteRegistration {
    client: XServerFrontendClientId,
    clients: Arc<Mutex<BTreeMap<XServerFrontendClientId, XServerFrontendClientRouteSenders>>>,
    surfaces: Arc<Mutex<BTreeMap<SurfaceId, XServerFrontendSurfaceRoute>>>,
    randr_subscriptions: Arc<Mutex<BTreeMap<XServerFrontendClientId, (XResourceId, u16)>>>,
    present_subscriptions:
        Arc<Mutex<BTreeMap<(XServerFrontendClientId, XResourceId), XPresentSubscription>>>,
    pending_presentations: Arc<XPendingPresentRegistry>,
    frozen_input: Arc<Mutex<VecDeque<XDeferredRoutedInput>>>,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
enum XkbWorkerCommand {
    Key {
        seat: SeatId,
        keycode: u32,
        pressed: bool,
    },
    Modifiers {
        seat: SeatId,
    },
}

#[cfg(unix)]
type XkbKeyboardReply = Option<(u8, u16, u16)>;

#[cfg(unix)]
type SharedXkbKeyboardReplies = Arc<Mutex<Receiver<XkbKeyboardReply>>>;

#[cfg(unix)]
#[derive(Clone)]
struct XkbKeyboardWorker {
    commands: SyncSender<XkbWorkerCommand>,
    replies: SharedXkbKeyboardReplies,
}

#[cfg(unix)]
impl XkbKeyboardWorker {
    fn spawn(config: crate::XkbRmlvoConfig) -> Self {
        let (commands, command_receiver) = sync_channel(64);
        let (reply_sender, replies) = sync_channel(64);
        std::thread::Builder::new()
            .name("sophia-xkb-authority".to_owned())
            .spawn(move || {
                let mut seats = BTreeMap::<SeatId, crate::XkbKeyboardState>::new();
                while let Ok(command) = command_receiver.recv() {
                    let seat_id = match command {
                        XkbWorkerCommand::Key { seat, .. }
                        | XkbWorkerCommand::Modifiers { seat } => seat,
                    };
                    let state = seats.entry(seat_id).or_insert_with(|| {
                        crate::XkbKeyboardState::new(&config)
                            .expect("validated XKB configuration must remain compilable")
                    });
                    let reply = match command {
                        XkbWorkerCommand::Key {
                            keycode, pressed, ..
                        } => state.map_evdev_key(keycode, pressed).map(|(keycode, before)| {
                            (keycode, before, state.modifier_mask())
                        }),
                        XkbWorkerCommand::Modifiers { .. } => {
                            let modifiers = state.modifier_mask();
                            Some((0, modifiers, modifiers))
                        }
                    };
                    if reply_sender.send(reply).is_err() {
                        break;
                    }
                }
            })
            .expect("Sophia XKB authority worker must start");
        Self {
            commands,
            replies: Arc::new(Mutex::new(replies)),
        }
    }

    fn request(
        &self,
        command: XkbWorkerCommand,
    ) -> Result<Option<(u8, u16, u16)>, XServerFrontendRouteError> {
        self.commands
            .try_send(command)
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        self.replies
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .recv()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)
    }
}

#[cfg(unix)]
impl XServerFrontendRouteRegistry {
    fn register_client(
        &self,
        client: XServerFrontendClientId,
    ) -> Result<
        (
            XServerFrontendClientRouteRegistration,
            XServerFrontendClientRouteChannels,
        ),
        XServerFrontendRouteError,
    > {
        let capacity = self.per_client_queue_capacity.get();
        let (input_sender, input) = sync_channel(capacity);
        let (control_sender, control) = sync_channel(capacity);
        let (protocol_sender, protocol) = sync_channel(capacity);
        let mut clients = self
            .clients
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        if clients.contains_key(&client) {
            return Err(XServerFrontendRouteError::DuplicateClient { client });
        }
        clients.insert(
            client,
            XServerFrontendClientRouteSenders {
                input: input_sender,
                control: control_sender,
                protocol: protocol_sender,
            },
        );
        Ok((
            XServerFrontendClientRouteRegistration {
                client,
                clients: self.clients.clone(),
                surfaces: self.surfaces.clone(),
                randr_subscriptions: self.randr_subscriptions.clone(),
                present_subscriptions: self.present_subscriptions.clone(),
                pending_presentations: self.pending_presentations.clone(),
                frozen_input: self.frozen_input.clone(),
            },
            XServerFrontendClientRouteChannels {
                input,
                control,
                protocol,
            },
        ))
    }

    fn route_input(
        &self,
        route: XAuthorityClientInputEvent,
    ) -> Result<(), XServerFrontendRouteError> {
        let sender = self.client_senders(route.client)?.input;
        self.route_to_client(route.client, sender, route)
    }

    fn register_surface(
        &self,
        client: XServerFrontendClientId,
        namespace: NamespaceId,
        surface: SurfaceId,
        window: XResourceId,
    ) -> Result<(), XServerFrontendRouteError> {
        self.surfaces
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .insert(
                surface,
                XServerFrontendSurfaceRoute {
                    client,
                    namespace,
                    window,
                },
            );
        Ok(())
    }

    fn select_randr_input(
        &self,
        client: XServerFrontendClientId,
        window: XResourceId,
        mask: u16,
    ) -> Result<(), XServerFrontendRouteError> {
        let mut subscriptions = self
            .randr_subscriptions
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        if mask == 0 {
            subscriptions.remove(&client);
        } else {
            subscriptions.insert(client, (window, mask));
        }
        Ok(())
    }

    fn select_present_input(
        &self,
        client: XServerFrontendClientId,
        event_id: XResourceId,
        window: XResourceId,
        mask: u32,
    ) -> Result<(), XServerFrontendRouteError> {
        let mut subscriptions = self
            .present_subscriptions
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        let key = (client, event_id);
        if mask == 0 {
            subscriptions.remove(&key);
        } else {
            subscriptions.insert(
                key,
                XPresentSubscription {
                    event_id,
                    window,
                    mask,
                },
            );
        }
        Ok(())
    }

    fn present_configure_event_ids(
        &self,
        client: XServerFrontendClientId,
        window: XResourceId,
    ) -> Result<Vec<XResourceId>, XServerFrontendRouteError> {
        Ok(self
            .present_subscriptions
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .iter()
            .filter_map(|((subscription_client, _), subscription)| {
                (*subscription_client == client
                    && subscription.window == window
                    && subscription.mask & 1 != 0)
                    .then_some(subscription.event_id)
            })
            .collect())
    }

    fn queue_present(
        &self,
        transaction: TransactionId,
        client: XServerFrontendClientId,
        window: XResourceId,
        pixmap: XResourceId,
        serial: u32,
        idle_fence: Option<XResourceId>,
    ) -> Result<(), XServerFrontendRouteError> {
        self.surfaces
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .iter()
            .find_map(|(surface, route)| {
                (route.client == client && route.window == window).then_some(*surface)
            })
            .ok_or(XServerFrontendRouteError::UnknownClient { client })?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut pending = self
            .pending_presentations
            .entries
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        if pending.contains_key(&transaction) {
            return Err(XServerFrontendRouteError::DuplicatePresentation { transaction });
        }
        while pending
            .values()
            .filter(|presentation| presentation.client == client)
            .count()
            >= self.per_client_queue_capacity.get()
        {
            let now = Instant::now();
            if now >= deadline {
                return Err(XServerFrontendRouteError::ClientQueueFull { client });
            }
            let (next, wait) = self
                .pending_presentations
                .capacity_changed
                .wait_timeout(pending, deadline.saturating_duration_since(now))
                .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
            pending = next;
            if wait.timed_out()
                && pending
                    .values()
                    .filter(|presentation| presentation.client == client)
                    .count()
                    >= self.per_client_queue_capacity.get()
            {
                return Err(XServerFrontendRouteError::ClientQueueFull { client });
            }
        }
        pending.insert(
            transaction,
            XPendingPresent {
                client,
                window,
                pixmap,
                serial,
                idle_fence,
                completed: false,
            },
        );
        Ok(())
    }

    fn route_present_complete(
        &self,
        transaction: TransactionId,
        ust: u64,
        msc: u64,
        mode: XPresentCompletionMode,
    ) -> Result<bool, XServerFrontendRouteError> {
        let presentation = {
            let mut pending = self
                .pending_presentations
                .entries
                .lock()
                .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
            let Some(presentation) = pending.get_mut(&transaction) else {
                return Ok(false);
            };
            if presentation.completed {
                return Ok(false);
            }
            presentation.completed = true;
            *presentation
        };
        let subscriptions = self
            .present_subscriptions
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .iter()
            .filter_map(|((client, _), subscription)| {
                (*client == presentation.client
                    && subscription.window == presentation.window
                    && subscription.mask & (1 << 1) != 0)
                    .then_some(*subscription)
            })
            .collect::<Vec<_>>();
        if subscriptions.is_empty() {
            return Ok(false);
        }
        for subscription in subscriptions {
            self.route_protocol(
                presentation.client,
                XClientEvent::PresentCompleteNotify {
                    sequence: 0,
                    event_id: subscription.event_id,
                    window: presentation.window,
                    serial: presentation.serial,
                    ust,
                    msc,
                    mode: mode as u8,
                },
            )?;
        }
        Ok(true)
    }

    fn cancel_present(&self, transaction: TransactionId) -> Result<(), XServerFrontendRouteError> {
        self.pending_presentations
            .entries
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .remove(&transaction);
        self.pending_presentations.capacity_changed.notify_all();
        Ok(())
    }

    fn route_present_idle(
        &self,
        transaction: TransactionId,
    ) -> Result<bool, XServerFrontendRouteError> {
        let presentation = {
            let mut pending = self
                .pending_presentations
                .entries
                .lock()
                .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
            let Some(front) = pending.get(&transaction).copied() else {
                return Ok(false);
            };
            if !front.completed {
                return Ok(false);
            }
            pending.remove(&transaction);
            self.pending_presentations.capacity_changed.notify_all();
            front
        };
        let subscriptions = self
            .present_subscriptions
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .iter()
            .filter_map(|((client, _), subscription)| {
                (*client == presentation.client
                    && subscription.window == presentation.window
                    && subscription.mask & (1 << 2) != 0)
                    .then_some(*subscription)
            })
            .collect::<Vec<_>>();
        if subscriptions.is_empty() {
            return Ok(false);
        }
        for subscription in subscriptions {
            self.route_protocol(
                presentation.client,
                XClientEvent::PresentIdleNotify {
                    sequence: 0,
                    event_id: subscription.event_id,
                    window: presentation.window,
                    serial: presentation.serial,
                    pixmap: presentation.pixmap,
                    idle_fence: presentation.idle_fence,
                },
            )?;
        }
        Ok(true)
    }

    fn broadcast_randr_update(
        &self,
        snapshot: &sophia_protocol::OutputTopologySnapshot,
    ) -> Result<usize, XServerFrontendRouteError> {
        let size = snapshot
            .root_size()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        let width =
            u16::try_from(size.width).map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        let height =
            u16::try_from(size.height).map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        let mm_width = u16::try_from((i64::from(size.width) * 254 + 480) / 960)
            .unwrap_or(u16::MAX)
            .max(1);
        let mm_height = u16::try_from((i64::from(size.height) * 254 + 480) / 960)
            .unwrap_or(u16::MAX)
            .max(1);
        let timestamp = u32::try_from(snapshot.generation)
            .unwrap_or(u32::MAX)
            .max(1);
        let subscriptions = self
            .randr_subscriptions
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .clone();
        let mut delivered = 0usize;
        for (client, (window, mask)) in subscriptions {
            if mask & 1 != 0 {
                self.route_protocol(
                    client,
                    XClientEvent::RandrScreenChange {
                        sequence: 0,
                        timestamp,
                        config_timestamp: timestamp,
                        root: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
                        request_window: window,
                        width,
                        height,
                        mm_width,
                        mm_height,
                    },
                )?;
                delivered = delivered.saturating_add(1);
            }
            for output in &snapshot.outputs {
                let identity = crate::dispatch::stable_randr_identity(output.output.raw());
                let crtc = 0x1000_0000 | identity;
                let output_id = 0x2000_0000 | identity;
                let mode = crate::dispatch::stable_randr_mode_id(
                    output.logical.width,
                    output.logical.height,
                    output.refresh_millihz,
                );
                if mask & (1 << 1) != 0 {
                    self.route_protocol(
                        client,
                        XClientEvent::RandrCrtcChange {
                            sequence: 0,
                            timestamp,
                            window,
                            crtc,
                            mode,
                            x: i16::try_from(output.logical.x).unwrap_or(i16::MAX),
                            y: i16::try_from(output.logical.y).unwrap_or(i16::MAX),
                            width: u16::try_from(output.logical.width).unwrap_or(u16::MAX),
                            height: u16::try_from(output.logical.height).unwrap_or(u16::MAX),
                        },
                    )?;
                    delivered = delivered.saturating_add(1);
                }
                if mask & (1 << 2) != 0 {
                    self.route_protocol(
                        client,
                        XClientEvent::RandrOutputChange {
                            sequence: 0,
                            timestamp,
                            window,
                            output: output_id,
                            crtc,
                            mode,
                        },
                    )?;
                    delivered = delivered.saturating_add(1);
                }
            }
            if mask & (1 << 6) != 0 {
                self.route_protocol(
                    client,
                    XClientEvent::RandrResourceChange {
                        sequence: 0,
                        timestamp,
                        window,
                    },
                )?;
                delivered = delivered.saturating_add(1);
            }
        }
        Ok(delivered)
    }

    fn route_engine_input(
        &self,
        route: XAuthorityRoutedInput,
    ) -> Result<(), XServerFrontendRouteError> {
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
            if frozen.len() >= self.per_client_queue_capacity.get() {
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
            InputEventKind::Key { keycode, pressed } => {
                if let Some(grab) = self
                    .input_authority
                    .lock()
                    .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                    .keyboard_grab(surface_route.namespace)
                {
                    client = XServerFrontendClientId(grab.owner);
                    target_window = Some(if grab.owner_events && client == surface_route.client {
                        surface_route.window
                    } else {
                        grab.window
                    });
                }
                let Some((keycode, state, modifiers_after)) =
                    self.xkb_worker.request(XkbWorkerCommand::Key {
                    seat: route.request.seat,
                    keycode,
                    pressed,
                })?
                else {
                    return self.send_input_delivery(
                        client,
                        route.delivery,
                        XAuthorityInputDeliveryOutcome::RouteRejected,
                    );
                };
                let passive = if pressed {
                    self.input_authority
                        .lock()
                        .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                        .activate_key(surface_route.namespace, keycode, state & 0xff)
                } else {
                    None
                };
                if let Some(grab) = passive {
                    client = XServerFrontendClientId(grab.owner);
                    target_window = Some(if grab.owner_events && client == surface_route.client {
                        surface_route.window
                    } else {
                        grab.window
                    });
                }
                if !pressed {
                    self.input_authority
                        .lock()
                        .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                        .release_key(surface_route.namespace, keycode);
                }
                XAuthorityInputEvent::Key(XAuthorityKeyEvent {
                    keycode,
                    pressed,
                    state: state | pointer.state(),
                    modifiers_after: modifiers_after as u8,
                    time_msec,
                })
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
                        .release_button(surface_route.namespace, button);
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
                let Some(button) =
                    crate::XCorePointerMapper::map_axis_to_button(horizontal_v120, vertical_v120)
                else {
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
                let pointer_event = |pressed| {
                    XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                        kind: XAuthorityPointerEventKind::Button { button, pressed },
                        surface: route.request.target_surface,
                        root_x: clamp_input_coordinate(route.request.global_position.x),
                        root_y: clamp_input_coordinate(route.request.global_position.y),
                        event_x: clamp_input_coordinate(route.request.local_position.x),
                        event_y: clamp_input_coordinate(route.request.local_position.y),
                        state,
                        time_msec,
                    })
                };
                drop(pointers);
                self.route_resolved_input(
                    surface_route.namespace,
                    client,
                    surface_route.window,
                    target_window,
                    pointer_event(true),
                    None,
                )?;
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
        self.route_resolved_input(
            surface_route.namespace,
            client,
            surface_route.window,
            target_window,
            event,
            route.delivery,
        )
    }

    fn route_control(
        &self,
        route: XAuthorityClientControlCommand,
    ) -> Result<(), XServerFrontendRouteError> {
        let sender = self.client_senders(route.client)?.control;
        self.route_to_client(route.client, sender, route.command)
    }

    fn route_protocol(
        &self,
        client: XServerFrontendClientId,
        event: XClientEvent,
    ) -> Result<(), XServerFrontendRouteError> {
        let sender = self.client_senders(client)?.protocol;
        match self.route_to_client(client, sender, event) {
            Err(XServerFrontendRouteError::ClientQueueDisconnected { .. }) => Ok(()),
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
        match sender.try_send(XAuthorityClientInputDelivery {
            client,
            delivery,
            outcome,
        }) {
            Ok(()) | Err(TrySendError::Disconnected(_)) => Ok(()),
            Err(TrySendError::Full(_)) => Err(XServerFrontendRouteError::InputDeliveryQueueFull),
        }
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
