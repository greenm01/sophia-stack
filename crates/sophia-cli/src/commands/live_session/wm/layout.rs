struct PendingLiveWmLayout {
    transaction: TransactionId,
    layers: Vec<LayerSnapshot>,
    requested_sizes: BTreeMap<SurfaceId, Size>,
    focus: Option<SurfaceId>,
    deadline: Instant,
    update: WmTransactionUpdate,
    moved_surfaces: usize,
    staged_transactions: BTreeMap<SurfaceId, SurfaceTransaction>,
    admission_surfaces: BTreeSet<SurfaceId>,
    source: Option<LiveWmProposalSource>,
    effects: Option<LiveWmCommitEffects>,
}

struct LiveAuthorityLayoutObservation {
    new_surfaces: Vec<SurfaceId>,
    output_reservations_changed: bool,
    admission_group_invalid: bool,
    admission_group_overflowed: bool,
}

const PRE_ADMISSION_GROUP_CAPACITY: usize = 256;

#[derive(Default)]
struct PersistentLiveLayout {
    layers: BTreeMap<SurfaceId, LayerSnapshot>,
    planning_surfaces: BTreeMap<SurfaceId, sophia_engine::SurfaceLayoutFacts>,
    admissions: sophia_engine::SurfaceAdmissionTable,
    dma_buf_sizes: BTreeMap<sophia_protocol::BufferHandle, Size>,
    cpu_buffer_sizes: BTreeMap<u64, Size>,
    deferred_dma_buf_releases: BTreeSet<sophia_protocol::BufferHandle>,
    deferred_fence_releases: BTreeSet<sophia_protocol::FenceHandle>,
    layout_epochs: LayoutEpochCoordinator,
    client_routes: XAuthorityClientSurfaceRoutes,
    presentation_roles: BTreeMap<SurfaceId, sophia_protocol::SurfacePresentationRole>,
    pre_admission_groups: VecDeque<LiveAdmissionAuthorityGroup>,
    released_admission_groups: VecDeque<LiveAdmissionAuthorityGroup>,
    output_reservations: sophia_engine::SurfaceOutputReservationState,
    unmanaged_surfaces: BTreeSet<SurfaceId>,
    admission_retries: BTreeMap<SurfaceId, u8>,
    pending: Option<PendingLiveWmLayout>,
    focus_to_apply: Option<(TransactionId, SurfaceId)>,
    retirement_focus: BTreeMap<SurfaceId, (TransactionId, TransactionId)>,
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
    ) -> LiveAuthorityLayoutObservation {
        let mut output_reservations_changed = false;
        let mut admission_group_invalid = false;
        let mut admission_group_overflowed = false;
        let mut new_surfaces = BTreeSet::new();
        self.client_routes.observe(batch);
        self.observe_presentation_intents(batch, &mut new_surfaces);
        for presentation in &batch.surface_presentations {
            self.presentation_roles
                .insert(presentation.surface, presentation.role);
            self.layout_epochs
                .set_declared_constraints(presentation.surface, presentation.constraints);
            output_reservations_changed |= self.output_reservations.observe_presentation(
                presentation.surface,
                presentation.role,
                presentation.mapped,
            );
            if let Some(layer) = self.layers.get_mut(&presentation.surface)
                && presentation.role
                    == sophia_protocol::SurfacePresentationRole::ClientPositioned
            {
                layer.geometry = presentation.geometry;
            }
            match presentation.role {
                sophia_protocol::SurfacePresentationRole::PolicyManaged => {
                    if presentation.mapped && self.layers.contains_key(&presentation.surface) {
                        self.unmanaged_surfaces.insert(presentation.surface);
                    }
                }
                sophia_protocol::SurfacePresentationRole::ClientPositioned => {
                    self.unmanaged_surfaces.remove(&presentation.surface);
                }
            }
        }
        for snapshot in &batch.surface_output_reservations {
            output_reservations_changed |=
                self.output_reservations.observe_reservations(snapshot.clone());
        }
        for registration in &batch.dma_buf_registrations {
            self.dma_buf_sizes
                .insert(registration.descriptor.handle, registration.descriptor.size);
        }
        for update in &batch.cpu_buffer_updates {
            match update {
                sophia_x_authority::XAuthorityCpuBufferUpdate::Replace(buffer) => {
                    self.cpu_buffer_sizes.insert(buffer.handle, buffer.size);
                }
                sophia_x_authority::XAuthorityCpuBufferUpdate::Patch(patch) => {
                    self.cpu_buffer_sizes.insert(patch.handle, patch.size);
                }
            }
        }
        match self.observe_pre_admission_groups(batch) {
            Ok(overflowed) => admission_group_overflowed |= overflowed,
            Err(_) => admission_group_invalid = true,
        }
        for handle in &batch.released_dma_bufs {
            if self.admission_groups_reference_dma_buf(*handle) {
                self.deferred_dma_buf_releases.insert(*handle);
            } else {
                self.dma_buf_sizes.remove(handle);
            }
        }
        for handle in &batch.released_fences {
            if self.admission_groups_reference_fence(*handle) {
                self.deferred_fence_releases.insert(*handle);
            }
        }
        for surface in &batch.removed_surfaces {
            output_reservations_changed |= self.output_reservations.remove_surface(*surface);
        }
        self.remove_surfaces(&batch.removed_surfaces);
        for (index, transaction) in batch.transactions.iter().enumerate() {
            let observed_size = live_transaction_observed_size(
                transaction,
                &self.dma_buf_sizes,
                &self.cpu_buffer_sizes,
            );
            if !self
                .layout_epochs
                .accept_observation(transaction.surface, observed_size)
            {
                continue;
            }
            self.layout_epochs
                .record_safe_observation(transaction.surface, observed_size);
            let resize_owned = self.pending.as_ref().is_some_and(|pending| {
                pending.requested_sizes.contains_key(&transaction.surface)
            });
            let staged_for_resize = self.pending.as_ref().is_some_and(|pending| {
                pending.requested_sizes.get(&transaction.surface) == Some(&observed_size)
            });
            if resize_owned {
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
                        layer.generation =
                            transaction.previous_committed_generation.saturating_add(1);
                    }
                }
                continue;
            }
            if self.surface_requires_admission(transaction.surface) {
                continue;
            }
            self.layout_epochs
                .record_committed(transaction.surface, observed_size);
            let observed_layer = match self.layers.get_mut(&transaction.surface) {
                Some(layer) => {
                    if self.presentation_roles.get(&transaction.surface)
                        == Some(&sophia_protocol::SurfacePresentationRole::ClientPositioned)
                    {
                        layer.geometry = transaction.target_geometry;
                    }
                    layer.source = transaction.target_buffer;
                    layer.damage = transaction.damage.clone();
                    layer.generation = transaction.previous_committed_generation.saturating_add(1);
                    layer.clone()
                }
                None => {
                    new_surfaces.insert(transaction.surface);
                    let policy_managed = self.presentation_roles.get(&transaction.surface)
                        != Some(&sophia_protocol::SurfacePresentationRole::ClientPositioned);
                    if policy_managed
                        && matches!(
                            self.admissions.state(transaction.surface),
                            sophia_engine::SurfacePresentationAdmissionState::PolicyPending
                        )
                    {
                        self.unmanaged_surfaces.insert(transaction.surface);
                        self.layout_epochs.set_admission(
                            transaction.surface,
                            sophia_engine::SurfaceAdmissionState::Unmanaged,
                        );
                    }
                    let mut geometry = transaction.target_geometry;
                    if policy_managed && self.stage_new_surfaces_offset {
                        geometry.x = geometry.x.saturating_add(80);
                        geometry.y = geometry.y.saturating_add(60);
                    } else if policy_managed
                        && let Some(output) = self.center_first_surface_in.take()
                    {
                        geometry = center_geometry_without_scaling(geometry, output);
                    }
                    let layer = LayerSnapshot {
                        surface: transaction.surface,
                        authority_local_id: None,
                        namespace: None,
                        stack_rank: if policy_managed {
                            u32::try_from(index).unwrap_or(u32::MAX - 1)
                        } else {
                            u32::MAX
                        },
                        geometry,
                        source: transaction.target_buffer,
                        damage: transaction.damage.clone(),
                        opacity: 1.0,
                        crop: None,
                        transform: Transform::IDENTITY,
                        generation: transaction.previous_committed_generation.saturating_add(1),
                        resize_sync: ResizeSyncCapability::ImplicitOnly,
                    };
                    self.layers.insert(transaction.surface, layer.clone());
                    layer
                }
            };
            self.merge_unrequested_observation_into_pending(observed_layer);
        }
        LiveAuthorityLayoutObservation {
            new_surfaces: new_surfaces.into_iter().collect(),
            output_reservations_changed,
            admission_group_invalid,
            admission_group_overflowed,
        }
    }

    fn active_output_reservations(&self) -> Vec<sophia_protocol::SurfaceOutputReservations> {
        self.output_reservations.active_reservations()
    }

    fn merge_unrequested_observation_into_pending(&mut self, observed: LayerSnapshot) {
        let geometry_authority = if self.is_client_positioned(observed.surface) {
            PendingLayoutGeometryAuthority::Observation
        } else {
            PendingLayoutGeometryAuthority::Layout
        };
        let Some(pending) = self.pending.as_mut() else {
            return;
        };
        let _ = merge_unrequested_layout_observation(
            &mut pending.layers,
            &pending.requested_sizes,
            observed,
            geometry_authority,
        );
    }

    fn remove_surfaces(&mut self, removed_surfaces: &[SurfaceId]) {
        if removed_surfaces.is_empty() {
            return;
        }
        self.layers
            .retain(|surface, _| !removed_surfaces.contains(surface));
        self.planning_surfaces
            .retain(|surface, _| !removed_surfaces.contains(surface));
        for surface in removed_surfaces {
            self.layout_epochs.remove(*surface);
            self.admissions.remove(*surface);
            self.admission_retries.remove(surface);
        }
        self.unmanaged_surfaces
            .retain(|surface| !removed_surfaces.contains(surface));
        self.presentation_roles
            .retain(|surface, _| !removed_surfaces.contains(surface));
        self.retirement_focus
            .retain(|surface, _| !removed_surfaces.contains(surface));
        for surface in removed_surfaces {
            self.remove_admission_groups(*surface);
        }
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
        if self.layout_epochs.rollback_surfaces().next().is_some() {
            return None;
        }
        self.unmanaged_surfaces
            .iter()
            .find(|surface| {
                self.knows_surface(**surface)
                    && self.admission_retries.get(surface).copied().unwrap_or(0) <= 1
            })
            .copied()
    }

    fn is_client_positioned(&self, surface: SurfaceId) -> bool {
        self.presentation_roles.get(&surface)
            == Some(&sophia_protocol::SurfacePresentationRole::ClientPositioned)
    }

    fn surface_requires_admission(&self, surface: SurfaceId) -> bool {
        self.presentation_roles.get(&surface)
            == Some(&sophia_protocol::SurfacePresentationRole::PolicyManaged)
            && !matches!(
                self.admissions.state(surface),
                sophia_engine::SurfacePresentationAdmissionState::Inactive
                    |
                sophia_engine::SurfacePresentationAdmissionState::Managed
            )
    }

    fn top_client_positioned_surface(&self) -> Option<SurfaceId> {
        self.layers
            .values()
            .filter(|layer| self.is_client_positioned(layer.surface))
            .max_by_key(|layer| (layer.stack_rank, layer.surface))
            .map(|layer| layer.surface)
    }

    fn stage(
        &mut self,
        mut proposal: LiveWmProposal,
        session_controls: &mut SessionControlQueue,
    ) -> Result<Option<LiveWmCommitResult>, Box<dyn std::error::Error>> {
        if self.pending.is_some() {
            println!(
                "sophia_live_wm schema=1 status=proposal_busy transaction={} preserved_layout=true",
                proposal.transaction.raw()
            );
            return Ok(None);
        }
        for layer in proposal
            .layers
            .iter()
            .filter(|layer| self.surface_awaits_visual_candidate(layer.surface))
        {
            proposal.requested_sizes.entry(layer.surface).or_insert(Size {
                width: layer.geometry.width,
                height: layer.geometry.height,
            });
        }
        for (surface, size) in &proposal.requested_sizes {
            if !self.layout_epochs.request_allowed(*surface, *size)
                && let Some(committed) = self.layout_epochs.committed_size(*surface)
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
            self.surface_awaits_visual_candidate(*surface)
                || (self.layout_epochs.request_allowed(*surface, *size)
                    && self.layout_epochs.committed_size(*surface) != Some(*size))
        });
        let mut staged_transactions = BTreeMap::new();
        for (surface, size) in &proposal.requested_sizes {
            let Some(transaction) = self.latest_pre_admission_transaction(*surface) else {
                continue;
            };
            if live_transaction_observed_size(
                transaction,
                &self.dma_buf_sizes,
                &self.cpu_buffer_sizes,
            ) != *size
            {
                continue;
            }
            staged_transactions.insert(*surface, transaction.clone());
            if let Some(layer) = proposal
                .layers
                .iter_mut()
                .find(|layer| layer.surface == *surface)
            {
                layer.source = transaction.target_buffer;
                layer.damage = transaction.damage.clone();
                layer.generation = transaction.previous_committed_generation.saturating_add(1);
            }
        }
        let mut admission_surfaces = BTreeSet::new();
        for (surface, size) in &proposal.requested_sizes {
            let geometry = proposal
                .layers
                .iter()
                .find(|layer| layer.surface == *surface)
                .map(|layer| layer.geometry)
                .ok_or("live WM configure has no planned geometry")?;
            let stage =
                self.stage_surface_control(proposal.transaction, *surface, geometry, *size)?;
            if stage.admission_owned {
                admission_surfaces.insert(*surface);
            }
            let Some(command) = stage.command else {
                continue;
            };
            let client = self
                .client_routes
                .client_for_surface(*surface)
                .ok_or("live WM configure has no X11 client route for its surface")?;
            session_controls
                .enqueue(
                    XAuthorityClientControlCommand { client, command },
                    Instant::now(),
                )
                .map_err(|error| {
                    format!("failed to queue WM configure control: {error:?}")
                })?;
        }
        let ready = proposal
            .requested_sizes
            .iter()
            .all(|(surface, size)| {
                self.layout_epochs.committed_size(*surface) == Some(*size)
                    && !admission_surfaces.contains(surface)
            });
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
            staged_transactions,
            admission_surfaces,
            source: proposal.source,
            effects: proposal.effects,
        });
        println!(
            "sophia_live_resize_epoch schema=1 status=held transaction={} surfaces={}",
            self.pending
                .as_ref()
                .expect("pending layout was just installed")
                .transaction
                .raw(),
            self.pending
                .as_ref()
                .expect("pending layout was just installed")
                .requested_sizes
                .len(),
        );
        Ok(None)
    }

    fn resolve_pending(&mut self) -> Option<LiveWmCommitResult> {
        let pending = self.pending.as_ref()?;
        let ready = pending.requested_sizes.iter().all(|(surface, size)| {
            let staged_matches = pending
                .staged_transactions
                .get(surface)
                .is_some_and(|transaction| {
                    live_transaction_observed_size(
                        transaction,
                        &self.dma_buf_sizes,
                        &self.cpu_buffer_sizes,
                    )
                        == *size
                });
            let retained_matches = self.layout_epochs.committed_size(*surface) == Some(*size)
                && pending
                    .layers
                    .iter()
                    .find(|layer| layer.surface == *surface)
                    .is_some_and(|layer| layer.source != BufferSource::None);
            let admission_ready = !pending.admission_surfaces.contains(surface)
                || matches!(
                    self.admissions.state(*surface),
                    sophia_engine::SurfacePresentationAdmissionState::AwaitingPixels { .. }
                        | sophia_engine::SurfacePresentationAdmissionState::Managed
                );
            let pixels_ready = if pending.admission_surfaces.contains(surface) {
                staged_matches
            } else {
                staged_matches || retained_matches
            };
            pixels_ready && admission_ready
        });
        if !ready {
            return None;
        }
        let pending = self.pending.take().expect("checked above");
        Some(self.commit_pending(pending))
    }

    fn expire_pending(
        &mut self,
        session_controls: &mut SessionControlQueue,
    ) -> Result<Option<LiveWmCommitResult>, Box<dyn std::error::Error>> {
        if !self
            .pending
            .as_ref()
            .is_some_and(|pending| Instant::now() >= pending.deadline)
        {
            return Ok(None);
        }
        let pending = self.pending.take().expect("checked above");
        let admission_surfaces = pending
            .layers
            .iter()
            .map(|layer| layer.surface)
            .filter(|surface| self.unmanaged_surfaces.contains(surface))
            .collect::<Vec<_>>();
        let terminal_admissions = admission_surfaces
            .iter()
            .copied()
            .filter(|surface| self.admission_retries.get(surface).copied().unwrap_or(0) >= 1)
            .collect::<BTreeSet<_>>();
        let rollback = self.layout_epochs.begin_recovery(
            pending
                .requested_sizes
                .iter()
                .filter(|(surface, _)| !terminal_admissions.contains(surface))
                .map(|(surface, size)| (*surface, *size)),
            admission_surfaces
                .iter()
                .copied()
                .filter(|surface| !terminal_admissions.contains(surface)),
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
            session_controls.enqueue(XAuthorityClientControlCommand {
                client,
                command: XAuthorityControlCommand::ConfigureSurface {
                    transaction: rollback_transaction,
                    surface,
                    size,
                },
            }, Instant::now()).map_err(|error| {
                format!("failed to queue WM rollback control: {error:?}")
            })?;
        }
        for surface in &terminal_admissions {
            let client = self
                .client_routes
                .client_for_surface(*surface)
                .ok_or("live WM withdrawal has no X11 client route")?;
            session_controls
                .enqueue(
                    XAuthorityClientControlCommand {
                        client,
                        command: XAuthorityControlCommand::WithdrawSurface {
                            transaction: pending.transaction,
                            surface: *surface,
                        },
                    },
                    Instant::now(),
                )
                .map_err(|error| {
                    format!("failed to queue terminal WM admission withdrawal: {error:?}")
                })?;
            self.admissions.remove(*surface);
            self.planning_surfaces.remove(surface);
            self.unmanaged_surfaces.remove(surface);
            self.admission_retries.remove(surface);
            self.layout_epochs.remove(*surface);
            self.remove_admission_groups(*surface);
        }
        let resize_state = pending
            .requested_sizes
            .iter()
            .map(|(surface, expected)| {
                let observed = self
                    .layout_epochs
                    .committed_size(*surface)
                    .unwrap_or(Size {
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
        for surface in admission_surfaces
            .into_iter()
            .filter(|surface| !terminal_admissions.contains(surface))
        {
            let attempts = self.admission_retries.entry(surface).or_default();
            *attempts = attempts.saturating_add(1);
        }
        println!(
            "sophia_live_resize_epoch schema=1 status=aborted transaction={} rejected_surfaces={}",
            pending.transaction.raw(),
            pending.requested_sizes.len(),
        );
        Ok(Some(LiveWmCommitResult {
            update: WmTransactionUpdate {
                commit: TransactionCommit {
                    transaction: pending.transaction,
                    outcome: TransactionOutcome::TimedOut,
                    applied_surfaces: Vec::new(),
                },
                ipc_error: None,
            },
            source: None,
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
            admission_surfaces: BTreeSet::new(),
            source: proposal.source,
            effects: proposal.effects,
        };
        self.commit_pending(pending)
    }

    fn commit_pending(&mut self, pending: PendingLiveWmLayout) -> LiveWmCommitResult {
        let matched_surfaces = pending.staged_transactions.len();
        for surface in &pending.admission_surfaces {
            let Some(transaction) = pending.staged_transactions.get(surface) else {
                continue;
            };
            match transaction.target_buffer {
                BufferSource::DmaBuf { .. } => {
                    if self
                        .admissions
                        .begin_retirement(*surface, transaction.transaction)
                    {
                        println!(
                            "sophia_live_visual_admission schema=1 status=armed transaction={} surface={}",
                            transaction.transaction.raw(),
                            surface.index(),
                        );
                    }
                }
                _ => {
                    if self.admissions.mark_managed(*surface) {
                        self.planning_surfaces.remove(surface);
                    }
                }
            }
        }
        if !pending.staged_transactions.is_empty() {
            for transaction in pending.staged_transactions.values() {
                if let Some(size) = live_transaction_pixel_size(
                    transaction.target_buffer,
                    &self.dma_buf_sizes,
                    &self.cpu_buffer_sizes,
                ) {
                    self.layout_epochs
                        .record_committed(transaction.surface, size);
                }
            }
        }
        self.layers = pending
            .layers
            .into_iter()
            .map(|layer| (layer.surface, layer))
            .collect();
        let selected_admission_transactions = pending
            .admission_surfaces
            .iter()
            .filter_map(|surface| {
                pending
                    .staged_transactions
                    .get(surface)
                    .map(|transaction| (*surface, transaction.transaction))
            })
            .collect();
        self.release_admission_groups(&selected_admission_transactions);
        self.unmanaged_surfaces.retain(|surface| {
            self.layers.contains_key(surface)
                && pending.effects.as_ref().is_none_or(|effects| {
                    effects.workspace_state.surface_workspace(*surface).is_none()
                })
        });
        self.admission_retries
            .retain(|surface, _| self.unmanaged_surfaces.contains(surface));
        for surface in self.layers.keys().copied() {
            if !self.unmanaged_surfaces.contains(&surface)
                && matches!(
                    self.admissions.state(surface),
                    sophia_engine::SurfacePresentationAdmissionState::Inactive
                        | sophia_engine::SurfacePresentationAdmissionState::Managed
                )
            {
                self.layout_epochs
                    .set_admission(surface, sophia_engine::SurfaceAdmissionState::Managed);
            }
        }
        if let Some(surface) = pending.focus {
            match self.admissions.state(surface) {
                sophia_engine::SurfacePresentationAdmissionState::AwaitingRetirement {
                    visual_transaction,
                    ..
                } => {
                    self.retirement_focus
                        .insert(surface, (visual_transaction, pending.transaction));
                }
                _ => self.focus_to_apply = Some((pending.transaction, surface)),
            }
        }
        println!(
            "sophia_live_wm schema=1 status=layout_committed transaction={} surfaces={} moved_surfaces={} configure_deliveries={} outcome={:?}",
            pending.transaction.raw(),
            self.layers.len(),
            pending.moved_surfaces,
            pending.requested_sizes.len(),
            pending.update.commit.outcome
        );
        println!(
            "sophia_live_resize_epoch schema=1 status=committed transaction={} matched_surfaces={}",
            pending.transaction.raw(),
            matched_surfaces,
        );
        LiveWmCommitResult {
            update: pending.update,
            source: pending.source,
            effects: pending.effects,
        }
    }

    fn projected_batch(
        &mut self,
        batch: &XAuthorityObservedTransactionBatch,
    ) -> (
        XAuthorityObservedTransactionBatch,
        Vec<LiveAdmissionAuthorityGroup>,
    ) {
        // Layout projection preserves normal authority generation order.
        // Unmapped managed surfaces are the exception: their latest drawing
        // transaction remains in the bounded admission quarantine until it is
        // released once at generation zero and the accepted geometry.
        let quarantined_transactions = self
            .pre_admission_groups
            .iter()
            .chain(self.released_admission_groups.iter())
            .map(|group| group.transaction)
            .collect::<BTreeSet<_>>();
        let mut projected = project_authority_batch_onto_layout(batch.clone(), &self.layers);
        projected.transactions.retain(|transaction| {
            !quarantined_transactions.contains(&transaction.transaction)
        });
        projected.present_submissions.retain(|submission| {
            !quarantined_transactions.contains(&submission.transaction)
        });
        let referenced_dma_bufs = self.admission_group_dma_bufs();
        let referenced_fences = self.admission_group_fences();
        projected
            .released_dma_bufs
            .retain(|handle| !referenced_dma_bufs.contains(handle));
        projected
            .released_fences
            .retain(|handle| !referenced_fences.contains(handle));
        let releasable_dma_bufs = self
            .deferred_dma_buf_releases
            .iter()
            .filter(|handle| !referenced_dma_bufs.contains(handle))
            .copied()
            .collect::<Vec<_>>();
        for handle in releasable_dma_bufs {
            self.deferred_dma_buf_releases.remove(&handle);
            self.dma_buf_sizes.remove(&handle);
            if !projected.released_dma_bufs.contains(&handle) {
                projected.released_dma_bufs.push(handle);
            }
        }
        let releasable_fences = self
            .deferred_fence_releases
            .iter()
            .filter(|handle| !referenced_fences.contains(handle))
            .copied()
            .collect::<Vec<_>>();
        for handle in releasable_fences {
            self.deferred_fence_releases.remove(&handle);
            if !projected.released_fences.contains(&handle) {
                projected.released_fences.push(handle);
            }
        }
        (
            projected,
            self.released_admission_groups.drain(..).collect(),
        )
    }

}

fn live_transaction_pixel_size(
    source: sophia_protocol::BufferSource,
    dma_buf_sizes: &BTreeMap<sophia_protocol::BufferHandle, Size>,
    cpu_buffer_sizes: &BTreeMap<u64, Size>,
) -> Option<Size> {
    match source {
        sophia_protocol::BufferSource::DmaBuf { handle } => {
            dma_buf_sizes.get(&sophia_protocol::BufferHandle::from_raw(handle)).copied()
        }
        sophia_protocol::BufferSource::CpuBuffer { handle } => {
            cpu_buffer_sizes.get(&handle).copied()
        }
        sophia_protocol::BufferSource::None
        | sophia_protocol::BufferSource::XPixmap { .. } => None,
    }
}

fn live_transaction_observed_size(
    transaction: &SurfaceTransaction,
    dma_buf_sizes: &BTreeMap<sophia_protocol::BufferHandle, Size>,
    cpu_buffer_sizes: &BTreeMap<u64, Size>,
) -> Size {
    live_transaction_pixel_size(transaction.target_buffer, dma_buf_sizes, cpu_buffer_sizes)
        .unwrap_or(Size {
            width: transaction.target_geometry.width,
            height: transaction.target_geometry.height,
        })
}

fn wm_update_coordinator_batch(
    transaction: TransactionId,
) -> XAuthorityObservedTransactionBatch {
    XAuthorityObservedTransactionBatch {
        client: None,
        transaction,
        transactions: Vec::new(),
        surface_presentations: Vec::new(),
        presentation_intents: Vec::new(),
        removed_surfaces: Vec::new(),
        surface_output_reservations: Vec::new(),
        cpu_buffer_updates: Vec::new(),
        dma_buf_registrations: Vec::new(),
        fence_registrations: Vec::new(),
        present_submissions: Vec::new(),
        released_dma_bufs: Vec::new(),
        released_fences: Vec::new(),
        protocol_errors: Vec::new(),
        expected_protocol_errors: Vec::new(),
        metadata: Vec::new(),
        selection_owner_change: false,
        selection_conversion: false,
    }
}

fn center_geometry_without_scaling(mut geometry: Rect, output: Size) -> Rect {
    geometry.x = output.width.saturating_sub(geometry.width).max(0) / 2;
    geometry.y = output.height.saturating_sub(geometry.height).max(0) / 2;
    geometry
}

fn validate_live_wm_transaction(
    transaction: &sophia_protocol::LayoutTransaction,
    layout: &PersistentLiveLayout,
    bounds: Rect,
) -> Result<(), Box<dyn std::error::Error>> {
    for placement in &transaction.render_positions {
        let known = layout.knows_surface(placement.surface);
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
        if !layout.knows_surface(request.surface)
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
        .is_some_and(|surface| !layout.knows_surface(surface))
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
