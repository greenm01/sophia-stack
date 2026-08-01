const WM_OWNER_REQUEST_CAPACITY: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
struct LiveWmLayoutFingerprint(Vec<SurfaceId>);

impl LiveWmLayoutFingerprint {
    fn capture(layout: &PersistentLiveLayout, state: &WmWorkspaceState) -> Self {
        Self(
            layout
                .layers
                .keys()
                .chain(layout.planning_surfaces.keys())
                .copied()
                .filter(|surface| layout.is_policy_managed(*surface))
                .filter(|surface| state.surface_workspace(*surface).is_some())
                .collect(),
        )
    }

    fn still_matches(&self, layout: &PersistentLiveLayout) -> bool {
        self.0
            .iter()
            .all(|surface| layout.knows_surface(*surface))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveWmProposalSource {
    Action(WmActionId),
    Focus(SurfaceId),
    Manage(SurfaceId),
    Relayout,
}

impl LiveWmProposalSource {
    const fn reduced_name(self) -> &'static str {
        match self {
            Self::Action(_) => "action",
            Self::Focus(_) => "focus",
            Self::Manage(_) => "manage",
            Self::Relayout => "relayout",
        }
    }
}

enum LiveWmQueuedKind {
    Proposal {
        base_state: WmWorkspaceState,
        fingerprint: LiveWmLayoutFingerprint,
        source: LiveWmProposalSource,
    },
    SurfaceRemoved {
        surface: SurfaceId,
    },
}

struct LiveWmQueuedRequest {
    packet: WmRequestPacket,
    kind: LiveWmQueuedKind,
    queued_at: Instant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveWmRequestAdmission {
    Admitted,
    RejectedCapacity,
    Duplicate,
}

fn require_wm_request_admission(
    admission: LiveWmRequestAdmission,
    source: &'static str,
) -> Result<(), Box<dyn std::error::Error>> {
    match admission {
        LiveWmRequestAdmission::Admitted | LiveWmRequestAdmission::Duplicate => Ok(()),
        LiveWmRequestAdmission::RejectedCapacity => {
            Err(format!("WM {source} request exceeded the owner queue capacity").into())
        }
    }
}

fn planning_state_for_response(
    current: &WmWorkspaceState,
    request: &WmRequestPacket,
) -> Result<WmWorkspaceState, Box<dyn std::error::Error>> {
    let mut planning_state = current.clone();
    if let WmRequestKind::ManageSurface(manage) = &request.kind
        && planning_state
            .surface_workspace(manage.node.surface)
            .is_none()
    {
        planning_state.register_surface(manage.node.surface, manage.workspace)?;
    }
    Ok(planning_state)
}

struct LiveWmSession {
    supervisor: ProcessSupervisor,
    supervisor_state: sophia_runtime::SupervisorState,
    restart_policy: RestartPolicy,
    socket_path: std::path::PathBuf,
    transport: Option<WmTransportWorker>,
    queued_requests: VecDeque<LiveWmQueuedRequest>,
    in_flight_request: Option<LiveWmQueuedRequest>,
    next_transaction: u64,
    requests: usize,
    request_peak_depth: usize,
    request_rejections: usize,
    stale_responses: usize,
    work_area_relayout_required: bool,
    shortcuts: Option<WmShortcutRouter>,
    wm_chrome_supported: bool,
    chrome: sophia_protocol::WmChromePolicy,
    fallback_chrome: sophia_engine::SurfaceChromeStyle,
    visual_chrome: sophia_engine::SurfaceChromeStyle,
    pending_visual_chrome: Option<sophia_engine::SurfaceChromeStyle>,
    pending_policy_update: Option<WmPolicyUpdate>,
    force_transport_restart: bool,
    workspace_state: WmWorkspaceState,
    session_actions: Vec<WmSessionAction>,
    committed: usize,
    last_committed_at: Option<Instant>,
    max_request: Duration,
    max_queue_dwell: Duration,
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
    source: Option<LiveWmProposalSource>,
    effects: Option<LiveWmCommitEffects>,
}

struct LiveWmCommitEffects {
    workspace_state: WmWorkspaceState,
    transaction: TransactionId,
    session_action: Option<(WmSessionAction, Option<SurfaceId>)>,
}

struct LiveWmCommitResult {
    update: WmTransactionUpdate,
    source: Option<LiveWmProposalSource>,
    effects: Option<LiveWmCommitEffects>,
}

struct LiveWmOwnerCommit {
    update: WmTransactionUpdate,
    physical_action: Option<WmActionId>,
    session_action: Option<(TransactionId, WmSessionAction, Option<SurfaceId>)>,
    workspace_projection: Option<LiveWmWorkspaceProjection>,
    clear_focus: Option<(TransactionId, SurfaceId)>,
    restore_focus: Option<(TransactionId, SurfaceId)>,
}

#[derive(Clone, Copy)]
struct LiveWmWorkspaceProjection {
    transaction: TransactionId,
    output: sophia_protocol::OutputId,
    workspace: WorkspaceId,
    visible_surfaces: usize,
    focus_present: bool,
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
            wm_chrome_supported: false,
            chrome: sophia_protocol::WmChromePolicy::default(),
            fallback_chrome: config.surface_chrome_style,
            visual_chrome: config.surface_chrome_style,
            pending_visual_chrome: None,
            pending_policy_update: None,
            force_transport_restart: false,
            workspace_state,
            session_actions,
            socket_path: config.wm_socket_path.clone(),
            transport: None,
            queued_requests: VecDeque::with_capacity(WM_OWNER_REQUEST_CAPACITY),
            in_flight_request: None,
            next_transaction: 1,
            requests: 0,
            request_peak_depth: 0,
            request_rejections: 0,
            stale_responses: 0,
            work_area_relayout_required: false,
            committed: 0,
            last_committed_at: None,
            max_request: Duration::ZERO,
            max_queue_dwell: Duration::ZERO,
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
        self.accept_negotiated_chrome(&registry);
        match self.shortcuts.as_mut() {
            Some(shortcuts) => shortcuts.replace_registry(registry),
            None => self.shortcuts = Some(WmShortcutRouter::new(registry)),
        }
        self.pending_policy_update = None;
        self.force_transport_restart = false;
        self.transport = Some(WmTransportWorker::new(transport)?);
        let (state, _) = update_supervisor(
            self.supervisor_state.clone(),
            SupervisorEvent::ProcessHealthy,
            self.restart_policy,
        );
        self.supervisor_state = state;
        Ok(())
    }

    fn request_transport_restart(&mut self, reason: &str, error: Option<&str>) {
        self.force_transport_restart = true;
        println!(
            "sophia_live_wm schema=2 status=restart_requested reason={reason} error={}",
            error.unwrap_or("none"),
        );
    }

    fn poll_restart(
        &mut self,
        layout: &PersistentLiveLayout,
        output: sophia_engine::HeadlessOutput,
    ) -> Result<Option<LiveWmProposal>, Box<dyn std::error::Error>> {
        if self.degraded {
            return Ok(None);
        }
        let restart_requested = self.force_transport_restart;
        let process_exited = self.supervisor.poll()?.is_some();
        if !restart_requested && !process_exited {
            return Ok(None);
        }
        if restart_requested && !process_exited {
            self.supervisor.terminate()?;
        }
        self.transport = None;
        self.pending_policy_update = None;
        self.force_transport_restart = false;
        self.queued_requests.clear();
        self.in_flight_request = None;
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
            let _ = self.enqueue_relayout(layout, output)?;
            Ok(None)
        }
    }

    fn enqueue_manage(
        &mut self,
        surface: SurfaceId,
        layout: &PersistentLiveLayout,
        output: sophia_engine::HeadlessOutput,
    ) -> Result<LiveWmRequestAdmission, Box<dyn std::error::Error>> {
        if !layout.is_policy_managed(surface) {
            return Ok(LiveWmRequestAdmission::Duplicate);
        }
        if self.has_request_source(LiveWmProposalSource::Manage(surface)) {
            return Ok(LiveWmRequestAdmission::Duplicate);
        }
        let facts = layout
            .layout_facts(surface)
            .ok_or("new WM surface is missing from live layout")?;
        let workspace = self
            .workspace_state
            .output(output.id)
            .ok_or("WM output is not configured")?
            .workspace;
        let bounds = self
            .workspace_state
            .output(output.id)
            .ok_or("WM output is not configured")?
            .bounds;
        let mut planning_state = self.workspace_state.clone();
        planning_state.register_surface(surface, workspace)?;
        let request = WmRequestPacket {
            transaction: self.mint_transaction()?,
            kind: WmRequestKind::ManageSurface(WmManageSurface {
                node: live_layout_node_from_facts(
                    facts,
                    workspace,
                    &layout.layout_epochs,
                    self.candidate_chrome_style(),
                )?,
                output: output.id,
                workspace,
                bounds,
            }),
        };
        let fingerprint = LiveWmLayoutFingerprint::capture(layout, &planning_state);
        Ok(self.enqueue_request(LiveWmQueuedRequest {
            packet: request,
            kind: LiveWmQueuedKind::Proposal {
                base_state: self.workspace_state.clone(),
                fingerprint,
                source: LiveWmProposalSource::Manage(surface),
            },
            queued_at: Instant::now(),
        })?)
    }

    fn enqueue_relayout(
        &mut self,
        layout: &PersistentLiveLayout,
        output: sophia_engine::HeadlessOutput,
    ) -> Result<LiveWmRequestAdmission, Box<dyn std::error::Error>> {
        if self.has_current_relayout_request(layout) {
            return Ok(LiveWmRequestAdmission::Duplicate);
        }
        let output_state = self
            .workspace_state
            .output(output.id)
            .ok_or("WM output is not configured")?;
        let workspace = output_state.workspace;
        let request = WmRequestPacket {
            transaction: self.mint_transaction()?,
            kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                output: output.id,
                workspace,
                bounds: output_state.bounds,
                nodes: layout
                    .layers
                    .values()
                    .filter(|layer| {
                        layout.is_policy_managed(layer.surface)
                            && self.workspace_state.surface_workspace(layer.surface)
                                == Some(workspace)
                    })
                    .map(|layer| {
                        live_layout_node(
                            layer,
                            workspace,
                            &layout.layout_epochs,
                            self.candidate_chrome_style(),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            }),
        };
        self.enqueue_request(LiveWmQueuedRequest {
            packet: request,
            kind: LiveWmQueuedKind::Proposal {
                base_state: self.workspace_state.clone(),
                fingerprint: LiveWmLayoutFingerprint::capture(layout, &self.workspace_state),
                source: LiveWmProposalSource::Relayout,
            },
            queued_at: Instant::now(),
        })
    }

    fn enqueue_surface_removed(
        &mut self,
        surface: SurfaceId,
    ) -> Result<LiveWmRequestAdmission, Box<dyn std::error::Error>> {
        let Some(workspace) = self.workspace_state.surface_workspace(surface) else {
            return Ok(LiveWmRequestAdmission::Duplicate);
        };
        if self.has_surface_removal(surface) {
            return Ok(LiveWmRequestAdmission::Duplicate);
        }
        let request = WmRequestPacket {
            transaction: self.mint_transaction()?,
            kind: WmRequestKind::SurfaceRemoved { surface, workspace },
        };
        self.enqueue_request(LiveWmQueuedRequest {
            packet: request,
            kind: LiveWmQueuedKind::SurfaceRemoved { surface },
            queued_at: Instant::now(),
        })
    }

    fn enqueue_action(
        &mut self,
        action: WmActionId,
        focused_surface: Option<SurfaceId>,
        layout: &PersistentLiveLayout,
        output: sophia_engine::HeadlessOutput,
    ) -> Result<LiveWmRequestAdmission, Box<dyn std::error::Error>> {
        let output_state = self
            .workspace_state
            .output(output.id)
            .ok_or("WM output is not configured")?;
        let nodes = layout
            .layers
            .values()
            .filter_map(|layer| {
                if !layout.is_policy_managed(layer.surface) {
                    return None;
                }
                let workspace = self.workspace_state.surface_workspace(layer.surface)?;
                (workspace == output_state.workspace).then_some((layer, workspace))
            })
            .map(|(layer, workspace)| {
                live_layout_node(
                    layer,
                    workspace,
                    &layout.layout_epochs,
                    self.candidate_chrome_style(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
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
        self.enqueue_request(LiveWmQueuedRequest {
            packet: request,
            kind: LiveWmQueuedKind::Proposal {
                base_state: self.workspace_state.clone(),
                fingerprint: LiveWmLayoutFingerprint::capture(layout, &self.workspace_state),
                source: LiveWmProposalSource::Action(action),
            },
            queued_at: Instant::now(),
        })
    }

    fn enqueue_focus(
        &mut self,
        surface: SurfaceId,
        layout: &PersistentLiveLayout,
        output: sophia_engine::HeadlessOutput,
    ) -> Result<LiveWmRequestAdmission, Box<dyn std::error::Error>> {
        let source = LiveWmProposalSource::Focus(surface);
        if self.has_request_source(source) {
            return Ok(LiveWmRequestAdmission::Duplicate);
        }
        if !layout.layers.contains_key(&surface) {
            return Err("pointer focus target is missing from the live layout".into());
        }
        let output_state = self
            .workspace_state
            .output(output.id)
            .ok_or("WM output is not configured")?;
        let workspace = self
            .workspace_state
            .surface_workspace(surface)
            .ok_or("pointer focus target is not registered with the WM")?;
        if workspace != output_state.workspace {
            return Err("pointer focus target is not on the active workspace".into());
        }
        let request = WmRequestPacket {
            transaction: self.mint_transaction()?,
            kind: WmRequestKind::FocusRequested(sophia_protocol::WmFocusRequest {
                surface,
                output: output.id,
                workspace,
            }),
        };
        self.enqueue_request(LiveWmQueuedRequest {
            packet: request,
            kind: LiveWmQueuedKind::Proposal {
                base_state: self.workspace_state.clone(),
                fingerprint: LiveWmLayoutFingerprint::capture(layout, &self.workspace_state),
                source,
            },
            queued_at: Instant::now(),
        })
    }

    fn enqueue_request(
        &mut self,
        request: LiveWmQueuedRequest,
    ) -> Result<LiveWmRequestAdmission, Box<dyn std::error::Error>> {
        let depth = self
            .queued_requests
            .len()
            .saturating_add(usize::from(self.in_flight_request.is_some()));
        if depth >= WM_OWNER_REQUEST_CAPACITY {
            self.request_rejections = self.request_rejections.saturating_add(1);
            return Ok(LiveWmRequestAdmission::RejectedCapacity);
        }
        self.queued_requests.push_back(request);
        self.request_peak_depth = self
            .request_peak_depth
            .max(self.queued_requests.len().saturating_add(usize::from(
                self.in_flight_request.is_some(),
            )));
        self.pump_transport()?;
        Ok(LiveWmRequestAdmission::Admitted)
    }

    fn poll_request(
        &mut self,
        layout: &PersistentLiveLayout,
        output: sophia_engine::HeadlessOutput,
    ) -> Result<Option<LiveWmProposal>, Box<dyn std::error::Error>> {
        if self.work_area_relayout_required {
            match self.enqueue_relayout(layout, output)? {
                LiveWmRequestAdmission::Admitted | LiveWmRequestAdmission::Duplicate => {}
                LiveWmRequestAdmission::RejectedCapacity => {
                    return Err("WM work-area relayout exceeded the owner request capacity".into());
                }
            }
        }
        self.pump_transport()?;
        let completion = match self
            .transport
            .as_ref()
            .ok_or("WM transport is unavailable")?
            .try_complete()
        {
            Ok(Some(completion)) => completion,
            Ok(None) => return Ok(None),
            Err(WmTransportSubmitError::Disconnected) => {
                self.request_transport_restart("request_completion_disconnected", None);
                return Ok(None);
            }
            Err(WmTransportSubmitError::Busy) => {
                return Err("WM transport worker returned an invalid busy completion".into());
            }
        };
        let queued = self
            .in_flight_request
            .take()
            .ok_or("WM transport completed without an in-flight request")?;
        if completion.transaction != queued.packet.transaction {
            return Err(format!(
                "WM transport completion mismatch: expected={} actual={}",
                queued.packet.transaction.raw(),
                completion.transaction.raw(),
            )
            .into());
        }
        self.max_request = self.max_request.max(completion.elapsed);
        self.requests = self.requests.saturating_add(1);
        let response = completion
            .result
            .map_err(|error| format!("WM transport request failed: {error}"))?;
        if response.commands.len() > 8_192 {
            return Err("WM response exceeds the live command limit".into());
        }
        let proposal = match queued.kind {
            LiveWmQueuedKind::SurfaceRemoved { surface } => {
                self.workspace_state.remove_surface(surface);
                match self.enqueue_relayout(layout, output)? {
                    LiveWmRequestAdmission::Admitted
                    | LiveWmRequestAdmission::Duplicate => {}
                    LiveWmRequestAdmission::RejectedCapacity => {
                        return Err(
                            "WM removal relayout exceeded the owner request capacity".into()
                        );
                    }
                }
                None
            }
            LiveWmQueuedKind::Proposal {
                fingerprint,
                source,
                ..
            } => {
                if !fingerprint.still_matches(layout) {
                    self.stale_responses = self.stale_responses.saturating_add(1);
                    println!(
                        "sophia_live_wm schema=2 status=response_rejected reason=stale_layout transaction={} source={}",
                        completion.transaction.raw(),
                        source.reduced_name(),
                    );
                    None
                } else {
                    let planning_state =
                        planning_state_for_response(&self.workspace_state, &queued.packet)?;
                    Some(self.proposal_from_response(
                        response,
                        planning_state,
                        source,
                        layout,
                        output,
                    )?)
                }
            }
        };
        if proposal.is_none() {
            self.pump_transport()?;
        }
        Ok(proposal)
    }

    fn proposal_from_response(
        &self,
        response: WmResponsePacket,
        planning_state: WmWorkspaceState,
        source: LiveWmProposalSource,
        layout: &PersistentLiveLayout,
        output: sophia_engine::HeadlessOutput,
    ) -> Result<LiveWmProposal, Box<dyn std::error::Error>> {
        let bounds = planning_state
            .output(output.id)
            .ok_or("WM output is not configured")?
            .bounds;
        let plan = planning_state.plan_response(&response, &self.session_actions)?;
        let transaction = sophia_engine::apply_surface_chrome_clearance(
            &plan.layout,
            self.candidate_chrome_style(),
        )?;
        let reconciliation = layout
            .layout_epochs
            .reconcile_transaction(&transaction, bounds)?;
        let transaction = reconciliation.transaction;
        if !reconciliation.adjusted_surfaces.is_empty() {
            println!(
                "sophia_live_wm schema=1 status=constraints_reconciled transaction={} adjusted_surfaces={}",
                transaction.transaction.raw(),
                reconciliation.adjusted_surfaces.len(),
            );
        }
        validate_live_wm_transaction(&transaction, layout, bounds)?;
        let mut proposed = layout.planning_layers();
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
            source: Some(source),
            effects: Some(LiveWmCommitEffects {
                workspace_state: plan.candidate,
                transaction: transaction.transaction,
                session_action: plan.session_action,
            }),
        })
    }

    fn pump_transport(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.in_flight_request.is_some() {
            return Ok(());
        }
        let Some(request) = self.queued_requests.pop_front() else {
            return Ok(());
        };
        let queue_dwell = request.queued_at.elapsed();
        self.max_queue_dwell = self.max_queue_dwell.max(queue_dwell);
        if queue_dwell >= Duration::from_millis(500) {
            println!(
                "sophia_live_wm schema=2 status=request_delayed transaction={} queue_dwell_msec={}",
                request.packet.transaction.raw(),
                queue_dwell.as_millis(),
            );
        }
        let packet = request.packet.clone();
        match self
            .transport
            .as_ref()
            .ok_or("WM transport is unavailable")?
            .try_submit(packet)
        {
            Ok(()) => {
                self.in_flight_request = Some(request);
                Ok(())
            }
            Err(WmTransportSubmitError::Busy) => {
                self.queued_requests.push_front(request);
                Ok(())
            }
            Err(WmTransportSubmitError::Disconnected) => {
                self.queued_requests.push_front(request);
                self.request_transport_restart("request_submit_disconnected", None);
                Ok(())
            }
        }
    }

    fn has_request_source(&self, source: LiveWmProposalSource) -> bool {
        self.in_flight_request
            .iter()
            .chain(self.queued_requests.iter())
            .any(|request| {
                matches!(
                    &request.kind,
                    LiveWmQueuedKind::Proposal {
                        source: pending,
                        ..
                    } if *pending == source
                )
            })
    }

    fn has_surface_removal(&self, surface: SurfaceId) -> bool {
        self.in_flight_request
            .iter()
            .chain(self.queued_requests.iter())
            .any(|request| {
                matches!(
                    &request.kind,
                    LiveWmQueuedKind::SurfaceRemoved { surface: pending }
                        if *pending == surface
                )
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
