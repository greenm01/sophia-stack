impl PersistentLiveLayout {
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
