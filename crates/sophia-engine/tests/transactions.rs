mod support;
use support::*;

#[path = "transactions/rendering.rs"]
mod rendering;

#[test]
fn layout_transaction_moves_and_resizes_layers_atomically() {
    let engine = HeadlessEngine::default();
    let surface = SurfaceId::new(0, 1);
    let layers = vec![test_layer(0, 0, 0, Region::empty())];
    let transaction = LayoutTransaction {
        transaction: TransactionId::from_raw(1),
        requested_sizes: Vec::new(),
        focus: Some(surface),
        render_positions: vec![SurfacePlacement {
            surface,
            geometry: Rect {
                x: 25,
                y: 30,
                width: 400,
                height: 300,
            },
            z_index: 7,
            crop: None,
            transform: Transform::IDENTITY,
        }],
        timeout_msec: 300,
    };

    let committed = engine
        .apply_layout_transaction(&transaction, layers)
        .unwrap();

    assert_eq!(committed[0].geometry.x, 25);
    assert_eq!(committed[0].geometry.width, 400);
    assert_eq!(committed[0].stack_rank, 7);
    assert_eq!(committed[0].generation, 2);
    assert_eq!(committed[0].damage.rects.len(), 2);
}

#[test]
fn layout_transaction_rejects_unknown_surfaces() {
    let engine = HeadlessEngine::default();
    let transaction = LayoutTransaction {
        transaction: TransactionId::from_raw(1),
        requested_sizes: Vec::new(),
        focus: None,
        render_positions: vec![SurfacePlacement {
            surface: SurfaceId::new(99, 1),
            geometry: Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
            z_index: 0,
            crop: None,
            transform: Transform::IDENTITY,
        }],
        timeout_msec: 300,
    };

    assert_eq!(
        engine.apply_layout_transaction(&transaction, vec![test_layer(0, 0, 0, Region::empty())]),
        Err(EngineError::InvalidSurface)
    );
}

#[test]
fn commit_layout_transaction_reports_outcome() {
    let engine = HeadlessEngine::default();
    let surface = SurfaceId::new(0, 1);
    let mut layers = vec![test_layer(0, 0, 0, Region::empty())];
    let transaction = LayoutTransaction {
        transaction: TransactionId::from_raw(44),
        requested_sizes: Vec::new(),
        focus: Some(surface),
        render_positions: vec![SurfacePlacement {
            surface,
            geometry: Rect {
                x: 0,
                y: 0,
                width: 500,
                height: 400,
            },
            z_index: 1,
            crop: Some(Rect {
                x: 0,
                y: 0,
                width: 250,
                height: 200,
            }),
            transform: Transform::IDENTITY,
        }],
        timeout_msec: 300,
    };

    let commit = engine.commit_layout_transaction(&transaction, &mut layers);

    assert_eq!(commit.transaction, TransactionId::from_raw(44));
    assert_eq!(commit.outcome, TransactionOutcome::Committed);
    assert_eq!(commit.applied_surfaces, vec![surface]);
    assert_eq!(
        layers[0].crop,
        Some(Rect {
            x: 0,
            y: 0,
            width: 250,
            height: 200,
        })
    );
}

#[test]
fn absent_wm_preserves_committed_layers() {
    let engine = HeadlessEngine::default();
    let layers = vec![test_layer(0, 0, 0, Region::empty())];
    let before = layers.clone();

    let commit = engine.preserve_layout_on_wm_absent(TransactionId::from_raw(45), &layers);

    assert_eq!(commit.transaction, TransactionId::from_raw(45));
    assert_eq!(commit.outcome, TransactionOutcome::TimedOut);
    assert!(commit.applied_surfaces.is_empty());
    assert_eq!(layers, before);
}

#[test]
fn ready_surface_transaction_commits_geometry_and_buffer_together() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let mut committed = vec![engine.committed_state_from_layer(&old_layer)];
    let mut next_layer = old_layer.clone();
    next_layer.geometry = Rect {
        x: 25,
        y: 30,
        width: 400,
        height: 300,
    };
    next_layer.source = BufferSource::DmaBuf { handle: 44 };
    next_layer.damage = Region::single(Rect {
        x: 25,
        y: 30,
        width: 400,
        height: 300,
    });
    let transaction = next_layer.to_surface_transaction(
        TransactionId::from_raw(70),
        AuthorityKind::SophiaNative,
        SurfaceTransactionReadiness::Ready,
        250,
        1,
    );

    let commit = engine.commit_surface_transactions(
        TransactionId::from_raw(70),
        &[transaction],
        &mut committed,
    );

    assert_eq!(commit.outcome, TransactionOutcome::Committed);
    assert_eq!(commit.applied_surfaces, vec![SurfaceId::new(0, 1)]);
    assert_eq!(committed[0].committed_generation, 2);
    assert_eq!(committed[0].geometry.width, 400);
    assert_eq!(committed[0].buffer(), BufferSource::DmaBuf { handle: 44 });
}

#[test]
fn committed_surface_state_projects_to_layer_with_committed_visual_truth() {
    let engine = HeadlessEngine::default();
    let mut template = test_layer(0, 9, 0, Region::empty());
    template.authority_local_id = Some(AuthorityLocalId::new(0x44, 1));
    template.namespace = Some(NamespaceId::from_raw(7));
    template.crop = Some(Rect {
        x: 0,
        y: 0,
        width: 50,
        height: 50,
    });
    let committed = CommittedSurfaceState {
        surface: template.surface,
        committed_generation: 12,
        geometry: Rect {
            x: 40,
            y: 50,
            width: 640,
            height: 480,
        },
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::DmaBuf { handle: 90 },
            sophia_protocol::Size {
                width: 640,
                height: 480,
            },
        ),
        damage: Region::single(Rect {
            x: 40,
            y: 50,
            width: 12,
            height: 12,
        }),
    };

    let layer = engine
        .project_committed_surface_state(&committed, &template)
        .unwrap();

    assert_eq!(layer.surface, template.surface);
    assert_eq!(
        layer.authority_local_id,
        Some(AuthorityLocalId::new(0x44, 1))
    );
    assert_eq!(layer.namespace, Some(NamespaceId::from_raw(7)));
    assert_eq!(layer.stack_rank, 9);
    assert_eq!(layer.crop, template.crop);
    assert_eq!(layer.geometry, committed.geometry);
    assert_eq!(layer.source, BufferSource::DmaBuf { handle: 90 });
    assert_eq!(layer.damage.rects.len(), 1);
    assert_eq!(layer.generation, 12);
}

#[test]
fn committed_surface_projection_drives_frame_planning() {
    let engine = HeadlessEngine::default();
    let template = test_layer(0, 0, 0, Region::empty());
    let committed = CommittedSurfaceState {
        surface: template.surface,
        committed_generation: 2,
        geometry: Rect {
            x: 100,
            y: 120,
            width: 300,
            height: 240,
        },
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::CpuBuffer { handle: 55 },
            sophia_protocol::Size {
                width: 300,
                height: 240,
            },
        ),
        damage: Region::single(Rect {
            x: 100,
            y: 120,
            width: 300,
            height: 240,
        }),
    };
    let layers = engine
        .project_committed_surface_states(std::slice::from_ref(&committed), &[template])
        .unwrap();

    let frame = engine
        .plan_frame(
            FramePlanRequest {
                output: engine.output().id,
                frame_serial: 77,
            },
            layers,
        )
        .unwrap();

    assert_eq!(frame.layers[0].geometry, committed.geometry);
    assert_eq!(
        frame.layers[0].source,
        BufferSource::CpuBuffer { handle: 55 }
    );
    assert_eq!(frame.commands[0].target.rects[0], committed.geometry);
}

#[test]
fn committed_surface_projection_rejects_missing_or_mismatched_templates() {
    let engine = HeadlessEngine::default();
    let committed = CommittedSurfaceState {
        surface: SurfaceId::new(0, 1),
        committed_generation: 1,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        },
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::CpuBuffer { handle: 1 },
            sophia_protocol::Size {
                width: 100,
                height: 100,
            },
        ),
        damage: Region::empty(),
    };
    let other_template = test_layer(1, 0, 0, Region::empty());

    assert_eq!(
        engine.project_committed_surface_state(&committed, &other_template),
        Err(EngineError::InvalidSurface)
    );
    assert_eq!(
        engine.project_committed_surface_states(&[committed], &[other_template]),
        Err(EngineError::InvalidSurface)
    );
}

#[test]
fn surface_visual_state_table_keeps_pending_separate_from_committed() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let committed = engine.committed_state_from_layer(&old_layer);
    let mut table = SurfaceVisualStateTable::from_committed_states([committed.clone()]);
    let mut next_layer = old_layer.clone();
    next_layer.geometry.width = 500;
    next_layer.source = BufferSource::DmaBuf { handle: 99 };
    let pending = next_layer.to_surface_transaction(
        TransactionId::from_raw(80),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Pending,
        250,
        committed.committed_generation,
    );

    table.stage_pending(pending.clone()).unwrap();

    assert_eq!(table.len(), 1);
    assert_eq!(table.committed(old_layer.surface), Some(&committed));
    assert_eq!(table.pending(old_layer.surface), Some(&pending));
    assert_eq!(
        table.committed(old_layer.surface).unwrap().geometry.width,
        old_layer.geometry.width
    );
    assert_eq!(
        table
            .pending(old_layer.surface)
            .unwrap()
            .target_geometry
            .width,
        500
    );
}

#[test]
fn surface_visual_state_table_can_clear_pending_without_dropping_committed() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let committed = engine.committed_state_from_layer(&old_layer);
    let mut table = SurfaceVisualStateTable::from_committed_states([committed.clone()]);
    let pending = old_layer.to_surface_transaction(
        TransactionId::from_raw(81),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        committed.committed_generation,
    );

    table.stage_pending(pending.clone()).unwrap();

    assert_eq!(table.clear_pending(old_layer.surface), Some(pending));
    assert_eq!(table.committed(old_layer.surface), Some(&committed));
    assert!(table.pending(old_layer.surface).is_none());
}

#[test]
fn surface_visual_state_table_rejects_invalid_pending_surface() {
    let mut table = SurfaceVisualStateTable::new();
    let mut layer = test_layer(0, 0, 0, Region::empty());
    layer.surface = SurfaceId::INVALID;
    let pending = layer.to_surface_transaction(
        TransactionId::from_raw(82),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        0,
    );

    assert_eq!(
        table.stage_pending(pending),
        Err(EngineError::InvalidSurface)
    );
    assert!(table.is_empty());
}

#[test]
fn surface_transaction_readiness_allows_null_buffer_unmap_after_mapping() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let committed = engine.committed_state_from_layer(&old_layer);
    let table = SurfaceVisualStateTable::from_committed_states([committed.clone()]);
    let ready = old_layer.to_surface_transaction(
        TransactionId::from_raw(83),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        committed.committed_generation,
    );
    let mut pending = ready.clone();
    pending.readiness = SurfaceTransactionReadiness::Pending;
    let mut missing_buffer = ready.clone();
    missing_buffer.content = sophia_protocol::SurfaceContentSet::singleton(
        BufferSource::None,
        missing_buffer.raster_extent(),
    );
    let mut empty_geometry = ready.clone();
    empty_geometry.target_geometry.width = 0;
    let mut stale = ready.clone();
    stale.previous_committed_generation = 99;

    assert_eq!(
        table.transaction_commit_readiness(&ready),
        SurfaceTransactionCommitReadiness::Ready
    );
    assert_eq!(
        table.transaction_commit_readiness(&pending),
        SurfaceTransactionCommitReadiness::NotReady(SurfaceTransactionReadiness::Pending)
    );
    assert_eq!(
        table.transaction_commit_readiness(&missing_buffer),
        SurfaceTransactionCommitReadiness::Ready
    );
    assert_eq!(
        SurfaceVisualStateTable::new().transaction_commit_readiness(&missing_buffer),
        SurfaceTransactionCommitReadiness::MissingBuffer
    );
    assert_eq!(
        table.transaction_commit_readiness(&empty_geometry),
        SurfaceTransactionCommitReadiness::EmptyGeometry
    );
    assert_eq!(
        table.transaction_commit_readiness(&stale),
        SurfaceTransactionCommitReadiness::StaleGeneration {
            current: committed.committed_generation,
            expected: 99
        }
    );
}

#[test]
fn null_buffer_unmaps_but_malformed_geometry_does_not_commit() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let mut committed = vec![engine.committed_state_from_layer(&old_layer)];
    let before = committed.clone();
    let mut missing_buffer = old_layer.to_surface_transaction(
        TransactionId::from_raw(84),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        before[0].committed_generation,
    );
    missing_buffer.content = sophia_protocol::SurfaceContentSet::singleton(
        BufferSource::None,
        missing_buffer.raster_extent(),
    );

    let commit = engine.commit_surface_transactions(
        TransactionId::from_raw(84),
        &[missing_buffer],
        &mut committed,
    );

    assert_eq!(commit.outcome, TransactionOutcome::Committed);
    assert_eq!(commit.applied_surfaces, vec![old_layer.surface]);
    assert_eq!(committed[0].buffer(), BufferSource::None);

    let mut committed = before.clone();

    let mut empty_geometry = old_layer.to_surface_transaction(
        TransactionId::from_raw(85),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        before[0].committed_generation,
    );
    empty_geometry.target_geometry.height = 0;
    let commit = engine.commit_surface_transactions(
        TransactionId::from_raw(85),
        &[empty_geometry],
        &mut committed,
    );

    assert_eq!(commit.outcome, TransactionOutcome::RejectedInvalidSurface);
    assert!(commit.applied_surfaces.is_empty());
    assert_eq!(committed, before);
}

#[test]
fn slow_client_timeout_preserves_committed_state_by_default() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let committed = engine.committed_state_from_layer(&old_layer);
    let table = SurfaceVisualStateTable::from_committed_states([committed.clone()]);
    let mut timed_out = old_layer.to_surface_transaction(
        TransactionId::from_raw(86),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::TimedOut,
        250,
        committed.committed_generation,
    );
    timed_out.target_geometry.width = 700;
    timed_out.content = sophia_protocol::SurfaceContentSet::singleton(
        BufferSource::DmaBuf { handle: 700 },
        timed_out.raster_extent(),
    );

    assert_eq!(
        table.slow_client_timeout_decision(&timed_out, SurfaceTimeoutPolicy::default()),
        SlowClientVisualDecision::PreserveCommitted {
            surface: old_layer.surface,
            committed: Some(committed.clone())
        }
    );
    assert_eq!(
        table.committed(old_layer.surface),
        Some(&committed),
        "timeout decision must not mutate committed visual state"
    );
}

#[test]
fn slow_client_timeout_replans_without_admitting_pending_pixels() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let committed = engine.committed_state_from_layer(&old_layer);
    let table = SurfaceVisualStateTable::from_committed_states([committed.clone()]);
    let mut timed_out = old_layer.to_surface_transaction(
        TransactionId::from_raw(87),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::TimedOut,
        250,
        committed.committed_generation,
    );
    timed_out.target_geometry.width = 701;
    timed_out.content = sophia_protocol::SurfaceContentSet::singleton(
        BufferSource::DmaBuf { handle: 701 },
        timed_out.raster_extent(),
    );

    let SlowClientVisualDecision::ReplanAtCommittedExtent {
        surface,
        committed: preserved,
        extent,
    } = table
        .slow_client_timeout_decision(&timed_out, SurfaceTimeoutPolicy::ReplanAtCommittedExtent)
    else {
        panic!("expected explicit replan decision");
    };

    assert_eq!(surface, old_layer.surface);
    assert_eq!(preserved, Some(committed.clone()));
    assert_eq!(
        extent,
        Some(Size {
            width: committed.geometry.width,
            height: committed.geometry.height,
        })
    );
    assert_eq!(
        table.committed(old_layer.surface),
        Some(&committed),
        "replan decision must not admit pending pixels"
    );
}

#[test]
fn slow_client_timeout_decision_ignores_non_timeout_transactions() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let committed = engine.committed_state_from_layer(&old_layer);
    let table = SurfaceVisualStateTable::from_committed_states([committed.clone()]);
    let ready = old_layer.to_surface_transaction(
        TransactionId::from_raw(88),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        committed.committed_generation,
    );

    assert_eq!(
        table.slow_client_timeout_decision(&ready, SurfaceTimeoutPolicy::ReplanAtCommittedExtent,),
        SlowClientVisualDecision::NotTimedOut {
            surface: old_layer.surface,
            readiness: SurfaceTransactionCommitReadiness::Ready
        }
    );
}

#[test]
fn pending_surface_transaction_preserves_committed_state() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let mut committed = vec![engine.committed_state_from_layer(&old_layer)];
    let before = committed.clone();
    let mut next_layer = old_layer.clone();
    next_layer.geometry.width = 500;
    let transaction = next_layer.to_surface_transaction(
        TransactionId::from_raw(71),
        AuthorityKind::SophiaNative,
        SurfaceTransactionReadiness::Pending,
        250,
        1,
    );

    let commit = engine.commit_surface_transactions(
        TransactionId::from_raw(71),
        &[transaction],
        &mut committed,
    );

    assert_eq!(commit.outcome, TransactionOutcome::TimedOut);
    assert!(commit.applied_surfaces.is_empty());
    assert_eq!(committed, before);
}

#[test]
fn prepared_surface_transaction_does_not_mutate_until_applied() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let mut committed = vec![engine.committed_state_from_layer(&old_layer)];
    let before = committed.clone();
    let mut next_layer = old_layer.clone();
    next_layer.geometry.width = 500;
    next_layer.source = BufferSource::DmaBuf { handle: 91 };
    let transaction = next_layer.to_surface_transaction(
        TransactionId::from_raw(701),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        1,
    );

    let prepared = engine.prepare_surface_transactions(
        TransactionId::from_raw(701),
        &[transaction],
        &committed,
    );

    assert!(prepared.is_ready());
    assert_eq!(committed, before);
    assert_eq!(prepared.candidate()[0].geometry.width, 500);
    assert_eq!(
        prepared.candidate()[0].buffer(),
        BufferSource::DmaBuf { handle: 91 }
    );

    let commit = engine.apply_prepared_surface_commit(prepared, &mut committed);
    assert_eq!(commit.outcome, TransactionOutcome::Committed);
    assert_eq!(committed[0].geometry.width, 500);
}

#[test]
fn prepared_surface_transaction_rejects_a_changed_baseline() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let mut committed = vec![engine.committed_state_from_layer(&old_layer)];
    let transaction = old_layer.to_surface_transaction(
        TransactionId::from_raw(702),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        1,
    );
    let prepared = engine.prepare_surface_transactions(
        TransactionId::from_raw(702),
        &[transaction],
        &committed,
    );
    committed[0].committed_generation = 2;
    let changed = committed.clone();

    let commit = engine.apply_prepared_surface_commit(prepared, &mut committed);

    assert_eq!(commit.outcome, TransactionOutcome::RejectedStaleSurface);
    assert!(commit.applied_surfaces.is_empty());
    assert_eq!(committed, changed);
}

#[test]
fn prepared_surface_transaction_preserves_an_unrelated_newer_commit() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let unrelated_layer = test_layer(1, 1, 0, Region::empty());
    let mut committed = vec![
        engine.committed_state_from_layer(&old_layer),
        engine.committed_state_from_layer(&unrelated_layer),
    ];
    let mut next_layer = old_layer.clone();
    next_layer.source = BufferSource::DmaBuf { handle: 92 };
    let transaction = next_layer.to_surface_transaction(
        TransactionId::from_raw(703),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        1,
    );
    let prepared = engine.prepare_surface_transactions(
        TransactionId::from_raw(703),
        &[transaction],
        &committed,
    );
    committed[1].committed_generation = 9;
    committed[1].geometry.x = 77;

    let commit = engine.apply_prepared_surface_commit(prepared, &mut committed);

    assert_eq!(commit.outcome, TransactionOutcome::Committed);
    assert_eq!(committed[0].buffer(), BufferSource::DmaBuf { handle: 92 });
    assert_eq!(committed[1].committed_generation, 9);
    assert_eq!(committed[1].geometry.x, 77);
}

#[test]
fn failed_surface_transaction_preserves_committed_state() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let mut committed = vec![engine.committed_state_from_layer(&old_layer)];
    let before = committed.clone();
    let transaction = old_layer.to_surface_transaction(
        TransactionId::from_raw(72),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Failed,
        250,
        1,
    );

    let commit = engine.commit_surface_transactions(
        TransactionId::from_raw(72),
        &[transaction],
        &mut committed,
    );

    assert_eq!(commit.outcome, TransactionOutcome::RejectedStaleSurface);
    assert!(commit.applied_surfaces.is_empty());
    assert_eq!(committed, before);
}

#[test]
fn stale_surface_transaction_preserves_committed_state() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let mut committed = vec![engine.committed_state_from_layer(&old_layer)];
    let before = committed.clone();
    let transaction = old_layer.to_surface_transaction(
        TransactionId::from_raw(73),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        99,
    );

    let commit = engine.commit_surface_transactions(
        TransactionId::from_raw(73),
        &[transaction],
        &mut committed,
    );

    assert_eq!(commit.outcome, TransactionOutcome::RejectedStaleSurface);
    assert!(commit.applied_surfaces.is_empty());
    assert_eq!(committed, before);
}

#[test]
fn invalid_surface_transaction_fails_closed() {
    let engine = HeadlessEngine::default();
    let mut committed = Vec::<CommittedSurfaceState>::new();
    let mut layer = test_layer(0, 0, 0, Region::empty());
    layer.surface = SurfaceId::INVALID;
    let transaction = layer.to_surface_transaction(
        TransactionId::from_raw(74),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        0,
    );

    let commit = engine.commit_surface_transactions(
        TransactionId::from_raw(74),
        &[transaction],
        &mut committed,
    );

    assert_eq!(commit.outcome, TransactionOutcome::RejectedInvalidSurface);
    assert!(commit.applied_surfaces.is_empty());
    assert!(committed.is_empty());
}

#[test]
fn committed_state_retains_the_whole_content_set() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let mut committed = vec![engine.committed_state_from_layer(&old_layer)];
    let extent = Size {
        width: 100,
        height: 100,
    };
    let multi_variant = sophia_protocol::SurfaceContentSet::new(
        extent,
        vec![
            sophia_protocol::SurfaceContentVariant {
                variant: 1,
                source: BufferSource::CpuBuffer { handle: 90 },
                pixel_size: extent,
                density_millis: 1_000,
                transform: sophia_protocol::SurfaceRasterTransform::Normal,
                fidelity: sophia_protocol::SurfaceContentFidelity::AuthorityRaster,
                damage: Region::single(Rect {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 100,
                }),
            },
            sophia_protocol::SurfaceContentVariant {
                variant: 2,
                source: BufferSource::CpuBuffer { handle: 91 },
                pixel_size: Size {
                    width: 200,
                    height: 200,
                },
                density_millis: 2_000,
                transform: sophia_protocol::SurfaceRasterTransform::Normal,
                fidelity: sophia_protocol::SurfaceContentFidelity::AuthorityRaster,
                damage: Region::single(Rect {
                    x: 0,
                    y: 0,
                    width: 200,
                    height: 200,
                }),
            },
        ],
    )
    .unwrap();
    let mut transaction = old_layer.to_surface_transaction(
        TransactionId::from_raw(71),
        AuthorityKind::SophiaNative,
        SurfaceTransactionReadiness::Ready,
        250,
        1,
    );
    transaction.content = multi_variant.clone();

    let commit = engine.commit_surface_transactions(
        TransactionId::from_raw(71),
        &[transaction],
        &mut committed,
    );

    // The committed state carries the authority's whole variant set; a commit
    // that squashed it to the canonical raster would strip the alternatives
    // before any head could select one.
    assert_eq!(commit.outcome, TransactionOutcome::Committed);
    assert_eq!(committed[0].content, multi_variant);
    assert_eq!(
        committed[0].buffer(),
        BufferSource::CpuBuffer { handle: 90 }
    );
}
