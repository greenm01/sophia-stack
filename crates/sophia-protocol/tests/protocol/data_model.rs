#[test]
fn simple_ids_start_after_zero() {
    let mut alloc = IdAllocator::<NamespaceId>::new();
    let first = alloc.next_id();
    let second = alloc.next_id();

    assert!(first.is_valid());
    assert_eq!(first.raw(), 1);
    assert_eq!(second.raw(), 2);
}

#[test]
fn buffer_and_fence_handles_are_typed_and_nonzero() {
    let mut buffers = IdAllocator::<BufferHandle>::new();
    let mut fences = IdAllocator::<FenceHandle>::new();

    assert_eq!(buffers.next_id().raw(), 1);
    assert_eq!(fences.next_id().raw(), 1);
    assert!(!BufferHandle::INVALID.is_valid());
    assert!(!FenceHandle::INVALID.is_valid());
}

#[test]
fn dma_buf_descriptor_validation_is_bounded() {
    let valid = DmaBufDescriptor {
        handle: BufferHandle::from_raw(7),
        size: Size {
            width: 640,
            height: 480,
        },
        format: DRM_FORMAT_XRGB8888,
        modifier: DRM_FORMAT_MOD_INVALID,
        plane_count: 1,
        planes: [
            Some(DmaBufPlaneDescriptor {
                offset: 0,
                stride: 2560,
            }),
            None,
            None,
            None,
        ],
    };
    assert_eq!(valid.validate(), Ok(()));

    assert_eq!(
        DmaBufDescriptor {
            plane_count: 0,
            ..valid
        }
        .validate(),
        Err(DmaBufDescriptorError::InvalidPlaneCount)
    );
    assert_eq!(
        DmaBufDescriptor {
            planes: [
                Some(DmaBufPlaneDescriptor {
                    offset: 0,
                    stride: 64,
                }),
                None,
                None,
                None,
            ],
            ..valid
        }
        .validate(),
        Err(DmaBufDescriptorError::InvalidStride)
    );
    assert_eq!(
        DmaBufDescriptor {
            size: Size {
                width: DMA_BUF_MAX_DIMENSION + 1,
                height: 1,
            },
            ..valid
        }
        .validate(),
        Err(DmaBufDescriptorError::InvalidSize)
    );
}

#[test]
fn namespace_capabilities_are_directional_and_bounded() {
    let capabilities = NamespaceCapabilities::NONE
        .with_request(NamespacePortalCapability::Clipboard)
        .with_publish(NamespacePortalCapability::Notification);

    assert!(capabilities.allows_request(NamespacePortalCapability::Clipboard));
    assert!(!capabilities.allows_publish(NamespacePortalCapability::Clipboard));
    assert!(capabilities.allows_publish(NamespacePortalCapability::Notification));
    assert!(!capabilities.allows_request(NamespacePortalCapability::Notification));
    assert_eq!(
        NamespaceCapabilities::from_bits(capabilities.request_bits(), capabilities.publish_bits()),
        Some(capabilities)
    );
    assert_eq!(NamespaceCapabilities::from_bits(1 << 63, 0), None);
}

#[test]
fn every_portal_kind_maps_to_its_explicit_namespace_capability() {
    let mappings = [
        (
            PortalTransferKind::Clipboard,
            NamespacePortalCapability::Clipboard,
        ),
        (
            PortalTransferKind::DragAndDrop,
            NamespacePortalCapability::DragAndDrop,
        ),
        (
            PortalTransferKind::FileHandoff,
            NamespacePortalCapability::FileHandoff,
        ),
        (
            PortalTransferKind::ScreenCapture,
            NamespacePortalCapability::ScreenCapture,
        ),
        (
            PortalTransferKind::ScreenRecording,
            NamespacePortalCapability::ScreenRecording,
        ),
        (
            PortalTransferKind::UriOpen,
            NamespacePortalCapability::UriOpen,
        ),
        (
            PortalTransferKind::Notification,
            NamespacePortalCapability::Notification,
        ),
    ];

    for (kind, capability) in mappings {
        assert_eq!(kind.capability(), capability);
    }
}

#[test]
fn namespace_and_admission_contexts_reject_invalid_identity() {
    assert_eq!(
        NamespaceContext::new(
            NamespaceId::INVALID,
            NamespaceProfile::Confined,
            NamespaceCapabilities::NONE,
        ),
        None
    );
    assert_eq!(
        ClientAuthProvenance::new(ClientAuthenticationMethod::MitMagicCookie1, 0),
        None
    );

    let namespace = NamespaceContext::new(
        NamespaceId::from_raw(9),
        NamespaceProfile::ClassicShared,
        NamespaceCapabilities::ALL,
    )
    .unwrap();
    let provenance =
        ClientAuthProvenance::new(ClientAuthenticationMethod::MitMagicCookie1, 4).unwrap();
    let admission =
        ClientAdmissionContext::new(ClientAdmissionId::from_raw(12), namespace, provenance)
            .unwrap();

    assert!(admission.is_valid());
    assert_eq!(admission.namespace.profile, NamespaceProfile::ClassicShared);
    assert_eq!(admission.auth_provenance.session_generation, 4);
    assert_eq!(
        ClientAdmissionContext::new(ClientAdmissionId::INVALID, namespace, provenance),
        None
    );
}

#[test]
fn foreign_xids_keep_generation() {
    let id = XWindowId::new(0x1200042, 7);

    assert!(id.is_valid());
    assert_eq!(id.xid(), 0x1200042);
    assert_eq!(id.generation(), 7);
}

#[test]
fn region_drops_empty_rectangles() {
    let mut region = Region::empty();
    region.push(Rect {
        x: 0,
        y: 0,
        width: 0,
        height: 10,
    });
    region.push(Rect {
        x: 1,
        y: 2,
        width: 3,
        height: 4,
    });

    assert_eq!(region.rects.len(), 1);
}

#[test]
fn stale_surface_id_fails_closed() {
    let mut table = SurfaceTable::new();
    let first = table.insert("first");

    assert_eq!(table.remove(first), Ok("first"));

    let second = table.insert("second");

    assert_ne!(first, second);
    assert_eq!(table.get(first), None);
    assert_eq!(table.get(second), Some(&"second"));
}

/// A committed raster keeps the size it was drawn at, not the size it is
/// placed at.
///
/// These part company for the whole window between a configure and the
/// client's redraw. Deriving one from the other reported a size no producer had
/// published, and a live session ended when the compositor compared that
/// invention against the buffer it actually held: planned 1920x1080, held
/// 1280x1440, one DMA-BUF.
#[test]
fn committed_state_keeps_the_raster_size_not_the_placement() {
    let layer = LayerSnapshot {
        surface: SurfaceId::new(6, 1),
        authority_local_id: None,
        namespace: None,
        stack_rank: 0,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
        source: BufferSource::DmaBuf { handle: 7 },
        source_size: Size {
            width: 1280,
            height: 1440,
        },
        damage: Region::empty(),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 3,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    };

    let state = CommittedSurfaceState::from_layer_snapshot(&layer);

    assert_eq!(
        state.content.canonical_variant().pixel_size,
        Size {
            width: 1280,
            height: 1440,
        }
    );
    assert_eq!(state.geometry, layer.geometry);
    assert_eq!(
        layer
            .to_surface_transaction(
                TransactionId::from_raw(9),
                AuthorityKind::SophiaX,
                SurfaceTransactionReadiness::Ready,
                250,
                2,
            )
            .target_content_size(),
        Size {
            width: 1280,
            height: 1440,
        }
    );
}

#[test]
fn layer_snapshot_is_cloneable_frame_data() {
    let surface = SurfaceId::new(0, 1);
    let snapshot = LayerSnapshot {
        surface,
        authority_local_id: Some(AuthorityLocalId::new(42, 1)),
        namespace: Some(NamespaceId::from_raw(1)),
        stack_rank: 0,
        geometry: Rect {
            x: 10,
            y: 20,
            width: 640,
            height: 480,
        },
        source: BufferSource::XPixmap { pixmap: 99 },
        source_size: Size {
            width: 640,
            height: 480,
        },
        damage: Region::single(Rect {
            x: 10,
            y: 20,
            width: 10,
            height: 10,
        }),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 3,
        resize_sync: ResizeSyncCapability::ExplicitSync,
    };

    let cloned = snapshot.clone();

    assert_eq!(cloned.surface, surface);
    assert_eq!(cloned.damage.rects.len(), 1);
    assert_eq!(cloned.resize_sync, ResizeSyncCapability::ExplicitSync);
}

#[test]
fn authority_local_id_preserves_raw_id_and_generation() {
    let local = AuthorityLocalId::from(XWindowId::new(0x1200042, 7));

    assert!(local.is_valid());
    assert_eq!(local.raw(), 0x1200042);
    assert_eq!(local.generation(), 7);
}

#[test]
fn authority_surface_carries_protocol_ownership_without_metadata() {
    let surface = SurfaceSnapshot {
        surface: SurfaceId::new(3, 1),
        window: XWindowId::new(0x42, 5),
        toplevel: None,
        client: None,
        namespace: Some(NamespaceId::from_raw(2)),
        mapped: true,
        stack_rank: 4,
        geometry: Rect {
            x: 10,
            y: 20,
            width: 640,
            height: 480,
        },
        source: BufferSource::XPixmap { pixmap: 77 },
        damage: Region::empty(),
        generation: 9,
        resize_sync: ResizeSyncCapability::ExplicitSync,
    };

    let authority_surface = surface.to_authority_surface(AuthorityKind::SophiaX);

    assert_eq!(authority_surface.authority, AuthorityKind::SophiaX);
    assert_eq!(authority_surface.local_id, AuthorityLocalId::new(0x42, 5));
    assert_eq!(authority_surface.surface, SurfaceId::new(3, 1));
    assert_eq!(authority_surface.namespace, Some(NamespaceId::from_raw(2)));
    assert!(authority_surface.mapped);
    assert_eq!(authority_surface.generation, 9);
}

#[test]
fn surface_transaction_carries_atomic_geometry_buffer_and_readiness() {
    let layer = LayerSnapshot {
        surface: SurfaceId::new(4, 1),
        authority_local_id: Some(AuthorityLocalId::new(0x99, 2)),
        namespace: Some(NamespaceId::from_raw(8)),
        stack_rank: 0,
        geometry: Rect {
            x: 30,
            y: 40,
            width: 800,
            height: 600,
        },
        source: BufferSource::DmaBuf { handle: 55 },
        source_size: Size {
            width: 800,
            height: 600,
        },
        damage: Region::single(Rect {
            x: 30,
            y: 40,
            width: 10,
            height: 10,
        }),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 6,
        resize_sync: ResizeSyncCapability::ExplicitSync,
    };

    let transaction = SurfaceTransaction::from_layer_snapshot(
        TransactionId::from_raw(12),
        AuthorityKind::SophiaX,
        &layer,
        SurfaceTransactionReadiness::Ready,
        250,
        5,
    );

    assert_eq!(transaction.transaction, TransactionId::from_raw(12));
    assert_eq!(transaction.authority, AuthorityKind::SophiaX);
    assert_eq!(transaction.surface, SurfaceId::new(4, 1));
    assert_eq!(transaction.target_geometry.width, 800);
    assert_eq!(
        transaction.target_content_size(),
        Size {
            width: 800,
            height: 600,
        }
    );
    assert_eq!(
        transaction.target_buffer(),
        BufferSource::DmaBuf { handle: 55 }
    );
    assert_eq!(transaction.damage.rects.len(), 1);
    assert_eq!(transaction.readiness, SurfaceTransactionReadiness::Ready);
    assert_eq!(transaction.previous_committed_generation, 5);
}

#[test]
fn committed_surface_state_is_cloneable_visual_state() {
    let layer = LayerSnapshot {
        surface: SurfaceId::new(5, 1),
        authority_local_id: None,
        namespace: None,
        stack_rank: 0,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 320,
            height: 240,
        },
        source: BufferSource::CpuBuffer { handle: 3 },
        source_size: Size {
            width: 320,
            height: 240,
        },
        damage: Region::empty(),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 11,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    };

    let state = CommittedSurfaceState::from_layer_snapshot(&layer);
    let cloned = state.clone();

    assert_eq!(cloned.surface, SurfaceId::new(5, 1));
    assert_eq!(cloned.committed_generation, 11);
    assert_eq!(cloned.geometry.width, 320);
    assert_eq!(cloned.buffer(), BufferSource::CpuBuffer { handle: 3 });
}

#[test]
fn layout_node_snapshot_carries_only_opaque_policy_data() {
    let node = LayoutNodeSnapshot {
        surface: SurfaceId::new(7, 1),
        workspace: WorkspaceId::from_raw(2),
        kind: LayoutNodeKind::Toplevel,
        placement_preference: SurfacePlacementPreference::Default,
        transient_owner: None,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        state: LayoutNodeState::NORMAL,
        constraints: SurfaceConstraints {
            min_size: Some(Size {
                width: 320,
                height: 200,
            }),
            max_size: None,
        },
        geometry: Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        },
        generation: 3,
    };

    assert_eq!(node.surface, SurfaceId::new(7, 1));
    assert_eq!(node.workspace, WorkspaceId::from_raw(2));
    assert!(node.capabilities.resizable);
    assert!(node.state.visible);
}

#[test]
fn chrome_descriptor_carries_redacted_metadata_separately() {
    let chrome = ChromeDescriptor {
        surface: SurfaceId::new(9, 1),
        label: Some(DisplayLabel {
            text: "Private Window".to_owned(),
            redacted: true,
        }),
        icon: Some(IconTokenId::from_raw(4)),
        trust_level: TrustLevel::Untrusted,
        attention: AttentionState::Notice,
        generation: 1,
    };

    assert_eq!(chrome.surface, SurfaceId::new(9, 1));
    assert_eq!(
        chrome.label.as_ref().map(|label| label.redacted),
        Some(true)
    );
    assert_eq!(chrome.icon, Some(IconTokenId::from_raw(4)));
}

#[test]
fn broker_health_packet_accepts_bounded_status_message() {
    let packet = BrokerHealthPacket::new(
        BrokerKind::Portal,
        BrokerHealthState::Ready,
        3,
        Some("ready".to_owned()),
    )
    .unwrap();

    assert_eq!(packet.broker, BrokerKind::Portal);
    assert_eq!(packet.state, BrokerHealthState::Ready);
    assert_eq!(packet.generation, 3);
    assert_eq!(packet.message.as_deref(), Some("ready"));
}

#[test]
fn broker_health_packet_accepts_empty_status_message() {
    let packet =
        BrokerHealthPacket::new(BrokerKind::Metadata, BrokerHealthState::Starting, 1, None)
            .unwrap();

    assert_eq!(packet.broker, BrokerKind::Metadata);
    assert_eq!(packet.state, BrokerHealthState::Starting);
    assert_eq!(packet.message, None);
}

#[test]
fn broker_health_packet_rejects_unbounded_status_message() {
    let message = "x".repeat(SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN + 1);

    assert_eq!(
        BrokerHealthPacket::new(
            BrokerKind::Portal,
            BrokerHealthState::Degraded,
            4,
            Some(message)
        ),
        Err(BrokerHealthError::MessageTooLong {
            len: SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN + 1,
            max: SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN,
        })
    );
}

#[test]
fn a_surface_without_a_rule_discloses_nothing() {
    // The default is load-bearing, not a convenience. A surface the broker has not
    // ruled on must not leak a title because some code path forgot to set a level.
    assert_eq!(MetadataDisclosure::default(), MetadataDisclosure::None);
    assert!(!MetadataDisclosure::None.discloses_text());
    assert!(MetadataDisclosure::ClassOnly.discloses_text());
    assert!(MetadataDisclosure::Full.discloses_text());
}

#[test]
fn disclosure_levels_order_from_least_to_most_revealing() {
    // Ordered so a policy that wants "at most this much" can compare rather than
    // enumerate, and so adding a value in the wrong position fails a test instead of
    // silently widening what some comparison admits.
    assert!(MetadataDisclosure::None < MetadataDisclosure::ClassOnly);
    assert!(MetadataDisclosure::ClassOnly < MetadataDisclosure::Full);
}

#[test]
fn a_reduced_candidate_carries_its_level_beside_its_label() {
    // "No title" and "not permitted to tell you the title" are different facts. A
    // receiver that had to infer the second from an absent label would guess wrong
    // for every untitled window.
    let withheld = ReducedMetadataCandidate {
        surface: SurfaceId::new(9, 1),
        label: None,
        disclosure: MetadataDisclosure::None,
        generation: 3,
    };
    let untitled = ReducedMetadataCandidate {
        surface: SurfaceId::new(9, 1),
        label: None,
        disclosure: MetadataDisclosure::Full,
        generation: 3,
    };

    assert_ne!(withheld, untitled);
    assert_eq!(withheld.label, untitled.label);
}

#[test]
fn one_label_bound_is_shared_by_every_hop() {
    // The authority reduces to this bound and Engine validates against it. Two
    // copies would let a label be valid where it is produced and rejected where it
    // is stored.
    assert_eq!(MAX_CHROME_LABEL_LEN, 128);
}

fn content_variant(variant: u32, density_millis: u32) -> SurfaceContentVariant {
    let width = i32::try_from((640_u64 * u64::from(density_millis) + 999) / 1_000).unwrap_or(0);
    let height = i32::try_from((480_u64 * u64::from(density_millis) + 999) / 1_000).unwrap_or(0);
    SurfaceContentVariant {
        variant,
        source: BufferSource::CpuBuffer {
            handle: u64::from(variant),
        },
        pixel_size: Size { width, height },
        density_millis,
        transform: SurfaceRasterTransform::Normal,
        fidelity: SurfaceContentFidelity::AuthorityRaster,
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width,
            height,
        }),
    }
}

#[test]
fn surface_content_set_construction_enforces_its_bounds() {
    let extent = Size {
        width: 640,
        height: 480,
    };

    assert_eq!(
        SurfaceContentSet::new(extent, Vec::new()),
        Err(SurfaceContentSetError::EmptyVariantSet)
    );
    assert_eq!(
        SurfaceContentSet::new(
            extent,
            (1..=u32::try_from(MAX_SURFACE_CONTENT_VARIANTS + 1).unwrap())
                .map(|index| content_variant(index, index * 500))
                .collect(),
        ),
        Err(SurfaceContentSetError::VariantCapacityExceeded {
            count: MAX_SURFACE_CONTENT_VARIANTS + 1
        })
    );
    assert_eq!(
        SurfaceContentSet::new(extent, vec![content_variant(0, 1_000)]),
        Err(SurfaceContentSetError::InvalidVariantIdentity)
    );
    assert_eq!(
        SurfaceContentSet::new(extent, vec![content_variant(1, 0)]),
        Err(SurfaceContentSetError::InvalidDensity { variant: 1 })
    );
    assert_eq!(
        SurfaceContentSet::new(
            extent,
            vec![content_variant(1, 1_000), content_variant(1, 2_000)],
        ),
        Err(SurfaceContentSetError::DuplicateVariantIdentity { variant: 1 })
    );
    assert_eq!(
        SurfaceContentSet::new(
            extent,
            vec![content_variant(1, 1_000), content_variant(2, 1_000)],
        ),
        Err(SurfaceContentSetError::DuplicateRasterClass {
            density_millis: 1_000,
            transform: SurfaceRasterTransform::Normal,
        })
    );
}

#[test]
fn surface_content_set_canonical_variant_prefers_identity_density() {
    let extent = Size {
        width: 640,
        height: 480,
    };
    let set = SurfaceContentSet::new(
        extent,
        vec![
            content_variant(3, 2_000),
            content_variant(1, 1_000),
            content_variant(2, 750),
        ],
    )
    .unwrap();

    assert_eq!(set.canonical_variant().variant, 1);
    assert_eq!(
        set.canonical_source(),
        BufferSource::CpuBuffer { handle: 1 }
    );

    // Without an exact 1x variant, the nearest density wins and ties break on
    // the stable variant identity.
    let set = SurfaceContentSet::new(
        extent,
        vec![content_variant(5, 1_250), content_variant(4, 750)],
    )
    .unwrap();
    assert_eq!(set.canonical_variant().variant, 4);
}

#[test]
fn surface_content_singleton_normalizes_one_identity_raster() {
    let extent = Size {
        width: 320,
        height: 200,
    };
    let set = SurfaceContentSet::singleton(BufferSource::DmaBuf { handle: 9 }, extent);

    assert_eq!(set.logical_extent(), extent);
    assert_eq!(set.variants().len(), 1);
    assert_eq!(set.variants()[0].variant, 1);
    assert_eq!(set.variants()[0].pixel_size, extent);
    assert_eq!(
        set.variants()[0].density_millis,
        SURFACE_CONTENT_DENSITY_1X_MILLIS
    );
    assert_eq!(set.canonical_source(), BufferSource::DmaBuf { handle: 9 });
}

#[test]
fn dma_buf_pairing_covers_every_variant_of_a_content_set() {
    let geometry = Rect {
        x: 0,
        y: 0,
        width: 640,
        height: 480,
    };
    let extent = Size {
        width: 640,
        height: 480,
    };
    let mut transaction = SurfaceTransaction {
        transaction: TransactionId::from_raw(7),
        authority: AuthorityKind::SophiaX,
        surface: SurfaceId::new(1, 1),
        namespace: None,
        target_geometry: geometry,
        content: SurfaceContentSet::new(
            extent,
            vec![
                content_variant(1, 1_000),
                SurfaceContentVariant {
                    variant: 2,
                    source: BufferSource::DmaBuf { handle: 44 },
                    pixel_size: Size {
                        width: 1_280,
                        height: 960,
                    },
                    density_millis: 2_000,
                    transform: SurfaceRasterTransform::Normal,
                    fidelity: SurfaceContentFidelity::AuthorityRaster,
                    damage: Region::single(Rect {
                        x: 0,
                        y: 0,
                        width: 1_280,
                        height: 960,
                    }),
                },
            ],
        )
        .unwrap(),
        damage: Region::empty(),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };
    let present = DmaBufPresentKey {
        transaction: TransactionId::from_raw(7),
        surface: SurfaceId::new(1, 1),
        buffer: BufferHandle::from_raw(44),
    };

    // A DMA-BUF variant anywhere in the set requires its exact Present pair,
    // even when the canonical variant is a CPU raster.
    assert!(!dma_buf_present_pairs_are_exact(
        std::slice::from_ref(&transaction),
        &[]
    ));
    assert!(dma_buf_present_pairs_are_exact(
        std::slice::from_ref(&transaction),
        std::slice::from_ref(&present)
    ));

    transaction.content = SurfaceContentSet::singleton(BufferSource::CpuBuffer { handle: 1 }, extent);
    assert!(!dma_buf_present_pairs_are_exact(
        std::slice::from_ref(&transaction),
        std::slice::from_ref(&present)
    ));
}

#[test]
fn surface_raster_requirements_are_bounded_protocol_neutral_classes() {
    let requirements = SurfaceRasterRequirements {
        surface: SurfaceId::new(4, 1),
        committed_content_generation: 7,
        requirement_generation: 9,
        logical_extent: Size {
            width: 640,
            height: 480,
        },
        classes: vec![
            SurfaceRasterClass {
                density_millis: 750,
                transform: SurfaceRasterTransform::Normal,
            },
            SurfaceRasterClass {
                density_millis: 1_000,
                transform: SurfaceRasterTransform::Normal,
            },
        ],
    };
    assert_eq!(requirements.validate(), Ok(()));

    let mut duplicate = requirements.clone();
    duplicate.classes.push(duplicate.classes[0]);
    assert_eq!(
        duplicate.validate(),
        Err(SurfaceRasterRequirementsError::DuplicateClass)
    );
}
