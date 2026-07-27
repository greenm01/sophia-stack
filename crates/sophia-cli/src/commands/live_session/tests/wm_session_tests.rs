use super::*;
use crate::commands::live_session::{
    LiveWmLayoutFingerprint, LiveWmProposal, PersistentLiveLayout, live_layout_node,
    live_layout_node_from_facts, planning_state_for_response,
};
use sophia_engine::WmWorkspaceState;
use sophia_protocol::{
    SurfaceConstraints, TransactionCommit, TransactionId, TransactionOutcome, WorkspaceId,
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
        target_buffer: BufferSource::XPixmap { pixmap: 0x900 },
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
    let projected = layout.projected_batch(&batch);

    assert_eq!(observation.new_surfaces, vec![surface]);
    assert!(layout.layers.is_empty());
    assert_eq!(
        layout.pre_admission_transactions.get(&surface),
        Some(&transaction)
    );
    assert!(projected.transactions.is_empty());
    assert!(projected.present_submissions.is_empty());
    assert_eq!(layout.pre_admission_present_submissions.len(), 1);
    assert!(!observation.admission_present_overflowed);
    assert_eq!(layout.next_unmanaged_surface(), Some(surface));
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
        target_buffer: BufferSource::XPixmap { pixmap: 0x901 },
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
    assert!(layout.resolve_pending().is_some());
    let empty =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(14));
    let projected = layout.projected_batch(&empty);

    assert_eq!(projected.transactions.len(), 1);
    assert_eq!(projected.transactions[0].surface, surface);
    assert_eq!(projected.transactions[0].target_geometry, geometry);
    assert_eq!(projected.transactions[0].previous_committed_generation, 0);
    assert_eq!(projected.present_submissions.len(), 1);
    assert_eq!(
        projected.present_submissions[0].transaction,
        TransactionId::from_raw(12)
    );
    assert_eq!(
        layout.layers[&surface].source,
        BufferSource::XPixmap { pixmap: 0x901 }
    );
    assert!(layout.pre_admission_transactions.is_empty());
    assert!(layout.released_admission_transactions.is_empty());
    assert!(layout.pre_admission_present_submissions.is_empty());
    assert!(layout.released_admission_present_submissions.is_empty());
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
            mapped: false,
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
        sophia_engine::SurfacePresentationAdmissionState::Managed
    );
    let empty =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(24));
    let projected = layout.projected_batch(&empty);
    assert_eq!(projected.transactions.len(), 1);
    assert_eq!(projected.transactions[0].transaction, pixel_transaction);
    assert_eq!(projected.present_submissions.len(), 1);
    assert_eq!(
        projected.present_submissions[0].transaction,
        pixel_transaction
    );
}

#[test]
fn pre_admission_present_queue_fails_closed_at_its_fixed_capacity() {
    let surface = SurfaceId::new(8, 1);
    let geometry = Rect {
        x: 0,
        y: 0,
        width: 64,
        height: 64,
    };
    let mut batch =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(20));
    batch.surface_presentations.push(
        sophia_x_authority::XAuthoritySurfacePresentationObservation {
            surface,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            mapped: false,
            geometry,
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    );
    batch.present_submissions.extend(
        (0..=crate::commands::live_session::PRE_ADMISSION_PRESENT_CAPACITY).map(|index| {
            sophia_x_authority::XAuthorityPresentSubmission {
                transaction: TransactionId::from_raw(u64::try_from(index + 1).unwrap()),
                surface,
                buffer: sophia_protocol::BufferHandle::from_raw(u64::try_from(index + 1).unwrap()),
                x_offset: 0,
                y_offset: 0,
                acquire_fence: None,
                idle_fence: None,
            }
        }),
    );
    let mut layout = PersistentLiveLayout::default();

    let observation = layout.observe_authority_batch(&batch);

    assert!(observation.admission_present_overflowed);
    assert_eq!(
        layout.pre_admission_present_submissions.len(),
        crate::commands::live_session::PRE_ADMISSION_PRESENT_CAPACITY
    );
}
