struct LiveWmSession {
    supervisor: ProcessSupervisor,
    supervisor_state: sophia_runtime::SupervisorState,
    restart_policy: RestartPolicy,
    socket_path: std::path::PathBuf,
    transport: Option<WmSocketTransport>,
    next_transaction: u64,
    requests: usize,
    shortcuts: Option<WmShortcutRouter>,
    workspace_state: WmWorkspaceState,
    session_actions: Vec<WmSessionAction>,
    committed: usize,
    last_committed_at: Option<Instant>,
    restarts: usize,
    degraded: bool,
}

struct LiveWmProposal {
    transaction: TransactionId,
    layers: Vec<LayerSnapshot>,
    requested_sizes: BTreeMap<SurfaceId, Size>,
    focus: Option<SurfaceId>,
    timeout: Duration,
    update: WmTransactionUpdate,
    moved_surfaces: usize,
    effects: Option<LiveWmCommitEffects>,
}

struct LiveWmCommitEffects {
    workspace_state: WmWorkspaceState,
    transaction: TransactionId,
    session_action: Option<(WmSessionAction, Option<SurfaceId>)>,
}

struct LiveWmCommitResult {
    update: WmTransactionUpdate,
    effects: Option<LiveWmCommitEffects>,
}

impl LiveWmSession {
    fn from_config(
        config: &PersistentXtermSessionConfig,
        outputs: &[sophia_engine::HeadlessOutput],
    ) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let Some(process) = config.wm_process.as_deref() else {
            return Ok(None);
        };
        let _ = std::fs::remove_file(&config.wm_socket_path);
        let socket_arg = format!("--socket={}", config.wm_socket_path.display());
        let spec = config.wm_process_args.iter().fold(
            ProcessLaunchSpec::new(process)
                .arg("serve-socket")
                .arg(socket_arg)
                .process_group(),
            |spec, argument| spec.arg(argument),
        );
        let workspace_state =
            WmWorkspaceState::new(wm_output_bounds(outputs), WM_DEFAULT_WORKSPACES)?;
        let mut session_actions = vec![WmSessionAction::CloseFocused, WmSessionAction::Logout];
        if !config.normal_session || config.applications.terminal.is_some() {
            session_actions.push(WmSessionAction::LaunchApplication {
                application: TERMINAL_APPLICATION_ID,
            });
        }
        if config.normal_session && config.applications.launcher.is_some() {
            session_actions.push(WmSessionAction::LaunchApplication {
                application: LAUNCHER_APPLICATION_ID,
            });
        }
        if config.normal_session && config.applications.firefox.is_some() {
            session_actions.push(WmSessionAction::LaunchApplication {
                application: BROWSER_APPLICATION_ID,
            });
        }
        if config.session_launcher.is_some() {
            session_actions.push(WmSessionAction::LaunchApplication {
                application: LAUNCHER_APPLICATION_ID,
            });
        }
        if config.session_firefox.is_some() {
            session_actions.push(WmSessionAction::LaunchApplication {
                application: BROWSER_APPLICATION_ID,
            });
        }
        let mut session = Self {
            supervisor: ProcessSupervisor::new(SupervisedProcessKind::WindowManager, spec),
            supervisor_state: sophia_runtime::SupervisorState::new(
                SupervisedProcessKind::WindowManager,
            ),
            restart_policy: RestartPolicy::default(),
            shortcuts: None,
            workspace_state,
            session_actions,
            socket_path: config.wm_socket_path.clone(),
            transport: None,
            next_transaction: 1,
            requests: 0,
            committed: 0,
            last_committed_at: None,
            restarts: 0,
            degraded: false,
        };
        session.start(SupervisorEvent::StartRequested)?;
        println!("sophia_live_wm schema=1 status=ready adapter=external socket=private restarts=0");
        Ok(Some(session))
    }

    fn start(&mut self, event: SupervisorEvent) -> Result<(), Box<dyn std::error::Error>> {
        let _ = std::fs::remove_file(&self.socket_path);
        let (state, command) =
            update_supervisor(self.supervisor_state.clone(), event, self.restart_policy);
        self.supervisor_state = state;
        let start_event = self
            .supervisor
            .apply(command)?
            .ok_or("WM supervisor did not start the configured process")?;
        let (state, _) = update_supervisor(
            self.supervisor_state.clone(),
            start_event,
            self.restart_policy,
        );
        self.supervisor_state = state;
        super::x_authority::wait_for_socket_path(&self.socket_path)?;
        let stream = UnixStream::connect(&self.socket_path)?;
        let mut transport = WmSocketTransport::new(
            stream,
            WmSocketTransportConfig {
                response_timeout: Duration::from_millis(500),
            },
        );
        let descriptor = self
            .workspace_state
            .descriptor(self.session_actions.clone());
        let registry = transport.negotiate(&descriptor)?;
        match self.shortcuts.as_mut() {
            Some(shortcuts) => shortcuts.replace_registry(registry),
            None => self.shortcuts = Some(WmShortcutRouter::new(registry)),
        }
        self.transport = Some(transport);
        let (state, _) = update_supervisor(
            self.supervisor_state.clone(),
            SupervisorEvent::ProcessHealthy,
            self.restart_policy,
        );
        self.supervisor_state = state;
        Ok(())
    }

    fn poll_restart(
        &mut self,
        layout: &PersistentLiveLayout,
        output: sophia_engine::HeadlessOutput,
    ) -> Result<Option<LiveWmProposal>, Box<dyn std::error::Error>> {
        if self.degraded || self.supervisor.poll()?.is_none() {
            return Ok(None);
        }
        self.transport = None;
        self.restarts = self.restarts.saturating_add(1);
        if let Err(error) = self.start(SupervisorEvent::ProcessExited) {
            if self.committed == 0 {
                return Err(error);
            }
            self.degraded = true;
            println!(
                "sophia_live_wm schema=1 status=degraded reason=restart_failed preserved_layout=true error={error:?}"
            );
            return Ok(None);
        }
        println!(
            "sophia_live_wm schema=1 status=restarted restarts={} preserved_layout=true",
            self.restarts
        );
        if layout.layers.is_empty() {
            Ok(None)
        } else {
            self.request_relayout(layout, output).map(Some)
        }
    }

    fn request_manage(
        &mut self,
        surface: SurfaceId,
        layout: &PersistentLiveLayout,
        output: sophia_engine::HeadlessOutput,
    ) -> Result<LiveWmProposal, Box<dyn std::error::Error>> {
        let node = layout
            .layers
            .get(&surface)
            .ok_or("new WM surface is missing from live layout")?;
        let workspace = self
            .workspace_state
            .output(output.id)
            .ok_or("WM output is not configured")?
            .workspace;
        self.workspace_state.register_surface(surface, workspace)?;
        let committed_state = self.workspace_state.clone();
        let request = WmRequestPacket {
            transaction: self.mint_transaction()?,
            kind: WmRequestKind::ManageSurface(WmManageSurface {
                node: live_layout_node(node, workspace),
                output: output.id,
                workspace,
                bounds: output_bounds(output),
            }),
        };
        let result = self.request(request, layout, output);
        self.workspace_state = committed_state;
        match result {
            Err(error)
                if error.downcast_ref::<WmPolicyError>()
                    == Some(&WmPolicyError::UnknownSurface) =>
            {
                println!(
                    "sophia_live_wm schema=1 status=manage_resync reason=stale_surface preserved_layout=true"
                );
                self.request_relayout(layout, output)
            }
            result => result,
        }
    }

    fn request_relayout(
        &mut self,
        layout: &PersistentLiveLayout,
        output: sophia_engine::HeadlessOutput,
    ) -> Result<LiveWmProposal, Box<dyn std::error::Error>> {
        let workspace = self
            .workspace_state
            .output(output.id)
            .ok_or("WM output is not configured")?
            .workspace;
        let request = WmRequestPacket {
            transaction: self.mint_transaction()?,
            kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                output: output.id,
                workspace,
                bounds: output_bounds(output),
                nodes: layout
                    .layers
                    .values()
                    .map(|layer| live_layout_node(layer, workspace))
                    .collect(),
            }),
        };
        self.request(request, layout, output)
    }

    fn notify_surface_removed(
        &mut self,
        surface: SurfaceId,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(workspace) = self.workspace_state.surface_workspace(surface) else {
            return Ok(());
        };
        let request = WmRequestPacket {
            transaction: self.mint_transaction()?,
            kind: WmRequestKind::SurfaceRemoved { surface, workspace },
        };
        let response = self
            .transport
            .as_mut()
            .ok_or("WM transport is unavailable")?
            .request(&request)?;
        self.requests = self.requests.saturating_add(1);
        if response.commands.len() > 8_192 {
            return Err("WM removal response exceeds the live command limit".into());
        }
        self.workspace_state.remove_surface(surface);
        Ok(())
    }

    fn request_action(
        &mut self,
        action: WmActionId,
        focused_surface: Option<SurfaceId>,
        layout: &PersistentLiveLayout,
        output: sophia_engine::HeadlessOutput,
    ) -> Result<LiveWmProposal, Box<dyn std::error::Error>> {
        let output_state = self
            .workspace_state
            .output(output.id)
            .ok_or("WM output is not configured")?;
        let nodes = layout
            .layers
            .values()
            .filter_map(|layer| {
                let workspace = self.workspace_state.surface_workspace(layer.surface)?;
                (workspace == output_state.workspace).then(|| live_layout_node(layer, workspace))
            })
            .collect();
        let request = WmRequestPacket {
            transaction: self.mint_transaction()?,
            kind: WmRequestKind::ActionActivated(WmActionActivation {
                action,
                output: output.id,
                workspace: output_state.workspace,
                focused_surface,
                nodes,
            }),
        };
        self.request(request, layout, output)
    }

    fn request(
        &mut self,
        request: WmRequestPacket,
        layout: &PersistentLiveLayout,
        output: sophia_engine::HeadlessOutput,
    ) -> Result<LiveWmProposal, Box<dyn std::error::Error>> {
        let response = self
            .transport
            .as_mut()
            .ok_or("WM transport is unavailable")?
            .request(&request)?;
        self.requests = self.requests.saturating_add(1);
        if response.commands.len() > 8_192 {
            return Err("WM response exceeds the live command limit".into());
        }
        let plan = self
            .workspace_state
            .plan_response(&response, &self.session_actions)?;
        let transaction = plan.layout;
        validate_live_wm_transaction(&transaction, layout, output_bounds(output))?;
        let mut proposed = layout.layers.values().cloned().collect::<Vec<_>>();
        let engine = HeadlessEngine::new(output);
        let commit = engine.commit_layout_transaction(&transaction, &mut proposed);
        if commit.outcome != TransactionOutcome::Committed {
            return Err(format!("Engine rejected live WM proposal: {:?}", commit.outcome).into());
        }
        let requested_sizes = transaction
            .requested_sizes
            .iter()
            .map(|request| (request.surface, request.size))
            .collect();
        let moved_surfaces = proposed
            .iter()
            .filter(|layer| {
                layout
                    .layers
                    .get(&layer.surface)
                    .is_some_and(|current| current.geometry != layer.geometry)
            })
            .count();
        let timeout = Duration::from_millis(u64::from(transaction.timeout_msec.clamp(100, 2_000)));
        Ok(LiveWmProposal {
            transaction: transaction.transaction,
            layers: proposed,
            requested_sizes,
            focus: transaction.focus,
            timeout,
            update: WmTransactionUpdate {
                commit,
                ipc_error: None,
            },
            moved_surfaces,
            effects: Some(LiveWmCommitEffects {
                workspace_state: plan.candidate,
                transaction: transaction.transaction,
                session_action: plan.session_action,
            }),
        })
    }

    fn mint_transaction(&mut self) -> Result<TransactionId, Box<dyn std::error::Error>> {
        let transaction = TransactionId::from_raw(self.next_transaction);
        self.next_transaction = self
            .next_transaction
            .checked_add(1)
            .ok_or("WM transaction ID space exhausted")?;
        Ok(transaction)
    }

    fn mark_committed(&mut self) {
        self.committed = self.committed.saturating_add(1);
        self.last_committed_at = Some(Instant::now());
    }
}

impl Drop for LiveWmSession {
    fn drop(&mut self) {
        let _ = self.supervisor.terminate();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

struct PendingLiveWmLayout {
    transaction: TransactionId,
    layers: Vec<LayerSnapshot>,
    requested_sizes: BTreeMap<SurfaceId, Size>,
    focus: Option<SurfaceId>,
    deadline: Instant,
    update: WmTransactionUpdate,
    moved_surfaces: usize,
    staged_transactions: BTreeMap<SurfaceId, SurfaceTransaction>,
    effects: Option<LiveWmCommitEffects>,
}

#[derive(Default)]
struct PersistentLiveLayout {
    layers: BTreeMap<SurfaceId, LayerSnapshot>,
    resize: ResizeRollbackCoordinator,
    client_routes: XAuthorityClientSurfaceRoutes,
    unmanaged_surfaces: BTreeSet<SurfaceId>,
    pending: Option<PendingLiveWmLayout>,
    focus_to_apply: Option<(TransactionId, SurfaceId)>,
    stage_new_surfaces_offset: bool,
    center_first_surface_in: Option<Size>,
}

impl PersistentLiveLayout {
    fn new(stage_new_surfaces_offset: bool, center_first_surface_in: Option<Size>) -> Self {
        Self {
            stage_new_surfaces_offset,
            center_first_surface_in,
            ..Self::default()
        }
    }

    fn observe_authority_batch(
        &mut self,
        batch: &XAuthorityObservedTransactionBatch,
    ) -> Vec<SurfaceId> {
        self.client_routes.observe(batch);
        self.remove_surfaces(&batch.removed_surfaces);
        let mut new_surfaces = Vec::new();
        for (index, transaction) in batch.transactions.iter().enumerate() {
            let size = Size {
                width: transaction.target_geometry.width,
                height: transaction.target_geometry.height,
            };
            if !self.resize.accept_observation(transaction.surface, size) {
                continue;
            }
            let staged_for_resize = self.pending.as_ref().is_some_and(|pending| {
                pending.requested_sizes.get(&transaction.surface) == Some(&size)
            });
            if staged_for_resize {
                let pending = self.pending.as_mut().expect("checked above");
                pending
                    .staged_transactions
                    .insert(transaction.surface, transaction.clone());
                if let Some(layer) = pending
                    .layers
                    .iter_mut()
                    .find(|layer| layer.surface == transaction.surface)
                {
                    layer.source = transaction.target_buffer;
                    layer.damage = transaction.damage.clone();
                    layer.generation = transaction.previous_committed_generation.saturating_add(1);
                }
                continue;
            }
            self.resize.record_committed(transaction.surface, size);
            match self.layers.get_mut(&transaction.surface) {
                Some(layer) => {
                    layer.source = transaction.target_buffer;
                    layer.damage = transaction.damage.clone();
                    layer.generation = transaction.previous_committed_generation.saturating_add(1);
                }
                None => {
                    new_surfaces.push(transaction.surface);
                    self.unmanaged_surfaces.insert(transaction.surface);
                    let mut geometry = transaction.target_geometry;
                    if self.stage_new_surfaces_offset {
                        geometry.x = geometry.x.saturating_add(80);
                        geometry.y = geometry.y.saturating_add(60);
                    } else if let Some(output) = self.center_first_surface_in.take() {
                        geometry = center_geometry_without_scaling(geometry, output);
                    }
                    self.layers.insert(
                        transaction.surface,
                        LayerSnapshot {
                            surface: transaction.surface,
                            authority_local_id: None,
                            namespace: None,
                            stack_rank: u32::try_from(index).unwrap_or(u32::MAX),
                            geometry,
                            source: transaction.target_buffer,
                            damage: transaction.damage.clone(),
                            opacity: 1.0,
                            crop: None,
                            transform: Transform::IDENTITY,
                            generation: transaction.previous_committed_generation.saturating_add(1),
                            resize_sync: ResizeSyncCapability::ImplicitOnly,
                        },
                    );
                }
            }
        }
        new_surfaces
    }

    fn remove_surfaces(&mut self, removed_surfaces: &[SurfaceId]) {
        if removed_surfaces.is_empty() {
            return;
        }
        self.layers
            .retain(|surface, _| !removed_surfaces.contains(surface));
        for surface in removed_surfaces {
            self.resize.remove(*surface);
        }
        self.unmanaged_surfaces
            .retain(|surface| !removed_surfaces.contains(surface));
        if self
            .focus_to_apply
            .is_some_and(|(_, surface)| removed_surfaces.contains(&surface))
        {
            self.focus_to_apply = None;
        }
        if let Some(pending) = self.pending.as_mut() {
            pending
                .layers
                .retain(|layer| !removed_surfaces.contains(&layer.surface));
            pending
                .requested_sizes
                .retain(|surface, _| !removed_surfaces.contains(surface));
            if pending
                .focus
                .is_some_and(|surface| removed_surfaces.contains(&surface))
            {
                pending.focus = None;
            }
        }
    }

    fn next_unmanaged_surface(&self) -> Option<SurfaceId> {
        self.unmanaged_surfaces.iter().next().copied()
    }

    fn mark_surface_managed(&mut self, surface: SurfaceId) {
        self.unmanaged_surfaces.remove(&surface);
    }

    fn stage(
        &mut self,
        mut proposal: LiveWmProposal,
        control_sender: &SyncSender<XAuthorityClientControlCommand>,
        control_ack_receiver: &Receiver<XAuthorityClientControlAck>,
    ) -> Result<Option<LiveWmCommitResult>, Box<dyn std::error::Error>> {
        if self.pending.is_some() {
            println!(
                "sophia_live_wm schema=1 status=proposal_busy transaction={} preserved_layout=true",
                proposal.transaction.raw()
            );
            return Ok(None);
        }
        for (surface, size) in &proposal.requested_sizes {
            if !self.resize.request_allowed(*surface, *size)
                && let Some(committed) = self.resize.committed_size(*surface)
                && let Some(layer) = proposal
                    .layers
                    .iter_mut()
                    .find(|layer| layer.surface == *surface)
            {
                layer.geometry.width = committed.width;
                layer.geometry.height = committed.height;
            }
        }
        proposal.requested_sizes.retain(|surface, size| {
            self.resize.committed_size(*surface) != Some(*size)
                && self.resize.request_allowed(*surface, *size)
        });
        for (surface, size) in &proposal.requested_sizes {
            let client = self
                .client_routes
                .client_for_surface(*surface)
                .ok_or("live WM configure has no X11 client route for its surface")?;
            control_sender.try_send(XAuthorityClientControlCommand {
                client,
                command: XAuthorityControlCommand::ConfigureSurface {
                    transaction: proposal.transaction,
                    surface: *surface,
                    size: *size,
                },
            })?;
        }
        for _ in 0..proposal.requested_sizes.len() {
            let acknowledgement = control_ack_receiver.recv_timeout(Duration::from_millis(500))?;
            let expected_client = self
                .client_routes
                .client_for_surface(acknowledgement.acknowledgement.surface);
            if acknowledgement.acknowledgement.transaction != proposal.transaction
                || acknowledgement.acknowledgement.outcome != XAuthorityControlOutcome::Delivered
                || expected_client != Some(acknowledgement.client)
            {
                return Err(format!(
                    "X Authority rejected WM configure transaction {} for surface {:?}: {:?}",
                    acknowledgement.acknowledgement.transaction.raw(),
                    acknowledgement.acknowledgement.surface,
                    acknowledgement.acknowledgement.outcome
                )
                .into());
            }
        }
        let ready = proposal
            .requested_sizes
            .iter()
            .all(|(surface, size)| self.resize.committed_size(*surface) == Some(*size));
        if ready {
            return Ok(Some(self.commit_proposal(proposal)));
        }
        self.pending = Some(PendingLiveWmLayout {
            transaction: proposal.transaction,
            layers: proposal.layers,
            requested_sizes: proposal.requested_sizes,
            focus: proposal.focus,
            deadline: Instant::now() + proposal.timeout,
            update: proposal.update,
            moved_surfaces: proposal.moved_surfaces,
            staged_transactions: BTreeMap::new(),
            effects: proposal.effects,
        });
        Ok(None)
    }

    fn resolve_pending(&mut self) -> Option<LiveWmCommitResult> {
        let pending = self.pending.as_ref()?;
        let ready = pending.requested_sizes.iter().all(|(surface, size)| {
            pending
                .staged_transactions
                .get(surface)
                .is_some_and(|transaction| {
                    transaction.target_geometry.width == size.width
                        && transaction.target_geometry.height == size.height
                })
        });
        if !ready {
            return None;
        }
        let pending = self.pending.take().expect("checked above");
        Some(self.commit_pending(pending))
    }

    fn expire_pending(
        &mut self,
        control_sender: &SyncSender<XAuthorityClientControlCommand>,
        control_ack_receiver: &Receiver<XAuthorityClientControlAck>,
    ) -> Result<Option<LiveWmCommitResult>, Box<dyn std::error::Error>> {
        if !self
            .pending
            .as_ref()
            .is_some_and(|pending| Instant::now() >= pending.deadline)
        {
            return Ok(None);
        }
        let pending = self.pending.take().expect("checked above");
        let rollback = self.resize.begin_rollback(
            pending
                .requested_sizes
                .iter()
                .map(|(surface, size)| (*surface, *size)),
        )?;
        let rollback_transaction = rollback
            .first()
            .map(|request| request.transaction)
            .unwrap_or(pending.transaction);
        for request in rollback {
            let surface = request.surface;
            let size = request.size;
            let client = self
                .client_routes
                .client_for_surface(surface)
                .ok_or("live WM rollback has no X11 client route")?;
            control_sender.try_send(XAuthorityClientControlCommand {
                client,
                command: XAuthorityControlCommand::ConfigureSurface {
                    transaction: rollback_transaction,
                    surface,
                    size,
                },
            })?;
        }
        for _ in 0..pending.requested_sizes.len() {
            let acknowledgement = control_ack_receiver.recv_timeout(Duration::from_millis(500))?;
            if acknowledgement.acknowledgement.transaction != rollback_transaction
                || acknowledgement.acknowledgement.outcome != XAuthorityControlOutcome::Delivered
                || self
                    .client_routes
                    .client_for_surface(acknowledgement.acknowledgement.surface)
                    != Some(acknowledgement.client)
            {
                return Err("X Authority rejected live WM rollback configure".into());
            }
        }
        let resize_state = pending
            .requested_sizes
            .iter()
            .map(|(surface, expected)| {
                let observed = self.resize.committed_size(*surface).unwrap_or(Size {
                    width: 0,
                    height: 0,
                });
                format!(
                    "{}x{}:{}x{}",
                    expected.width, expected.height, observed.width, observed.height
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "sophia_live_wm schema=1 status=layout_timeout transaction={} preserved_layout=true rollback_transaction={} rollback_configures={} resize_state={}",
            pending.transaction.raw(),
            rollback_transaction.raw(),
            pending.requested_sizes.len(),
            resize_state,
        );
        if let Some(surface) = pending.focus {
            self.focus_to_apply = Some((pending.transaction, surface));
        }
        Ok(Some(LiveWmCommitResult {
            update: WmTransactionUpdate {
                commit: TransactionCommit {
                    transaction: pending.transaction,
                    outcome: TransactionOutcome::TimedOut,
                    applied_surfaces: Vec::new(),
                },
                ipc_error: None,
            },
            effects: None,
        }))
    }

    fn commit_proposal(&mut self, proposal: LiveWmProposal) -> LiveWmCommitResult {
        let pending = PendingLiveWmLayout {
            transaction: proposal.transaction,
            layers: proposal.layers,
            requested_sizes: proposal.requested_sizes,
            focus: proposal.focus,
            deadline: Instant::now(),
            update: proposal.update,
            moved_surfaces: proposal.moved_surfaces,
            staged_transactions: BTreeMap::new(),
            effects: proposal.effects,
        };
        self.commit_pending(pending)
    }

    fn commit_pending(&mut self, pending: PendingLiveWmLayout) -> LiveWmCommitResult {
        if !pending.staged_transactions.is_empty() {
            for transaction in pending.staged_transactions.values() {
                self.resize.record_committed(
                    transaction.surface,
                    Size {
                        width: transaction.target_geometry.width,
                        height: transaction.target_geometry.height,
                    },
                );
            }
        }
        self.layers = pending
            .layers
            .into_iter()
            .map(|layer| (layer.surface, layer))
            .collect();
        if let Some(surface) = pending.focus {
            self.focus_to_apply = Some((pending.transaction, surface));
        }
        println!(
            "sophia_live_wm schema=1 status=layout_committed transaction={} surfaces={} moved_surfaces={} configure_acks={} outcome={:?}",
            pending.transaction.raw(),
            self.layers.len(),
            pending.moved_surfaces,
            pending.requested_sizes.len(),
            pending.update.commit.outcome
        );
        LiveWmCommitResult {
            update: pending.update,
            effects: pending.effects,
        }
    }

    fn projected_batch(
        &mut self,
        batch: &XAuthorityObservedTransactionBatch,
    ) -> XAuthorityObservedTransactionBatch {
        // Resize quarantine controls geometry and presentation, not authority
        // intake. Dropping a drawing transaction here would break the X
        // authority's per-surface generation chain permanently. Preserve each
        // transaction and buffer update in order, but pin its geometry to the
        // last layout decision until a coherent proposal commits.
        project_authority_batch_onto_layout(batch.clone(), &self.layers)
    }
}

fn center_geometry_without_scaling(mut geometry: Rect, output: Size) -> Rect {
    geometry.x = output.width.saturating_sub(geometry.width).max(0) / 2;
    geometry.y = output.height.saturating_sub(geometry.height).max(0) / 2;
    geometry
}

fn output_bounds(output: sophia_engine::HeadlessOutput) -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: output.size.width,
        height: output.size.height,
    }
}

fn live_layout_node(layer: &LayerSnapshot, workspace: WorkspaceId) -> LayoutNodeSnapshot {
    LayoutNodeSnapshot {
        surface: layer.surface,
        workspace,
        kind: LayoutNodeKind::Toplevel,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        state: LayoutNodeState::NORMAL,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        geometry: layer.geometry,
        generation: layer.generation,
    }
}

fn validate_live_wm_transaction(
    transaction: &sophia_protocol::LayoutTransaction,
    layout: &PersistentLiveLayout,
    bounds: Rect,
) -> Result<(), Box<dyn std::error::Error>> {
    for placement in &transaction.render_positions {
        let known = layout.layers.contains_key(&placement.surface);
        let empty = placement.geometry.is_empty();
        let within = rect_is_within(bounds, placement.geometry);
        if !known || empty || !within {
            return Err(format!(
                "live WM returned invalid placement: known={known} empty={empty} within={within} geometry={:?} bounds={bounds:?}",
                placement.geometry
            )
            .into());
        }
    }
    for request in &transaction.requested_sizes {
        if !layout.layers.contains_key(&request.surface)
            || request.size.width <= 0
            || request.size.height <= 0
            || request.size.width > i32::from(u16::MAX)
            || request.size.height > i32::from(u16::MAX)
        {
            return Err("live WM returned an invalid surface size request".into());
        }
    }
    if transaction
        .focus
        .is_some_and(|surface| !layout.layers.contains_key(&surface))
    {
        return Err("live WM returned an unknown focus surface".into());
    }
    Ok(())
}

fn rect_is_within(bounds: Rect, geometry: Rect) -> bool {
    let Some(bounds_right) = bounds.x.checked_add(bounds.width) else {
        return false;
    };
    let Some(bounds_bottom) = bounds.y.checked_add(bounds.height) else {
        return false;
    };
    let Some(right) = geometry.x.checked_add(geometry.width) else {
        return false;
    };
    let Some(bottom) = geometry.y.checked_add(geometry.height) else {
        return false;
    };
    geometry.x >= bounds.x
        && geometry.y >= bounds.y
        && right <= bounds_right
        && bottom <= bounds_bottom
}

fn successful_primary_exit_ends_session(input_proof_requested: bool) -> bool {
    !input_proof_requested
}

fn global_runtime_deadline_ends_session(input_proof_requested: bool) -> bool {
    !input_proof_requested
}
