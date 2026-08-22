use sophia_runtime::{
    ProtectionDomainRole, ProtectionDomainSpec, ProtectionDomainSpecError, ProtectionNetworkAccess,
    ProtectionPath,
};

#[test]
fn wm_cannot_share_a_domain_with_metadata_roles() {
    for conflicting in [
        ProtectionDomainRole::MetadataShell,
        ProtectionDomainRole::MetadataBroker,
        ProtectionDomainRole::PortalBroker,
        ProtectionDomainRole::ApplicationFrontend,
    ] {
        assert_eq!(
            ProtectionDomainSpec::bubblewrap([ProtectionDomainRole::SpatialPolicy, conflicting,]),
            Err(ProtectionDomainSpecError::ForbiddenRoleComposition {
                spatial_policy: ProtectionDomainRole::SpatialPolicy,
                conflicting,
            })
        );
    }
}

#[test]
fn path_grants_reject_root_aliases_and_overlapping_destinations() {
    assert_eq!(
        ProtectionDomainSpec::bubblewrap([ProtectionDomainRole::MetadataBroker])
            .unwrap()
            .path(ProtectionPath::read_only("/tmp/../")),
        Err(ProtectionDomainSpecError::NonNormalizedPath(
            "/tmp/../".into()
        ))
    );

    let domain = ProtectionDomainSpec::bubblewrap([ProtectionDomainRole::MetadataBroker])
        .unwrap()
        .path(ProtectionPath::read_only("/run/sophia/metadata.sock"))
        .unwrap();
    assert_eq!(
        domain.path(ProtectionPath::read_write("/run/sophia")),
        Err(ProtectionDomainSpecError::OverlappingDestination {
            existing: "/run/sophia/metadata.sock".into(),
            requested: "/run/sophia".into(),
        })
    );
}

#[test]
fn wm_may_hold_the_output_role_in_its_own_domain() {
    let spec = ProtectionDomainSpec::bubblewrap([
        ProtectionDomainRole::SpatialPolicy,
        ProtectionDomainRole::OutputAuthority,
    ])
    .unwrap();
    assert!(spec.roles().contains(&ProtectionDomainRole::SpatialPolicy));
    assert!(
        spec.roles()
            .contains(&ProtectionDomainRole::OutputAuthority)
    );
    assert_eq!(spec.network(), ProtectionNetworkAccess::Denied);
}
