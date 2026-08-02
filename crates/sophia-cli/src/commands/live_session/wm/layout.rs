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
    withdrawn_surfaces: Vec<SurfaceId>,
    output_reservations_changed: bool,
    admission_group_invalid: bool,
    admission_group_overflowed: bool,
}

enum LiveLayoutProgress {
    Blocked,
    DeferredReady,
    Committed(LiveWmCommitResult),
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
    presentation_owners: BTreeMap<SurfaceId, SurfaceId>,
    mapped_surfaces: BTreeSet<SurfaceId>,
    pre_admission_groups: VecDeque<LiveAdmissionAuthorityGroup>,
    released_admission_groups: VecDeque<LiveAdmissionAuthorityGroup>,
    output_reservations: sophia_engine::SurfaceOutputReservationState,
    unmanaged_surfaces: BTreeSet<SurfaceId>,
    admission_retries: BTreeMap<SurfaceId, u8>,
    pending: Option<PendingLiveWmLayout>,
    focus_to_apply: Option<(TransactionId, SurfaceId)>,
    retirement_focus: BTreeMap<SurfaceId, (TransactionId, TransactionId)>,
    bypass_policy_admission: bool,
    stage_new_surfaces_offset: bool,
    center_first_surface_in: Option<Size>,
    constraint_relayout_required: bool,
    awaiting_visual_commits: ResizeVisualCommitTracker,
}

impl PersistentLiveLayout {
    fn new(external_wm_present: bool, center_first_surface_in: Option<Size>) -> Self {
        Self {
            bypass_policy_admission: !external_wm_present,
            stage_new_surfaces_offset: external_wm_present,
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
        let mut withdrawn_surfaces = BTreeSet::new();
        self.client_routes.observe(batch);
        self.observe_presentation_intents(
            batch,
            &mut new_surfaces,
            &mut withdrawn_surfaces,
        );
        for presentation in &batch.surface_presentations {
            let previous_role = self
                .presentation_roles
                .insert(presentation.surface, presentation.role);
            if presentation.mapped {
                self.mapped_surfaces.insert(presentation.surface);
            } else {
                self.mapped_surfaces.remove(&presentation.surface);
            }
            if let Some(owner) = presentation.owner {
                self.presentation_owners.insert(presentation.surface, owner);
            } else {
                self.presentation_owners.remove(&presentation.surface);
            }
            if previous_role
                == Some(sophia_protocol::SurfacePresentationRole::PolicyManaged)
                && presentation.role
                    == sophia_protocol::SurfacePresentationRole::ClientPositioned
            {
                withdrawn_surfaces.insert(presentation.surface);
                self.admissions.remove(presentation.surface);
                self.planning_surfaces.remove(&presentation.surface);
                self.unmanaged_surfaces.remove(&presentation.surface);
            } else if previous_role
                == Some(sophia_protocol::SurfacePresentationRole::ClientPositioned)
                && presentation.role
                    == sophia_protocol::SurfacePresentationRole::PolicyManaged
                && presentation.mapped
            {
                let intent = sophia_protocol::SurfacePresentationIntent {
                    surface: presentation.surface,
                    kind: sophia_protocol::SurfacePresentationIntentKind::Request,
                    role: presentation.role,
                    geometry: presentation.geometry,
                    constraints: presentation.constraints,
                    generation: presentation.generation,
                };
                let facts = sophia_engine::SurfaceLayoutFacts::from(intent);
                self.planning_surfaces.insert(presentation.surface, facts);
                if !self.bypass_policy_admission {
                    self.admissions.observe_intent(intent);
                    self.unmanaged_surfaces.insert(presentation.surface);
                    self.layout_epochs.set_admission(
                        presentation.surface,
                        sophia_engine::SurfaceAdmissionState::Unmanaged,
                    );
                }
                new_surfaces.insert(presentation.surface);
            }
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
                    if !self.bypass_policy_admission
                        && presentation.mapped
                        && self.layers.contains_key(&presentation.surface)
                    {
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
                sophia_x_authority::XAuthorityCpuBufferUpdate::PatchBatch(batch) => {
                    self.cpu_buffer_sizes.insert(batch.handle, batch.size);
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
            let visual_evidence = live_transaction_visual_evidence(
                transaction,
                batch.presented_surfaces.contains(&transaction.surface),
            );
            let candidate_selected = self.layout_epochs.record_safe_observation(
                transaction.surface,
                transaction.transaction,
                observed_size,
                visual_evidence,
            );
            if candidate_selected && self.surface_requires_admission(transaction.surface) {
                println!(
                    "sophia_live_visual_candidate schema=1 status=selected transaction={} surface={} width={} height={} evidence={:?}",
                    transaction.transaction.raw(),
                    transaction.surface.index(),
                    observed_size.width,
                    observed_size.height,
                    visual_evidence,
                );
            }
            let resize_owned = self.pending.as_ref().is_some_and(|pending| {
                pending.requested_sizes.contains_key(&transaction.surface)
            }) || self
                .awaiting_visual_commits
                .surface_awaiting(transaction.surface)
                || self
                    .layout_epochs
                    .pending_target(transaction.surface)
                    .is_some();
            let staged_for_resize = self.pending.as_ref().is_some_and(|pending| {
                pending.requested_sizes.get(&transaction.surface) == Some(&observed_size)
            });
            if resize_owned {
                let selected_for_admission = !self.surface_requires_admission(transaction.surface)
                    || self
                        .layout_epochs
                        .safe_observation(transaction.surface)
                        .is_some_and(|selected| {
                            selected.transaction == Some(transaction.transaction)
                        });
                let evidence_allowed = self
                    .layout_epochs
                    .resize_evidence_allowed(transaction.surface, visual_evidence);
                if staged_for_resize && selected_for_admission && evidence_allowed {
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
                    if !self.bypass_policy_admission
                        && policy_managed
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
            withdrawn_surfaces: withdrawn_surfaces.into_iter().collect(),
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
            self.awaiting_visual_commits.remove_surface(*surface);
            self.admissions.remove(*surface);
            self.admission_retries.remove(surface);
        }
        self.unmanaged_surfaces
            .retain(|surface| !removed_surfaces.contains(surface));
        self.presentation_roles
            .retain(|surface, _| !removed_surfaces.contains(surface));
        // Preserve a surviving transient's attachment to a removed owner.  A
        // stale owner is deliberately non-visible until the client publishes
        // a new ownership snapshot; dropping the relation here would promote
        // the transient to an unattached, visible client-positioned surface.
        self.presentation_owners
            .retain(|surface, _| !removed_surfaces.contains(surface));
        self.mapped_surfaces
            .retain(|surface| !removed_surfaces.contains(surface));
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
        !self.bypass_policy_admission
            && self.presentation_roles.get(&surface)
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
            .filter(|layer| {
                self.is_client_positioned(layer.surface)
                    && self.mapped_surfaces.contains(&layer.surface)
            })
            .max_by_key(|layer| (layer.stack_rank, layer.surface))
            .map(|layer| layer.surface)
    }

    fn release_recovery_extent(&mut self, surface: SurfaceId, reason: &'static str) -> bool {
        if !self.layout_epochs.clear_recovery_extent(surface) {
            return false;
        }
        self.constraint_relayout_required = true;
        println!(
            "sophia_live_resize_epoch schema=2 status=recovery_extent_cleared surface={} reason={reason}",
            surface.index(),
        );
        true
    }

    fn complete_visual_commit(
        &mut self,
        transaction: TransactionId,
        surface: SurfaceId,
        size: Size,
    ) -> bool {
        let Some(candidate) = self
            .awaiting_visual_commits
            .complete(transaction, surface, size)
        else {
            return false;
        };
        self.layout_epochs
            .record_committed(candidate.surface, candidate.size);
        println!(
            "sophia_live_resize_epoch schema=3 status=visual_committed transaction={} surface={} width={} height={}",
            candidate.transaction.raw(),
            candidate.surface.index(),
            candidate.size.width,
            candidate.size.height,
        );
        true
    }

    fn constraint_relayout_required(&self) -> bool {
        self.constraint_relayout_required
    }

    fn acknowledge_constraint_relayout(&mut self) {
        self.constraint_relayout_required = false;
    }

    fn recovery_extent_count(&self) -> usize {
        self.layout_epochs.recovery_extent_count()
    }

    fn client_positioned_mapped(&self, surface: SurfaceId) -> bool {
        self.is_client_positioned(surface) && self.mapped_surfaces.contains(&surface)
    }

    fn presentation_owner(&self, surface: SurfaceId) -> Option<SurfaceId> {
        self.presentation_owners.get(&surface).copied()
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
        // Drive a standing target left by an aborted launch epoch through this
        // resize transaction rather than out of band. A first-launch client is
        // fenced and admitted at whatever extent it first mapped; its blind-WM
        // target is retained as an obligation and injected here so the same
        // ConfigureSurface, exact-size epoch gate, and record_committed run as
        // one transaction. That keeps the client, the Engine committed size,
        // and the WM layer in agreement, so the resized frame is accepted (not
        // rejected as a stale surface) and a denied reactive client configure
        // is answered with the target rather than the welded launch size.
        let standing_targets = self
            .layout_epochs
            .pending_target_surfaces()
            .filter(|(surface, target)| {
                self.layout_epochs.recovery_extent(*surface).is_none()
                    && !self.awaiting_visual_commits.surface_awaiting(*surface)
                    && self.layout_epochs.committed_size(*surface) != Some(*target)
            })
            .collect::<Vec<_>>();
        for (surface, target) in standing_targets {
            if let Some(layer) = proposal
                .layers
                .iter_mut()
                .find(|layer| layer.surface == surface)
            {
                layer.geometry.width = target.width;
                layer.geometry.height = target.height;
                proposal.requested_sizes.insert(surface, target);
            }
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
            let Some(transaction) = self.selected_pre_admission_transaction(*surface, *size)
            else {
                continue;
            };
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
        if !self.pending_is_ready() {
            return None;
        }
        let pending = self.pending.take().expect("checked above");
        Some(self.commit_pending(pending))
    }

    fn pending_is_ready(&self) -> bool {
        let Some(pending) = self.pending.as_ref() else {
            return false;
        };
        pending.requested_sizes.iter().all(|(surface, size)| {
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
        })
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
        // A first-launch admission surface is fenced through a fixed recovery
        // extent (the `fixed_surfaces` argument below), not rolled back. Rolling
        // it back would issue a ConfigureSurface at its own initial buffer size
        // and mark the blind-WM target as rejected, welding the client to the
        // size it happened to map at (e.g. Firefox's 1280x1040 default) instead
        // of converging on the WM tile. Only already-managed surfaces roll back
        // to a known-good size.
        let rollback = self.layout_epochs.begin_recovery(
            pending
                .requested_sizes
                .iter()
                .filter(|(surface, _)| {
                    !terminal_admissions.contains(surface)
                        && !admission_surfaces.contains(surface)
                })
                .map(|(surface, size)| (*surface, *size)),
            admission_surfaces
                .iter()
                .copied()
                .filter(|surface| !terminal_admissions.contains(surface)),
        )?;
        // Retain each fenced admission surface's blind-WM target as a standing
        // obligation. Once its temporary recovery extent clears it is driven to
        // that size rather than staying welded to the extent it first mapped at.
        for surface in &admission_surfaces {
            if terminal_admissions.contains(surface) {
                continue;
            }
            if let Some(target) = pending.requested_sizes.get(surface) {
                self.layout_epochs.set_pending_target(*surface, *target);
            }
        }
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
                BufferSource::CpuBuffer { .. } => {
                    if self.admissions.mark_managed(*surface) {
                        self.planning_surfaces.remove(surface);
                        self.release_recovery_extent(*surface, "cpu_admission_committed");
                        println!(
                            "sophia_live_visual_admission schema=1 status=committed transaction={} surface={} source=cpu_snapshot",
                            transaction.transaction.raw(),
                            surface.index(),
                        );
                    }
                }
                _ => {
                    if self.admissions.mark_managed(*surface) {
                        self.planning_surfaces.remove(surface);
                        self.release_recovery_extent(*surface, "admission_committed");
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
                    if matches!(transaction.target_buffer, BufferSource::DmaBuf { .. }) {
                        self.awaiting_visual_commits
                            .arm(ResizeVisualCommit {
                                transaction: transaction.transaction,
                                surface: transaction.surface,
                                size,
                            })
                            .expect("a staged layout owns one bounded visual candidate");
                        println!(
                            "sophia_live_resize_epoch schema=3 status=visual_armed epoch={} transaction={} surface={} width={} height={}",
                            pending.transaction.raw(),
                            transaction.transaction.raw(),
                            transaction.surface.index(),
                            size.width,
                            size.height,
                        );
                    } else {
                        self.layout_epochs
                            .record_committed(transaction.surface, size);
                    }
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
        projected.software_present_submissions.retain(|submission| {
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
        let mut released = Vec::new();
        let mut retained = VecDeque::new();
        while let Some(group) = self.released_admission_groups.pop_front() {
            if group.contains_any_surface(&self.unmanaged_surfaces) {
                retained.push_back(group);
            } else {
                released.push(group);
            }
        }
        self.released_admission_groups = retained;
        (projected, released)
    }
}
