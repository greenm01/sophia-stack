/// Engine-facing ingress and per-client queue registry for a routed X11
/// session.
///
/// Engine code sends client-addressed input through the bounded ingress queues,
/// then its session loop calls [`Self::route_pending`] to move it into the
/// registered worker's private queue. Latency-sensitive control can instead use
/// [`Self::control_router`] to reach the selected client's bounded queue
/// directly. The broker never broadcasts a route. Routes whose client
/// disappeared after Engine selection are retired with a negative
/// acknowledgement. A client that saturates its private input queue is
/// quarantined without terminating the shared frontend; corruption of shared
/// registry state remains service-fatal.
#[cfg(unix)]
pub struct XServerFrontendRouteBroker {
    registry: XServerFrontendRouteRegistry,
    input_sender: SyncSender<XAuthorityClientInputEvent>,
    input_receiver: Receiver<XAuthorityClientInputEvent>,
    routed_input_sender: SyncSender<XAuthorityRoutedInput>,
    routed_input_receiver: Receiver<XAuthorityRoutedInput>,
    control_sender: SyncSender<XAuthorityClientControlCommand>,
    control_receiver: Receiver<XAuthorityClientControlCommand>,
    acknowledgement_receiver: Option<Receiver<XAuthorityClientControlAck>>,
    source_payload_receiver: Receiver<crate::ClipboardSourcePayload>,
}

/// Cloneable protocol-feedback handle for Engine/backend presentation code.
///
/// This handle can outlive the broker value moved into the X11 service loop,
/// but it exposes only frontend protocol completion. It cannot route input,
/// mutate scene state, submit scanout, or access native renderer resources.
#[cfg(unix)]
#[derive(Clone)]
pub struct XServerFrontendProtocolRouter {
    registry: XServerFrontendRouteRegistry,
}

/// Cloneable control handle for Engine-owned focus and configure commands.
///
/// This handle routes directly to the selected client's bounded queue so
/// latency-sensitive control does not wait behind frontend input processing.
#[cfg(unix)]
#[derive(Clone)]
pub struct XServerFrontendControlRouter {
    registry: XServerFrontendRouteRegistry,
}

/// Independent bounds for routes whose payloads have different expansion
/// factors and service rates at the X11 socket boundary.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XServerFrontendRouteCapacities {
    pub input: NonZeroUsize,
    pub control: NonZeroUsize,
    pub protocol: NonZeroUsize,
    pub presentations: NonZeroUsize,
}

#[cfg(unix)]
impl XServerFrontendRouteCapacities {
    pub const fn uniform(capacity: NonZeroUsize) -> Self {
        Self {
            input: capacity,
            control: capacity,
            protocol: capacity,
            presentations: capacity,
        }
    }

    pub const fn new(
        input: NonZeroUsize,
        control: NonZeroUsize,
        protocol: NonZeroUsize,
        presentations: NonZeroUsize,
    ) -> Self {
        Self {
            input,
            control,
            protocol,
            presentations,
        }
    }
}

#[cfg(unix)]
impl XServerFrontendControlRouter {
    pub fn route_control(
        &self,
        route: XAuthorityClientControlCommand,
    ) -> Result<(), XServerFrontendRouteError> {
        match self.registry.route_control(route) {
            Ok(()) => Ok(()),
            Err(
                XServerFrontendRouteError::UnknownClient { .. }
                | XServerFrontendRouteError::ClientQueueDisconnected { .. },
            ) => self.registry.acknowledge_stale_control(route),
            Err(error) => Err(error),
        }
    }
}

#[cfg(unix)]
impl XServerFrontendProtocolRouter {
    pub fn route_present_complete(
        &self,
        transaction: TransactionId,
        ust: u64,
        msc: u64,
        mode: XPresentCompletionMode,
    ) -> Result<bool, XServerFrontendRouteError> {
        self.registry
            .route_present_complete(transaction, ust, msc, mode)
    }

    pub fn route_present_idle(
        &self,
        transaction: TransactionId,
    ) -> Result<bool, XServerFrontendRouteError> {
        self.registry.route_present_idle(transaction)
    }
}

#[cfg(unix)]
impl XServerFrontendRouteBroker {
    pub fn new(queue_capacity: NonZeroUsize) -> Self {
        let capacity = queue_capacity.get();
        let (acknowledgement_sender, acknowledgement_receiver) = sync_channel(capacity);
        Self::with_transports(
            XServerFrontendRouteCapacities::uniform(queue_capacity),
            acknowledgement_sender,
            Some(acknowledgement_receiver),
            None,
        )
    }

    /// Creates a broker whose control acknowledgements return to the supplied
    /// Engine-owned bounded queue.
    pub fn with_control_ack_sender(
        queue_capacity: NonZeroUsize,
        acknowledgement_sender: SyncSender<XAuthorityClientControlAck>,
    ) -> Self {
        Self::with_transports(
            XServerFrontendRouteCapacities::uniform(queue_capacity),
            acknowledgement_sender,
            None,
            None,
        )
    }

    /// Creates a broker whose focus/configure and input-flush acknowledgements
    /// return through Engine-owned queues.
    pub fn with_control_and_input_delivery_senders(
        queue_capacity: NonZeroUsize,
        acknowledgement_sender: SyncSender<XAuthorityClientControlAck>,
        input_delivery_sender: Sender<XAuthorityClientInputDelivery>,
    ) -> Self {
        Self::with_transports(
            XServerFrontendRouteCapacities::uniform(queue_capacity),
            acknowledgement_sender,
            None,
            Some(input_delivery_sender),
        )
    }

    pub fn with_control_and_input_delivery_senders_and_xkb_config(
        queue_capacity: NonZeroUsize,
        acknowledgement_sender: SyncSender<XAuthorityClientControlAck>,
        input_delivery_sender: Sender<XAuthorityClientInputDelivery>,
        xkb_config: crate::XkbRmlvoConfig,
    ) -> Result<Self, crate::XkbKeyboardError> {
        crate::XkbKeyboardState::new(&xkb_config)?;
        let mut broker = Self::with_transports(
            XServerFrontendRouteCapacities::uniform(queue_capacity),
            acknowledgement_sender,
            None,
            Some(input_delivery_sender),
        );
        broker.registry.xkb_config = xkb_config.clone();
        broker.registry.xkb_worker = XkbKeyboardWorker::spawn(xkb_config);
        Ok(broker)
    }

    pub fn with_route_capacities_and_xkb_config(
        capacities: XServerFrontendRouteCapacities,
        acknowledgement_sender: SyncSender<XAuthorityClientControlAck>,
        input_delivery_sender: Sender<XAuthorityClientInputDelivery>,
        xkb_config: crate::XkbRmlvoConfig,
    ) -> Result<Self, crate::XkbKeyboardError> {
        crate::XkbKeyboardState::new(&xkb_config)?;
        let mut broker = Self::with_transports(
            capacities,
            acknowledgement_sender,
            None,
            Some(input_delivery_sender),
        );
        broker.registry.xkb_config = xkb_config.clone();
        broker.registry.xkb_worker = XkbKeyboardWorker::spawn(xkb_config);
        Ok(broker)
    }

    fn with_transports(
        capacities: XServerFrontendRouteCapacities,
        acknowledgement_sender: SyncSender<XAuthorityClientControlAck>,
        acknowledgement_receiver: Option<Receiver<XAuthorityClientControlAck>>,
        input_delivery_sender: Option<Sender<XAuthorityClientInputDelivery>>,
    ) -> Self {
        let (input_sender, input_receiver) = sync_channel(capacities.input.get());
        let (routed_input_sender, routed_input_receiver) = sync_channel(capacities.input.get());
        let (control_sender, control_receiver) = sync_channel(capacities.control.get());
        let (source_payload_sender, source_payload_receiver) =
            sync_channel(capacities.input.get());
        Self {
            registry: XServerFrontendRouteRegistry {
                clients: Arc::new(Mutex::new(BTreeMap::new())),
                surfaces: Arc::new(Mutex::new(BTreeMap::new())),
                focused_surface: Arc::new(Mutex::new(None)),
                window_parents: Arc::new(Mutex::new(BTreeMap::new())),
                core_event_subscriptions: Arc::new(Mutex::new(BTreeMap::new())),
                randr_subscriptions: Arc::new(Mutex::new(BTreeMap::new())),
                present_subscriptions: Arc::new(Mutex::new(BTreeMap::new())),
                pending_presentations: Arc::new(XPendingPresentRegistry::default()),
                pointer_state: Arc::new(Mutex::new(BTreeMap::new())),
                input_authority: Arc::new(Mutex::new(crate::XInputAuthorityState::default())),
                frozen_input: Arc::new(Mutex::new(VecDeque::new())),
                xkb_config: crate::XkbRmlvoConfig::default(),
                xkb_worker: XkbKeyboardWorker::spawn(crate::XkbRmlvoConfig::default()),
                acknowledgement_sender,
                input_delivery_sender,
                per_client_input_capacity: capacities.input,
                per_client_control_capacity: capacities.control,
                per_client_protocol_capacity: capacities.protocol,
                per_client_presentation_capacity: capacities.presentations,
                source_payload_sender,
            },
            input_sender,
            input_receiver,
            routed_input_sender,
            routed_input_receiver,
            control_sender,
            control_receiver,
            acknowledgement_receiver,
            source_payload_receiver,
        }
    }

    pub fn input_sender(&self) -> SyncSender<XAuthorityClientInputEvent> {
        self.input_sender.clone()
    }

    pub fn routed_input_sender(&self) -> SyncSender<XAuthorityRoutedInput> {
        self.routed_input_sender.clone()
    }

    pub fn control_sender(&self) -> SyncSender<XAuthorityClientControlCommand> {
        self.control_sender.clone()
    }

    pub fn control_router(&self) -> XServerFrontendControlRouter {
        XServerFrontendControlRouter {
            registry: self.registry.clone(),
        }
    }

    pub fn recv_control_ack_timeout(
        &self,
        timeout: Duration,
    ) -> Result<XAuthorityClientControlAck, RecvTimeoutError> {
        self.acknowledgement_receiver
            .as_ref()
            .ok_or(RecvTimeoutError::Disconnected)?
            .recv_timeout(timeout)
    }

    pub fn recv_clipboard_source_payload_timeout(
        &self,
        timeout: Duration,
    ) -> Result<crate::ClipboardSourcePayload, RecvTimeoutError> {
        self.source_payload_receiver.recv_timeout(timeout)
    }

    /// Routes every value currently available at the bounded ingress.
    pub fn route_pending(&mut self) -> Result<usize, XServerFrontendRouteError> {
        let mut routed = 0usize;
        loop {
            let mut progressed = false;
            match self.routed_input_receiver.try_recv() {
                Ok(route) => {
                    match self.registry.route_engine_input(route) {
                        Ok(()) => routed = routed.saturating_add(1),
                        Err(
                            XServerFrontendRouteError::UnknownSurface { .. }
                            | XServerFrontendRouteError::ClientQueueDisconnected { .. }
                            | XServerFrontendRouteError::UnknownClient { .. }
                            | XServerFrontendRouteError::ClientQueueFull { .. },
                        ) => {}
                        Err(error) => return Err(error),
                    }
                    progressed = true;
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
            }
            match self.input_receiver.try_recv() {
                Ok(route) => {
                    if let Err(error) = self.registry.route_input(route) {
                        self.registry.send_input_delivery(
                            route.client,
                            route.delivery,
                            XAuthorityInputDeliveryOutcome::RouteRejected,
                        )?;
                        match error {
                            XServerFrontendRouteError::UnknownClient { .. }
                            | XServerFrontendRouteError::ClientQueueDisconnected { .. }
                            | XServerFrontendRouteError::ClientQueueFull { .. } => {}
                            error => return Err(error),
                        }
                    } else {
                        routed = routed.saturating_add(1);
                    }
                    progressed = true;
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
            }
            match self.control_receiver.try_recv() {
                Ok(route) => {
                    match self.registry.route_control(route) {
                        Ok(()) => {}
                        Err(
                            XServerFrontendRouteError::UnknownClient { .. }
                            | XServerFrontendRouteError::ClientQueueDisconnected { .. },
                        ) => {
                            self.registry.acknowledge_stale_control(route)?;
                        }
                        Err(error) => return Err(error),
                    }
                    routed = routed.saturating_add(1);
                    progressed = true;
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
            }
            let thawed = match self.registry.drain_thawed_input() {
                Ok(thawed) => thawed,
                Err(
                    XServerFrontendRouteError::UnknownSurface { .. }
                    | XServerFrontendRouteError::ClientQueueDisconnected { .. }
                    | XServerFrontendRouteError::UnknownClient { .. }
                    | XServerFrontendRouteError::ClientQueueFull { .. },
                ) => {
                    progressed = true;
                    0
                }
                Err(error) => return Err(error),
            };
            if thawed != 0 {
                routed = routed.saturating_add(thawed);
                progressed = true;
            }
            if !progressed {
                return Ok(routed);
            }
        }
    }

    pub fn registered_client_count(&self) -> usize {
        self.registry.registered_client_count()
    }

    pub fn protocol_router(&self) -> XServerFrontendProtocolRouter {
        XServerFrontendProtocolRouter {
            registry: self.registry.clone(),
        }
    }

    pub fn route_present_complete(
        &self,
        transaction: TransactionId,
        ust: u64,
        msc: u64,
        mode: XPresentCompletionMode,
    ) -> Result<bool, XServerFrontendRouteError> {
        self.registry
            .route_present_complete(transaction, ust, msc, mode)
    }

    pub fn route_present_idle(
        &self,
        transaction: TransactionId,
    ) -> Result<bool, XServerFrontendRouteError> {
        self.registry.route_present_idle(transaction)
    }
}
