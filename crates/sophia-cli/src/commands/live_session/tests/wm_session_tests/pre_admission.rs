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
        presentation_extent: Size {
            width: (geometry).width,
            height: (geometry).height,
        },
        content: sophia_protocol::SurfaceContentSet::singleton(BufferSource::DmaBuf { handle: 44 }, sophia_protocol::Size {
            width: geometry.width,
            height: geometry.height,
        }),

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
    let client = sophia_x_authority::XServerFrontendClientId::from_raw(1);
    batch.client = Some(client);
    add_test_surface_route(&mut batch, surface, client);
    batch.surface_presentations.push(
        sophia_x_authority::XAuthoritySurfacePresentationObservation {
            surface,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            kind: sophia_protocol::LayoutNodeKind::Toplevel,
            placement_preference: sophia_protocol::SurfacePlacementPreference::Default,
            stack_rank: 0,
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
            surface_kind: sophia_protocol::LayoutNodeKind::Toplevel,
            placement_preference: sophia_protocol::SurfacePlacementPreference::Default,
            presentation_owner: None,
            stack_rank: 0,
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
