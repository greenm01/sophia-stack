#[derive(Clone, Debug)]
struct LivePublicPolicyCause {
    source: LiveWmProposalSource,
    cause: sophia_protocol::PolicyRequestCause,
    affected_outputs: Vec<sophia_protocol::OutputId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LivePolicySettlementIdentity {
    connection_epoch: u64,
    request_id: u64,
    scene_generation: u64,
    transaction: TransactionId,
    expect_session_operation: bool,
    session_operation: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublicPolicyFaultPoint {
    ProposalStaged,
    FrontendPending,
    Prepared,
    TerminalOutcomeQueued,
}

impl PublicPolicyFaultPoint {
    fn parse(value: &str) -> Result<Self, Box<dyn std::error::Error>> {
        match value {
            "proposal_staged" => Ok(Self::ProposalStaged),
            "frontend_pending" => Ok(Self::FrontendPending),
            "prepared" => Ok(Self::Prepared),
            "terminal_outcome_queued" => Ok(Self::TerminalOutcomeQueued),
            _ => Err(format!(
                "--wm-proof-fault-after expects proposal_staged, frontend_pending, prepared, or terminal_outcome_queued; got {value:?}"
            )
            .into()),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::ProposalStaged => "proposal_staged",
            Self::FrontendPending => "frontend_pending",
            Self::Prepared => "prepared",
            Self::TerminalOutcomeQueued => "terminal_outcome_queued",
        }
    }
}

struct LivePublicPolicyState {
    _profile_fragments: sophia_config::DesktopProfileFragments,
    _profile_slot: PreparedAuthorityFragment,
    directory: PolicySessionDirectory,
    checkpoint_path: std::path::PathBuf,
    worker: Option<PolicyTransportWorker>,
    reducer: sophia_engine::PolicyProjectionReducer,
    connection_epoch: u64,
    next_connection_epoch: u64,
    next_transaction: u64,
    configured: bool,
    negotiated: bool,
    cycle_submitted: bool,
    transport_ready: bool,
    queue: VecDeque<LivePublicPolicyCause>,
    pending_dirty_outputs: BTreeSet<sophia_protocol::OutputId>,
    in_flight_source: Option<LiveWmProposalSource>,
    in_flight_request: Option<sophia_protocol::PolicyProjectionRequest>,
    staged: Option<sophia_engine::StagedPolicyProjection>,
    prepared: Option<LivePolicySettlementIdentity>,
    shortcut_profile_slot:
        sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopShortcutCandidate>,
    actions: Vec<sophia_protocol::PolicyActionRegistration>,
    outputs: Vec<sophia_engine::HeadlessOutput>,
    output_generations: BTreeMap<sophia_protocol::OutputId, u64>,
    live_output_ids: BTreeSet<sophia_protocol::OutputId>,
    work_areas: BTreeMap<sophia_protocol::OutputId, Rect>,
    session_operations: Vec<sophia_protocol::PolicySessionOperation>,
    operation_actions: BTreeMap<u64, WmSessionAction>,
    expected_operation_slot: Option<u16>,
    pending_operation: Option<(TransactionId, sophia_protocol::PolicySessionOperationRequest)>,
    active_output: sophia_protocol::OutputId,
    deferred_command: Option<PolicyTransportCommand>,
    transport_unavailable: bool,
    proof_fault_after: Option<PublicPolicyFaultPoint>,
    proof_fault_triggered: bool,
}

struct PreparedPublicPolicyLaunch {
    profile_fragments: sophia_config::DesktopProfileFragments,
    directory: PolicySessionDirectory,
    policy_profile: PreparedAuthorityFragment,
    shell_profile: PreparedAuthorityFragment,
    shortcut_profile_slot:
        sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopShortcutCandidate>,
    broker_profile: PreparedAuthorityFragment,
}

impl PreparedPublicPolicyLaunch {
    fn new(config: &PersistentXtermSessionConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let directory = PolicySessionDirectory::create(
            config.wm_socket_path.with_extension("policy"),
        )?;
        let profile_fragments =
            sophia_config::stage_desktop_profile(&config.desktop_profile, directory.path())?;
        sophia_config::validate_desktop_profile_fragments(
            &profile_fragments,
            sophia_config::DesktopProfileActivationKey::from(&config.desktop_profile),
        )?;
        let key = sophia_config::DesktopProfileActivationKey::from(&config.desktop_profile);
        let policy_profile = PreparedAuthorityFragment::new(
            &profile_fragments,
            sophia_config::DesktopAuthority::Policy,
            key,
        )?;
        let shell_profile = PreparedAuthorityFragment::new(
            &profile_fragments,
            sophia_config::DesktopAuthority::Shell,
            key,
        )?;
        let shortcut_profile_slot = sophia_config::DesktopProfileCandidateSlot::with_candidate(
            config.shortcut_profile_candidate.clone(),
        )?;
        let broker_profile = PreparedAuthorityFragment::new(
            &profile_fragments,
            sophia_config::DesktopAuthority::Broker,
            key,
        )?;
        Ok(Self {
            profile_fragments,
            directory,
            policy_profile,
            shell_profile,
            shortcut_profile_slot,
            broker_profile,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublicPolicyRestartDecision {
    Idle,
    AbortSettlement,
    Restart,
}

const fn public_policy_restart_decision(
    restart_requested: bool,
    process_exited: bool,
    settlement_pending: bool,
) -> PublicPolicyRestartDecision {
    if !restart_requested && !process_exited {
        PublicPolicyRestartDecision::Idle
    } else if settlement_pending {
        PublicPolicyRestartDecision::AbortSettlement
    } else {
        PublicPolicyRestartDecision::Restart
    }
}

impl LivePublicPolicyState {
    fn initial_scene(
        outputs: &[sophia_engine::HeadlessOutput],
        active_output: sophia_protocol::OutputId,
        session_operations: Vec<sophia_protocol::PolicySessionOperation>,
    ) -> sophia_protocol::PolicySceneSnapshot {
        let bounds = wm_output_bounds(outputs);
        sophia_protocol::PolicySceneSnapshot {
            generation: 1,
            active_output,
            outputs: bounds
                .into_iter()
                .map(|(output, bounds)| sophia_protocol::PolicyOutputSnapshot {
                    output,
                    generation: 1,
                    focus: None,
                    bounds,
                    work_area: bounds,
                })
                .collect(),
            surfaces: Vec::new(),
            session_operations,
        }
    }

    fn mint_transaction(&mut self) -> Result<TransactionId, Box<dyn std::error::Error>> {
        let transaction = TransactionId::from_raw(self.next_transaction);
        self.next_transaction = self
            .next_transaction
            .checked_add(1)
            .ok_or("public WM transaction identity exhausted")?;
        Ok(transaction)
    }

    fn all_outputs(&self, active: sophia_protocol::OutputId) -> Vec<sophia_protocol::OutputId> {
        let mut outputs = self.outputs.iter().map(|output| output.id).collect::<Vec<_>>();
        outputs.sort_by_key(|output| output.raw());
        if let Some(index) = outputs.iter().position(|output| *output == active) {
            outputs.swap(0, index);
        }
        outputs
    }

    fn observe_outputs(
        &mut self,
        outputs: &[sophia_engine::HeadlessOutput],
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let descriptors_changed = self.outputs != outputs;
        let topology_changed = observe_public_output_topology(
            &mut self.output_generations,
            &mut self.live_output_ids,
            &mut self.active_output,
            outputs,
        )?;
        let next = outputs.iter().map(|output| output.id).collect::<BTreeSet<_>>();
        self.outputs = outputs.to_vec();
        self.work_areas.retain(|output, _| next.contains(output));
        Ok(topology_changed || descriptors_changed)
    }

    fn queue_cause(&mut self, cause: LivePublicPolicyCause) -> LiveWmRequestAdmission {
        if !matches!(cause.source, LiveWmProposalSource::Action(_))
            && (self.in_flight_source == Some(cause.source)
                || self.queue.iter().any(|pending| pending.source == cause.source))
        {
            return LiveWmRequestAdmission::Duplicate;
        }
        if self.queue.len().saturating_add(usize::from(self.in_flight_request.is_some()))
            >= WM_OWNER_REQUEST_CAPACITY
        {
            return LiveWmRequestAdmission::RejectedCapacity;
        }
        self.queue.push_back(cause);
        LiveWmRequestAdmission::Admitted
    }

    fn admit_dirty(
        &mut self,
        request: sophia_protocol::PolicyDirtyRequest,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if request.connection_epoch != self.connection_epoch || request.affected_outputs.is_empty() {
            return Err("public WM dirty request has an invalid connection or empty scope".into());
        }
        let affected = request
            .affected_outputs
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if affected.len() != request.affected_outputs.len()
            || !affected.is_subset(&self.live_output_ids)
        {
            return Err("public WM dirty request has duplicate or unknown outputs".into());
        }
        self.reducer
            .admit_policy_generation(request.policy_generation)?;
        self.pending_dirty_outputs.extend(affected);
        Ok(())
    }

    fn materialize_pending_dirty(&mut self) {
        if self.pending_dirty_outputs.is_empty()
            || self.in_flight_source == Some(LiveWmProposalSource::Relayout)
        {
            return;
        }
        if let Some(pending) = self
            .queue
            .iter_mut()
            .find(|pending| pending.source == LiveWmProposalSource::Relayout)
        {
            let mut outputs = pending
                .affected_outputs
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            outputs.append(&mut self.pending_dirty_outputs);
            pending.affected_outputs = outputs.into_iter().collect();
            return;
        }
        let affected_outputs = std::mem::take(&mut self.pending_dirty_outputs)
            .into_iter()
            .collect();
        self.queue.push_back(LivePublicPolicyCause {
            source: LiveWmProposalSource::Relayout,
            cause: sophia_protocol::PolicyRequestCause::SceneChanged,
            affected_outputs,
        });
    }

    fn submit_or_defer(
        &mut self,
        command: PolicyTransportCommand,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.deferred_command.is_some() {
            return Err("public WM already has a deferred transport command".into());
        }
        if self.transport_unavailable {
            return Ok(());
        }
        let worker = self
            .worker
            .as_ref()
            .ok_or("public WM transport is unavailable")?;
        if let Err(command) = worker.try_command(command) {
            self.deferred_command = Some(command);
        }
        Ok(())
    }

    fn flush_deferred_command(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(command) = self.deferred_command.take() else {
            return Ok(());
        };
        if self.transport_unavailable {
            return Ok(());
        }
        let worker = self
            .worker
            .as_ref()
            .ok_or("public WM transport is unavailable")?;
        if let Err(command) = worker.try_command(command) {
            self.deferred_command = Some(command);
        }
        Ok(())
    }

    fn settle_rejected_projection(
        &mut self,
        projection: &sophia_protocol::PolicyProjectionProposal,
        outcome: sophia_protocol::PolicyProjectionOutcome,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let _ = self.reducer.timeout(projection.request_id);
        self.submit_or_defer(PolicyTransportCommand::ProjectionOutcome {
            transaction: projection.transaction,
            request_id: projection.request_id,
            scene_generation: self.reducer.scene().generation,
            outcome,
            expect_session_operation: false,
        })?;
        self.cycle_submitted = false;
        self.in_flight_request = None;
        self.in_flight_source = None;
        self.expected_operation_slot = None;
        self.staged = None;
        Ok(())
    }

    fn snapshot(
        &self,
        layout: &PersistentLiveLayout,
    ) -> Result<sophia_protocol::PolicySceneSnapshot, Box<dyn std::error::Error>> {
        let previous = self.reducer.scene();
        let mut current_output = BTreeMap::new();
        let mut committed_geometry = BTreeMap::new();
        let mut committed_presentation = BTreeMap::new();
        for projection in self.reducer.committed() {
            for placement in projection.placements {
                current_output.insert(placement.surface, projection.output);
                committed_geometry.insert(placement.surface, placement.geometry);
                committed_presentation.insert(placement.surface, placement.presentation);
            }
        }
        let surfaces = public_policy_surface_snapshots(
            layout,
            &current_output,
            &committed_geometry,
            &committed_presentation,
        )?;
        println!(
            "sophia_live_wm_snapshot schema=1 status=complete surfaces={} minimized={} unassigned={}",
            surfaces.len(),
            surfaces
                .iter()
                .filter(|surface| surface.current_state.minimized)
                .count(),
            surfaces
                .iter()
                .filter(|surface| surface.current_output.is_none())
                .count(),
        );
        let bounds = wm_output_bounds(&self.outputs);
        let outputs = bounds
            .into_iter()
            .map(|(output, bounds)| {
                sophia_protocol::PolicyOutputSnapshot {
                    output,
                    generation: self.output_generations.get(&output).copied().unwrap_or(1),
                    focus: self
                        .reducer
                        .committed()
                        .into_iter()
                        .find(|projection| projection.output == output)
                        .and_then(|projection| projection.focus),
                    bounds,
                    work_area: self.work_areas.get(&output).copied().unwrap_or(bounds),
                }
            })
            .collect();
        let mut candidate = sophia_protocol::PolicySceneSnapshot {
            generation: previous.generation,
            active_output: self.active_output,
            outputs,
            surfaces,
            session_operations: self.session_operations.clone(),
        };
        let same_facts = candidate.active_output == previous.active_output
            && candidate.outputs == previous.outputs
            && candidate.surfaces == previous.surfaces
            && candidate.session_operations == previous.session_operations;
        if !same_facts {
            candidate.generation = previous
                .generation
                .checked_add(1)
                .ok_or("public WM scene generation exhausted")?;
        }
        Ok(candidate)
    }
}

fn public_policy_surface_snapshots(
    layout: &PersistentLiveLayout,
    current_output: &BTreeMap<SurfaceId, sophia_protocol::OutputId>,
    committed_geometry: &BTreeMap<SurfaceId, Rect>,
    committed_presentation: &BTreeMap<
        SurfaceId,
        sophia_protocol::PolicyPresentationState,
    >,
) -> Result<Vec<sophia_protocol::PolicySurfaceSnapshot>, Box<dyn std::error::Error>> {
    let mut surface_ids = layout
        .layers
        .keys()
        .chain(layout.planning_surfaces.keys())
        .chain(layout.authority_surface_facts.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    surface_ids.retain(|surface| {
        layout.is_policy_managed(*surface)
            && layout.client_routes.client_for_surface(*surface).is_some()
    });
    let mut surfaces = Vec::with_capacity(surface_ids.len());
    for surface in surface_ids {
        let facts = layout
            .layout_facts(surface)
            .ok_or("public WM scene lost a known surface")?;
        let kind = match facts.kind {
            sophia_protocol::LayoutNodeKind::Toplevel => {
                sophia_protocol::PolicySurfaceKind::Toplevel
            }
            sophia_protocol::LayoutNodeKind::Dialog => {
                sophia_protocol::PolicySurfaceKind::Dialog
            }
            sophia_protocol::LayoutNodeKind::Utility => {
                sophia_protocol::PolicySurfaceKind::Utility
            }
            sophia_protocol::LayoutNodeKind::Popup => sophia_protocol::PolicySurfaceKind::Popup,
            sophia_protocol::LayoutNodeKind::Unknown => {
                sophia_protocol::PolicySurfaceKind::Unknown
            }
        };
        surfaces.push(sophia_protocol::PolicySurfaceSnapshot {
            surface,
            generation: facts.generation.max(1),
            current_output: current_output.get(&surface).copied(),
            kind,
            capabilities: sophia_protocol::LayoutNodeCapabilities::STANDARD_TOPLEVEL,
            constraints: facts.constraints,
            exact_size: None,
            requested_state: committed_presentation
                .get(&surface)
                .copied()
                .unwrap_or_default(),
            current_state: committed_presentation
                .get(&surface)
                .copied()
                .unwrap_or_default(),
            transient_owner: facts.presentation_owner,
            geometry: committed_geometry
                .get(&surface)
                .copied()
                .unwrap_or(facts.geometry),
        });
    }
    surfaces.sort_by_key(|surface| surface.surface);
    Ok(surfaces)
}

impl Drop for LivePublicPolicyState {
    fn drop(&mut self) {
        // The checkpoint parent outlives each peer endpoint so supervised
        // replacement can preserve private policy state. Drop the endpoint
        // worker first, then remove the checkpoint and its session directory.
        self.worker.take();
        let _ = std::fs::remove_file(&self.checkpoint_path);
    }
}

fn observe_public_output_generations(
    generations: &mut BTreeMap<sophia_protocol::OutputId, u64>,
    live: &mut BTreeSet<sophia_protocol::OutputId>,
    outputs: &[sophia_engine::HeadlessOutput],
) -> Result<(), Box<dyn std::error::Error>> {
    let next = outputs.iter().map(|output| output.id).collect::<BTreeSet<_>>();
    for output in next.difference(live) {
        let generation = generations.entry(*output).or_insert(0);
        *generation = generation
            .checked_add(1)
            .ok_or("public WM output generation exhausted")?;
    }
    *live = next;
    Ok(())
}

fn observe_public_output_topology(
    generations: &mut BTreeMap<sophia_protocol::OutputId, u64>,
    live: &mut BTreeSet<sophia_protocol::OutputId>,
    active: &mut sophia_protocol::OutputId,
    outputs: &[sophia_engine::HeadlessOutput],
) -> Result<bool, Box<dyn std::error::Error>> {
    let topology = output_topology_from_engine_outputs(outputs)?;
    let next = outputs.iter().map(|output| output.id).collect::<BTreeSet<_>>();
    let changed = next != *live;
    let mut candidate_generations = generations.clone();
    let mut candidate_live = live.clone();
    observe_public_output_generations(
        &mut candidate_generations,
        &mut candidate_live,
        outputs,
    )?;
    let candidate_active = if next.contains(active) {
        *active
    } else {
        topology.primary
    };
    *generations = candidate_generations;
    *live = candidate_live;
    *active = candidate_active;
    Ok(changed)
}

fn public_session_operations(
    config: &PersistentXtermSessionConfig,
) -> (
    Vec<sophia_protocol::PolicySessionOperation>,
    BTreeMap<u64, WmSessionAction>,
) {
    let issuer = NEXT_POLICY_OPERATION_ISSUER.fetch_add(1, Ordering::Relaxed);
    assert!(
        issuer != 0 && issuer <= (u64::MAX >> 16),
        "public policy operation issuer identity exhausted"
    );
    let token = |slot: u16| (issuer << 16) | u64::from(slot);
    let mut operations = Vec::new();
    let mut actions = BTreeMap::new();
    let mut admit = |slot: u16, token: u64, action: WmSessionAction, target: bool| {
        operations.push(sophia_protocol::PolicySessionOperation {
            token,
            slot,
            permits_surface_target: target,
        });
        actions.insert(token, action);
    };
    if !config.normal_session || config.applications.terminal.is_some() {
        admit(
            1,
            token(1),
            WmSessionAction::LaunchApplication {
                application: TERMINAL_APPLICATION_ID,
            },
            false,
        );
    }
    if config.normal_session && config.applications.firefox.is_some() {
        admit(
            2,
            token(2),
            WmSessionAction::LaunchApplication {
                application: BROWSER_APPLICATION_ID,
            },
            false,
        );
    }
    admit(3, token(3), WmSessionAction::CloseFocused, true);
    if config.applications.logout_enabled {
        admit(4, token(4), WmSessionAction::Logout, false);
    }
    (operations, actions)
}

fn public_policy_launch_spec(
    config: &PersistentXtermSessionConfig,
    process: &str,
    socket_path: &std::path::Path,
    checkpoint_path: &std::path::Path,
    candidate_path: &std::path::Path,
) -> ProcessLaunchSpec {
    config.wm_process_args.iter().fold(
        ProcessLaunchSpec::new(process)
            .env(sophia_runtime::SOPHIA_WM_SOCKET_ENV, socket_path)
            .env("HAGIA_POLICY_CHECKPOINT", checkpoint_path)
            .env("HAGIA_POLICY_CANDIDATE", candidate_path)
            .process_group(),
        |spec, argument| spec.arg(argument),
    )
}

impl LiveWmSession {
    fn from_public_config(
        config: &PersistentXtermSessionConfig,
        outputs: &[sophia_engine::HeadlessOutput],
        process: &str,
        prepared_launch: PreparedPublicPolicyLaunch,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let PreparedPublicPolicyLaunch {
            profile_fragments,
            directory,
            policy_profile,
            shell_profile,
            shortcut_profile_slot,
            broker_profile,
        } = prepared_launch;
        let mut transport = sophia_runtime::PolicyWmSessionTransport::bind_for_supervised_uid(
            directory.endpoint_path(),
            rustix::process::geteuid().as_raw(),
        )?;
        let socket_path = transport.socket_path().to_path_buf();
        let checkpoint_path = directory.checkpoint_path();
        let spec = public_policy_launch_spec(
            config,
            process,
            &socket_path,
            &checkpoint_path,
            profile_fragments.path(sophia_config::DesktopAuthority::Policy),
        );
        let mut supervisor = ProcessSupervisor::new(SupervisedProcessKind::WindowManager, spec);
        let restart_policy = RestartPolicy::default();
        let mut supervisor_state =
            sophia_runtime::SupervisorState::new(SupervisedProcessKind::WindowManager);
        let (state, command) = update_supervisor(
            supervisor_state,
            SupervisorEvent::StartRequested,
            restart_policy,
        );
        supervisor_state = state;
        let started = supervisor
            .apply(command)?
            .ok_or("public WM supervisor did not start Hagia")?;
        let child_pid = supervisor
            .child_id()
            .ok_or("public WM supervisor did not retain Hagia's PID")?;
        transport.authorize_supervised_pid(child_pid)?;
        let (state, _) = update_supervisor(supervisor_state, started, restart_policy);
        supervisor_state = state;

        let (session_operations, operation_actions) = public_session_operations(config);
        let active = outputs
            .first()
            .map(|output| output.id)
            .ok_or("public WM requires at least one output")?;
        let scene = LivePublicPolicyState::initial_scene(outputs, active, session_operations.clone());
        let mut reducer = sophia_engine::PolicyProjectionReducer::new(scene)?;
        reducer.connect(1)?;
        let worker = PolicyTransportWorker::new(transport, 1)?;
        let work_areas = wm_output_bounds(outputs).into_iter().collect();
        let output_generations = outputs
            .iter()
            .map(|output| (output.id, 1))
            .collect::<BTreeMap<_, _>>();
        let live_output_ids = outputs
            .iter()
            .map(|output| output.id)
            .collect::<BTreeSet<_>>();
        let mut public = LivePublicPolicyState {
            _profile_fragments: profile_fragments,
            _profile_slot: policy_profile,
            directory,
            checkpoint_path,
            worker: Some(worker),
            reducer,
            connection_epoch: 1,
            next_connection_epoch: 2,
            next_transaction: 1,
            configured: false,
            negotiated: false,
            cycle_submitted: false,
            transport_ready: false,
            queue: VecDeque::with_capacity(WM_OWNER_REQUEST_CAPACITY),
            pending_dirty_outputs: BTreeSet::new(),
            in_flight_source: None,
            in_flight_request: None,
            staged: None,
            prepared: None,
            shortcut_profile_slot,
            actions: Vec::new(),
            outputs: outputs.to_vec(),
            output_generations,
            live_output_ids,
            work_areas,
            session_operations,
            operation_actions,
            expected_operation_slot: None,
            pending_operation: None,
            active_output: active,
            deferred_command: None,
            transport_unavailable: false,
            proof_fault_after: config.wm_public_fault_after,
            proof_fault_triggered: false,
        };
        public.queue.push_back(LivePublicPolicyCause {
            source: LiveWmProposalSource::Relayout,
            cause: sophia_protocol::PolicyRequestCause::SceneChanged,
            affected_outputs: public.all_outputs(active),
        });
        let workspace_state =
            WmWorkspaceState::new(wm_output_bounds(outputs), WM_DEFAULT_WORKSPACES)?;
        let session = Self {
            supervisor,
            supervisor_state,
            restart_policy,
            socket_path,
            transport: None,
            public: Some(public),
            _shell_profile: Some(shell_profile),
            _broker_profile: Some(broker_profile),
            queued_requests: LiveWmOwnerQueue::with_capacity(WM_OWNER_REQUEST_CAPACITY),
            in_flight_request: None,
            next_transaction: 1,
            requests: 0,
            request_peak_depth: 0,
            request_rejections: 0,
            action_requests_ordered: 0,
            stale_responses: 0,
            work_area_relayout_required: false,
            shortcuts: None,
            wm_chrome_supported: true,
            chrome: sophia_protocol::WmChromePolicy::default(),
            fallback_chrome: config.surface_chrome_style,
            visual_chrome: config.surface_chrome_style,
            pending_visual_chrome: None,
            pending_policy_update: None,
            force_transport_restart: false,
            workspace_state,
            session_actions: Vec::new(),
            committed: 0,
            last_committed_at: None,
            max_request: Duration::ZERO,
            max_queue_dwell: Duration::ZERO,
            restarts: 0,
            degraded: false,
        };
        println!(
            "sophia_live_wm schema=4 status=ready adapter=sophia_wm_v1 socket=session_owned epoch=1 restarts=0"
        );
        Ok(session)
    }

    fn poll_public_request(
        &mut self,
        layout: &mut PersistentLiveLayout,
        _output: sophia_engine::HeadlessOutput,
    ) -> Result<Option<LiveWmProposal>, Box<dyn std::error::Error>> {
        let mut public = self.public.take().expect("public WM state is present");
        public.flush_deferred_command()?;
        let event = public
            .worker
            .as_ref()
            .ok_or("public WM transport is unavailable")?
            .try_event();
        let mut transport_failed = None;
        let mut defer_cycle = false;
        let proposal = match event {
            Ok(Some(PolicyTransportEvent::Negotiated)) => {
                public.negotiated = true;
                None
            }
            Ok(Some(PolicyTransportEvent::ReadyForCycle)) => {
                public.transport_ready = true;
                None
            }
            Ok(Some(PolicyTransportEvent::Configuration {
                transaction,
                configuration,
            })) => {
                defer_cycle = true;
                let admitted_slots = public
                    .session_operations
                    .iter()
                    .map(|operation| operation.slot)
                    .collect::<BTreeSet<_>>();
                let slots_valid = configuration.actions.iter().all(|action| {
                    action
                        .session_operation_slot
                        .is_none_or(|slot| admitted_slots.contains(&slot))
                });
                let registry = slots_valid
                    .then(|| {
                        resolve_public_shortcuts(
                            public
                                .shortcut_profile_slot
                                .candidate()
                                .expect("public policy retains its prepared shortcut candidate"),
                            &configuration,
                        )
                    })
                    .and_then(Result::ok);
                let outcome = match registry {
                    Some(registry)
                        if configuration.connection_epoch == public.connection_epoch => {
                        self.chrome = configuration.chrome;
                        self.stage_visual_chrome(self.candidate_chrome_style());
                        self.shortcuts = Some(sophia_engine::WmShortcutRouter::new(registry));
                        public.actions = configuration.actions.clone();
                        public.configured = true;
                        sophia_protocol::PolicyProjectionOutcome::Committed
                    }
                    _ => sophia_protocol::PolicyProjectionOutcome::RejectedInvalid,
                };
                public.submit_or_defer(PolicyTransportCommand::ConfigurationOutcome {
                        transaction,
                        generation: configuration.generation,
                        outcome,
                    })?;
                if outcome != sophia_protocol::PolicyProjectionOutcome::Committed {
                    transport_failed = Some("invalid_configuration".to_owned());
                }
                None
            }
            Ok(Some(PolicyTransportEvent::Projection(projection))) => {
                let source = public
                    .in_flight_source
                    .ok_or("public WM projection has no owner cause")?;
                // Surface withdrawal may race a policy response. Advance the
                // canonical scene before touching response placements so a
                // proposal derived from the retired snapshot is rejected as
                // stale instead of trying to materialize a dead surface.
                let current_scene = public.snapshot(layout)?;
                if current_scene.generation > public.reducer.scene().generation {
                    public.reducer.observe_scene(current_scene)?;
                }
                if projection.base_generation != public.reducer.scene().generation {
                    defer_cycle = true;
                    public.settle_rejected_projection(
                        &projection,
                        sophia_protocol::PolicyProjectionOutcome::RejectedStale,
                    )?;
                    self.stale_responses = self.stale_responses.saturating_add(1);
                    println!(
                        "sophia_live_wm schema=1 status=stale_response_rejected transaction={} reason=scene_advanced",
                        projection.transaction.raw(),
                    );
                    None
                } else {
                    if let LiveWmProposalSource::Manage(surface) = source {
                        layout.prime_admission_extent(surface);
                    }
                    let (projection, adjusted_surfaces) = reconcile_public_policy_proposal(
                        layout,
                        &projection,
                        &public.work_areas,
                    )?;
                    if adjusted_surfaces != 0 {
                        println!(
                            "sophia_live_wm schema=1 status=constraints_reconciled transaction={} adjusted_surfaces={adjusted_surfaces}",
                            projection.transaction.raw(),
                        );
                    }
                    match public.reducer.stage_proposal(&projection) {
                    Ok(staged) => {
                        let expected_operation_slot = match source {
                            LiveWmProposalSource::Action(action) => public
                                .actions
                                .iter()
                                .find(|registered| registered.action == action)
                                .and_then(|registered| registered.session_operation_slot),
                            _ => None,
                        };
                        let expect_session_operation = expected_operation_slot.is_some();
                        let identity = LivePolicySettlementIdentity {
                            connection_epoch: projection.connection_epoch,
                            request_id: projection.request_id,
                            scene_generation: projection.base_generation,
                            transaction: projection.transaction,
                            expect_session_operation,
                            session_operation: false,
                        };
                        public.expected_operation_slot = expected_operation_slot;
                        let projections = staged.projections();
                        let active_output = projection.active_output;
                        public.staged = Some(staged);
                        Some(public_live_proposal(
                            layout,
                            active_output,
                            projections,
                            projection.transaction,
                            source,
                            identity,
                        )?)
                    }
                    Err(outcome) => {
                        defer_cycle = true;
                        public.settle_rejected_projection(&projection, outcome)?;
                        None
                    }
                    }
                }
            }
            Ok(Some(PolicyTransportEvent::Dirty(request))) => {
                if let Err(error) = public.admit_dirty(request) {
                    transport_failed = Some(format!("invalid_dirty:{error}"));
                }
                None
            }
            Ok(Some(PolicyTransportEvent::SessionOperation {
                transaction,
                request,
            })) => {
                let identity = LivePolicySettlementIdentity {
                    connection_epoch: request.connection_epoch,
                    request_id: request.request_id,
                    scene_generation: public.reducer.scene().generation,
                    transaction,
                    expect_session_operation: false,
                    session_operation: true,
                };
                let action = public.operation_actions.get(&request.operation).copied();
                let operation = public
                    .session_operations
                    .iter()
                    .find(|operation| operation.token == request.operation);
                let expected_slot = public.expected_operation_slot.take();
                let valid_target = request.target.is_none_or(|target| {
                    public
                        .reducer
                        .scene()
                        .surfaces
                        .iter()
                        .any(|surface| surface.surface == target)
                });
                let target_permitted = match (operation, request.target) {
                    (Some(operation), Some(_)) => operation.permits_surface_target,
                    (Some(_), None) => true,
                    (None, _) => false,
                };
                if request.connection_epoch != public.connection_epoch
                    || action.is_none()
                    || operation.map(|operation| operation.slot) != expected_slot
                    || !valid_target
                    || !target_permitted
                {
                    defer_cycle = true;
                    public.submit_or_defer(PolicyTransportCommand::SessionOperationOutcome {
                            transaction,
                            request_id: request.request_id,
                            outcome: sophia_protocol::PolicyProjectionOutcome::RejectedInvalid,
                        })?;
                    None
                } else {
                    public.pending_operation = Some((transaction, request));
                    Some(public_operation_proposal(
                        layout,
                        transaction,
                        identity,
                    ))
                }
            }
            Ok(Some(PolicyTransportEvent::Failed(error))) => {
                transport_failed = Some(error);
                None
            }
            Ok(None) => None,
            Err(()) => {
                transport_failed = Some("worker_disconnected".to_owned());
                None
            }
        };

        public.materialize_pending_dirty();

        if proposal.is_none()
            && transport_failed.is_none()
            && !defer_cycle
            && public.configured
            && !public.cycle_submitted
            && public.transport_ready
            && public.in_flight_request.is_none()
            && public.deferred_command.is_none()
            && let Some(cause) = public.queue.pop_front()
        {
            let scene = public.snapshot(layout)?;
            if scene.generation > public.reducer.scene().generation {
                public.reducer.observe_scene(scene.clone())?;
            }
            let request = public
                .reducer
                .issue_request_with_cause(cause.affected_outputs, cause.cause)?;
            let snapshot_transaction = public.mint_transaction()?;
            let request_transaction = public.mint_transaction()?;
            public
                .worker
                .as_ref()
                .ok_or("public WM transport is unavailable")?
                .try_command(PolicyTransportCommand::Cycle {
                    snapshot_transaction,
                    request_transaction,
                    scene,
                    actions: public.actions.clone(),
                    request: request.clone(),
                })
                .map_err(|_| "public WM cycle queue is busy")?;
            public.in_flight_source = Some(cause.source);
            public.in_flight_request = Some(request);
            public.cycle_submitted = true;
            public.transport_ready = false;
            self.requests = self.requests.saturating_add(1);
        }
        self.public = Some(public);
        if let Some(error) = transport_failed {
            self.request_transport_restart("public_transport_failed", Some(&error));
        }
        Ok(proposal)
    }

    fn poll_public_restart(
        &mut self,
        layout: &mut PersistentLiveLayout,
        output: sophia_engine::HeadlessOutput,
    ) -> Result<Option<LiveWmProposal>, Box<dyn std::error::Error>> {
        if self.degraded {
            return Ok(None);
        }
        let restart_requested = self.force_transport_restart;
        let process_exited = self.supervisor.poll()?.is_some();
        let settlement_pending = layout
            .pending
            .as_ref()
            .is_some_and(|pending| pending.policy_settlement.is_some());
        match public_policy_restart_decision(
            restart_requested,
            process_exited,
            settlement_pending,
        ) {
            PublicPolicyRestartDecision::Idle => return Ok(None),
            PublicPolicyRestartDecision::AbortSettlement => {
                if !process_exited {
                    self.supervisor.terminate()?;
                }
                let public = self.public.as_mut().expect("public WM state is present");
                public.worker.take();
                public.transport_unavailable = true;
                public.deferred_command = None;
                self.force_transport_restart = true;
                layout.force_pending_timeout();
                println!(
                    "sophia_live_wm schema=4 status=settlement_aborting adapter=sophia_wm_v1 reason=transport_lost preserved_layout=true"
                );
                return Ok(None);
            }
            PublicPolicyRestartDecision::Restart => {}
        }
        if restart_requested && !process_exited {
            self.supervisor.terminate()?;
        }
        let mut public = self.public.take().expect("public WM state is present");
        public.worker.take();
        let _ = public.reducer.disconnect(public.connection_epoch);
        self.shortcuts = None;
        self.force_transport_restart = false;
        self.restarts = self.restarts.saturating_add(1);
        let next_epoch = public.next_connection_epoch;
        public.next_connection_epoch = public
            .next_connection_epoch
            .checked_add(1)
            .ok_or("public WM connection epoch exhausted")?;
        let mut transport = sophia_runtime::PolicyWmSessionTransport::bind_for_supervised_uid(
            public.directory.endpoint_path(),
            rustix::process::geteuid().as_raw(),
        )?;
        let (state, command) = update_supervisor(
            self.supervisor_state.clone(),
            SupervisorEvent::ProcessExited,
            self.restart_policy,
        );
        self.supervisor_state = state;
        let started = match self.supervisor.apply(command) {
            Ok(Some(started)) => started,
            Ok(None) => return Err("public WM supervisor did not restart Hagia".into()),
            Err(error) => {
                if self.committed == 0 {
                    return Err(error.into());
                }
                self.degraded = true;
                self.public = Some(public);
                println!(
                    "sophia_live_wm schema=4 status=degraded adapter=sophia_wm_v1 reason=restart_failed preserved_layout=true error={error:?}"
                );
                return Ok(None);
            }
        };
        let pid = self
            .supervisor
            .child_id()
            .ok_or("restarted public WM has no supervised PID")?;
        transport.authorize_supervised_pid(pid)?;
        let (state, _) = update_supervisor(self.supervisor_state.clone(), started, self.restart_policy);
        self.supervisor_state = state;
        public.reducer.connect(next_epoch)?;
        public.worker = Some(PolicyTransportWorker::new(transport, next_epoch)?);
        public.connection_epoch = next_epoch;
        public.configured = false;
        public.negotiated = false;
        public.cycle_submitted = false;
        public.transport_ready = false;
        public.in_flight_request = None;
        public.in_flight_source = None;
        public.staged = None;
        public.prepared = None;
        public.pending_operation = None;
        public.expected_operation_slot = None;
        public.deferred_command = None;
        public.transport_unavailable = false;
        public.actions.clear();
        public.queue.clear();
        public.pending_dirty_outputs.clear();
        let affected_outputs = public.all_outputs(output.id);
        public.queue.push_back(LivePublicPolicyCause {
            source: LiveWmProposalSource::Relayout,
            cause: sophia_protocol::PolicyRequestCause::SceneChanged,
            affected_outputs,
        });
        self.public = Some(public);
        println!(
            "sophia_live_wm schema=4 status=restarted adapter=sophia_wm_v1 epoch={next_epoch} restarts={} preserved_layout=true",
            self.restarts
        );
        Ok(None)
    }

    fn update_public_work_areas(
        &mut self,
        layout: &PersistentLiveLayout,
        outputs: &[sophia_engine::HeadlessOutput],
        primary: sophia_engine::HeadlessOutput,
    ) -> Result<LiveWmRequestAdmission, Box<dyn std::error::Error>> {
        let full_bounds = wm_output_bounds(outputs);
        let root = full_bounds.iter().try_fold(
            Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            |root, (_, bounds)| {
                Some(Rect {
                    x: 0,
                    y: 0,
                    width: root.width.max(bounds.x.checked_add(bounds.width)?),
                    height: root.height.max(bounds.y.checked_add(bounds.height)?),
                })
            },
        );
        let Some(root) = root.filter(|root| !root.is_empty()) else {
            return Err("public WM output topology has no valid root bounds".into());
        };
        let reduced = sophia_engine::reduce_output_work_areas(
            root,
            full_bounds,
            &layout.active_output_reservations(),
        );
        let public = self.public.as_mut().expect("public WM state is present");
        let mut changed = public.observe_outputs(outputs)?;
        for area in reduced {
            let Some(work) = area.work else {
                continue;
            };
            changed |= public.work_areas.insert(area.output, work) != Some(work);
        }
        if !changed {
            return Ok(LiveWmRequestAdmission::Duplicate);
        }
        // Advance the reducer scene at the owner-observation boundary, before
        // a replacement request is issued. An in-flight response derived from
        // the retired output set is stale as soon as the owner accepts the new
        // topology; waiting until the next cycle would leave a click-through
        // window in which that response could still stage.
        let scene = public.snapshot(layout)?;
        if scene.generation > public.reducer.scene().generation {
            public.reducer.observe_scene(scene)?;
        }
        let affected_outputs = public.all_outputs(primary.id);
        Ok(public.queue_cause(LivePublicPolicyCause {
            source: LiveWmProposalSource::Relayout,
            cause: sophia_protocol::PolicyRequestCause::SceneChanged,
            affected_outputs,
        }))
    }

    fn prepare_public_layout_commit(
        &mut self,
        layout: &PersistentLiveLayout,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(identity) = layout
            .pending
            .as_ref()
            .and_then(|pending| pending.policy_settlement)
        else {
            return Ok(());
        };
        if identity.session_operation {
            return Ok(());
        }
        let public = self.public.as_mut().ok_or("public settlement lost its session")?;
        if public.prepared == Some(identity) {
            return Ok(());
        }
        let staged = public
            .staged
            .as_ref()
            .ok_or("ready public layout lost its staged reducer successor")?;
        let outcome = public.reducer.revalidate_staged(staged);
        if outcome != sophia_protocol::PolicyProjectionOutcome::Committed {
            return Err(format!(
                "ready public layout failed canonical revalidation: {outcome:?}"
            )
            .into());
        }
        public.prepared = Some(identity);
        Ok(())
    }

    fn trigger_public_proof_fault(&mut self, point: PublicPolicyFaultPoint) -> bool {
        let trigger = self.public.as_mut().is_some_and(|public| {
            if public.proof_fault_triggered || public.proof_fault_after != Some(point) {
                return false;
            }
            public.proof_fault_triggered = true;
            true
        });
        if trigger {
            self.request_transport_restart("public_policy_proof_fault", Some(point.name()));
            println!(
                "sophia_live_wm schema=4 status=proof_fault_triggered adapter=sophia_wm_v1 phase={} preserved_layout=true",
                point.name(),
            );
        }
        trigger
    }

    fn public_settlement_abort_required(&self) -> bool {
        self.public
            .as_ref()
            .is_some_and(|public| public.transport_unavailable)
    }
}

include!("public_policy/proposal.rs");
