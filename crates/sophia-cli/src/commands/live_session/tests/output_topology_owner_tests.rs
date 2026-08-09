use super::super::{LiveOutputTopologyOwner, LiveOutputTopologyPhase, LiveOutputTopologyRebuild};
use sophia_protocol::{OutputId, Size};

fn output(raw: u64, width: i32) -> sophia_engine::HeadlessOutput {
    sophia_engine::HeadlessOutput {
        id: OutputId::from_raw(raw),
        size: Size { width, height: 720 },
        scale: 1,
    }
}

fn owner() -> LiveOutputTopologyOwner {
    LiveOutputTopologyOwner::new_at_generation(vec![output(1, 1280)], 1).unwrap()
}

#[test]
fn changed_topology_advances_public_identity_once() {
    let mut owner = owner();
    assert_eq!(owner.begin_rescan(1), Ok(true));
    assert_eq!(
        owner.observe_rebuild(vec![output(1, 1920), output(2, 1280)]),
        Ok(LiveOutputTopologyRebuild::TopologyChanged)
    );
    assert_eq!(owner.topology_epoch, 2);
    assert_eq!(owner.publication_generation, 2);
    owner.mark_published(7, true).unwrap();
    assert!(!owner.observe_presentation(8));
    owner.mark_policy_committed(8).unwrap();
    assert!(!owner.observe_presentation(7));
    assert!(!owner.observe_presentation(8));
    assert!(owner.observe_presentation(9));
    assert!(!owner.input_quarantined());
}

#[test]
fn configured_initial_publication_generation_advances_from_its_baseline() {
    let mut owner = LiveOutputTopologyOwner::new_at_generation(vec![output(1, 1280)], 2).unwrap();
    owner.begin_rescan(1).unwrap();
    owner.observe_rebuild(vec![output(1, 1920)]).unwrap();
    assert_eq!(owner.publication_generation, 3);
}

#[test]
fn retry_does_not_consume_another_security_or_public_epoch() {
    let mut owner = owner();
    assert_eq!(owner.begin_rescan(4), Ok(true));
    assert_eq!(
        owner.observe_rebuild(Vec::new()),
        Ok(LiveOutputTopologyRebuild::Unavailable)
    );
    assert_eq!(owner.begin_rescan(5), Ok(false));
    assert_eq!(owner.topology_epoch, 1);
    assert_eq!(owner.publication_generation, 1);
    assert_eq!(
        owner.observe_rebuild(vec![output(1, 1280)]),
        Ok(LiveOutputTopologyRebuild::TransportReplaced)
    );
}

#[test]
fn transport_replacement_still_waits_for_new_presentation() {
    let mut owner = owner();
    owner.begin_rescan(1).unwrap();
    owner.observe_rebuild(vec![output(1, 1280)]).unwrap();
    owner.mark_published(11, false).unwrap();
    assert!(!owner.observe_presentation(11));
    assert!(owner.observe_presentation(12));
}

#[test]
fn newer_notice_restarts_publication_without_reconsuming_security_epoch() {
    let mut owner = owner();
    assert!(owner.begin_rescan(1).unwrap());
    owner.observe_rebuild(vec![output(1, 1920)]).unwrap();
    owner.mark_published(3, true).unwrap();
    assert!(!owner.begin_rescan(2).unwrap());
    assert_eq!(owner.phase, LiveOutputTopologyPhase::Quarantined);
    assert_eq!(owner.transition, 2);
}

#[test]
fn redundant_notice_cannot_bypass_pending_policy_settlement() {
    let mut owner = owner();
    owner.begin_rescan(1).unwrap();
    owner.observe_rebuild(vec![output(1, 1920)]).unwrap();
    owner.mark_published(3, true).unwrap();
    owner.begin_rescan(2).unwrap();
    owner.observe_rebuild(vec![output(1, 1920)]).unwrap();
    owner.mark_published(4, false).unwrap();
    assert_eq!(owner.phase, LiveOutputTopologyPhase::Published);
    assert!(owner.policy_settlement_pending);
}

#[test]
fn duplicate_coalescer_token_does_not_restart_a_transition() {
    let mut owner = owner();
    assert!(owner.begin_rescan(3).unwrap());
    assert!(!owner.begin_rescan(3).unwrap());
    assert_eq!(owner.transition, 1);
    assert_eq!(owner.phase, LiveOutputTopologyPhase::Quarantined);
}
