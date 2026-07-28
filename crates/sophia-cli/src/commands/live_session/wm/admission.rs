struct LiveSurfaceControlStage {
    admission_owned: bool,
    command: Option<XAuthorityControlCommand>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LiveAdmissionAuthorityGroup {
    transaction: TransactionId,
    transactions: Vec<SurfaceTransaction>,
    present_submissions: Vec<sophia_x_authority::XAuthorityPresentSubmission>,
}

impl LiveAdmissionAuthorityGroup {
    fn validate(&self) -> Result<(), &'static str> {
        if !self.transaction.is_valid() {
            return Err("pre-admission authority group has an invalid transaction");
        }
        if self
            .transactions
            .iter()
            .any(|transaction| transaction.transaction != self.transaction)
        {
            return Err("pre-admission authority group contains a mismatched transaction");
        }
        if self
            .present_submissions
            .iter()
            .any(|submission| submission.transaction != self.transaction)
        {
            return Err("pre-admission authority group contains a mismatched Present");
        }
        Ok(())
    }

    fn contains_surface(&self, surface: SurfaceId) -> bool {
        self.transactions
            .iter()
            .any(|transaction| transaction.surface == surface)
            || self
                .present_submissions
                .iter()
                .any(|submission| submission.surface == surface)
    }

    fn reproject_surface(&mut self, surface: SurfaceId, geometry: Rect) {
        for transaction in &mut self.transactions {
            if transaction.surface == surface {
                transaction.target_geometry = geometry;
            }
        }
    }
}

impl PersistentLiveLayout {
    fn stage_surface_control(
        &mut self,
        transaction: TransactionId,
        surface: SurfaceId,
        geometry: Rect,
        size: Size,
    ) -> Result<LiveSurfaceControlStage, Box<dyn std::error::Error>> {
        use sophia_engine::SurfacePresentationAdmissionState::{
            AwaitingPixels, ControlPending, Inactive, Managed, PolicyPending,
        };

        let stage = match self.admissions.state(surface) {
            PolicyPending => {
                if !self
                    .admissions
                    .begin_control(surface, transaction, geometry)
                {
                    return Err("Engine rejected live WM admission transition".into());
                }
                LiveSurfaceControlStage {
                    admission_owned: true,
                    command: Some(XAuthorityControlCommand::AdmitSurface {
                        transaction,
                        surface,
                        geometry,
                    }),
                }
            }
            ControlPending { .. } => LiveSurfaceControlStage {
                admission_owned: true,
                command: None,
            },
            AwaitingPixels { .. } => LiveSurfaceControlStage {
                admission_owned: true,
                command: Some(XAuthorityControlCommand::ConfigureSurface {
                    transaction,
                    surface,
                    size,
                }),
            },
            Inactive | Managed => LiveSurfaceControlStage {
                admission_owned: false,
                command: Some(XAuthorityControlCommand::ConfigureSurface {
                    transaction,
                    surface,
                    size,
                }),
            },
        };
        Ok(stage)
    }

    fn observe_pre_admission_groups(
        &mut self,
        batch: &XAuthorityObservedTransactionBatch,
    ) -> Result<bool, &'static str> {
        let transactions = batch
            .transactions
            .iter()
            .filter(|transaction| self.surface_requires_admission(transaction.surface))
            .cloned()
            .collect::<Vec<_>>();
        let present_submissions = batch
            .present_submissions
            .iter()
            .filter(|submission| self.surface_requires_admission(submission.surface))
            .copied()
            .collect::<Vec<_>>();
        if transactions.is_empty() && present_submissions.is_empty() {
            return Ok(false);
        }
        let group = LiveAdmissionAuthorityGroup {
            transaction: batch.transaction,
            transactions,
            present_submissions,
        };
        group.validate()?;
        if self.pre_admission_groups.len() >= PRE_ADMISSION_GROUP_CAPACITY {
            return Ok(true);
        }
        self.pre_admission_groups.push_back(group);
        Ok(false)
    }

    fn release_admission_groups(&mut self, surfaces: &BTreeSet<SurfaceId>) {
        let mut retained = VecDeque::with_capacity(self.pre_admission_groups.len());
        let mut committed_generations = BTreeMap::<SurfaceId, u64>::new();
        while let Some(mut group) = self.pre_admission_groups.pop_front() {
            let touched = group
                .transactions
                .iter()
                .map(|transaction| transaction.surface)
                .chain(
                    group
                        .present_submissions
                        .iter()
                        .map(|submission| submission.surface),
                )
                .collect::<BTreeSet<_>>();
            if !touched.is_empty() && touched.iter().all(|surface| surfaces.contains(surface)) {
                for surface in &touched {
                    if let Some(layer) = self.layers.get(surface) {
                        group.reproject_surface(*surface, layer.geometry);
                    }
                }
                for transaction in &mut group.transactions {
                    let generation = committed_generations
                        .entry(transaction.surface)
                        .or_insert(0);
                    transaction.previous_committed_generation = *generation;
                    *generation = generation.saturating_add(1);
                }
                self.released_admission_groups.push_back(group);
            } else {
                retained.push_back(group);
            }
        }
        self.pre_admission_groups = retained;
    }

    fn remove_admission_groups(&mut self, surface: SurfaceId) {
        self.pre_admission_groups
            .retain(|group| !group.contains_surface(surface));
        self.released_admission_groups
            .retain(|group| !group.contains_surface(surface));
    }

    fn latest_pre_admission_transaction(&self, surface: SurfaceId) -> Option<&SurfaceTransaction> {
        self.pre_admission_groups
            .iter()
            .rev()
            .flat_map(|group| group.transactions.iter().rev())
            .find(|transaction| transaction.surface == surface)
    }

    fn observe_presentation_intents(
        &mut self,
        batch: &XAuthorityObservedTransactionBatch,
        new_surfaces: &mut BTreeSet<SurfaceId>,
    ) {
        for intent in &batch.presentation_intents {
            let changed = self.admissions.observe_intent(*intent);
            match intent.kind {
                sophia_protocol::SurfacePresentationIntentKind::Request => {
                    let facts = sophia_engine::SurfaceLayoutFacts::from(*intent);
                    self.planning_surfaces.insert(intent.surface, facts);
                    self.presentation_roles.insert(intent.surface, intent.role);
                    self.layout_epochs
                        .set_declared_constraints(intent.surface, intent.constraints);
                    self.unmanaged_surfaces.insert(intent.surface);
                    self.layout_epochs.set_admission(
                        intent.surface,
                        sophia_engine::SurfaceAdmissionState::Unmanaged,
                    );
                    if changed {
                        new_surfaces.insert(intent.surface);
                    }
                }
                sophia_protocol::SurfacePresentationIntentKind::Withdraw => {
                    self.planning_surfaces.remove(&intent.surface);
                    self.unmanaged_surfaces.remove(&intent.surface);
                    self.remove_admission_groups(intent.surface);
                }
            }
        }
    }

    fn acknowledge_admission_control(
        &mut self,
        transaction: TransactionId,
        surface: SurfaceId,
    ) -> bool {
        self.admissions.acknowledge_control(surface, transaction)
    }

    fn knows_surface(&self, surface: SurfaceId) -> bool {
        self.layers.contains_key(&surface) || self.planning_surfaces.contains_key(&surface)
    }

    fn layout_facts(&self, surface: SurfaceId) -> Option<sophia_engine::SurfaceLayoutFacts> {
        self.planning_surfaces.get(&surface).copied().or_else(|| {
            let layer = self.layers.get(&surface)?;
            Some(sophia_engine::SurfaceLayoutFacts {
                surface,
                role: self
                    .presentation_roles
                    .get(&surface)
                    .copied()
                    .unwrap_or_default(),
                geometry: layer.geometry,
                constraints: self.layout_epochs.effective_constraints(surface),
                generation: layer.generation,
            })
        })
    }

    fn planning_layers(&self) -> Vec<LayerSnapshot> {
        let mut layers = self.layers.values().cloned().collect::<Vec<_>>();
        for facts in self.planning_surfaces.values() {
            if self.layers.contains_key(&facts.surface) {
                continue;
            }
            layers.push(LayerSnapshot {
                surface: facts.surface,
                authority_local_id: None,
                namespace: None,
                stack_rank: u32::try_from(layers.len()).unwrap_or(u32::MAX - 1),
                geometry: facts.geometry,
                source: BufferSource::None,
                damage: Region::empty(),
                opacity: 1.0,
                crop: None,
                transform: Transform::IDENTITY,
                generation: facts.generation,
                resize_sync: ResizeSyncCapability::ImplicitOnly,
            });
        }
        layers
    }

    fn present_layout_disposition(
        &self,
        surface: SurfaceId,
        buffer: sophia_protocol::BufferHandle,
    ) -> sophia_backend_live::LiveProductionPresentDisposition {
        let Some(pending) = self.pending.as_ref() else {
            return sophia_backend_live::LiveProductionPresentDisposition::Immediate;
        };
        let Some(expected) = pending.requested_sizes.get(&surface) else {
            return sophia_backend_live::LiveProductionPresentDisposition::Immediate;
        };
        match self.dma_buf_sizes.get(&buffer) {
            Some(actual) if actual == expected => {
                sophia_backend_live::LiveProductionPresentDisposition::StageLayout {
                    epoch: pending.transaction,
                }
            }
            Some(_) => {
                sophia_backend_live::LiveProductionPresentDisposition::RejectLayoutMismatch
            }
            None => sophia_backend_live::LiveProductionPresentDisposition::Immediate,
        }
    }
}

fn live_layout_node(
    layer: &LayerSnapshot,
    workspace: WorkspaceId,
    coordinator: &LayoutEpochCoordinator,
    chrome: sophia_engine::SurfaceChromeStyle,
) -> Result<LayoutNodeSnapshot, sophia_engine::ChromeLayoutError> {
    live_layout_node_from_facts(
        sophia_engine::SurfaceLayoutFacts {
            surface: layer.surface,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            geometry: layer.geometry,
            constraints: coordinator.effective_constraints(layer.surface),
            generation: layer.generation,
        },
        workspace,
        coordinator,
        chrome,
    )
}

fn live_layout_node_from_facts(
    facts: sophia_engine::SurfaceLayoutFacts,
    workspace: WorkspaceId,
    coordinator: &LayoutEpochCoordinator,
    chrome: sophia_engine::SurfaceChromeStyle,
) -> Result<LayoutNodeSnapshot, sophia_engine::ChromeLayoutError> {
    let mut capabilities = LayoutNodeCapabilities::STANDARD_TOPLEVEL;
    capabilities.resizable = coordinator.surface_resizable(facts.surface);
    Ok(LayoutNodeSnapshot {
        surface: facts.surface,
        workspace,
        kind: LayoutNodeKind::Toplevel,
        capabilities,
        state: LayoutNodeState::NORMAL,
        constraints: sophia_engine::outer_surface_constraints(facts.constraints, chrome)?,
        geometry: sophia_engine::outer_surface_geometry(facts.geometry, chrome)?,
        generation: facts.generation,
    })
}
