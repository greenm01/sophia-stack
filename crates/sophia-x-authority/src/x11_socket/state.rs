/// Authority-owned state shared by every client accepted by one X11 socket
/// listener. Client sequence numbers remain connection-local.
#[cfg(unix)]
#[derive(Clone)]
pub struct X11CoreSocketServerState {
    runtime: Arc<Mutex<XAuthorityRuntime>>,
    atoms: Arc<Mutex<XAtomTable>>,
    properties: Arc<Mutex<XPropertyTable>>,
    clients: Arc<Mutex<X11CoreClientLeaseState>>,
    next_transaction_id: Arc<AtomicU64>,
    render_device_provider: Option<Arc<dyn XServerFrontendRenderDeviceProvider>>,
}

#[cfg(unix)]
impl core::fmt::Debug for X11CoreSocketServerState {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("X11CoreSocketServerState")
            .field("runtime", &self.runtime)
            .field("atoms", &self.atoms)
            .field("properties", &self.properties)
            .field("clients", &self.clients)
            .field(
                "next_transaction_id",
                &self.next_transaction_id.load(Ordering::Relaxed),
            )
            .field(
                "has_render_device_provider",
                &self.render_device_provider.is_some(),
            )
            .finish()
    }
}

/// The small part of socket state that must be serialized across connection
/// setup and teardown. Protocol dispatch itself uses the independent runtime,
/// atom, and property locks above.
#[cfg(unix)]
#[derive(Debug)]
struct X11CoreClientLeaseState {
    next_client_resource_range: u16,
    next_client_id: u64,
    client_leases: BTreeMap<XServerFrontendClientId, XServerFrontendClientLease>,
}

#[cfg(unix)]
impl Default for X11CoreSocketServerState {
    fn default() -> Self {
        Self {
            runtime: Default::default(),
            atoms: Default::default(),
            properties: Default::default(),
            clients: Arc::new(Mutex::new(X11CoreClientLeaseState {
                next_client_resource_range: 1,
                next_client_id: 1,
                client_leases: Default::default(),
            })),
            next_transaction_id: Arc::new(AtomicU64::new(1)),
            render_device_provider: None,
        }
    }
}

#[cfg(unix)]
impl X11CoreSocketServerState {
    pub fn new() -> Self {
        Self::default()
    }

    fn allocate_transaction(&self) -> Result<TransactionId, X11SetupSocketError> {
        let raw = self
            .next_transaction_id
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| X11SetupSocketError::new("X11 transaction identity space exhausted"))?;
        Ok(TransactionId::from_raw(raw))
    }

    pub fn with_render_device_provider(
        mut self,
        provider: Arc<dyn XServerFrontendRenderDeviceProvider>,
    ) -> Self {
        self.render_device_provider = Some(provider);
        self
    }

    fn with_optional_render_device_provider(
        mut self,
        provider: Option<Arc<dyn XServerFrontendRenderDeviceProvider>>,
    ) -> Self {
        self.render_device_provider = provider;
        self
    }

    fn open_render_device_fd(&self) -> Result<OwnedFd, XServerFrontendRenderDeviceError> {
        self.render_device_provider
            .as_ref()
            .ok_or(XServerFrontendRenderDeviceError::Unavailable)?
            .open_render_device_fd()
    }

    fn has_render_device_provider(&self) -> bool {
        self.render_device_provider.is_some()
    }

    fn set_policy_map_deferred(&self, deferred: bool) -> Result<(), X11SetupSocketError> {
        self.runtime
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?
            .set_policy_map_deferred(deferred);
        Ok(())
    }

    pub fn with_output_topology(
        output_topology: sophia_protocol::OutputTopologySnapshot,
    ) -> Result<Self, X11SetupSocketError> {
        let runtime =
            XAuthorityRuntime::with_output_topology(output_topology).map_err(|error| {
                X11SetupSocketError::new(format!("invalid Engine output topology: {error:?}"))
            })?;
        Ok(Self {
            runtime: Arc::new(Mutex::new(runtime)),
            ..Self::default()
        })
    }

    pub fn with_output_topology_and_xkb_config(
        output_topology: sophia_protocol::OutputTopologySnapshot,
        xkb_config: &crate::XkbRmlvoConfig,
    ) -> Result<Self, X11SetupSocketError> {
        let runtime =
            XAuthorityRuntime::with_output_topology_and_xkb_config(output_topology, xkb_config)
                .map_err(|error| X11SetupSocketError::new(error.to_string()))?;
        Ok(Self {
            runtime: Arc::new(Mutex::new(runtime)),
            ..Self::default()
        })
    }

    fn next_client_setup_success(
        &self,
    ) -> Result<(XServerFrontendClientLease, XSetupSuccess), X11SetupSocketError> {
        let root_size = self
            .runtime
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?
            .output_topology()
            .root_size()
            .map_err(|error| {
                X11SetupSocketError::new(format!(
                    "invalid Engine output topology during setup: {error:?}"
                ))
            })?;
        let mut clients = self
            .clients
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 client lease lock poisoned"))?;
        if clients.next_client_resource_range > X11_MAX_CLIENT_RESOURCE_RANGES {
            return Err(X11SetupSocketError::new(
                "Sophia X Server Frontend exhausted X11 client resource ranges",
            ));
        }
        let resource_id_base =
            u32::from(clients.next_client_resource_range) * X11_CLIENT_RESOURCE_RANGE_SIZE;
        clients.next_client_resource_range = clients.next_client_resource_range.saturating_add(1);
        let client = XServerFrontendClientId(clients.next_client_id);
        clients.next_client_id = clients.next_client_id.checked_add(1).ok_or_else(|| {
            X11SetupSocketError::new("Sophia X Server Frontend exhausted client identities")
        })?;
        let resource_id_range = crate::XWireClientResourceRange {
            base: resource_id_base,
            mask: X_SETUP_DEFAULT_RESOURCE_ID_MASK,
        };
        Ok((
            XServerFrontendClientLease {
                client,
                resource_id_range,
            },
            XSetupSuccess {
                resource_id_base,
                resource_id_mask: X_SETUP_DEFAULT_RESOURCE_ID_MASK,
                root_size,
                ..XSetupSuccess::client_compatible()
            },
        ))
    }

    fn register_client(
        &self,
        lease: XServerFrontendClientLease,
    ) -> Result<(), X11SetupSocketError> {
        if self
            .clients
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 client lease lock poisoned"))?
            .client_leases
            .insert(lease.client, lease)
            .is_some()
        {
            return Err(X11SetupSocketError::new(
                "Sophia X Server Frontend assigned a duplicate client identity",
            ));
        }
        Ok(())
    }

    fn release_client(
        &self,
        client: XServerFrontendClientId,
    ) -> Result<XServerFrontendClientLease, X11SetupSocketError> {
        self.clients
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 client lease lock poisoned"))?
            .client_leases
            .remove(&client)
            .ok_or_else(|| {
                X11SetupSocketError::new("Sophia X Server Frontend lost a client connection lease")
            })
    }

    fn active_client_count(&self) -> usize {
        self.clients
            .lock()
            .map(|clients| clients.client_leases.len())
            .unwrap_or(0)
    }

    fn client_for_resource(
        &self,
        resource: XResourceId,
    ) -> Result<Option<XServerFrontendClientId>, X11SetupSocketError> {
        let raw = u32::try_from(resource.local.raw()).ok();
        let clients = self
            .clients
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 client lease lock poisoned"))?;
        Ok(raw.and_then(|raw| {
            clients.client_leases.iter().find_map(|(client, lease)| {
                lease
                    .resource_id_range
                    .owns_new_resource(raw)
                    .then_some(*client)
            })
        }))
    }
}

#[cfg(unix)]
fn release_x11_client_lease(
    state: &X11CoreSocketServerState,
    namespace: NamespaceId,
    lease: XServerFrontendClientLease,
) -> Result<crate::XAuthorityClientResourceRelease, X11SetupSocketError> {
    // Keep authority resource destruction and property removal together. X11
    // request dispatch acquires the runtime lock before the property lock, so
    // this prevents another client observing a destroyed window with stale
    // properties between the two cleanup steps.
    let mut runtime = state
        .runtime
        .lock()
        .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?;
    let release = runtime
        .release_client_resource_range(namespace, lease.resource_id_range)
        .map_err(|error| {
            X11SetupSocketError::new(format!("failed to release X11 client resources: {error:?}"))
        })?;
    let mut properties = state
        .properties
        .lock()
        .map_err(|_| X11SetupSocketError::new("X11 property table lock poisoned"))?;
    for window in &release.destroyed_windows {
        properties.remove_window(namespace, *window);
    }
    Ok(release)
}
