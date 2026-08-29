use super::*;
use std::collections::{BTreeMap, BTreeSet};

fn action(
    token: u64,
    issuer_epoch: u64,
    revocation_epoch: u64,
    generation: u64,
) -> sophia_protocol::ToplevelActionCapabilityRef {
    sophia_protocol::ToplevelActionCapabilityRef {
        token,
        issuer_epoch,
        issuer_revocation_epoch: revocation_epoch,
        recipient_epoch: 7,
        target_slot: 3,
        target_generation: generation,
    }
}

#[test]
fn broker_dispatch_requires_the_exact_current_issuer_tuple() {
    let surface = SurfaceId::new(41, 2);
    let mut descriptors = sophia_engine::ChromeDescriptorTable::default();
    descriptors.upsert(sophia_protocol::ChromeDescriptor {
        surface,
        label: Some(sophia_protocol::DisplayLabel {
            text: "Terminal".to_owned(),
            redacted: false,
        }),
        icon: None,
        trust_level: sophia_protocol::TrustLevel::Trusted,
        attention: sophia_protocol::AttentionState::None,
        generation: 9,
    });
    let grants = BTreeMap::from([(
        surface,
        sophia_protocol::BrokerToplevelActionGrant {
            token: 11,
            revocation_epoch: 5,
            target_generation: 9,
        },
    )]);

    assert_eq!(
        resolve_live_broker_toplevel_action(4, &grants, &descriptors, action(11, 4, 5, 9)),
        Some(surface)
    );
    for stale in [
        action(12, 4, 5, 9),
        action(11, 3, 5, 9),
        action(11, 4, 4, 9),
        action(11, 4, 5, 8),
    ] {
        assert_eq!(
            resolve_live_broker_toplevel_action(4, &grants, &descriptors, stale),
            None
        );
    }
}

#[test]
fn descriptor_generation_change_revokes_an_old_presented_action() {
    let surface = SurfaceId::new(41, 2);
    let mut descriptors = sophia_engine::ChromeDescriptorTable::default();
    descriptors.upsert(sophia_protocol::ChromeDescriptor {
        surface,
        label: None,
        icon: None,
        trust_level: sophia_protocol::TrustLevel::Unknown,
        attention: sophia_protocol::AttentionState::Notice,
        generation: 10,
    });
    let grants = BTreeMap::from([(
        surface,
        sophia_protocol::BrokerToplevelActionGrant {
            token: 11,
            revocation_epoch: 5,
            target_generation: 10,
        },
    )]);

    assert_eq!(
        resolve_live_broker_toplevel_action(4, &grants, &descriptors, action(11, 4, 5, 9)),
        None
    );
}

#[test]
fn switcher_admits_only_presented_policy_managed_surfaces() {
    let managed = SurfaceId::new(41, 2);
    let popup = SurfaceId::new(42, 2);
    let hidden = SurfaceId::new(43, 2);
    let layer = |surface| LayerSnapshot {
        surface,
        authority_local_id: None,
        namespace: None,
        stack_rank: 0,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 60,
        },
        source: BufferSource::None,
        source_size: Size {
            width: 80,
            height: 60,
        },
        damage: Region::empty(),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 1,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    };
    let layers = BTreeMap::from([(managed, layer(managed)), (popup, layer(popup))]);
    let roles = BTreeMap::from([
        (
            managed,
            sophia_protocol::SurfacePresentationRole::PolicyManaged,
        ),
        (
            popup,
            sophia_protocol::SurfacePresentationRole::ClientPositioned,
        ),
        (
            hidden,
            sophia_protocol::SurfacePresentationRole::PolicyManaged,
        ),
    ]);

    assert_eq!(
        live_shell_activation_surfaces(&layers, &roles),
        BTreeSet::from([managed])
    );
}
