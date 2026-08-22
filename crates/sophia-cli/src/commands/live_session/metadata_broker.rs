use super::*;

/// Session-owned host for the metadata authority and Engine's sanitized table.
///
/// The process owns disclosure policy. This owner knows only the admitted X
/// client route, the reduced candidate, and the sanitized descriptor returned by
/// the broker; raw X properties never enter this process boundary.
pub(super) struct LiveMetadataBroker {
    supervisor: ProcessSupervisor,
    transport: sophia_runtime::MetadataBrokerSessionTransport,
    descriptors: sophia_engine::ChromeDescriptorTable,
    admitted: BTreeMap<SurfaceId, sophia_x_authority::XServerFrontendClientId>,
    next_transaction: u64,
}

impl LiveMetadataBroker {
    pub(super) fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let executable = std::env::current_exe()?;
        let directory = std::env::temp_dir().join(format!(
            "sophia-live-metadata-broker-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        let mut transport =
            sophia_runtime::MetadataBrokerSessionTransport::bind_for_supervised_uid(
                &directory,
                rustix::process::geteuid().as_raw(),
            )?;
        let socket = transport.socket_path().to_path_buf();
        let domain = sophia_runtime::ProtectionDomainSpec::bubblewrap([
            sophia_runtime::ProtectionDomainRole::MetadataBroker,
        ])?
        .path(sophia_runtime::ProtectionPath::read_only(
            socket
                .parent()
                .expect("metadata broker socket always has a parent"),
        ))?;
        let spec = ProcessLaunchSpec::new(executable)
            .arg("metadata-broker-serve")
            .env(sophia_runtime::SOPHIA_BROKER_SOCKET_ENV, &socket)
            .env("SOPHIA_BROKER_DEFAULT_DISCLOSURE", "class-only")
            .process_group()
            .protection_domain(domain);
        let mut supervisor = ProcessSupervisor::new(SupervisedProcessKind::MetadataBroker, spec);
        supervisor.apply(sophia_runtime::SupervisorCommand::StartProcess {
            process: SupervisedProcessKind::MetadataBroker,
            delay: Duration::ZERO,
        })?;
        let ready = (|| -> Result<_, Box<dyn std::error::Error>> {
            // Admission takes the launch record, not the PID read off it: the
            // broker is a metadata-bearing role, so the transport requires
            // evidence that the domain above actually carries MetadataBroker.
            let evidence = supervisor
                .protection_evidence()
                .ok_or("metadata broker supervisor omitted its protection domain")?
                .clone();
            transport.authorize_protected_peer(&evidence)?;
            let welcome = transport.accept_and_negotiate(1, Duration::from_secs(5))?;
            Ok((evidence.peer_pid, welcome))
        })();
        let (peer_pid, welcome) = match ready {
            Ok(ready) => ready,
            Err(error) => {
                let _ = transport.disconnect();
                let _ = supervisor.terminate();
                return Err(error);
            }
        };
        println!(
            "sophia_live_metadata_broker schema=1 status=ready protected=true peer_pid={} revision={}",
            peer_pid, welcome.selected_revision,
        );
        Ok(Self {
            supervisor,
            transport,
            descriptors: Default::default(),
            admitted: Default::default(),
            next_transaction: 1,
        })
    }

    pub(super) fn poll(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.supervisor.poll()?.is_some() {
            return Err("protected metadata broker exited".into());
        }
        Ok(())
    }

    pub(super) fn drain_candidates(
        &mut self,
        receiver: &Receiver<sophia_x_authority::XAuthorityClientMetadataCandidate>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            match receiver.try_recv() {
                Ok(delivery) => self.apply_candidate(delivery)?,
                Err(std::sync::mpsc::TryRecvError::Empty) => return Ok(()),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Err("X frontend reduced metadata route disconnected".into());
                }
            }
        }
    }

    pub(super) fn admit_surface(
        &mut self,
        client: sophia_x_authority::XServerFrontendClientId,
        surface: SurfaceId,
        profile: NamespaceProfile,
    ) -> Result<Option<XAuthorityClientControlCommand>, Box<dyn std::error::Error>> {
        if let Some(existing) = self.admitted.get(&surface) {
            if *existing == client {
                return Ok(None);
            }
            return Err("metadata surface changed owning X client without retirement".into());
        }
        let transaction = self.next_transaction()?;
        let response = self.transport.request(
            transaction,
            &sophia_protocol::BrokerV1Request::SurfaceAdmitted {
                connection_epoch: 1,
                surface,
                profile,
            },
        )?;
        let sophia_protocol::BrokerV1Response::PublishRule { rule, .. } = response else {
            return Err(format!("metadata broker rejected surface admission: {response:?}").into());
        };
        if rule.surface != surface {
            return Err("metadata broker returned a rule for another surface".into());
        }
        self.admitted.insert(surface, client);
        Ok(Some(XAuthorityClientControlCommand {
            client,
            command: XAuthorityControlCommand::PublishMetadataRule {
                transaction,
                surface,
                rule,
            },
        }))
    }

    pub(super) fn apply_candidate(
        &mut self,
        delivery: sophia_x_authority::XAuthorityClientMetadataCandidate,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.admitted.get(&delivery.candidate.surface) != Some(&delivery.client) {
            return Err("reduced metadata arrived outside its admitted X client route".into());
        }
        let transaction = self.next_transaction()?;
        let response = self.transport.request(
            transaction,
            &sophia_protocol::BrokerV1Request::CandidateReduced {
                connection_epoch: 1,
                candidate: delivery.candidate,
            },
        )?;
        match response {
            sophia_protocol::BrokerV1Response::EmitDescriptor { descriptor, .. } => {
                match self.descriptors.apply_metadata(descriptor) {
                    sophia_engine::MetadataChromeUpdate::Upserted { surface } => {
                        println!(
                            "sophia_live_metadata_broker schema=1 status=descriptor_committed surface={} content=redacted",
                            surface.index(),
                        );
                        Ok(())
                    }
                    outcome => Err(format!(
                        "Engine rejected sanitized metadata descriptor: {outcome:?}"
                    )
                    .into()),
                }
            }
            sophia_protocol::BrokerV1Response::NoChange { .. } => Ok(()),
            response => {
                Err(format!("metadata broker rejected reduced candidate: {response:?}").into())
            }
        }
    }

    pub(super) fn retire_surface(
        &mut self,
        surface: SurfaceId,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.admitted.remove(&surface).is_none() {
            return Ok(());
        }
        let transaction = self.next_transaction()?;
        let response = self.transport.request(
            transaction,
            &sophia_protocol::BrokerV1Request::SurfaceRemoved {
                connection_epoch: 1,
                surface,
            },
        )?;
        match response {
            sophia_protocol::BrokerV1Response::RetireSurface {
                surface: retired, ..
            } if retired == surface => {
                self.descriptors.remove_surface(surface);
                Ok(())
            }
            response => {
                Err(format!("metadata broker rejected surface retirement: {response:?}").into())
            }
        }
    }

    fn next_transaction(&mut self) -> Result<TransactionId, Box<dyn std::error::Error>> {
        let transaction = TransactionId::from_raw(self.next_transaction);
        self.next_transaction = self
            .next_transaction
            .checked_add(1)
            .ok_or("metadata broker transaction identity exhausted")?;
        Ok(transaction)
    }
}

impl Drop for LiveMetadataBroker {
    fn drop(&mut self) {
        let transport_stopped = self.transport.disconnect().is_ok();
        let process_stopped = self.supervisor.terminate().is_ok();
        if transport_stopped && process_stopped {
            println!(
                "sophia_live_metadata_broker schema=1 status=stopped transport=disconnected process=terminated"
            );
        } else {
            eprintln!(
                "sophia_live_metadata_broker schema=1 status=failed stage=shutdown transport={} process={}",
                if transport_stopped {
                    "disconnected"
                } else {
                    "failed"
                },
                if process_stopped {
                    "terminated"
                } else {
                    "failed"
                },
            );
        }
    }
}
