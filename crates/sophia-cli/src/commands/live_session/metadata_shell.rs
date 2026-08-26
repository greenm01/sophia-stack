use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LiveMetadataShellPoll {
    Healthy,
    Reconnected { connection_epoch: u64 },
    Unavailable,
}

const SHELL_RECONNECT_RETRY_DELAY: Duration = Duration::from_secs(1);

#[derive(Clone, Debug)]
struct PendingShellPresentation {
    transaction: TransactionId,
    candidate_generation: u64,
    connection_epoch: u64,
    output: sophia_protocol::OutputId,
    visible: bool,
    actions: BTreeMap<sophia_protocol::ToplevelActionCapabilityRef, SurfaceId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PresentedShellCandidate {
    candidate_generation: u64,
    presentation_epoch: u64,
    output: sophia_protocol::OutputId,
}

#[derive(Clone, Copy, Debug)]
struct ShellOutputIdentity {
    descriptor: Option<sophia_engine::HeadlessOutput>,
    generation: u64,
}

/// Session owner for the separately protected metadata shell.
///
/// The shell sees only bounded descriptors and opaque slots. This owner keeps
/// every slot-to-surface relation, validates its returned candidate through
/// Engine, and waits for the output-local presentation boundary before enabling
/// activation.
pub(super) struct LiveMetadataShell {
    supervisor: ProcessSupervisor,
    transport: sophia_runtime::ShellSessionTransport,
    slots: BTreeMap<SurfaceId, u16>,
    next_slot: u16,
    outputs: BTreeMap<sophia_protocol::OutputId, ShellOutputIdentity>,
    next_connection_epoch: u64,
    next_snapshot_generation: u64,
    next_projection: u64,
    next_transaction: u64,
    pending: Option<PendingShellPresentation>,
    presented: Option<PresentedShellCandidate>,
    presented_actions: BTreeMap<sophia_protocol::ToplevelActionCapabilityRef, SurfaceId>,
    reservations: sophia_engine::ShellWorkAreaCoordinator,
    connected: bool,
    reconnect_at: Option<Instant>,
}

impl LiveMetadataShell {
    pub(super) fn start(
        executable: &str,
        panel_thickness: Option<u16>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let directory = std::env::temp_dir().join(format!(
            "sophia-live-metadata-shell-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        let transport = sophia_runtime::ShellSessionTransport::bind_for_supervised_uid(
            &directory,
            rustix::process::geteuid().as_raw(),
        )?;
        let socket = transport.socket_path().to_path_buf();
        let domain = sophia_runtime::ProtectionDomainSpec::bubblewrap([
            sophia_runtime::ProtectionDomainRole::MetadataShell,
        ])?
        .path(sophia_runtime::ProtectionPath::read_only(
            socket
                .parent()
                .expect("metadata shell socket always has a parent"),
        ))?;
        let mut spec = ProcessLaunchSpec::new(executable)
            .arg("--serve")
            .env(sophia_runtime::SOPHIA_SHELL_SOCKET_ENV, &socket)
            .process_group()
            .protection_domain(domain);
        // The session decides how much desktop a panel may claim, so the
        // thickness crosses into the protected domain the same way the socket
        // does. Absent, the shell reserves nothing.
        if let Some(thickness) = panel_thickness {
            spec = spec.env("SOPHIA_SHELL_BAR_THICKNESS", thickness.to_string());
        }
        let supervisor = ProcessSupervisor::new(SupervisedProcessKind::Shell, spec);
        let mut shell = Self {
            supervisor,
            transport,
            slots: BTreeMap::new(),
            next_slot: 1,
            outputs: BTreeMap::new(),
            next_connection_epoch: 1,
            next_snapshot_generation: 1,
            next_projection: 1,
            next_transaction: 1,
            pending: None,
            presented: None,
            presented_actions: BTreeMap::new(),
            reservations: sophia_engine::ShellWorkAreaCoordinator::new(),
            connected: false,
            reconnect_at: None,
        };
        let (peer_pid, revision, connection_epoch) = shell.launch_and_negotiate()?;
        shell.connected = true;
        println!(
            "sophia_live_metadata_shell schema=1 status=ready protected=true peer_pid={peer_pid} revision={revision} connection_epoch={connection_epoch}"
        );
        Ok(shell)
    }

    pub(super) fn poll(&mut self) -> Result<LiveMetadataShellPoll, Box<dyn std::error::Error>> {
        if self.connected {
            if self.supervisor.poll()?.is_none() {
                return Ok(LiveMetadataShellPoll::Healthy);
            }
            self.connected = false;
            return self.reconnect_or_defer("process_exit");
        }
        if self
            .reconnect_at
            .is_some_and(|deadline| Instant::now() < deadline)
        {
            return Ok(LiveMetadataShellPoll::Unavailable);
        }
        self.reconnect_or_defer("retry")
    }

    pub(super) fn recover_transport(
        &mut self,
        reason: &str,
    ) -> Result<LiveMetadataShellPoll, Box<dyn std::error::Error>> {
        self.supervisor.terminate()?;
        self.connected = false;
        self.reconnect_or_defer(reason)
    }

    pub(super) fn observe_outputs(
        &mut self,
        outputs: &[sophia_engine::HeadlessOutput],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let live = outputs
            .iter()
            .map(|output| output.id)
            .collect::<BTreeSet<_>>();
        for identity in self.outputs.values_mut() {
            if identity
                .descriptor
                .is_some_and(|descriptor| !live.contains(&descriptor.id))
            {
                identity.descriptor = None;
            }
        }
        for output in outputs.iter().copied() {
            match self.outputs.get_mut(&output.id) {
                Some(identity) if identity.descriptor == Some(output) => {}
                Some(identity) => {
                    identity.generation = identity
                        .generation
                        .checked_add(1)
                        .ok_or("shell output generation exhausted")?;
                    identity.descriptor = Some(output);
                }
                None => {
                    self.outputs.insert(
                        output.id,
                        ShellOutputIdentity {
                            descriptor: Some(output),
                            generation: 1,
                        },
                    );
                }
            }
        }
        Ok(())
    }

    pub(super) fn request_candidate(
        &mut self,
        broker: &LiveMetadataBroker,
        output: sophia_engine::HeadlessOutput,
        bounds: sophia_protocol::Rect,
        root: sophia_protocol::Rect,
        output_bounds: &[(sophia_protocol::OutputId, sophia_protocol::Rect)],
        activation_surfaces: &BTreeSet<SurfaceId>,
    ) -> Result<Option<sophia_engine::DescriptorOverlayProjection>, Box<dyn std::error::Error>>
    {
        if self.pending.is_some() {
            return Err("metadata shell already has an unpresented candidate".into());
        }
        let output_generation = self
            .outputs
            .get(&output.id)
            .filter(|identity| identity.descriptor == Some(output))
            .map(|identity| identity.generation)
            .ok_or("metadata shell candidate targets an unobserved output")?;
        let sources = broker
            .shell_sources(activation_surfaces)
            .into_iter()
            .take(sophia_protocol::SOPHIA_SHELL_MAX_DESCRIPTORS)
            .collect::<Vec<_>>();
        for source in &sources {
            self.ensure_slot(source.surface)?;
        }
        let broker_revocation_epoch = sources
            .first()
            .map_or(1, |source| source.grant.revocation_epoch);
        if sources
            .iter()
            .any(|source| source.grant.revocation_epoch != broker_revocation_epoch)
        {
            return Err("metadata shell snapshot spans broker revocation epochs".into());
        }
        let connection_epoch = self.transport.connection_epoch();
        let snapshot_generation = self.take_snapshot_generation()?;
        let descriptors = sources
            .iter()
            .map(|source| {
                let slot = self.slots[&source.surface];
                sophia_protocol::ShellV1Descriptor {
                    slot,
                    generation: source.descriptor.generation,
                    label: source.descriptor.label.clone(),
                    trust_level: source.descriptor.trust_level,
                    attention: source.descriptor.attention,
                    action: sophia_protocol::ToplevelActionCapabilityRef {
                        token: source.grant.token,
                        issuer_epoch: broker.connection_epoch(),
                        issuer_revocation_epoch: source.grant.revocation_epoch,
                        recipient_epoch: connection_epoch,
                        target_slot: slot,
                        target_generation: source.grant.target_generation,
                    },
                }
            })
            .collect::<Vec<_>>();
        let snapshot = sophia_protocol::ShellV1DescriptorSnapshot {
            connection_epoch,
            snapshot_generation,
            output: output.id,
            output_generation,
            broker_epoch: broker.connection_epoch(),
            broker_revocation_epoch,
            descriptors,
        };
        let transaction = self.take_transaction()?;
        let candidate = self.transport.request_candidate(transaction, &snapshot)?;
        let mut actions = BTreeMap::new();
        let entries = candidate
            .entries
            .iter()
            .map(|entry| {
                let source = sources
                    .iter()
                    .find(|source| {
                        self.slots.get(&source.surface) == Some(&entry.slot)
                            && source.descriptor.generation == entry.generation
                    })
                    .ok_or("metadata shell candidate escaped its descriptor snapshot")?;
                let descriptor = snapshot
                    .descriptors
                    .iter()
                    .find(|descriptor| {
                        descriptor.slot == entry.slot && descriptor.generation == entry.generation
                    })
                    .ok_or("metadata shell candidate action is missing")?;
                if actions.insert(descriptor.action, source.surface).is_some() {
                    return Err("metadata shell candidate repeats an action".into());
                }
                Ok(sophia_engine::DescriptorOverlayEntry {
                    slot: entry.slot,
                    surface: source.surface,
                    descriptor_generation: entry.generation,
                    action: descriptor.action,
                })
            })
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
        let overlay = if candidate.visible {
            Some(sophia_engine::descriptor_overlay_projection(
                &sophia_engine::DescriptorOverlayCandidate {
                    projection: self.take_projection()?,
                    generation: candidate.candidate_generation,
                    output: output.id,
                    broker_epoch: snapshot.broker_epoch,
                    broker_revocation_epoch: snapshot.broker_revocation_epoch,
                    shell_session_epoch: connection_epoch,
                    selected_slot: candidate.selected_slot,
                    entries,
                },
                broker.descriptors(),
                bounds,
            )?)
        } else {
            if !entries.is_empty() || candidate.selected_slot.is_some() {
                return Err("hidden metadata shell candidate retained entries".into());
            }
            None
        };
        // The claim is admitted before the candidate is prepared: a refusal
        // must not reach the shell as a prepared candidate it can expect to
        // present. Admission reduces nothing yet -- only the commit that
        // follows presentation moves the work area.
        match self.reservations.admit(
            connection_epoch,
            candidate.connection_epoch,
            candidate.candidate_generation,
            output.id,
            candidate.reservation,
            root,
            output_bounds,
        ) {
            Ok(prepared) => {
                if let Some(admitted) = prepared.reservation {
                    println!(
                        "sophia_live_metadata_shell schema=1 status=reservation_admitted candidate_generation={} output={} depth={}",
                        candidate.candidate_generation,
                        output.id.raw(),
                        admitted.band.depth,
                    );
                }
            }
            Err(refusal) => {
                eprintln!(
                    "sophia_live_metadata_shell schema=1 status=reservation_refused candidate_generation={} output={} reason={}",
                    candidate.candidate_generation,
                    output.id.raw(),
                    refusal.reason(),
                );
                return Err(
                    format!("metadata shell reservation refused: {}", refusal.reason()).into(),
                );
            }
        }
        self.transport.send_candidate_outcome(
            transaction,
            sophia_protocol::ShellV1CandidateOutcome {
                connection_epoch,
                candidate_generation: candidate.candidate_generation,
                presentation_epoch: 0,
                kind: sophia_protocol::ShellV1CandidateOutcomeKind::Prepared,
            },
        )?;
        self.pending = Some(PendingShellPresentation {
            transaction,
            candidate_generation: candidate.candidate_generation,
            connection_epoch: candidate.connection_epoch,
            output: output.id,
            visible: candidate.visible,
            actions,
        });
        Ok(overlay)
    }

    pub(super) fn observe_presentation(
        &mut self,
        runtime: &sophia_backend_live::LiveProductionVisualRuntime,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let Some(pending) = self.pending.as_ref() else {
            return Ok(false);
        };
        let Some(presentation_epoch) = runtime.descriptor_overlay_presentation_epoch(
            pending.output,
            pending.candidate_generation,
            pending.visible,
        ) else {
            return Ok(false);
        };
        let pending = self.pending.take().expect("checked above");
        // The work area moves here and nowhere else: the claim becomes
        // presented in the same step its pixels do.
        if self
            .reservations
            .commit(pending.connection_epoch, pending.candidate_generation)
            && let Some(presented) = self.reservations.presented()
        {
            println!(
                "sophia_live_metadata_shell schema=1 status=reservation_presented candidate_generation={} output={} depth={}",
                pending.candidate_generation,
                presented.output.raw(),
                presented.band.depth,
            );
        }
        self.transport.send_candidate_outcome(
            pending.transaction,
            sophia_protocol::ShellV1CandidateOutcome {
                connection_epoch: self.transport.connection_epoch(),
                candidate_generation: pending.candidate_generation,
                presentation_epoch,
                kind: sophia_protocol::ShellV1CandidateOutcomeKind::Presented,
            },
        )?;
        if pending.visible {
            self.presented_actions = pending.actions;
            self.presented = Some(PresentedShellCandidate {
                candidate_generation: pending.candidate_generation,
                presentation_epoch,
                output: pending.output,
            });
        } else {
            self.presented_actions.clear();
            self.presented = None;
        }
        println!(
            "sophia_live_metadata_shell schema=1 status=presented candidate_generation={} presentation_epoch={} output={} visible={}",
            pending.candidate_generation,
            presentation_epoch,
            pending.output.raw(),
            pending.visible,
        );
        Ok(true)
    }

    /// The shell's committed work-area claim, as bands the reduction consumes.
    pub(super) fn work_area_bands(&self) -> Vec<sophia_protocol::OutputReservation> {
        self.reservations.active_bands()
    }

    pub(super) fn reject_pending(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        // A rejected bundle takes its claim with it; the presented work area
        // is preserved, since nothing coherent has replaced it.
        self.reservations.reject_prepared();
        let Some(pending) = self.pending.take() else {
            return Ok(false);
        };
        self.transport.send_candidate_outcome(
            pending.transaction,
            sophia_protocol::ShellV1CandidateOutcome {
                connection_epoch: self.transport.connection_epoch(),
                candidate_generation: pending.candidate_generation,
                presentation_epoch: 0,
                kind: sophia_protocol::ShellV1CandidateOutcomeKind::Rejected,
            },
        )?;
        Ok(true)
    }

    pub(super) fn dispatch_activation(
        &mut self,
        broker: &LiveMetadataBroker,
        action: sophia_protocol::ToplevelActionCapabilityRef,
        activation: u64,
    ) -> Result<(Option<SurfaceId>, sophia_protocol::OutputId), Box<dyn std::error::Error>> {
        let surface = self
            .presented_actions
            .get(&action)
            .copied()
            .ok_or("metadata shell activation was not presented")?;
        let transaction = self.take_transaction()?;
        let presented = self
            .presented
            .ok_or("metadata shell has no presented activation epoch")?;
        self.transport.queue_activation(
            transaction,
            sophia_protocol::ShellV1Activation {
                connection_epoch: self.transport.connection_epoch(),
                candidate_generation: presented.candidate_generation,
                presentation_epoch: presented.presentation_epoch,
                activation,
                action,
            },
        )?;
        let acknowledgement = self.transport.receive_activation_ack()?;
        if acknowledgement.disposition != sophia_protocol::ShellV1ActivationDisposition::Consumed {
            return Err("metadata shell rejected a current presented activation".into());
        }
        let resolved = (broker.resolve_toplevel_action(action) == Some(surface)).then_some(surface);
        if resolved.is_some() {
            println!(
                "sophia_live_metadata_broker schema=1 status=issuer_validated activation={activation} target=redacted"
            );
        }
        Ok((resolved, presented.output))
    }

    pub(super) fn interaction_presented(&self) -> bool {
        !self.presented_actions.is_empty() || self.pending.is_some()
    }

    pub(super) fn revoke_interaction(&mut self) {
        self.presented_actions.clear();
    }

    fn launch_and_negotiate(&mut self) -> Result<(u32, u16, u64), Box<dyn std::error::Error>> {
        self.supervisor
            .apply(sophia_runtime::SupervisorCommand::StartProcess {
                process: SupervisedProcessKind::Shell,
                delay: Duration::ZERO,
            })?;
        let evidence = self
            .supervisor
            .protection_evidence()
            .ok_or("metadata shell supervisor omitted its protection domain")?
            .clone();
        self.transport.authorize_protected_peer(&evidence)?;
        let connection_epoch = self.next_connection_epoch;
        let welcome = match self
            .transport
            .accept_and_negotiate(connection_epoch, Duration::from_secs(5))
        {
            Ok(welcome) => welcome,
            Err(error) => {
                let _ = self.transport.disconnect();
                let _ = self.supervisor.terminate();
                return Err(error.into());
            }
        };
        self.next_connection_epoch = self
            .next_connection_epoch
            .checked_add(1)
            .ok_or("metadata shell connection epoch exhausted")?;
        Ok((
            evidence.peer_pid,
            welcome.selected_revision,
            connection_epoch,
        ))
    }

    fn reconnect_or_defer(
        &mut self,
        reason: &str,
    ) -> Result<LiveMetadataShellPoll, Box<dyn std::error::Error>> {
        if let Err(error) = self.transport.disconnect() {
            eprintln!(
                "sophia_live_metadata_shell schema=1 status=disconnect_failed reason={reason} error={error}"
            );
        }
        self.pending = None;
        self.presented = None;
        self.presented_actions.clear();
        // The in-flight claim dies with the connection. The presented one is
        // deliberately retained beside the inert pixels: growing the work area
        // while no shell can reproject it is the half-new desktop the
        // coordination model rules out. A fresh epoch withdraws it by
        // presenting a candidate that reserves nothing.
        self.reservations.on_disconnect();
        match self.launch_and_negotiate() {
            Ok((peer_pid, revision, connection_epoch)) => {
                self.connected = true;
                self.reconnect_at = None;
                println!(
                    "sophia_live_metadata_shell schema=1 status=reconnected protected=true peer_pid={peer_pid} revision={revision} connection_epoch={connection_epoch} reason={reason}"
                );
                Ok(LiveMetadataShellPoll::Reconnected { connection_epoch })
            }
            Err(error) => {
                self.connected = false;
                self.reconnect_at = Some(Instant::now() + SHELL_RECONNECT_RETRY_DELAY);
                eprintln!(
                    "sophia_live_metadata_shell schema=1 status=unavailable reason={reason} retry_ms={} error={error}",
                    SHELL_RECONNECT_RETRY_DELAY.as_millis(),
                );
                Ok(LiveMetadataShellPoll::Unavailable)
            }
        }
    }

    fn ensure_slot(&mut self, surface: SurfaceId) -> Result<u16, Box<dyn std::error::Error>> {
        if let Some(slot) = self.slots.get(&surface).copied() {
            return Ok(slot);
        }
        let slot = self.next_slot;
        if slot == 0 || slot == u16::MAX {
            return Err("metadata shell slot identity exhausted".into());
        }
        self.next_slot = self
            .next_slot
            .checked_add(1)
            .ok_or("metadata shell slot identity exhausted")?;
        self.slots.insert(surface, slot);
        Ok(slot)
    }

    fn take_snapshot_generation(&mut self) -> Result<u64, Box<dyn std::error::Error>> {
        let generation = self.next_snapshot_generation;
        self.next_snapshot_generation = generation
            .checked_add(1)
            .ok_or("metadata shell snapshot generation exhausted")?;
        Ok(generation)
    }

    fn take_projection(&mut self) -> Result<u64, Box<dyn std::error::Error>> {
        let projection = self.next_projection;
        self.next_projection = projection
            .checked_add(1)
            .ok_or("metadata shell projection identity exhausted")?;
        Ok(projection)
    }

    fn take_transaction(&mut self) -> Result<TransactionId, Box<dyn std::error::Error>> {
        let transaction = TransactionId::from_raw(self.next_transaction);
        self.next_transaction = self
            .next_transaction
            .checked_add(1)
            .ok_or("metadata shell transaction identity exhausted")?;
        Ok(transaction)
    }
}

pub(super) fn live_shell_activation_surfaces(
    layers: &BTreeMap<SurfaceId, LayerSnapshot>,
    presentation_roles: &BTreeMap<SurfaceId, sophia_protocol::SurfacePresentationRole>,
) -> BTreeSet<SurfaceId> {
    layers
        .keys()
        .filter(|surface| {
            presentation_roles.get(surface)
                == Some(&sophia_protocol::SurfacePresentationRole::PolicyManaged)
        })
        .copied()
        .collect()
}

impl Drop for LiveMetadataShell {
    fn drop(&mut self) {
        let transport_stopped = self.transport.disconnect().is_ok();
        let process_stopped = self.supervisor.terminate().is_ok();
        if transport_stopped && process_stopped {
            println!(
                "sophia_live_metadata_shell schema=1 status=stopped transport=disconnected process=terminated"
            );
        } else {
            eprintln!(
                "sophia_live_metadata_shell schema=1 status=failed transport_stopped={transport_stopped} process_stopped={process_stopped}"
            );
        }
    }
}
