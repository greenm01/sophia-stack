#[test]
fn resource_lookup_is_namespace_scoped() {
    let trusted = NamespaceId::from_raw(1);
    let untrusted = NamespaceId::from_raw(2);
    let window = XResourceId::new(0x20, 1);
    let mut resources = XResourceTable::new();

    resources
        .insert(window, XResourceKind::Window, trusted, 1)
        .unwrap();

    assert_eq!(
        resources
            .lookup(trusted, window, XResourceKind::Window)
            .unwrap()
            .owner_namespace,
        trusted
    );
    assert_eq!(
        resources.lookup(untrusted, window, XResourceKind::Window),
        Err(XAuthorityAccessError::CrossNamespaceDenied)
    );
    assert_eq!(
        resources.lookup(trusted, window, XResourceKind::Pixmap),
        Err(XAuthorityAccessError::WrongResourceKind)
    );
}

#[test]
fn event_subscriptions_do_not_cross_namespaces() {
    let trusted = NamespaceId::from_raw(1);
    let untrusted = NamespaceId::from_raw(2);
    let window = XResourceId::new(0x30, 1);
    let mut resources = XResourceTable::new();
    let mut subscriptions = XEventSubscriptionTable::new();

    resources
        .insert(window, XResourceKind::Window, trusted, 1)
        .unwrap();
    subscriptions
        .subscribe(&resources, trusted, window, XEventClass::Structure)
        .unwrap();

    assert_eq!(
        subscriptions.subscribe(&resources, untrusted, window, XEventClass::Structure),
        Err(XAuthorityAccessError::CrossNamespaceDenied)
    );
    assert_eq!(
        subscriptions.subscribers(window, trusted, XEventClass::Structure),
        vec![trusted]
    );
    assert!(
        subscriptions
            .subscribers(window, untrusted, XEventClass::Structure)
            .is_empty()
    );
}

#[test]
fn window_lifecycle_creates_authority_surface_records() {
    let namespace = NamespaceId::from_raw(7);
    let window = XResourceId::new(0x40, 1);
    let surface = SurfaceId::new(3, 1);
    let mut windows = XWindowTable::new();

    let created = windows
        .apply(XWindowLifecycleEvent::Created {
            id: window,
            surface,
            namespace,
            geometry: Rect {
                x: 10,
                y: 20,
                width: 640,
                height: 480,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        })
        .unwrap()
        .expect("created window should emit authority surface");

    assert_eq!(created.authority, AuthorityKind::SophiaX);
    assert_eq!(created.local_id, window.local);
    assert_eq!(created.surface, surface);
    assert_eq!(created.namespace, Some(namespace));
    assert!(!created.mapped);

    let mapped = windows
        .apply(XWindowLifecycleEvent::Mapped {
            id: window,
            generation: 2,
        })
        .unwrap()
        .expect("mapped window should emit authority surface");

    assert!(mapped.mapped);
    assert_eq!(mapped.generation, 1);

    let destroyed = windows
        .apply(XWindowLifecycleEvent::Destroyed { id: window })
        .unwrap();

    assert_eq!(destroyed, None);
    assert!(windows.is_empty());
}

#[test]
fn present_pixmap_update_becomes_ready_surface_transaction() {
    let namespace = NamespaceId::from_raw(7);
    let window = XResourceId::new(0x50, 1);
    let mut windows = window_table_with_surface(window, namespace);

    windows
        .apply(XWindowLifecycleEvent::Mapped {
            id: window,
            generation: 2,
        })
        .unwrap();

    let transaction = surface_transaction_from_drawing_update(
        &windows,
        XDrawingUpdate::present_pixmap(
            TransactionId::from_raw(9),
            namespace,
            window,
            0x900,
            Region::single(Rect {
                x: 10,
                y: 20,
                width: 32,
                height: 24,
            }),
            4,
            250,
        ),
    )
    .unwrap();

    assert_eq!(transaction.transaction, TransactionId::from_raw(9));
    assert_eq!(transaction.authority, AuthorityKind::SophiaX);
    assert_eq!(transaction.surface, SurfaceId::new(3, 1));
    assert_eq!(transaction.namespace, Some(namespace));
    assert_eq!(
        transaction.target_buffer,
        BufferSource::XPixmap { pixmap: 0x900 }
    );
    assert_eq!(transaction.readiness, SurfaceTransactionReadiness::Ready);
    assert_eq!(transaction.previous_committed_generation, 4);
    assert_eq!(transaction.timeout_msec, 250);
    assert_eq!(transaction.damage.rects.len(), 1);
}

#[test]
fn shm_and_core_draw_updates_become_ready_cpu_buffer_transactions() {
    let namespace = NamespaceId::from_raw(8);
    let window = XResourceId::new(0x60, 1);
    let windows = window_table_with_surface(window, namespace);

    let shm = surface_transaction_from_drawing_update(
        &windows,
        XDrawingUpdate::shm_put_image(
            TransactionId::from_raw(10),
            namespace,
            window,
            100,
            Region::single(Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 480,
            }),
            1,
            300,
        ),
    )
    .unwrap();
    let core = surface_transaction_from_drawing_update(
        &windows,
        XDrawingUpdate::core_draw(
            TransactionId::from_raw(11),
            namespace,
            window,
            101,
            Region::single(Rect {
                x: 5,
                y: 6,
                width: 7,
                height: 8,
            }),
            2,
            300,
        ),
    )
    .unwrap();

    assert_eq!(shm.target_buffer, BufferSource::CpuBuffer { handle: 100 });
    assert_eq!(shm.readiness, SurfaceTransactionReadiness::Ready);
    assert_eq!(shm.previous_committed_generation, 1);
    assert_eq!(core.target_buffer, BufferSource::CpuBuffer { handle: 101 });
    assert_eq!(core.damage.rects[0].width, 7);
    assert_eq!(core.previous_committed_generation, 2);
}

#[test]
fn repeated_runtime_draws_advance_surface_generations() {
    let namespace = NamespaceId::from_raw(8);
    let window = XResourceId::new(0x61, 1);
    let mut runtime = XAuthorityRuntime::new();
    let created = runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(12),
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window,
            surface: SurfaceId::new(12, 1),
            geometry: Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 40,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 5,
        },
    });
    assert_eq!(created.outcome, XAuthorityResponseOutcome::Accepted);

    let damage = Region::single(Rect {
        x: 1,
        y: 2,
        width: 8,
        height: 12,
    });
    let first = runtime.apply_core_draw(
        TransactionId::from_raw(13),
        namespace,
        window,
        damage.clone(),
    );
    let mapped = runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(14),
        namespace,
        kind: XAuthorityRequestKind::MapWindow {
            window,
            generation: 99,
        },
    });
    assert_eq!(mapped.outcome, XAuthorityResponseOutcome::Accepted);
    runtime
        .configure_window_geometry(
            namespace,
            window,
            XWindowGeometryUpdate {
                generation: 100,
                ..XWindowGeometryUpdate::default()
            },
        )
        .unwrap();
    let second = runtime.apply_core_draw(TransactionId::from_raw(15), namespace, window, damage);

    assert_eq!(first.transactions[0].previous_committed_generation, 5);
    assert_eq!(second.transactions[0].previous_committed_generation, 6);
}

#[test]
fn client_resource_range_release_reclaims_only_its_supported_resources() {
    let namespace = NamespaceId::from_raw(17);
    let departing_window = XResourceId::new(0x0020_0001, 1);
    let retained_window = XResourceId::new(0x0040_0001, 1);
    let mut runtime = XAuthorityRuntime::new();
    for (window, surface) in [(departing_window, 201), (retained_window, 401)] {
        assert_eq!(
            runtime
                .apply(XAuthorityRequestPacket {
                    transaction: TransactionId::from_raw(u64::from(surface)),
                    namespace,
                    kind: XAuthorityRequestKind::CreateWindow {
                        window,
                        surface: SurfaceId::new(surface, 1),
                        geometry: Rect {
                            x: 0,
                            y: 0,
                            width: 80,
                            height: 60,
                        },
                        constraints: SurfaceConstraints {
                            min_size: None,
                            max_size: None,
                        },
                        generation: 1,
                    },
                })
                .outcome,
            XAuthorityResponseOutcome::Accepted
        );
    }
    runtime
        .create_pixmap(
            namespace,
            XResourceId::new(0x0020_0002, 1),
            Size {
                width: 16,
                height: 16,
            },
            1,
        )
        .unwrap();
    runtime
        .open_font(namespace, XResourceId::new(0x0020_0003, 1), 1)
        .unwrap();
    runtime
        .create_cursor(namespace, XResourceId::new(0x0020_0004, 1), 1)
        .unwrap();
    runtime
        .create_graphics_context(
            namespace,
            XResourceId::new(0x0020_0005, 1),
            departing_window,
            XGraphicsContextValues::default(),
        )
        .unwrap();
    runtime
        .attach_shm_segment(namespace, XResourceId::new(0x0020_0006, 1), 10, false, 1)
        .unwrap();
    runtime
        .create_glx_context(namespace, XResourceId::new(0x0020_0007, 1), 3, true)
        .unwrap();
    runtime
        .create_glx_window(
            namespace,
            XResourceId::new(0x0020_0008, 1),
            departing_window,
            3,
        )
        .unwrap();
    let sync_counter = XResourceId::new(0x0020_0009, 1);
    runtime
        .create_sync_counter(namespace, sync_counter, 1, 41)
        .unwrap();
    runtime
        .change_sync_counter(namespace, sync_counter, 1)
        .unwrap();
    assert_eq!(runtime.sync_counter(namespace, sync_counter), Ok(42));

    let release = runtime
        .release_client_resource_range(
            namespace,
            XWireClientResourceRange {
                base: 0x0020_0000,
                mask: X_SETUP_DEFAULT_RESOURCE_ID_MASK,
            },
        )
        .unwrap();

    assert_eq!(release.destroyed_windows, vec![departing_window]);
    assert_eq!(release.removed_surfaces, vec![SurfaceId::new(201, 1)]);
    assert_eq!(release.released_pixmaps, 1);
    assert_eq!(release.released_fonts, 1);
    assert_eq!(release.released_cursors, 1);
    assert_eq!(release.released_graphics_contexts, 1);
    assert_eq!(release.released_shm_segments, 1);
    assert_eq!(release.released_glx_contexts, 1);
    assert_eq!(release.released_glx_windows, 1);
    assert_eq!(runtime.window_count(), 1);
    assert_eq!(runtime.resource_count(), 1);
    assert_eq!(runtime.shm_segment_count(), 0);
    assert_eq!(
        runtime.validate_window_access(namespace, departing_window),
        Err(XAuthorityRuntimeError::UnknownResource)
    );
    assert_eq!(
        runtime.validate_window_access(namespace, retained_window),
        Ok(())
    );
    assert_eq!(
        runtime.sync_counter(namespace, sync_counter),
        Err(XAuthorityRuntimeError::UnknownResource)
    );
}

#[test]
fn engine_size_control_updates_authority_geometry_without_consuming_client_generation() {
    let namespace = NamespaceId::from_raw(18);
    let window = XResourceId::new(0x62, 1);
    let surface = SurfaceId::new(18, 1);
    let mut runtime = XAuthorityRuntime::new();
    let created = runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(18),
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window,
            surface,
            geometry: Rect {
                x: 9,
                y: 11,
                width: 80,
                height: 40,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 5,
        },
    });
    assert_eq!(created.outcome, XAuthorityResponseOutcome::Accepted);

    assert_eq!(
        runtime
            .configure_window_size_from_engine(
                namespace,
                window,
                Size {
                    width: 120,
                    height: 70,
                },
            )
            .unwrap(),
        Rect {
            x: 9,
            y: 11,
            width: 120,
            height: 70,
        }
    );
    let draw = runtime.apply_core_draw(
        TransactionId::from_raw(19),
        namespace,
        window,
        Region::single(Rect {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        }),
    );
    assert_eq!(draw.transactions[0].previous_committed_generation, 5);
    assert_eq!(draw.transactions[0].target_geometry.width, 120);
    assert_eq!(draw.transactions[0].target_geometry.height, 70);
}

#[test]
fn cpu_buffer_submissions_use_stable_damage_generations_and_resize_replacement() {
    let namespace = NamespaceId::from_raw(19);
    let window = XResourceId::new(0x63, 1);
    let mut runtime = XAuthorityRuntime::new();
    runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(20),
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window,
            surface: SurfaceId::new(19, 1),
            geometry: Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 40,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    });

    runtime.apply_core_draw(
        TransactionId::from_raw(21),
        namespace,
        window,
        Region::single(Rect {
            x: 1,
            y: 1,
            width: 3,
            height: 3,
        }),
    );
    let first = runtime.take_cpu_buffer_update().unwrap();
    assert!(matches!(first, XAuthorityCpuBufferUpdate::Replace(_)));
    let first_handle = first.handle();
    runtime.apply_core_draw(
        TransactionId::from_raw(22),
        namespace,
        window,
        Region::single(Rect {
            x: 10,
            y: 10,
            width: 2,
            height: 2,
        }),
    );
    let second = runtime.take_cpu_buffer_update().unwrap();
    assert!(matches!(
        second,
        XAuthorityCpuBufferUpdate::PatchBatch(_)
    ));
    assert_eq!(second.handle(), first_handle);

    let mut materialized = std::collections::BTreeMap::new();
    first.apply_to(&mut materialized).unwrap();
    let first_bytes = materialized.get(&first_handle).unwrap().bytes.clone();
    assert_eq!(materialized.get(&first_handle).unwrap().generation, 1);
    second.apply_to(&mut materialized).unwrap();
    assert_eq!(materialized.len(), 1);
    assert_eq!(materialized.get(&first_handle).unwrap().generation, 2);
    assert_eq!(
        materialized.get(&first_handle).unwrap().bytes.len(),
        first_bytes.len()
    );

    runtime
        .configure_window_size_from_engine(
            namespace,
            window,
            Size {
                width: 120,
                height: 70,
            },
        )
        .unwrap();
    runtime.apply_core_draw(
        TransactionId::from_raw(23),
        namespace,
        window,
        Region::single(Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        }),
    );
    let replacement = runtime.take_cpu_buffer_update().unwrap();
    assert!(matches!(replacement, XAuthorityCpuBufferUpdate::Replace(_)));
    assert_ne!(replacement.handle(), second.handle());
    assert_eq!(replacement.generation(), 3);
    replacement.apply_to(&mut materialized).unwrap();
    let resized = materialized.get(&replacement.handle()).unwrap();
    assert_eq!(resized.size.width, 120);
    assert_eq!(resized.size.height, 70);
}

#[test]
fn descendant_software_drawing_reduces_to_its_toplevel_surface() {
    let namespace = NamespaceId::from_raw(20);
    let toplevel = XResourceId::new(0x70, 1);
    let child = XResourceId::new(0x71, 1);
    let toplevel_surface = SurfaceId::new(20, 1);
    let mut runtime = XAuthorityRuntime::new();
    for (transaction, window, surface, geometry) in [
        (
            30,
            toplevel,
            toplevel_surface,
            Rect {
                x: 40,
                y: 50,
                width: 100,
                height: 80,
            },
        ),
        (
            31,
            child,
            SurfaceId::new(21, 1),
            Rect {
                x: 5,
                y: 7,
                width: 90,
                height: 60,
            },
        ),
    ] {
        runtime.apply(XAuthorityRequestPacket {
            transaction: TransactionId::from_raw(transaction),
            namespace,
            kind: XAuthorityRequestKind::CreateWindow {
                window,
                surface,
                geometry,
                constraints: SurfaceConstraints {
                    min_size: None,
                    max_size: None,
                },
                generation: 1,
            },
        });
    }
    runtime
        .set_window_parent(namespace, child, toplevel)
        .unwrap();

    let response = runtime.apply_core_draw(
        TransactionId::from_raw(32),
        namespace,
        child,
        Region::single(Rect {
            x: 1,
            y: 2,
            width: 3,
            height: 4,
        }),
    );
    let update = runtime.take_cpu_buffer_update().unwrap();

    assert_eq!(response.transactions.len(), 1);
    assert_eq!(response.transactions[0].surface, toplevel_surface);
    assert_eq!(
        response.transactions[0].target_geometry,
        Rect {
            x: 40,
            y: 50,
            width: 100,
            height: 80,
        }
    );
    assert_eq!(
        response.transactions[0].damage,
        Region::single(Rect {
            x: 6,
            y: 9,
            width: 3,
            height: 4,
        })
    );
    let XAuthorityCpuBufferUpdate::Replace(snapshot) = update else {
        panic!("presentation composition must preserve immutable replacements");
    };
    assert_eq!(snapshot.drawable, toplevel);
    assert_eq!(
        snapshot.size,
        Size {
            width: 95,
            height: 67,
        }
    );
    assert_eq!(
        response.transactions[0].target_buffer,
        BufferSource::CpuBuffer {
            handle: snapshot.handle
        }
    );
}

#[test]
fn offscreen_pixmap_upload_survives_copy_into_presented_window() {
    let namespace = NamespaceId::from_raw(20);
    let window = XResourceId::new(0x64, 1);
    let pixmap = XResourceId::new(0x65, 1);
    let mut runtime = XAuthorityRuntime::new();
    runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(24),
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window,
            surface: SurfaceId::new(20, 1),
            geometry: Rect {
                x: 0,
                y: 0,
                width: 8,
                height: 8,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    });
    runtime
        .create_pixmap(
            namespace,
            pixmap,
            Size {
                width: 4,
                height: 4,
            },
            1,
        )
        .unwrap();

    let image = vec![0x7f; 4 * 4 * 4];
    let upload = runtime.apply_put_image(
        TransactionId::from_raw(25),
        namespace,
        pixmap,
        Region::single(Rect {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        }),
        Some(&image),
    );
    assert_eq!(upload.outcome, XAuthorityResponseOutcome::Accepted);
    assert!(upload.transactions.is_empty());
    assert!(runtime.take_cpu_buffer_update().is_none());

    let copy = runtime.apply_copy_area_with_gc(
        TransactionId::from_raw(26),
        namespace,
        pixmap,
        window,
        0,
        0,
        2,
        2,
        4,
        4,
        &XGraphicsContextValues::default(),
    );
    assert_eq!(copy.outcome, XAuthorityResponseOutcome::Accepted);
    assert_eq!(copy.transactions.len(), 1);
    let XAuthorityCpuBufferUpdate::Replace(snapshot) =
        runtime.take_cpu_buffer_update().expect("window copy update")
    else {
        panic!("first window copy must replace its CPU buffer");
    };
    assert_eq!(snapshot.drawable, window);
    assert!(snapshot.bytes.iter().any(|byte| *byte == 0x7f));
}

#[test]
fn software_present_materializes_pixmap_pixels_for_the_renderer() {
    let namespace = NamespaceId::from_raw(21);
    let window = XResourceId::new(0x66, 1);
    let pixmap = XResourceId::new(0x67, 1);
    let surface = SurfaceId::new(21, 1);
    let mut runtime = XAuthorityRuntime::new();
    runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(27),
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window,
            surface,
            geometry: Rect {
                x: 0,
                y: 0,
                width: 8,
                height: 8,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    });
    runtime
        .create_pixmap(
            namespace,
            pixmap,
            Size {
                width: 4,
                height: 4,
            },
            1,
        )
        .unwrap();
    let image = vec![0x5a; 4 * 4 * 4];
    assert_eq!(
        runtime
            .apply_put_image(
                TransactionId::from_raw(28),
                namespace,
                pixmap,
                Region::single(Rect {
                    x: 0,
                    y: 0,
                    width: 4,
                    height: 4,
                }),
                Some(&image),
            )
            .outcome,
        XAuthorityResponseOutcome::Accepted
    );

    let response = runtime.present_standard_pixmap(
        TransactionId::from_raw(29),
        namespace,
        window,
        pixmap,
        2,
        1,
        None,
        None,
    );

    assert_eq!(response.outcome, XAuthorityResponseOutcome::Accepted);
    assert_eq!(response.transactions.len(), 1);
    let XAuthorityCpuBufferUpdate::Replace(snapshot) = runtime
        .take_cpu_buffer_update()
        .expect("software Present must export immutable pixels")
    else {
        panic!("first software Present must replace the presentation buffer");
    };
    assert_eq!(snapshot.drawable, window);
    assert!(snapshot.bytes.iter().any(|byte| *byte == 0x5a));
    assert_eq!(
        response.transactions[0].target_buffer,
        BufferSource::CpuBuffer {
            handle: snapshot.handle
        }
    );
    assert_eq!(
        response.transactions[0].damage,
        Region::single(Rect {
            x: 2,
            y: 1,
            width: 4,
            height: 4,
        })
    );

    let changed = vec![0x6b; 4 * 4 * 4];
    runtime.apply_put_image(
        TransactionId::from_raw(30),
        namespace,
        pixmap,
        Region::single(Rect {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        }),
        Some(&changed),
    );
    let update_region = Region {
        rects: vec![
            Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            Rect {
                x: 3,
                y: 3,
                width: 1,
                height: 1,
            },
        ],
    };
    let response = runtime.present_standard_pixmap(
        TransactionId::from_raw(31),
        namespace,
        window,
        pixmap,
        2,
        1,
        None,
        Some(update_region),
    );
    let XAuthorityCpuBufferUpdate::PatchBatch(batch) = runtime
        .take_cpu_buffer_update()
        .expect("later software Present must export damage patches")
    else {
        panic!("later software Present must retain the presentation handle");
    };
    assert_eq!(batch.handle, snapshot.handle);
    assert_eq!(batch.generation, 2);
    assert_eq!(
        batch
            .patches
            .iter()
            .map(|patch| patch.rect)
            .collect::<Vec<_>>(),
        vec![
            Rect {
                x: 2,
                y: 1,
                width: 1,
                height: 1,
            },
            Rect {
                x: 5,
                y: 4,
                width: 1,
                height: 1,
            },
        ]
    );
    assert_eq!(response.transactions[0].damage.rects, batch.patches.iter().map(|patch| {
        Rect {
            x: patch.rect.x,
            y: patch.rect.y,
            width: patch.rect.width,
            height: patch.rect.height,
        }
    }).collect::<Vec<_>>());
    let mut materialized = std::collections::BTreeMap::new();
    XAuthorityCpuBufferUpdate::Replace(snapshot.clone())
        .apply_to(&mut materialized)
        .unwrap();
    XAuthorityCpuBufferUpdate::PatchBatch(batch)
        .apply_to(&mut materialized)
        .unwrap();
    let materialized = materialized.get(&snapshot.handle).unwrap();
    assert_eq!(materialized.generation, 2);
    assert!(materialized.bytes.iter().any(|byte| *byte == 0x5a));
    assert!(materialized.bytes.iter().any(|byte| *byte == 0x6b));
}

#[test]
fn software_present_rejects_pixmap_without_materialized_pixels() {
    let namespace = NamespaceId::from_raw(22);
    let window = XResourceId::new(0x68, 1);
    let pixmap = XResourceId::new(0x69, 1);
    let mut runtime = XAuthorityRuntime::new();
    runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(30),
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window,
            surface: SurfaceId::new(22, 1),
            geometry: Rect {
                x: 0,
                y: 0,
                width: 8,
                height: 8,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    });
    runtime
        .create_pixmap(
            namespace,
            pixmap,
            Size {
                width: 4,
                height: 4,
            },
            1,
        )
        .unwrap();

    let response = runtime.present_standard_pixmap(
        TransactionId::from_raw(31),
        namespace,
        window,
        pixmap,
        0,
        0,
        None,
        None,
    );

    assert_eq!(
        response.outcome,
        XAuthorityResponseOutcome::Rejected(XAuthorityRuntimeError::InvalidResource)
    );
    assert!(response.transactions.is_empty());
    assert!(runtime.take_cpu_buffer_update().is_none());
}
