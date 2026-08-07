#[test]
fn map_deferral_requires_an_external_policy_owner() {
    let direct = LivePolicyMapMode::from_external_wm(false);
    assert!(!direct.frontend_deferred());
    assert!(direct.bypass_engine_admission());
    assert!(direct.engine_owns_initial_placement());

    let deferred = LivePolicyMapMode::from_external_wm(true);
    assert!(deferred.frontend_deferred());
    assert!(!deferred.bypass_engine_admission());
    assert!(!deferred.engine_owns_initial_placement());
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
    let transaction = TransactionId::from_raw(111);
    let batch = direct_map_batch(surface, transaction, geometry, 111);
    let output = Size {
        width: 2560,
        height: 1440,
    };
    let mut layout = PersistentLiveLayout::new(LivePolicyMapMode::Direct, output);
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
    assert_eq!(
        layout.layers.get(&surface).unwrap().geometry,
        center_geometry_without_scaling(geometry, output)
    );
    assert_eq!(projected.transactions.len(), 1);
    assert!(released.is_empty());
    assert!(layout.pre_admission_groups.is_empty());
}

#[test]
fn no_wm_session_keeps_first_toplevel_chrome_inside_the_output() {
    let surface = SurfaceId::new(56, 1);
    let output = Size {
        width: 2560,
        height: 1440,
    };
    let content = Rect {
        x: 0,
        y: 0,
        width: 2556,
        height: 1422,
    };
    let transaction = TransactionId::from_raw(112);
    let batch = direct_map_batch(surface, transaction, content, 112);
    let mut layout = PersistentLiveLayout::new(LivePolicyMapMode::Direct, output);
    layout.cpu_buffer_sizes.insert(
        112,
        Size {
            width: content.width,
            height: content.height,
        },
    );

    layout.observe_authority_batch(&batch);

    assert_eq!(
        layout.layers.get(&surface).unwrap().geometry,
        Rect {
            x: 2,
            y: 9,
            ..content
        }
    );
    let chrome = sophia_engine::SurfaceChromeStyle {
        focus_ring: sophia_engine::FocusRingStyle {
            width: 2,
            ..sophia_engine::FocusRingStyle::default()
        },
        ..sophia_engine::SurfaceChromeStyle::default()
    };
    let outer = sophia_engine::outer_surface_geometry(
        layout.layers.get(&surface).unwrap().geometry,
        chrome,
    )
    .unwrap();
    assert_eq!(
        outer,
        Rect {
            x: 0,
            y: 7,
            width: output.width,
            height: 1426,
        }
    );
    assert!(outer.y + outer.height <= output.height);
    assert_eq!(batch.transactions[0].target_geometry, content);
}

fn direct_map_batch(
    surface: SurfaceId,
    transaction: TransactionId,
    geometry: Rect,
    cpu_buffer: u64,
) -> sophia_x_authority::XAuthorityObservedTransactionBatch {
    let mut batch = crate::commands::live_session::wm_update_coordinator_batch(transaction);
    batch
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
    batch.transactions.push(SurfaceTransaction {
        transaction,
        authority: sophia_protocol::AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry: geometry,
        target_content_size: Size {
            width: geometry.width,
            height: geometry.height,
        },
        target_buffer: BufferSource::CpuBuffer { handle: cpu_buffer },
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
    batch
}
