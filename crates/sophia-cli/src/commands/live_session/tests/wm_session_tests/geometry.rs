fn register_test_routes(layout: &mut PersistentLiveLayout, surfaces: &[SurfaceId]) {
    let mut batch =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(1));
    batch.client = Some(sophia_x_authority::XServerFrontendClientId::from_raw(1));
    for surface in surfaces {
        batch
            .presentation_intents
            .push(sophia_protocol::SurfacePresentationIntent {
                surface: *surface,
                kind: sophia_protocol::SurfacePresentationIntentKind::Request,
                role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
                surface_kind: sophia_protocol::LayoutNodeKind::Toplevel,
                placement_preference: sophia_protocol::SurfacePlacementPreference::Default,
                presentation_owner: None,
                stack_rank: 0,
                geometry: Rect::default(),
                constraints: SurfaceConstraints {
                    min_size: None,
                    max_size: None,
                },
                generation: 1,
            });
    }
    layout.client_routes.observe(&batch);
}

fn drain_test_controls(
    controls: &mut sophia_cli::session_control::SessionControlQueue,
) -> Vec<sophia_x_authority::XAuthorityClientControlCommand> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(8);
    let (_acknowledgements, acknowledgement_receiver) = std::sync::mpsc::sync_channel(8);
    controls
        .service(
            &sender,
            &acknowledgement_receiver,
            Instant::now(),
            &mut Vec::new(),
        )
        .unwrap();
    receiver.try_iter().collect()
}

#[test]
fn move_only_surface_receives_geometry_control_without_becoming_a_resize_obligation() {
    let firefox = SurfaceId::new(1, 1);
    let terminal_a = SurfaceId::new(2, 1);
    let terminal_b = SurfaceId::new(3, 1);
    let surfaces = [firefox, terminal_a, terminal_b];
    let old = [
        Rect {
            x: 647,
            y: 21,
            width: 1276,
            height: 1422,
        },
        Rect {
            x: 1927,
            y: 21,
            width: 636,
            height: 1422,
        },
        Rect {
            x: 7,
            y: 21,
            width: 636,
            height: 1422,
        },
    ];
    let next = [
        Rect { x: 7, ..old[0] },
        Rect {
            x: 1282,
            y: 16,
            width: 1276,
            height: 709,
        },
        Rect {
            x: 1282,
            y: 729,
            width: 1276,
            height: 709,
        },
    ];
    let mut layout = PersistentLiveLayout::default();
    register_test_routes(&mut layout, &surfaces);
    for (index, surface) in surfaces.iter().copied().enumerate() {
        layout.layers.insert(surface, test_layer(surface, old[index]));
        layout.layout_epochs.record_committed(
            surface,
            Size {
                width: old[index].width,
                height: old[index].height,
            },
        );
    }
    let transaction = TransactionId::from_raw(9);
    let proposal = LiveWmProposal {
        transaction,
        layers: surfaces
            .iter()
            .enumerate()
            .map(|(index, surface)| test_layer(*surface, next[index]))
            .collect(),
        requested_sizes: BTreeMap::from([
            (
                terminal_a,
                Size {
                    width: next[1].width,
                    height: next[1].height,
                },
            ),
            (
                terminal_b,
                Size {
                    width: next[2].width,
                    height: next[2].height,
                },
            ),
        ]),
        configure_deliveries: 0,
        focus: Some(firefox),
        timeout: Duration::from_secs(1),
        update: sophia_engine::WmTransactionUpdate {
            commit: TransactionCommit {
                transaction,
                outcome: TransactionOutcome::Committed,
                applied_surfaces: surfaces.to_vec(),
            },
            ipc_error: None,
        },
        moved_surfaces: 0,
        source: Some(LiveWmProposalSource::Action(WmActionId::from_raw(3))),
        effects: None,
        policy_settlement: None,
    };
    let mut controls = sophia_cli::session_control::SessionControlQueue::default();

    assert!(layout.stage(proposal, &mut controls).unwrap().is_none());
    let pending = layout.pending.as_ref().unwrap();
    assert_eq!(pending.requested_sizes.len(), 2);
    assert!(!pending.requested_sizes.contains_key(&firefox));
    assert_eq!(pending.moved_surfaces, 3);
    assert_eq!(pending.configure_deliveries, 3);
    let commands = drain_test_controls(&mut controls);
    assert_eq!(commands.len(), 3);
    for (index, surface) in surfaces.iter().copied().enumerate() {
        assert!(commands.contains(&sophia_x_authority::XAuthorityClientControlCommand {
            client: sophia_x_authority::XServerFrontendClientId::from_raw(1),
            command: sophia_x_authority::XAuthorityControlCommand::ConfigureSurface {
                transaction,
                surface,
                geometry: next[index],
            },
        }));
    }
}

#[test]
fn focus_only_layout_emits_no_geometry_control() {
    let surface = SurfaceId::new(4, 1);
    let geometry = Rect {
        x: 7,
        y: 21,
        width: 1276,
        height: 1422,
    };
    let transaction = TransactionId::from_raw(10);
    let mut layout = PersistentLiveLayout::default();
    register_test_routes(&mut layout, &[surface]);
    layout.layers.insert(surface, test_layer(surface, geometry));
    let proposal = LiveWmProposal {
        transaction,
        layers: vec![test_layer(surface, geometry)],
        requested_sizes: BTreeMap::new(),
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
        source: Some(LiveWmProposalSource::Focus(surface)),
        effects: None,
        policy_settlement: None,
    };
    let mut controls = sophia_cli::session_control::SessionControlQueue::default();

    assert!(layout.stage(proposal, &mut controls).unwrap().is_some());
    assert_eq!(controls.pending_len(), 0);
}

#[test]
fn recovery_reseed_reasserts_geometry_when_only_committed_pixels_are_stale() {
    let surface = SurfaceId::new(5, 1);
    let geometry = Rect {
        x: 7,
        y: 21,
        width: 1276,
        height: 1422,
    };
    let transaction = TransactionId::from_raw(11);
    let mut layout = PersistentLiveLayout::default();
    register_test_routes(&mut layout, &[surface]);
    layout.layers.insert(surface, test_layer(surface, geometry));
    layout.layout_epochs.record_committed(
        surface,
        Size {
            width: 636,
            height: 1422,
        },
    );
    let requested_size = Size {
        width: geometry.width,
        height: geometry.height,
    };
    let proposal = LiveWmProposal {
        transaction,
        layers: vec![test_layer(surface, geometry)],
        requested_sizes: BTreeMap::from([(surface, requested_size)]),
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
        moved_surfaces: 0,
        source: Some(LiveWmProposalSource::Relayout),
        effects: None,
        policy_settlement: None,
    };
    let mut controls = sophia_cli::session_control::SessionControlQueue::default();

    assert!(layout.stage(proposal, &mut controls).unwrap().is_none());
    let pending = layout.pending.as_ref().unwrap();
    assert_eq!(pending.requested_sizes, BTreeMap::from([(surface, requested_size)]));
    assert_eq!(pending.moved_surfaces, 0);
    assert_eq!(pending.configure_deliveries, 1);
    let commands = drain_test_controls(&mut controls);
    assert_eq!(commands.len(), 1);
    assert_eq!(
        commands[0].command,
        sophia_x_authority::XAuthorityControlCommand::ConfigureSurface {
            transaction,
            surface,
            geometry,
        }
    );
}

#[test]
fn resize_timeout_restores_the_complete_committed_rectangle() {
    let surface = SurfaceId::new(6, 1);
    let committed = Rect {
        x: 647,
        y: 21,
        width: 1276,
        height: 1422,
    };
    let rejected = Rect {
        x: 7,
        y: 21,
        width: 1000,
        height: 700,
    };
    let transaction = TransactionId::from_raw(12);
    let mut layout = PersistentLiveLayout::default();
    register_test_routes(&mut layout, &[surface]);
    layout.layers.insert(surface, test_layer(surface, committed));
    layout.layout_epochs.record_committed(
        surface,
        Size {
            width: committed.width,
            height: committed.height,
        },
    );
    layout.pending = Some(PendingLiveWmLayout {
        transaction,
        layers: vec![test_layer(surface, rejected)],
        requested_sizes: BTreeMap::from([(
            surface,
            Size {
                width: rejected.width,
                height: rejected.height,
            },
        )]),
        configure_deliveries: 1,
        focus: Some(surface),
        deadline: Instant::now(),
        update: sophia_engine::WmTransactionUpdate {
            commit: TransactionCommit {
                transaction,
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![surface],
            },
            ipc_error: None,
        },
        moved_surfaces: 1,
        staged_transactions: BTreeMap::new(),
        admission_surfaces: BTreeSet::new(),
        source: Some(LiveWmProposalSource::Action(WmActionId::from_raw(3))),
        effects: None,
        policy_settlement: None,
    });
    let mut controls = sophia_cli::session_control::SessionControlQueue::default();

    let result = layout.expire_pending(&mut controls).unwrap().unwrap();
    assert_eq!(result.update.commit.outcome, TransactionOutcome::TimedOut);
    let commands = drain_test_controls(&mut controls);
    assert_eq!(commands.len(), 1);
    assert_eq!(
        commands[0].command,
        sophia_x_authority::XAuthorityControlCommand::ConfigureSurface {
            transaction: commands[0].command.transaction(),
            surface,
            geometry: committed,
        }
    );
}
