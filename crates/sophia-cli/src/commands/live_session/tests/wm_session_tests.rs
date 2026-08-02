use super::*;
use crate::commands::live_session::{
    LiveWmLayoutFingerprint, LiveWmProposal, PendingLiveWmLayout, PersistentLiveLayout,
    live_layout_node, live_layout_node_from_facts, planning_state_for_response,
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
        surface,
        TransactionId::from_raw(830),
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
    present.presented_surfaces.push(surface);
    layout.observe_authority_batch(&present);

    assert!(layout.resolve_pending().is_some());
    assert!(layout.awaiting_visual_commits.surface_awaiting(surface));
    assert_eq!(layout.layout_epochs.committed_size(surface), Some(launch));
    assert_eq!(layout.layout_epochs.pending_target(surface), Some(target));
    assert!(layout.complete_visual_commit(present_transaction, surface, target));
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
fn explicit_software_present_can_complete_presented_surface_resize() {
    let surface = SurfaceId::new(85, 1);
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
    layout.layout_epochs.record_safe_observation(
        surface,
        TransactionId::from_raw(850),
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
        TransactionId::from_raw(851),
        target_geometry,
    );

    let buffer = 852;
    layout.cpu_buffer_sizes.insert(buffer, target);
    let transaction = TransactionId::from_raw(852);
    let mut presented = crate::commands::live_session::wm_update_coordinator_batch(transaction);
    presented.transactions.push(SurfaceTransaction {
        transaction,
        authority: AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry,
        target_buffer: BufferSource::CpuBuffer { handle: buffer },
        damage: Region::single(target_geometry),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 1,
    });
    presented.presented_surfaces.push(surface);
    presented.software_present_submissions.push(
        sophia_x_authority::XAuthoritySoftwarePresentSubmission {
            transaction,
            surface,
            acquire_fence: None,
            idle_fence: None,
        },
    );
    layout.observe_authority_batch(&presented);

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
            node: live_layout_node(
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
fn recovery_content_extent_crosses_wm_boundary_as_outer_allocation() {
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

    let node = live_layout_node(
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

    let outer = Size {
        width: 504,
        height: 504,
    };
    assert_eq!(node.constraints.min_size, Some(outer));
    assert_eq!(node.constraints.max_size, Some(outer));
    assert_eq!(
        node.geometry,
        Rect {
            x: 0,
            y: 0,
            width: outer.width,
            height: outer.height,
        }
    );
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
    assert_eq!(layout.planning_layers()[0].source, BufferSource::None);
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

#[test]
fn pre_admission_pixels_are_quarantined_from_layout_and_runtime() {
    let surface = SurfaceId::new(5, 1);
    let geometry = Rect {
        x: 20,
        y: 30,
        width: 640,
        height: 480,
    };
    let constraints = SurfaceConstraints {
        min_size: None,
        max_size: None,
    };
    let transaction = SurfaceTransaction {
        transaction: TransactionId::from_raw(11),
        authority: sophia_protocol::AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry: geometry,
        target_buffer: BufferSource::DmaBuf { handle: 44 },
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: geometry.width,
            height: geometry.height,
        }),
        readiness: sophia_protocol::SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };
    let mut batch =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(11));
    batch.client = Some(sophia_x_authority::XServerFrontendClientId::from_raw(1));
    batch.surface_presentations.push(
        sophia_x_authority::XAuthoritySurfacePresentationObservation {
            surface,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            owner: None,
            mapped: false,
            geometry,
            constraints,
            generation: 1,
        },
    );
    batch
        .presentation_intents
        .push(sophia_protocol::SurfacePresentationIntent {
            surface,
            kind: sophia_protocol::SurfacePresentationIntentKind::Request,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            geometry,
            constraints,
            generation: 1,
        });
    batch.transactions.push(transaction.clone());
    batch
        .present_submissions
        .push(sophia_x_authority::XAuthorityPresentSubmission {
            transaction: TransactionId::from_raw(11),
            surface,
            buffer: sophia_protocol::BufferHandle::from_raw(44),
            x_offset: 0,
            y_offset: 0,
            acquire_fence: None,
            idle_fence: None,
        });
    let mut layout = PersistentLiveLayout::default();

    let observation = layout.observe_authority_batch(&batch);
    let (projected, released) = layout.projected_batch(&batch);

    assert_eq!(observation.new_surfaces, vec![surface]);
    assert!(layout.layers.is_empty());
    assert_eq!(
        layout.selected_pre_admission_transaction(
            surface,
            Size {
                width: geometry.width,
                height: geometry.height,
            },
        ),
        Some(&transaction)
    );
    assert!(projected.transactions.is_empty());
    assert!(projected.present_submissions.is_empty());
    assert!(released.is_empty());
    assert_eq!(layout.pre_admission_groups.len(), 1);
    assert!(!observation.admission_group_overflowed);
    assert_eq!(layout.next_unmanaged_surface(), Some(surface));
}

#[test]
fn no_wm_session_commits_policy_managed_pixels_without_admission() {
    let surface = SurfaceId::new(55, 1);
    let geometry = Rect {
        x: 20,
        y: 30,
        width: 640,
        height: 480,
    };
    let constraints = SurfaceConstraints {
        min_size: None,
        max_size: None,
    };
    let transaction = TransactionId::from_raw(111);
    let mut batch = crate::commands::live_session::wm_update_coordinator_batch(transaction);
    batch
        .presentation_intents
        .push(sophia_protocol::SurfacePresentationIntent {
            surface,
            kind: sophia_protocol::SurfacePresentationIntentKind::Request,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            geometry,
            constraints,
            generation: 1,
        });
    batch.transactions.push(SurfaceTransaction {
        transaction,
        authority: sophia_protocol::AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry: geometry,
        target_buffer: BufferSource::CpuBuffer { handle: 111 },
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: geometry.width,
            height: geometry.height,
        }),
        readiness: sophia_protocol::SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    });
    let mut layout = PersistentLiveLayout::new(false, None);
    layout.cpu_buffer_sizes.insert(
        111,
        Size {
            width: geometry.width,
            height: geometry.height,
        },
    );

    let observation = layout.observe_authority_batch(&batch);
    let (projected, released) = layout.projected_batch(&batch);

    assert_eq!(observation.new_surfaces, vec![surface]);
    assert_eq!(layout.next_unmanaged_surface(), None);
    assert_eq!(
        layout.admissions.state(surface),
        sophia_engine::SurfacePresentationAdmissionState::Inactive
    );
    assert_eq!(
        layout.layers.get(&surface).unwrap().source,
        BufferSource::CpuBuffer { handle: 111 }
    );
    assert_eq!(projected.transactions.len(), 1);
    assert!(released.is_empty());
    assert!(layout.pre_admission_groups.is_empty());
}

#[test]
fn admitted_pixels_cross_the_visual_boundary_once_at_planned_geometry() {
    let surface = SurfaceId::new(6, 1);
    let client = sophia_x_authority::XServerFrontendClientId::from_raw(1);
    let geometry = Rect {
        x: 10,
        y: 15,
        width: 640,
        height: 480,
    };
    let constraints = SurfaceConstraints {
        min_size: None,
        max_size: None,
    };
    let pixels = SurfaceTransaction {
        transaction: TransactionId::from_raw(12),
        authority: sophia_protocol::AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry: Rect {
            x: 0,
            y: 0,
            ..geometry
        },
        target_buffer: BufferSource::DmaBuf { handle: 45 },
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: geometry.width,
            height: geometry.height,
        }),
        readiness: sophia_protocol::SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 3,
    };
    let mut observed =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(12));
    observed.client = Some(client);
    observed.surface_presentations.push(
        sophia_x_authority::XAuthoritySurfacePresentationObservation {
            surface,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            owner: None,
            mapped: false,
            geometry,
            constraints,
            generation: 1,
        },
    );
    observed
        .presentation_intents
        .push(sophia_protocol::SurfacePresentationIntent {
            surface,
            kind: sophia_protocol::SurfacePresentationIntentKind::Request,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            geometry,
            constraints,
            generation: 1,
        });
    observed.transactions.push(pixels);
    observed
        .present_submissions
        .push(sophia_x_authority::XAuthorityPresentSubmission {
            transaction: TransactionId::from_raw(12),
            surface,
            buffer: sophia_protocol::BufferHandle::from_raw(45),
            x_offset: 0,
            y_offset: 0,
            acquire_fence: None,
            idle_fence: None,
        });
    let mut layout = PersistentLiveLayout::default();
    layout.observe_authority_batch(&observed);
    let transaction = TransactionId::from_raw(13);
    let proposal = LiveWmProposal {
        transaction,
        layers: layout.planning_layers(),
        requested_sizes: BTreeMap::from([(
            surface,
            Size {
                width: geometry.width,
                height: geometry.height,
            },
        )]),
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
        moved_surfaces: 0,
        source: None,
        effects: None,
    };
    let mut controls = sophia_cli::session_control::SessionControlQueue::default();

    assert!(layout.stage(proposal, &mut controls).unwrap().is_none());
    assert_eq!(controls.pending_len(), 1);
    assert!(layout.acknowledge_admission_control(transaction, surface));
    assert!(matches!(
        crate::commands::live_session::reconcile_live_layout_progress(&mut layout, false),
        crate::commands::live_session::LiveLayoutProgress::DeferredReady
    ));
    assert!(layout.pending.is_some());
    assert!(matches!(
        crate::commands::live_session::reconcile_live_layout_progress(&mut layout, true),
        crate::commands::live_session::LiveLayoutProgress::Committed(_)
    ));
    // Admission can resolve in the same owner iteration that still carries
    // the original observation. The released group must replace, not
    // duplicate, that observation at production intake.
    let (projected, released) = layout.projected_batch(&observed);

    assert!(projected.transactions.is_empty());
    assert!(projected.present_submissions.is_empty());
    assert!(released.is_empty());
    layout.unmanaged_surfaces.remove(&surface);
    let empty =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(14));
    let (_, released) = layout.projected_batch(&empty);
    assert_eq!(released.len(), 1);
    assert_eq!(released[0].transactions.len(), 1);
    assert_eq!(released[0].transactions[0].surface, surface);
    assert_eq!(released[0].transactions[0].target_geometry, geometry);
    assert_eq!(released[0].transactions[0].previous_committed_generation, 0);
    assert_eq!(released[0].present_submissions.len(), 1);
    assert_eq!(
        released[0].present_submissions[0].transaction,
        TransactionId::from_raw(12)
    );
    assert_eq!(
        layout.layers[&surface].source,
        BufferSource::DmaBuf { handle: 45 }
    );
    assert!(layout.pre_admission_groups.is_empty());
    assert!(layout.released_admission_groups.is_empty());
}

#[test]
fn released_admission_keeps_its_transaction_separate_from_the_current_batch() {
    let current_surface = SurfaceId::new(60, 1);
    let admitted_surface = SurfaceId::new(61, 1);
    let geometry = Rect {
        x: 0,
        y: 0,
        width: 640,
        height: 480,
    };
    let transaction = |transaction, surface, target_buffer| SurfaceTransaction {
        transaction,
        authority: sophia_protocol::AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry: geometry,
        target_buffer,
        damage: Region::single(geometry),
        readiness: sophia_protocol::SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };
    let current_transaction = TransactionId::from_raw(367);
    let admitted_transaction = TransactionId::from_raw(858);
    let mut current =
        crate::commands::live_session::wm_update_coordinator_batch(current_transaction);
    current.transactions.push(transaction(
        current_transaction,
        current_surface,
        BufferSource::CpuBuffer { handle: 367 },
    ));
    let released = [crate::commands::live_session::LiveAdmissionAuthorityGroup {
        transaction: admitted_transaction,
        transactions: vec![transaction(
            admitted_transaction,
            admitted_surface,
            BufferSource::DmaBuf { handle: 858 },
        )],
        present_submissions: vec![sophia_x_authority::XAuthorityPresentSubmission {
            transaction: admitted_transaction,
            surface: admitted_surface,
            buffer: sophia_protocol::BufferHandle::from_raw(858),
            x_offset: 0,
            y_offset: 0,
            acquire_fence: None,
            idle_fence: None,
        }],
        software_present_submissions: Vec::new(),
        superseded: false,
    }];
    let production = crate::commands::live_session::production_authority_batch(
        &current,
        &released,
        &PersistentLiveLayout::default(),
    );

    production.validate().unwrap();
    assert_eq!(
        production
            .groups
            .iter()
            .map(|group| group.transaction)
            .collect::<Vec<_>>(),
        vec![current_transaction, admitted_transaction]
    );
    assert_eq!(
        production.groups[0].transactions[0].transaction,
        current_transaction
    );
    assert_eq!(
        production.groups[1].transactions[0].transaction,
        admitted_transaction
    );
    assert_eq!(
        production.groups[1].present_submissions[0].transaction,
        admitted_transaction
    );

    let output = sophia_engine::HeadlessOutput {
        id: sophia_protocol::OutputId::from_raw(1),
        size: Size {
            width: 640,
            height: 480,
        },
        scale: 1,
    };
    let mut runtime =
        sophia_backend_live::LiveProductionVisualRuntime::new(&[output], None, None).unwrap();
    let prepared = runtime
        .prepare_authority_groups(&production.groups)
        .unwrap();
    assert_eq!(prepared.authority_commits.len(), 2);
    assert!(
        prepared
            .authority_commits
            .iter()
            .all(|commit| { commit.outcome == sophia_protocol::TransactionOutcome::Committed })
    );
}

#[test]
fn recovered_awaiting_pixels_admission_releases_its_present_at_commit() {
    let surface = SurfaceId::new(7, 1);
    let client = sophia_x_authority::XServerFrontendClientId::from_raw(1);
    let geometry = Rect {
        x: 10,
        y: 15,
        width: 640,
        height: 480,
    };
    let pixel_transaction = TransactionId::from_raw(20);
    let buffer = sophia_protocol::BufferHandle::from_raw(21);
    let pixels = SurfaceTransaction {
        transaction: pixel_transaction,
        authority: sophia_protocol::AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry: geometry,
        target_buffer: BufferSource::DmaBuf {
            handle: buffer.raw(),
        },
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: geometry.width,
            height: geometry.height,
        }),
        readiness: sophia_protocol::SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };
    let intent = sophia_protocol::SurfacePresentationIntent {
        surface,
        kind: sophia_protocol::SurfacePresentationIntentKind::Request,
        role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
        geometry,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        generation: 1,
    };
    let mut observed =
        crate::commands::live_session::wm_update_coordinator_batch(pixel_transaction);
    observed.client = Some(client);
    observed.presentation_intents.push(intent);
    observed.surface_presentations.push(
        sophia_x_authority::XAuthoritySurfacePresentationObservation {
            surface,
            role: intent.role,
            owner: None,
            // Admission is an engine lifecycle, not a mutable X mapped-bit
            // predicate. Pixels must remain quarantined even if X already
            // reports the window mapped.
            mapped: true,
            geometry,
            constraints: intent.constraints,
            generation: intent.generation,
        },
    );
    observed.transactions.push(pixels);
    observed
        .present_submissions
        .push(sophia_x_authority::XAuthorityPresentSubmission {
            transaction: pixel_transaction,
            surface,
            buffer,
            x_offset: 0,
            y_offset: 0,
            acquire_fence: None,
            idle_fence: None,
        });
    observed.released_dma_bufs.push(buffer);
    let mut layout = PersistentLiveLayout::default();
    layout.observe_authority_batch(&observed);
    let original_admission = TransactionId::from_raw(22);
    assert!(
        layout
            .admissions
            .begin_control(surface, original_admission, geometry)
    );
    assert!(
        layout
            .admissions
            .acknowledge_control(surface, original_admission)
    );
    let recovery_transaction = TransactionId::from_raw(23);
    let proposal = LiveWmProposal {
        transaction: recovery_transaction,
        layers: layout.planning_layers(),
        requested_sizes: BTreeMap::from([(
            surface,
            Size {
                width: geometry.width,
                height: geometry.height,
            },
        )]),
        focus: Some(surface),
        timeout: Duration::from_secs(1),
        update: sophia_engine::WmTransactionUpdate {
            commit: TransactionCommit {
                transaction: recovery_transaction,
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![surface],
            },
            ipc_error: None,
        },
        moved_surfaces: 0,
        source: None,
        effects: None,
    };
    let mut controls = sophia_cli::session_control::SessionControlQueue::default();

    assert!(layout.stage(proposal, &mut controls).unwrap().is_none());
    assert_eq!(
        layout
            .pending
            .as_ref()
            .map(|pending| &pending.admission_surfaces),
        Some(&BTreeSet::from([surface]))
    );
    assert!(layout.resolve_pending().is_some());
    assert_eq!(
        layout.admissions.state(surface),
        sophia_engine::SurfacePresentationAdmissionState::AwaitingRetirement {
            admission_transaction: original_admission,
            visual_transaction: pixel_transaction,
            geometry,
        }
    );
    assert_eq!(layout.focus_to_apply, None);
    let empty =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(24));
    let (projected, released) = layout.projected_batch(&empty);
    assert!(projected.transactions.is_empty());
    assert!(projected.present_submissions.is_empty());
    assert!(released.is_empty());
    layout.unmanaged_surfaces.remove(&surface);
    let (projected, released) = layout.projected_batch(&empty);
    assert_eq!(released.len(), 1);
    assert_eq!(released[0].transactions.len(), 1);
    assert_eq!(released[0].transactions[0].transaction, pixel_transaction);
    assert_eq!(released[0].present_submissions.len(), 1);
    assert_eq!(
        released[0].present_submissions[0].transaction,
        pixel_transaction
    );
    assert!(projected.released_dma_bufs.is_empty());
    let (projected, released) = layout.projected_batch(&empty);
    assert!(released.is_empty());
    assert_eq!(projected.released_dma_bufs, vec![buffer]);
    layout.layout_epochs.set_recovery_extent(
        surface,
        Size {
            width: geometry.width,
            height: geometry.height,
        },
    );
    assert_eq!(layout.recovery_extent_count(), 1);
    assert!(layout.complete_admission_retirement(surface, pixel_transaction));
    assert_eq!(layout.recovery_extent_count(), 0);
    assert!(layout.constraint_relayout_required());
    assert_eq!(
        layout.admissions.state(surface),
        sophia_engine::SurfacePresentationAdmissionState::Managed
    );
    assert_eq!(layout.focus_to_apply, Some((recovery_transaction, surface)));
}

#[test]
fn recovery_cannot_publish_admission_chrome_from_retained_size_without_pixels() {
    let surface = SurfaceId::new(70, 1);
    let client = sophia_x_authority::XServerFrontendClientId::from_raw(1);
    let geometry = Rect {
        x: 20,
        y: 30,
        width: 500,
        height: 500,
    };
    let intent = sophia_protocol::SurfacePresentationIntent {
        surface,
        kind: sophia_protocol::SurfacePresentationIntentKind::Request,
        role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
        geometry,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        generation: 1,
    };
    let mut observed =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(70));
    observed.client = Some(client);
    observed.presentation_intents.push(intent);
    observed.surface_presentations.push(
        sophia_x_authority::XAuthoritySurfacePresentationObservation {
            surface,
            role: intent.role,
            owner: None,
            mapped: true,
            geometry,
            constraints: intent.constraints,
            generation: intent.generation,
        },
    );
    let mut layout = PersistentLiveLayout::default();
    layout.observe_authority_batch(&observed);
    let admission_transaction = TransactionId::from_raw(71);
    assert!(
        layout
            .admissions
            .begin_control(surface, admission_transaction, geometry)
    );
    assert!(
        layout
            .admissions
            .acknowledge_control(surface, admission_transaction)
    );
    let size = Size {
        width: geometry.width,
        height: geometry.height,
    };
    layout.layout_epochs.record_committed(surface, size);
    let recovery_transaction = TransactionId::from_raw(72);
    let proposal = LiveWmProposal {
        transaction: recovery_transaction,
        layers: layout.planning_layers(),
        requested_sizes: BTreeMap::new(),
        focus: Some(surface),
        timeout: Duration::from_secs(1),
        update: sophia_engine::WmTransactionUpdate {
            commit: TransactionCommit {
                transaction: recovery_transaction,
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![surface],
            },
            ipc_error: None,
        },
        moved_surfaces: 0,
        source: None,
        effects: None,
    };
    let mut controls = sophia_cli::session_control::SessionControlQueue::default();

    assert!(layout.stage(proposal, &mut controls).unwrap().is_none());
    assert_eq!(controls.pending_len(), 1);
    assert_eq!(
        layout
            .pending
            .as_ref()
            .map(|pending| &pending.admission_surfaces),
        Some(&BTreeSet::from([surface]))
    );
    assert!(layout.resolve_pending().is_none());
    assert!(layout.layers.is_empty());
    assert_eq!(layout.focus_to_apply, None);
    assert_eq!(
        layout.admissions.state(surface),
        sophia_engine::SurfacePresentationAdmissionState::AwaitingPixels {
            transaction: admission_transaction,
            geometry,
        }
    );
}

#[test]
fn selected_present_settles_older_present_group_without_committing_it() {
    let surface = SurfaceId::new(82, 1);
    let geometry = Rect {
        x: 0,
        y: 0,
        width: 500,
        height: 500,
    };
    let mut intent =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(90));
    intent
        .presentation_intents
        .push(sophia_protocol::SurfacePresentationIntent {
            surface,
            kind: sophia_protocol::SurfacePresentationIntentKind::Request,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            geometry,
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        });
    let mut layout = PersistentLiveLayout::default();
    layout.observe_authority_batch(&intent);
    layout.layers.insert(surface, test_layer(surface, geometry));

    for raw in [91, 92] {
        let transaction = TransactionId::from_raw(raw);
        let buffer = sophia_protocol::BufferHandle::from_raw(raw);
        layout.dma_buf_sizes.insert(
            buffer,
            Size {
                width: geometry.width,
                height: geometry.height,
            },
        );
        let mut present = crate::commands::live_session::wm_update_coordinator_batch(transaction);
        present.transactions.push(SurfaceTransaction {
            transaction,
            authority: sophia_protocol::AuthorityKind::SophiaX,
            surface,
            namespace: None,
            target_geometry: geometry,
            target_buffer: BufferSource::DmaBuf {
                handle: buffer.raw(),
            },
            damage: Region::single(geometry),
            readiness: sophia_protocol::SurfaceTransactionReadiness::Ready,
            timeout_msec: 250,
            previous_committed_generation: raw - 91,
        });
        present
            .present_submissions
            .push(sophia_x_authority::XAuthorityPresentSubmission {
                transaction,
                surface,
                buffer,
                x_offset: 0,
                y_offset: 0,
                acquire_fence: None,
                idle_fence: None,
            });
        layout.observe_authority_batch(&present);
    }

    layout.release_admission_groups(&BTreeMap::from([(surface, TransactionId::from_raw(92))]));

    assert!(layout.pre_admission_groups.is_empty());
    assert_eq!(layout.released_admission_groups.len(), 2);
    assert!(layout.released_admission_groups[0].superseded);
    assert!(!layout.released_admission_groups[1].superseded);
    assert_eq!(
        layout.released_admission_groups[1].transactions[0].previous_committed_generation,
        0
    );
}

#[test]
fn pre_admission_group_with_mixed_transaction_identity_fails_closed() {
    let surface = SurfaceId::new(9, 1);
    let geometry = Rect {
        x: 0,
        y: 0,
        width: 500,
        height: 500,
    };
    let mut intent =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(30));
    intent.client = Some(sophia_x_authority::XServerFrontendClientId::from_raw(1));
    intent.surface_presentations.push(
        sophia_x_authority::XAuthoritySurfacePresentationObservation {
            surface,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            owner: None,
            mapped: false,
            geometry,
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    );
    intent
        .presentation_intents
        .push(sophia_protocol::SurfacePresentationIntent {
            surface,
            kind: sophia_protocol::SurfacePresentationIntentKind::Request,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            geometry,
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        });
    let mut layout = PersistentLiveLayout::default();
    layout.observe_authority_batch(&intent);

    let mut malformed =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(31));
    malformed
        .present_submissions
        .push(sophia_x_authority::XAuthorityPresentSubmission {
            transaction: TransactionId::from_raw(32),
            surface,
            buffer: sophia_protocol::BufferHandle::from_raw(1),
            x_offset: 0,
            y_offset: 0,
            acquire_fence: None,
            idle_fence: None,
        });
    let observation = layout.observe_authority_batch(&malformed);

    assert!(observation.admission_group_invalid);
    assert!(layout.pre_admission_groups.is_empty());
}
