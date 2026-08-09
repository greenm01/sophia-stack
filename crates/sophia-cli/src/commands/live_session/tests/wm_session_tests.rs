use super::*;

#[test]
fn completed_pointer_geometry_reduces_raw_motion_to_one_bounded_target() {
    let initial = Rect {
        x: 100,
        y: 80,
        width: 300,
        height: 200,
    };
    let resize = sophia_protocol::WmPointerGestureCompleted {
        surface: SurfaceId::new(91, 1),
        output: OutputId::INVALID,
        workspace: sophia_protocol::WorkspaceId::INVALID,
        mode: sophia_protocol::WmPointerGestureMode::Resize,
        start: sophia_protocol::WmPointerPosition { x: 120, y: 100 },
        end: sophia_protocol::WmPointerPosition { x: 220, y: 50 },
    };
    assert_eq!(
        completed_pointer_gesture_geometry(resize, initial),
        Rect {
            width: 400,
            height: 150,
            ..initial
        }
    );
}
use crate::commands::live_session::{
    LivePolicyMapMode, LivePolicySettlementIdentity, LiveWmLayoutFingerprint, LiveWmProposal,
    LiveWmProposalSource, LiveWmResponseLifetime, PendingLiveWmLayout, PersistentLiveLayout,
    ResizeVisualCommit, committed_relayout_nodes, live_layout_node_from_facts,
    ordered_wm_action_request, planning_state_for_response, public_policy_surface_snapshots,
    reconcile_public_policy_proposal, wm_transport_requires_reseed,
};
use sophia_engine::WmWorkspaceState;
use sophia_protocol::{
    BufferHandle, SurfaceConstraints, TransactionCommit, TransactionId, TransactionOutcome,
    WorkspaceId,
};
use std::collections::{BTreeMap, BTreeSet};

fn test_layer(surface: SurfaceId, geometry: Rect) -> LayerSnapshot {
    LayerSnapshot {
        surface,
        authority_local_id: None,
        namespace: None,
        stack_rank: 0,
        geometry,
        source: BufferSource::None,
        damage: Region::single(geometry),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 1,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    }
}

fn dma_candidate(
    transaction: TransactionId,
    surface: SurfaceId,
    buffer: BufferHandle,
) -> sophia_protocol::SurfaceTransactionKey {
    sophia_protocol::SurfaceTransactionKey {
        transaction,
        surface,
        target_buffer: BufferSource::DmaBuf {
            handle: buffer.raw(),
        },
    }
}

fn test_live_layout_node(
    layer: &LayerSnapshot,
    workspace: WorkspaceId,
    coordinator: &sophia_engine::LayoutEpochCoordinator,
    chrome: sophia_engine::SurfaceChromeStyle,
) -> Result<sophia_protocol::LayoutNodeSnapshot, sophia_engine::ChromeLayoutError> {
    live_layout_node_from_facts(
        sophia_engine::SurfaceLayoutFacts {
            surface: layer.surface,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            kind: sophia_protocol::LayoutNodeKind::Toplevel,
            placement_preference: sophia_protocol::SurfacePlacementPreference::Default,
            presentation_owner: None,
            stack_rank: layer.stack_rank,
            geometry: layer.geometry,
            constraints: coordinator.declared_constraints(layer.surface),
            generation: layer.generation,
        },
        workspace,
        coordinator,
        chrome,
    )
}

fn planning_layers_for(
    layout: &PersistentLiveLayout,
    surfaces: impl IntoIterator<Item = SurfaceId>,
) -> Vec<LayerSnapshot> {
    let output = sophia_protocol::OutputId::from_raw(1);
    let workspace = WorkspaceId::from_raw(1);
    let mut workspace_state = WmWorkspaceState::new(
        [(
            output,
            Rect {
                x: 0,
                y: 0,
                width: 2560,
                height: 1440,
            },
        )],
        1,
    )
    .unwrap();
    for surface in surfaces {
        workspace_state
            .register_surface(surface, workspace)
            .unwrap();
    }
    layout.planning_layers_for_workspace_state(&workspace_state)
}

#[test]
fn public_policy_snapshot_retains_an_admitted_surface_while_it_is_hidden() {
    let surface = SurfaceId::new(92, 4);
    let geometry = Rect {
        x: 24,
        y: 32,
        width: 640,
        height: 480,
    };
    let mut layout = PersistentLiveLayout::default();
    let mut observed =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(1));
    observed.client = Some(sophia_x_authority::XServerFrontendClientId::from_raw(1));
    observed.surface_presentations.push(
        sophia_x_authority::XAuthoritySurfacePresentationObservation {
            surface,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            kind: sophia_protocol::LayoutNodeKind::Toplevel,
            placement_preference: sophia_protocol::SurfacePlacementPreference::Default,
            owner: None,
            stack_rank: 3,
            mapped: false,
            geometry,
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 7,
        },
    );
    observed
        .presentation_intents
        .push(sophia_protocol::SurfacePresentationIntent {
            surface,
            kind: sophia_protocol::SurfacePresentationIntentKind::Request,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            surface_kind: sophia_protocol::LayoutNodeKind::Toplevel,
            placement_preference: sophia_protocol::SurfacePlacementPreference::Default,
            presentation_owner: None,
            stack_rank: 3,
            geometry,
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 7,
        });
    layout.observe_authority_batch(&observed);
    // Engine admission consumes planning ownership. The X frontend's
    // observation remains `mapped=false` because policy admission is not a
    // second client MapWindow request; neither fact ends policy ownership.
    layout.planning_surfaces.remove(&surface);

    let unrouted = SurfaceId::new(93, 1);
    let mut direct_observation =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(2));
    direct_observation.surface_presentations.push(
        sophia_x_authority::XAuthoritySurfacePresentationObservation {
            surface: unrouted,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            kind: sophia_protocol::LayoutNodeKind::Toplevel,
            placement_preference: sophia_protocol::SurfacePlacementPreference::Default,
            owner: None,
            stack_rank: 4,
            mapped: false,
            geometry,
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    );
    layout.observe_authority_batch(&direct_observation);

    assert!(layout.layers.is_empty());
    assert!(layout.planning_surfaces.is_empty());
    assert!(!layout.mapped_surfaces.contains(&surface));
    let surfaces = public_policy_surface_snapshots(
        &layout,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap();

    assert_eq!(surfaces.len(), 1);
    assert_eq!(surfaces[0].surface, surface);
    assert_eq!(surfaces[0].generation, 7);
    assert_eq!(surfaces[0].current_output, None);
    assert_eq!(surfaces[0].geometry, geometry);

    let mut withdrawn =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(3));
    withdrawn.surface_presentations.push(
        sophia_x_authority::XAuthoritySurfacePresentationObservation {
            mapped: false,
            ..observed.surface_presentations[0]
        },
    );
    withdrawn
        .presentation_intents
        .push(sophia_protocol::SurfacePresentationIntent {
            surface,
            kind: sophia_protocol::SurfacePresentationIntentKind::Withdraw,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            surface_kind: sophia_protocol::LayoutNodeKind::Toplevel,
            placement_preference: sophia_protocol::SurfacePlacementPreference::Default,
            presentation_owner: None,
            stack_rank: 3,
            geometry,
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 7,
        });
    layout.observe_authority_batch(&withdrawn);
    assert!(
        public_policy_surface_snapshots(
            &layout,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn wm_projection_clears_focus_but_never_duplicates_positive_layout_focus() {
    let transaction = TransactionId::from_raw(17);
    let previous = SurfaceId::new(3, 1);
    let next = SurfaceId::new(4, 1);

    assert_eq!(
        hidden_wm_focus_to_clear(transaction, Some(previous), Some(next)),
        None
    );
    assert_eq!(
        hidden_wm_focus_to_clear(transaction, Some(previous), None),
        Some((transaction, previous))
    );
    assert_eq!(hidden_wm_focus_to_clear(transaction, None, None), None);
}

#[test]
fn newer_committed_policy_replaces_deferred_retirement_focus() {
    let old_surface = SurfaceId::new(5, 1);
    let new_surface = SurfaceId::new(6, 1);
    let old_transaction = TransactionId::from_raw(18);
    let new_transaction = TransactionId::from_raw(19);
    let geometry = Rect {
        x: 0,
        y: 0,
        width: 1280,
        height: 720,
    };
    let mut layout = PersistentLiveLayout::default();
    layout.retirement_focus.insert(
        old_surface,
        (
            sophia_protocol::SurfaceTransactionKey {
                transaction: old_transaction,
                surface: old_surface,
                target_buffer: BufferSource::None,
            },
            old_transaction,
        ),
    );

    layout.commit_proposal(LiveWmProposal {
        transaction: new_transaction,
        layers: vec![test_layer(new_surface, geometry)],
        requested_sizes: BTreeMap::new(),
        presentation_states: BTreeMap::new(),
        configure_deliveries: 0,
        focus: Some(new_surface),
        timeout: Duration::from_secs(1),
        update: sophia_engine::WmTransactionUpdate {
            commit: TransactionCommit {
                transaction: new_transaction,
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![new_surface],
            },
            ipc_error: None,
        },
        moved_surfaces: 0,
        source: Some(LiveWmProposalSource::Action(WmActionId::from_raw(1))),
        effects: None,
        policy_settlement: None,
    });

    assert!(layout.retirement_focus.is_empty());
    assert_eq!(layout.focus_to_apply, Some((new_transaction, new_surface)));
}

#[test]
fn first_admission_primes_the_selected_safe_pixel_extent() {
    let surface = SurfaceId::new(7, 1);
    let extent = Size {
        width: 500,
        height: 570,
    };
    let mut layout = PersistentLiveLayout::default();
    layout.unmanaged_surfaces.insert(surface);
    layout.presentation_roles.insert(
        surface,
        sophia_protocol::SurfacePresentationRole::PolicyManaged,
    );
    layout
        .admissions
        .observe_intent(sophia_protocol::SurfacePresentationIntent {
            surface,
            kind: sophia_protocol::SurfacePresentationIntentKind::Request,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            surface_kind: sophia_protocol::LayoutNodeKind::Toplevel,
            placement_preference: sophia_protocol::SurfacePlacementPreference::Default,
            presentation_owner: None,
            stack_rank: 0,
            geometry: Rect {
                x: 0,
                y: 0,
                width: extent.width,
                height: extent.height,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        });
    layout
        .layout_epochs
        .set_admission(surface, sophia_engine::SurfaceAdmissionState::Unmanaged);
    layout.layout_epochs.record_safe_observation(
        dma_candidate(
            TransactionId::from_raw(20),
            surface,
            BufferHandle::from_raw(9),
        ),
        extent,
        sophia_engine::SurfaceVisualEvidence::PresentedBuffer,
    );

    layout.prime_admission_extent(surface);

    assert_eq!(layout.layout_epochs.recovery_extent(surface), Some(extent));
    assert_eq!(
        layout.layout_epochs.admission(surface),
        sophia_engine::SurfaceAdmissionState::PendingLayout
    );
}

#[test]
fn public_policy_admission_reconciles_to_the_engine_safe_extent_before_staging() {
    let output = OutputId::from_raw(1);
    let surface = SurfaceId::new(8, 1);
    let safe = Size {
        width: 1323,
        height: 1424,
    };
    let proposed = Size {
        width: 2560,
        height: 1440,
    };
    let mut layout = PersistentLiveLayout::default();
    layout.layout_epochs.set_recovery_extent(surface, safe);
    let proposal = sophia_protocol::PolicyProjectionProposal {
        transaction: TransactionId::from_raw(9),
        connection_epoch: 1,
        request_id: 1,
        base_generation: 1,
        active_output: output,
        outputs: vec![sophia_protocol::PolicyOutputProjection {
            output,
            placements: vec![sophia_protocol::PolicySurfacePlacement {
                surface,
                surface_generation: 1,
                geometry: Rect {
                    x: 0,
                    y: 0,
                    width: proposed.width,
                    height: proposed.height,
                },
                requested_size: Some(proposed),
                crop: None,
                transform: sophia_protocol::PolicyTransform::Identity,
                presentation: sophia_protocol::PolicyPresentationState::default(),
            }],
            focus: Some(surface),
        }],
        indicators: Vec::new(),
        output_statuses: Vec::new(),
    };

    let (reconciled, adjusted) = reconcile_public_policy_proposal(
        &layout,
        &proposal,
        &BTreeMap::from([(
            output,
            Rect {
                x: 0,
                y: 0,
                width: proposed.width,
                height: proposed.height,
            },
        )]),
    )
    .unwrap();

    assert_eq!(adjusted, 1);
    assert_eq!(
        reconciled.outputs[0].placements[0].requested_size,
        Some(safe)
    );
    assert_eq!(
        reconciled.outputs[0].placements[0].geometry,
        Rect {
            x: 0,
            y: 0,
            width: safe.width,
            height: safe.height,
        }
    );
    assert_eq!(
        proposal.outputs[0].placements[0].requested_size,
        Some(proposed)
    );
}

#[test]
fn public_policy_reconciliation_preserves_an_omitted_content_size_request() {
    let output = OutputId::from_raw(1);
    let surface = SurfaceId::new(9, 1);
    let geometry = Rect {
        x: 20,
        y: 30,
        width: 500,
        height: 400,
    };
    let proposal = sophia_protocol::PolicyProjectionProposal {
        transaction: TransactionId::from_raw(10),
        connection_epoch: 1,
        request_id: 1,
        base_generation: 1,
        active_output: output,
        outputs: vec![sophia_protocol::PolicyOutputProjection {
            output,
            placements: vec![sophia_protocol::PolicySurfacePlacement {
                surface,
                surface_generation: 1,
                geometry,
                requested_size: None,
                crop: None,
                transform: sophia_protocol::PolicyTransform::Identity,
                presentation: sophia_protocol::PolicyPresentationState::default(),
            }],
            focus: Some(surface),
        }],
        indicators: Vec::new(),
        output_statuses: Vec::new(),
    };

    let (reconciled, adjusted) = reconcile_public_policy_proposal(
        &PersistentLiveLayout::default(),
        &proposal,
        &BTreeMap::from([(
            output,
            Rect {
                x: 0,
                y: 0,
                width: 2560,
                height: 1440,
            },
        )]),
    )
    .unwrap();

    assert_eq!(adjusted, 0);
    assert_eq!(reconciled.outputs[0].placements[0].geometry, geometry);
    assert_eq!(reconciled.outputs[0].placements[0].requested_size, None);
}

#[test]
fn committed_public_manage_consumes_planning_ownership_before_visual_retirement() {
    let surface = SurfaceId::new(9, 1);
    let geometry = Rect {
        x: 0,
        y: 0,
        width: 640,
        height: 400,
    };
    let transaction = TransactionId::from_raw(10);
    let mut layout = PersistentLiveLayout::default();
    let mut observed =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(9));
    observed
        .presentation_intents
        .push(sophia_protocol::SurfacePresentationIntent {
            surface,
            kind: sophia_protocol::SurfacePresentationIntentKind::Request,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            surface_kind: sophia_protocol::LayoutNodeKind::Toplevel,
            placement_preference: sophia_protocol::SurfacePlacementPreference::Default,
            presentation_owner: None,
            stack_rank: 0,
            geometry,
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        });
    layout.observe_authority_batch(&observed);
    assert_eq!(layout.next_unmanaged_surface(), Some(surface));

    let result = layout.commit_proposal(LiveWmProposal {
        transaction,
        layers: vec![test_layer(surface, geometry)],
        requested_sizes: BTreeMap::new(),
        presentation_states: BTreeMap::new(),
        configure_deliveries: 0,
        focus: Some(surface),
        timeout: Duration::from_secs(1),
        update: sophia_engine::WmTransactionUpdate {
            commit: TransactionCommit {
                transaction,
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![surface],
            },
            ipc_error: None,
        },
        moved_surfaces: 1,
        source: Some(LiveWmProposalSource::Manage(surface)),
        effects: None,
        policy_settlement: Some(LivePolicySettlementIdentity {
            connection_epoch: 1,
            request_id: 1,
            scene_generation: 2,
            transaction,
            expect_session_operation: false,
            session_operation: false,
        }),
    });

    assert_eq!(result.source, Some(LiveWmProposalSource::Manage(surface)));
    assert_eq!(layout.next_unmanaged_surface(), None);
    assert!(layout.planning_surfaces.contains_key(&surface));
    assert!(layout.surface_requires_admission(surface));
}

fn hold_test_resize(
    layout: &mut PersistentLiveLayout,
    surface: SurfaceId,
    transaction: TransactionId,
    geometry: Rect,
) {
    layout.pending = Some(PendingLiveWmLayout {
        transaction,
        layers: vec![test_layer(surface, geometry)],
        requested_sizes: BTreeMap::from([(
            surface,
            Size {
                width: geometry.width,
                height: geometry.height,
            },
        )]),
        presentation_states: BTreeMap::new(),
        presentation_settlements: BTreeSet::new(),
        configure_deliveries: 0,
        focus: Some(surface),
        deadline: Instant::now() + Duration::from_secs(1),
        update: sophia_engine::WmTransactionUpdate {
            commit: TransactionCommit {
                transaction,
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![surface],
            },
            ipc_error: None,
        },
        moved_surfaces: 0,
        staged_transactions: BTreeMap::new(),
        admission_surfaces: BTreeSet::new(),
        source: None,
        effects: None,
        policy_settlement: None,
    });
}

#[test]
fn presented_resize_ignores_exact_backing_snapshot_until_present_retires() {
    let surface = SurfaceId::new(83, 1);
    let launch = Size {
        width: 1280,
        height: 1040,
    };
    let target = Size {
        width: 1276,
        height: 1422,
    };
    let target_geometry = Rect {
        x: 0,
        y: 0,
        width: target.width,
        height: target.height,
    };
    let mut layout = PersistentLiveLayout::default();
    layout.layout_epochs.record_safe_observation(
        dma_candidate(
            TransactionId::from_raw(830),
            surface,
            BufferHandle::from_raw(830),
        ),
        launch,
        sophia_engine::SurfaceVisualEvidence::PresentedBuffer,
    );
    layout.layout_epochs.record_committed(surface, launch);
    layout
        .layout_epochs
        .set_admission(surface, sophia_engine::SurfaceAdmissionState::Managed);
    layout.layout_epochs.set_pending_target(surface, target);
    hold_test_resize(
        &mut layout,
        surface,
        TransactionId::from_raw(831),
        target_geometry,
    );

    let backing_handle = 832;
    layout.cpu_buffer_sizes.insert(backing_handle, target);
    let backing_transaction = TransactionId::from_raw(832);
    let mut backing =
        crate::commands::live_session::wm_update_coordinator_batch(backing_transaction);
    backing.transactions.push(SurfaceTransaction {
        transaction: backing_transaction,
        authority: AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry,
        target_content_size: Size {
            width: target_geometry.width,
            height: target_geometry.height,
        },
        target_buffer: BufferSource::CpuBuffer {
            handle: backing_handle,
        },
        damage: Region::single(target_geometry),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 1,
    });
    layout.observe_authority_batch(&backing);

    assert!(
        layout
            .pending
            .as_ref()
            .unwrap()
            .staged_transactions
            .is_empty()
    );
    assert!(layout.resolve_pending().is_none());
    assert_eq!(layout.layout_epochs.committed_size(surface), Some(launch));
    assert_eq!(layout.layout_epochs.pending_target(surface), Some(target));

    let present_buffer = BufferHandle::from_raw(833);
    layout.dma_buf_sizes.insert(present_buffer, target);
    let present_transaction = TransactionId::from_raw(833);
    let mut present =
        crate::commands::live_session::wm_update_coordinator_batch(present_transaction);
    present.transactions.push(SurfaceTransaction {
        transaction: present_transaction,
        authority: AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry,
        target_content_size: Size {
            width: target_geometry.width,
            height: target_geometry.height,
        },
        target_buffer: BufferSource::DmaBuf {
            handle: present_buffer.raw(),
        },
        damage: Region::single(target_geometry),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 1,
    });
    present
        .present_submissions
        .push(sophia_x_authority::XAuthorityPresentSubmission {
            transaction: present_transaction,
            surface,
            buffer: present_buffer,
            x_offset: 0,
            y_offset: 0,
            acquire_fence: None,
            idle_fence: None,
        });
    layout.observe_authority_batch(&present);

    assert!(layout.resolve_pending().is_some());
    assert!(layout.awaiting_visual_commits.surface_awaiting(surface));
    assert_eq!(layout.layout_epochs.committed_size(surface), Some(launch));
    assert_eq!(layout.layout_epochs.pending_target(surface), Some(target));
    assert!(layout.complete_visual_commit(
        dma_candidate(present_transaction, surface, present_buffer),
        target,
    ));
    assert_eq!(layout.layout_epochs.committed_size(surface), Some(target));
    assert_eq!(layout.layout_epochs.pending_target(surface), None);
}

#[test]
fn backing_resize_still_commits_for_cpu_only_surface() {
    let surface = SurfaceId::new(84, 1);
    let launch = Size {
        width: 640,
        height: 480,
    };
    let target = Size {
        width: 800,
        height: 600,
    };
    let target_geometry = Rect {
        x: 0,
        y: 0,
        width: target.width,
        height: target.height,
    };
    let mut layout = PersistentLiveLayout::default();
    layout.layout_epochs.record_committed(surface, launch);
    layout
        .layout_epochs
        .set_admission(surface, sophia_engine::SurfaceAdmissionState::Managed);
    layout.layout_epochs.set_pending_target(surface, target);
    hold_test_resize(
        &mut layout,
        surface,
        TransactionId::from_raw(840),
        target_geometry,
    );

    let buffer = 841;
    layout.cpu_buffer_sizes.insert(buffer, target);
    let transaction = TransactionId::from_raw(841);
    let mut backing = crate::commands::live_session::wm_update_coordinator_batch(transaction);
    backing.transactions.push(SurfaceTransaction {
        transaction,
        authority: AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry,
        target_content_size: Size {
            width: target_geometry.width,
            height: target_geometry.height,
        },
        target_buffer: BufferSource::CpuBuffer { handle: buffer },
        damage: Region::single(target_geometry),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 1,
    });
    layout.observe_authority_batch(&backing);

    assert!(layout.resolve_pending().is_some());
    assert!(!layout.awaiting_visual_commits.surface_awaiting(surface));
    assert_eq!(layout.layout_epochs.committed_size(surface), Some(target));
    assert_eq!(layout.layout_epochs.pending_target(surface), None);
}

#[test]
fn wm_layout_fingerprint_tracks_planned_surface_lifetimes_only() {
    let managed = SurfaceId::new(1, 1);
    let concurrent = SurfaceId::new(2, 1);
    let output = sophia_protocol::OutputId::from_raw(1);
    let geometry = Rect {
        x: 0,
        y: 0,
        width: 640,
        height: 480,
    };
    let mut state = WmWorkspaceState::new([(output, geometry)], 1).unwrap();
    let workspace = sophia_protocol::WorkspaceId::from_raw(1);
    state.register_surface(managed, workspace).unwrap();
    let mut layout = PersistentLiveLayout::default();
    layout.layers.insert(managed, test_layer(managed, geometry));
    let fingerprint = LiveWmLayoutFingerprint::capture(&layout, &state);

    layout.layers.insert(
        concurrent,
        test_layer(concurrent, Rect { x: 640, ..geometry }),
    );
    assert!(fingerprint.still_matches(&layout));

    layout.layers.get_mut(&managed).unwrap().geometry.x = 1;
    assert!(fingerprint.still_matches(&layout));

    layout.layers.remove(&managed);
    assert!(!fingerprint.still_matches(&layout));
}

#[test]
fn queued_manage_response_rebases_on_the_latest_committed_state() {
    let first = SurfaceId::new(1, 1);
    let queued = SurfaceId::new(2, 1);
    let output = sophia_protocol::OutputId::from_raw(1);
    let workspace = sophia_protocol::WorkspaceId::from_raw(1);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 1280,
        height: 720,
    };
    let mut current = WmWorkspaceState::new([(output, bounds)], 2).unwrap();
    current.register_surface(first, workspace).unwrap();
    let request = sophia_protocol::WmRequestPacket {
        transaction: sophia_protocol::TransactionId::from_raw(2),
        kind: sophia_protocol::WmRequestKind::ManageSurface(sophia_protocol::WmManageSurface {
            node: test_live_layout_node(
                &test_layer(queued, bounds),
                workspace,
                &sophia_engine::LayoutEpochCoordinator::default(),
                sophia_engine::SurfaceChromeStyle::default(),
            )
            .unwrap(),
            output,
            workspace,
            bounds,
        }),
    };

    let rebased = planning_state_for_response(&current, &request).unwrap();
    assert_eq!(rebased.surface_workspace(first), Some(workspace));
    assert_eq!(rebased.surface_workspace(queued), Some(workspace));
}

#[test]
fn ordered_action_request_rebases_on_the_latest_committed_state() {
    let output = sophia_engine::HeadlessOutput::deterministic();
    let bounds = Rect {
        x: 0,
        y: 0,
        width: output.size.width,
        height: output.size.height,
    };
    let workspace = WorkspaceId::from_raw(1);
    let surface = SurfaceId::new(9, 1);
    let action = WmActionId::from_raw(5);
    let transaction = TransactionId::from_raw(6);
    let mut layout = PersistentLiveLayout::default();
    layout.layers.insert(surface, test_layer(surface, bounds));
    let initial = WmWorkspaceState::new([(output.id, bounds)], 1).unwrap();
    let mut current = initial.clone();
    current.register_surface(surface, workspace).unwrap();

    let (initial_packet, _) = ordered_wm_action_request(
        transaction,
        action,
        &layout,
        &initial,
        output,
        sophia_engine::SurfaceChromeStyle::default(),
    )
    .unwrap();
    let (rebased_packet, _) = ordered_wm_action_request(
        transaction,
        action,
        &layout,
        &current,
        output,
        sophia_engine::SurfaceChromeStyle::default(),
    )
    .unwrap();

    let sophia_protocol::WmRequestKind::ActionActivated(initial_action) = initial_packet.kind
    else {
        panic!("expected an action request");
    };
    let sophia_protocol::WmRequestKind::ActionActivated(rebased_action) = rebased_packet.kind
    else {
        panic!("expected an action request");
    };
    assert!(initial_action.nodes.is_empty());
    assert_eq!(
        rebased_action
            .nodes
            .iter()
            .map(|node| node.surface)
            .collect::<Vec<_>>(),
        vec![surface]
    );
}

#[test]
fn forced_wm_timeout_retains_its_source_for_transport_reseed() {
    let surface = SurfaceId::new(3, 1);
    let geometry = Rect {
        x: 0,
        y: 0,
        width: 640,
        height: 480,
    };
    let transaction = TransactionId::from_raw(3);
    let mut layout = PersistentLiveLayout::default();
    layout.pending = Some(PendingLiveWmLayout {
        transaction,
        layers: vec![test_layer(surface, geometry)],
        requested_sizes: BTreeMap::new(),
        presentation_states: BTreeMap::new(),
        presentation_settlements: BTreeSet::new(),
        configure_deliveries: 0,
        focus: None,
        deadline: Instant::now() + Duration::from_secs(60),
        update: sophia_engine::WmTransactionUpdate {
            commit: TransactionCommit {
                transaction,
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![surface],
            },
            ipc_error: None,
        },
        moved_surfaces: 0,
        staged_transactions: BTreeMap::new(),
        admission_surfaces: BTreeSet::new(),
        source: Some(LiveWmProposalSource::Manage(surface)),
        effects: None,
        policy_settlement: None,
    });

    assert!(
        layout
            .expire_pending(&mut sophia_cli::session_control::SessionControlQueue::default())
            .unwrap()
            .is_none()
    );
    assert!(layout.force_pending_timeout());

    let result = layout
        .expire_pending(&mut sophia_cli::session_control::SessionControlQueue::default())
        .unwrap()
        .unwrap();

    assert_eq!(result.update.commit.outcome, TransactionOutcome::TimedOut);
    assert_eq!(result.source, Some(LiveWmProposalSource::Manage(surface)));
    assert!(wm_transport_requires_reseed(&result));
}

#[test]
fn restart_relayout_contains_only_committed_surfaces_in_admission_order() {
    let output = sophia_protocol::OutputId::from_raw(1);
    let workspace = WorkspaceId::from_raw(1);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 2560,
        height: 1440,
    };
    let first = SurfaceId::new(1, 1);
    let second = SurfaceId::new(2, 1);
    let pending = SurfaceId::new(3, 1);
    let mut layout = PersistentLiveLayout::default();
    for surface in [first, second, pending] {
        layout.layers.insert(surface, test_layer(surface, bounds));
    }
    let mut workspace_state = WmWorkspaceState::new([(output, bounds)], 1).unwrap();
    workspace_state.register_surface(first, workspace).unwrap();
    workspace_state.register_surface(second, workspace).unwrap();

    let nodes = committed_relayout_nodes(
        &layout,
        &workspace_state,
        workspace,
        sophia_engine::SurfaceChromeStyle::default(),
    )
    .unwrap();

    assert_eq!(
        nodes.iter().map(|node| node.surface).collect::<Vec<_>>(),
        vec![first, second]
    );
}

#[test]
fn committed_reseed_preserves_pending_visual_candidate_for_manage_replay() {
    let output = sophia_protocol::OutputId::from_raw(1);
    let workspace = WorkspaceId::from_raw(1);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 2560,
        height: 1440,
    };
    let committed_a = SurfaceId::new(1, 1);
    let committed_b = SurfaceId::new(2, 1);
    let firefox = SurfaceId::new(3, 1);
    let fallback = Size {
        width: 1280,
        height: 1040,
    };
    let tile = Size {
        width: 1276,
        height: 1422,
    };
    let fallback_geometry = Rect {
        x: 0,
        y: 0,
        width: fallback.width,
        height: fallback.height,
    };
    let pixel_transaction = TransactionId::from_raw(1529);
    let pixel_buffer = BufferHandle::from_raw(1530);
    let admission_transaction = TransactionId::from_raw(5);
    let mut layout = PersistentLiveLayout::default();
    layout
        .layers
        .insert(committed_a, test_layer(committed_a, bounds));
    layout
        .layers
        .insert(committed_b, test_layer(committed_b, bounds));
    layout.dma_buf_sizes.insert(pixel_buffer, fallback);

    let intent = sophia_protocol::SurfacePresentationIntent {
        surface: firefox,
        kind: sophia_protocol::SurfacePresentationIntentKind::Request,
        role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
        surface_kind: sophia_protocol::LayoutNodeKind::Toplevel,
        placement_preference: sophia_protocol::SurfacePlacementPreference::Default,
        presentation_owner: None,
        stack_rank: 0,
        geometry: fallback_geometry,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        generation: 1,
    };
    let mut observed =
        crate::commands::live_session::wm_update_coordinator_batch(pixel_transaction);
    observed.client = Some(sophia_x_authority::XServerFrontendClientId::from_raw(1));
    observed.presentation_intents.push(intent);
    observed.surface_presentations.push(
        sophia_x_authority::XAuthoritySurfacePresentationObservation {
            surface: firefox,
            role: intent.role,
            kind: intent.surface_kind,
            placement_preference: intent.placement_preference,
            owner: None,
            stack_rank: intent.stack_rank,
            mapped: true,
            geometry: fallback_geometry,
            constraints: intent.constraints,
            generation: intent.generation,
        },
    );
    observed.transactions.push(SurfaceTransaction {
        transaction: pixel_transaction,
        authority: sophia_protocol::AuthorityKind::SophiaX,
        surface: firefox,
        namespace: None,
        target_geometry: fallback_geometry,
        target_content_size: Size {
            width: fallback_geometry.width,
            height: fallback_geometry.height,
        },
        target_buffer: BufferSource::DmaBuf {
            handle: pixel_buffer.raw(),
        },
        damage: Region::single(fallback_geometry),
        readiness: sophia_protocol::SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    });
    observed
        .present_submissions
        .push(sophia_x_authority::XAuthorityPresentSubmission {
            transaction: pixel_transaction,
            surface: firefox,
            buffer: pixel_buffer,
            x_offset: 0,
            y_offset: 0,
            acquire_fence: None,
            idle_fence: None,
        });
    layout.observe_authority_batch(&observed);
    assert!(
        layout
            .admissions
            .begin_control(firefox, admission_transaction, fallback_geometry)
    );
    assert!(
        layout
            .admissions
            .acknowledge_control(firefox, admission_transaction)
    );
    layout.admission_retries.insert(firefox, 1);
    layout.layout_epochs.set_recovery_extent(firefox, fallback);
    layout.layout_epochs.set_pending_target(firefox, tile);

    let mut committed_state = WmWorkspaceState::new([(output, bounds)], 1).unwrap();
    committed_state
        .register_surface(committed_a, workspace)
        .unwrap();
    committed_state
        .register_surface(committed_b, workspace)
        .unwrap();
    let relayout_transaction = TransactionId::from_raw(6);
    let relayout_layers = layout.planning_layers_for_workspace_state(&committed_state);
    assert_eq!(
        relayout_layers
            .iter()
            .map(|layer| layer.surface)
            .collect::<Vec<_>>(),
        vec![committed_a, committed_b]
    );
    let relayout = LiveWmProposal {
        transaction: relayout_transaction,
        layers: relayout_layers,
        requested_sizes: BTreeMap::new(),
        presentation_states: BTreeMap::new(),
        configure_deliveries: 0,
        focus: Some(committed_b),
        timeout: Duration::from_secs(1),
        update: sophia_engine::WmTransactionUpdate {
            commit: TransactionCommit {
                transaction: relayout_transaction,
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![committed_a, committed_b],
            },
            ipc_error: None,
        },
        moved_surfaces: 0,
        source: Some(LiveWmProposalSource::Relayout),
        effects: Some(crate::commands::live_session::LiveWmCommitEffects {
            workspace_state: committed_state.clone(),
            transaction: relayout_transaction,
            session_action: None,
        }),
        policy_settlement: None,
    };
    let mut controls = sophia_cli::session_control::SessionControlQueue::default();

    assert!(layout.stage(relayout, &mut controls).unwrap().is_some());
    assert_eq!(controls.pending_len(), 0);
    assert!(layout.unmanaged_surfaces.contains(&firefox));
    assert_eq!(layout.admission_retries.get(&firefox), Some(&1));
    assert_eq!(layout.pre_admission_groups.len(), 1);
    assert!(layout.released_admission_groups.is_empty());
    assert!(!layout.awaiting_visual_commits.surface_awaiting(firefox));
    assert_eq!(
        layout.admissions.state(firefox),
        sophia_engine::SurfacePresentationAdmissionState::AwaitingPixels {
            transaction: admission_transaction,
            geometry: fallback_geometry,
        }
    );

    let mut managed_state = committed_state;
    managed_state.register_surface(firefox, workspace).unwrap();
    let manage_transaction = TransactionId::from_raw(7);
    let pixel_candidate = dma_candidate(pixel_transaction, firefox, pixel_buffer);
    let manage_layers = layout.planning_layers_for_workspace_state(&managed_state);
    assert_eq!(
        manage_layers
            .iter()
            .map(|layer| layer.surface)
            .collect::<Vec<_>>(),
        vec![committed_a, committed_b, firefox]
    );
    let manage = LiveWmProposal {
        transaction: manage_transaction,
        layers: manage_layers,
        requested_sizes: BTreeMap::from([(firefox, fallback)]),
        presentation_states: BTreeMap::new(),
        configure_deliveries: 0,
        focus: Some(firefox),
        timeout: Duration::from_secs(1),
        update: sophia_engine::WmTransactionUpdate {
            commit: TransactionCommit {
                transaction: manage_transaction,
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![firefox],
            },
            ipc_error: None,
        },
        moved_surfaces: 0,
        source: Some(LiveWmProposalSource::Manage(firefox)),
        effects: Some(crate::commands::live_session::LiveWmCommitEffects {
            workspace_state: managed_state,
            transaction: manage_transaction,
            session_action: None,
        }),
        policy_settlement: None,
    };

    assert!(layout.stage(manage, &mut controls).unwrap().is_none());
    assert_eq!(controls.pending_len(), 1);
    assert!(layout.resolve_pending().is_some());
    assert!(!layout.unmanaged_surfaces.contains(&firefox));
    assert!(layout.pre_admission_groups.is_empty());
    assert_eq!(layout.released_admission_groups.len(), 1);
    assert!(
        layout
            .awaiting_visual_commits
            .exact_candidate(pixel_candidate, fallback)
    );
    assert_eq!(
        layout.admissions.state(firefox),
        sophia_engine::SurfacePresentationAdmissionState::AwaitingRetirement {
            admission_transaction,
            visual_candidate: pixel_candidate,
            geometry: fallback_geometry,
        }
    );
    assert!(layout.complete_visual_commit(pixel_candidate, fallback));
    assert!(layout.complete_admission_retirement(pixel_candidate));
    assert_eq!(
        layout.admissions.state(firefox),
        sophia_engine::SurfacePresentationAdmissionState::Managed
    );
    assert_eq!(layout.layout_epochs.pending_target(firefox), Some(tile));
    assert_eq!(layout.layout_epochs.recovery_extent(firefox), None);
    assert!(layout.constraint_relayout_required());
}

#[test]
fn recovery_content_extent_stays_behind_the_wm_policy_boundary() {
    let surface = SurfaceId::new(3, 1);
    let workspace = sophia_protocol::WorkspaceId::from_raw(1);
    let content = Size {
        width: 500,
        height: 500,
    };
    let mut epochs = sophia_engine::LayoutEpochCoordinator::default();
    epochs.record_committed(surface, content);
    epochs
        .begin_recovery(
            [(
                surface,
                Size {
                    width: 1276,
                    height: 1422,
                },
            )],
            [surface],
        )
        .unwrap();
    let style = sophia_engine::SurfaceChromeStyle {
        frame: sophia_engine::SurfaceFrameStyle {
            width: 2,
            ..sophia_engine::SurfaceFrameStyle::default()
        },
        ..sophia_engine::SurfaceChromeStyle::default()
    };

    let node = test_live_layout_node(
        &test_layer(
            surface,
            Rect {
                x: 2,
                y: 2,
                width: content.width,
                height: content.height,
            },
        ),
        workspace,
        &epochs,
        style,
    )
    .unwrap();

    assert_eq!(node.constraints.min_size, None);
    assert_eq!(node.constraints.max_size, None);
    assert!(node.capabilities.resizable);
    assert_eq!(
        node.geometry,
        Rect {
            x: 0,
            y: 0,
            width: 504,
            height: 504,
        }
    );
}

#[test]
fn declared_fixed_extent_still_crosses_the_wm_policy_boundary() {
    let surface = SurfaceId::new(30, 1);
    let workspace = sophia_protocol::WorkspaceId::from_raw(1);
    let fixed = Size {
        width: 500,
        height: 500,
    };
    let mut epochs = sophia_engine::LayoutEpochCoordinator::default();
    epochs.set_declared_constraints(
        surface,
        SurfaceConstraints {
            min_size: Some(fixed),
            max_size: Some(fixed),
        },
    );
    let style = sophia_engine::SurfaceChromeStyle::default();
    let node = test_live_layout_node(
        &test_layer(
            surface,
            Rect {
                x: 0,
                y: 0,
                width: fixed.width,
                height: fixed.height,
            },
        ),
        workspace,
        &epochs,
        style,
    )
    .unwrap();

    let outer = sophia_engine::outer_surface_constraints(
        SurfaceConstraints {
            min_size: Some(fixed),
            max_size: Some(fixed),
        },
        style,
    )
    .unwrap();
    assert_eq!(node.constraints, outer);
    assert!(!node.capabilities.resizable);
}

#[test]
fn presentation_request_produces_a_wm_node_before_pixels_exist() {
    let surface = SurfaceId::new(4, 1);
    let geometry = Rect {
        x: 80,
        y: 60,
        width: 500,
        height: 500,
    };
    let intent = sophia_protocol::SurfacePresentationIntent {
        surface,
        kind: sophia_protocol::SurfacePresentationIntentKind::Request,
        role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
        surface_kind: sophia_protocol::LayoutNodeKind::Toplevel,
        placement_preference: sophia_protocol::SurfacePlacementPreference::Default,
        presentation_owner: None,
        stack_rank: 0,
        geometry,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        generation: 1,
    };
    let mut batch =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(10));
    batch.client = Some(sophia_x_authority::XServerFrontendClientId::from_raw(1));
    batch.presentation_intents.push(intent);
    let mut layout = PersistentLiveLayout::default();

    let observation = layout.observe_authority_batch(&batch);

    assert_eq!(observation.new_surfaces, vec![surface]);
    assert_eq!(layout.next_unmanaged_surface(), Some(surface));
    assert!(layout.layers.is_empty());
    assert_eq!(
        planning_layers_for(&layout, [surface])[0].source,
        BufferSource::None
    );
    let chrome = sophia_engine::SurfaceChromeStyle::default();
    let node = live_layout_node_from_facts(
        layout.layout_facts(surface).unwrap(),
        WorkspaceId::from_raw(1),
        &layout.layout_epochs,
        chrome,
    )
    .unwrap();
    assert_eq!(node.surface, surface);
    assert_eq!(
        node.geometry,
        sophia_engine::outer_surface_geometry(geometry, chrome).unwrap()
    );
}

include!("wm_session_tests/admission.rs");
include!("wm_session_tests/direct_map.rs");
include!("wm_session_tests/geometry.rs");
include!("wm_session_tests/pre_admission.rs");
include!("wm_session_tests/recovery.rs");
include!("wm_session_tests/response_lifetime.rs");
