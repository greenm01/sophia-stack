use super::*;
use std::collections::BTreeMap;

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
